use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{Duration, Instant};

use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::{Mutex as AsyncMutex, Notify, SetOnce, watch};
use tracing::info;
use wf_core::{
    Catalog, Core, Ducatering, MarketWindow, PriceCache, PriceSource, Snapshot, Store, Turnover,
};
use wf_data::load_or_fetch;
use wf_market::{Client, Platform, PriceTable};

use crate::auctions::AuctionCache;
use crate::focus::GameFocus;
use crate::images::ImageCache;
use crate::market::{Listings, MarketItems, Presence};
use crate::overlay::{OverlaySupport, Overlays};
use crate::settings::{self, MarketAccount, Settings};

pub const DATA_DIR: &str = "Merframe";
pub const LOG_FILE: &str = "merframe.log";

pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn read<T>(cell: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    match cell.read() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn write<T>(cell: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    match cell.write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[derive(Default)]
pub struct SharedPrices {
    cache: Mutex<PriceCache>,
}

impl SharedPrices {
    pub fn load(&self, table: &PriceTable, now: DateTime<Utc>) -> usize {
        lock(&self.cache).load(table, now)
    }

    pub fn checked(&self, now: DateTime<Utc>) {
        lock(&self.cache).checked(now);
    }

    pub fn checked_at(&self) -> Option<DateTime<Utc>> {
        lock(&self.cache).checked_at()
    }

    pub fn turnover(&self, window: MarketWindow) -> HashMap<String, Turnover> {
        lock(&self.cache).turnover(window)
    }
}

impl PriceSource for SharedPrices {
    fn plat(&self, market_slug: &str) -> Option<f64> {
        lock(&self.cache).plat(market_slug)
    }

    fn plat_max_rank(&self, market_slug: &str) -> Option<f64> {
        lock(&self.cache).plat_max_rank(market_slug)
    }

    fn buy_plat(&self, market_slug: &str) -> Option<f64> {
        lock(&self.cache).buy_plat(market_slug)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum InventorySource {
    #[default]
    None,
    Live,
    Cached,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct GameStatus {
    pub game_detected: bool,
    pub pid: Option<u32>,
    pub scanning: bool,
    pub source: InventorySource,
    pub last_scan_at: Option<DateTime<Utc>>,
    pub last_scan_error: Option<String>,
    pub last_sync_oid: Option<String>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub inventory_age_secs: Option<i64>,
    pub world_state_at: Option<DateTime<Utc>>,
    pub price_table_at: Option<DateTime<Utc>>,
    pub log_file: Option<String>,
    pub log_attached: bool,
    pub market_account: Option<MarketAccount>,
    pub market_unread: u32,
    pub trades_remaining: Option<u32>,
    pub overlay_support: OverlaySupport,
    pub event_feed: EventFeedStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum EventFeedStatus {
    #[default]
    Off,
    Listening {
        port: u16,
    },
    Failed {
        port: u16,
        cause: String,
    },
}

impl GameStatus {
    pub fn remember_snapshot(&mut self, snapshot: &Snapshot) {
        self.last_sync_oid = Some(snapshot.last_sync_oid.clone());
        self.last_sync_at = Some(snapshot.taken_at);
    }

    fn inventory_age(&self, now: DateTime<Utc>) -> Option<i64> {
        self.last_sync_at
            .map(|taken_at| now.signed_duration_since(taken_at).num_seconds())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueSession {
    pub pid: u32,
    pub clients: Option<wf_scan::HttpClients>,
}

pub type Startup = std::result::Result<Arc<AppState>, String>;

pub type AppStateCell = SetOnce<Startup>;

pub struct AppState {
    pub core: Mutex<Core>,
    pub market: RwLock<Arc<Client>>,
    pub market_items: AsyncMutex<Option<Arc<MarketItems>>>,
    pub market_activity: Mutex<Option<Instant>>,
    pub market_presence: RwLock<Presence>,
    pub market_presence_wake: Notify,
    pub listings: Listings,
    pub prices: Arc<SharedPrices>,
    pub status: RwLock<GameStatus>,
    pub focus: GameFocus,
    pub settings: RwLock<Settings>,
    pub rescan: Notify,
    pub http_clients: Mutex<Option<QueueSession>>,
    pub capturing: AtomicBool,
    pub relic_picks: Mutex<HashMap<u64, String>>,
    pub watching_picker: AtomicBool,
    pub trade_screen_open: watch::Sender<bool>,
    pub kiosk_open: watch::Sender<bool>,
    pub ducatering: watch::Sender<Option<Ducatering>>,
    pub prices_wake: Notify,
    pub arbitration_refresh_failed: AtomicBool,
    pub overlays: Overlays,
    pub auctions: AuctionCache,
    pub http: reqwest::Client,
    pub data_dir: PathBuf,
    pub images: ImageCache,
}

fn anonymous_client(http: &reqwest::Client) -> Client {
    Client::new(http.clone(), Platform::Pc)
}

pub fn data_dir<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<PathBuf> {
    Ok(app
        .path()
        .data_dir()
        .context("Resolving the user data directory")?
        .join(DATA_DIR))
}

async fn load_catalog(dir: &Path, http: &reqwest::Client) -> anyhow::Result<Catalog> {
    let started = Instant::now();
    let game_data = load_or_fetch(dir, http)
        .await
        .context("Loading the WFCD game data")?;
    let catalog = Catalog::new(game_data);
    info!(
        items = catalog.data().items().len(),
        relics = catalog.data().relics().len(),
        millis = started.elapsed().as_millis(),
        "Game data loaded"
    );
    Ok(catalog)
}

pub(crate) const USER_AGENT: &str = concat!(
    "Merframe/",
    env!("CARGO_PKG_VERSION"),
    " (+https://yareli.net/merframe)"
);

impl AppState {
    pub async fn build<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<Self> {
        let data_dir = data_dir(app)?;
        tokio::fs::create_dir_all(&data_dir)
            .await
            .with_context(|| format!("Creating {}", data_dir.display()))?;

        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(30))
            .build()
            .context("Building the http client")?;

        let images = ImageCache::new(&data_dir, http.clone());
        images
            .prepare()
            .await
            .with_context(|| format!("Creating {}", images.dir().display()))?;

        let catalog = load_catalog(&data_dir.join("gamedata"), &http).await?;
        let store =
            Store::open(&data_dir.join("merframe.sqlite")).context("Opening the local store")?;

        let settings = settings::load(app)?;
        let prices = Arc::new(SharedPrices::default());

        let price_source = Arc::clone(&prices) as Arc<dyn PriceSource + Send + Sync>;
        let core = Core::new(store, catalog, price_source, settings.alerts.clone())
            .context("Reading the local store")?;

        let mut market = anonymous_client(&http);
        let session_loaded = match settings::token(app) {
            Ok(Some(token)) => {
                market = market.with_token(token);
                true
            }
            Ok(None) => false,
            Err(error) => {
                tracing::warn!(%error, "Saved market session unavailable; continuing anonymously");
                false
            }
        };

        let status = GameStatus {
            market_account: if session_loaded {
                settings::account(app)?
            } else {
                None
            },
            overlay_support: crate::overlay::probe(),
            ..GameStatus::default()
        };

        Ok(Self {
            core: Mutex::new(core),
            market: RwLock::new(Arc::new(market)),
            market_items: AsyncMutex::new(None),
            market_activity: Mutex::new(None),
            market_presence: RwLock::new(Presence {
                choice: settings::presence(app)?,
                live: None,
            }),
            market_presence_wake: Notify::new(),
            listings: Listings::default(),
            prices,
            status: RwLock::new(status),
            focus: GameFocus::default(),
            settings: RwLock::new(settings),
            rescan: Notify::new(),
            http_clients: Mutex::new(None),
            capturing: AtomicBool::new(false),
            relic_picks: Mutex::new(HashMap::new()),
            watching_picker: AtomicBool::new(false),
            trade_screen_open: watch::Sender::new(false),
            kiosk_open: watch::Sender::new(false),
            ducatering: watch::Sender::new(None),
            prices_wake: Notify::new(),
            arbitration_refresh_failed: AtomicBool::new(false),
            overlays: Overlays::default(),
            auctions: AuctionCache::default(),
            http,
            data_dir,
            images,
        })
    }

    pub fn market(&self) -> Arc<Client> {
        Arc::clone(&read(&self.market))
    }

    pub fn anonymous_market(&self) -> Client {
        anonymous_client(&self.http)
    }

    pub fn market_slug(&self) -> Option<String> {
        read(&self.status)
            .market_account
            .as_ref()
            .map(|account| account.slug.clone())
    }

    pub fn status_snapshot(&self) -> GameStatus {
        let mut status = read(&self.status).clone();
        status.price_table_at = self.prices.checked_at();
        status.inventory_age_secs = status.inventory_age(Utc::now());
        status
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use wf_core::PriceSource;

    use super::*;

    const TABLE: &str = include_str!("../../../crates/wf-market/tests/fixtures/prices_bulk.json");

    #[test]
    fn shared_prices_lookup() {
        let table = serde_json::from_str::<wf_market::PriceTable>(TABLE).unwrap();
        let now = DateTime::<Utc>::from_timestamp(1_757_410_800, 0).unwrap();
        let prices = SharedPrices::default();
        assert_eq!(prices.checked_at(), None);

        assert_eq!(prices.load(&table, now), 10);
        assert_eq!(prices.checked_at(), Some(now));
        assert_eq!(prices.plat("arcane_energize"), Some(55.0));
        assert_eq!(prices.plat_max_rank("arcane_energize"), Some(940.0));
        assert_eq!(prices.buy_plat("arcane_energize"), Some(40.0));
        assert_eq!(prices.plat("braton_prime_barrel"), Some(8.0));
        assert_eq!(prices.plat_max_rank("braton_prime_barrel"), None);
        assert_eq!(prices.plat("nothing_is_traded_here"), None);

        let later = now + chrono::TimeDelta::minutes(15);
        prices.checked(later);
        assert_eq!(prices.checked_at(), Some(later));
        assert_eq!(prices.plat("arcane_energize"), Some(55.0));
    }

    #[test]
    fn status_snapshot() {
        let store = Store::in_memory().unwrap();
        let inventory =
            wf_inventory::Inventory::parse(include_str!("../../../fixtures/inventory.json"))
                .unwrap();
        let taken_at = DateTime::<Utc>::from_timestamp(1_757_410_800, 0).unwrap();
        store.record_snapshot(&inventory, taken_at).unwrap();
        let snapshot = store.latest_snapshot().unwrap().unwrap();

        let mut status = GameStatus::default();
        assert_eq!(status.inventory_age(taken_at), None);
        status.remember_snapshot(&snapshot);
        assert_eq!(
            status.last_sync_oid.as_deref(),
            Some("6a9eeb1f000000000000c001")
        );
        assert_eq!(status.last_sync_at, Some(taken_at));
        let later = taken_at + chrono::TimeDelta::minutes(90);
        assert_eq!(status.inventory_age(later), Some(5400));
    }

    #[test]
    fn inventory_source_json() {
        let status = GameStatus::default();
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["source"], "none");
        assert_eq!(json["last_sync_at"], serde_json::Value::Null);

        let cached = GameStatus {
            source: InventorySource::Cached,
            ..GameStatus::default()
        };
        assert_eq!(serde_json::to_value(&cached).unwrap()["source"], "cached");
        let live = GameStatus {
            source: InventorySource::Live,
            ..GameStatus::default()
        };
        assert_eq!(serde_json::to_value(&live).unwrap()["source"], "live");
    }
}
