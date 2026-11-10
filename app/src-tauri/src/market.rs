use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use chrono::{DateTime, Duration, Utc};
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};
use wf_core::{Core, ItemRecord, ItemSummary, ItemTable, MarketListings, PriceSource, market_name};
use wf_market::{Auction, Item, Order, OrderType, UserStatus};

use crate::settings::MarketSettings;
use crate::state::{AppState, lock, read, write};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum MarketCategory {
    Parts,
    Relics,
    Mods,
    Arcanes,
    Sets,
    Misc,
}

pub(crate) fn has_tag(item: &Item, tag: &str) -> bool {
    item.tags.iter().any(|owned| owned == tag)
}

impl MarketCategory {
    pub(crate) fn of(item: &Item) -> Self {
        let tagged = |tag| has_tag(item, tag);
        if tagged("component") || tagged("blueprint") || item.slug.contains("kavasa") {
            return Self::Parts;
        }
        if tagged("relic") {
            return Self::Relics;
        }
        if tagged("mod") {
            return Self::Mods;
        }
        if tagged("arcane") || tagged("arcane_enhancement") {
            return Self::Arcanes;
        }
        if tagged("set") {
            return Self::Sets;
        }
        Self::Misc
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum MarketPresenceChoice {
    #[default]
    Untouched,
    Invisible,
    Online,
    Ingame,
    FollowGame,
}

impl MarketPresenceChoice {
    pub fn wanted(self, game_detected: bool) -> Option<UserStatus> {
        match self {
            Self::Untouched => None,
            Self::Online => Some(UserStatus::Online),
            Self::Ingame => Some(UserStatus::Ingame),
            Self::FollowGame if game_detected => Some(UserStatus::Ingame),
            Self::Invisible | Self::FollowGame => Some(UserStatus::Invisible),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct MarketPresence {
    pub choice: MarketPresenceChoice,
    pub live: Option<UserStatus>,
    #[serde(skip)]
    pub pause: MarketPresencePause,
}

/// One-off for going offline after the last trade of the day,
/// to allow the explicit status choice to take over the next choice/restart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MarketPresencePause {
    #[default]
    Off,
    Due {
        standing: Option<UserStatus>,
    },
    Holding {
        standing: Option<UserStatus>,
    },
}

impl MarketPresence {
    pub fn is_live_online(self) -> bool {
        matches!(self.live, Some(UserStatus::Online | UserStatus::Ingame))
    }

    pub fn go_offline_after_last_trade(&mut self, game_detected: bool) {
        self.pause = MarketPresencePause::Due {
            standing: self.choice.wanted(game_detected),
        };
    }

    pub fn next_status(self, game_detected: bool) -> Option<UserStatus> {
        let wanted = self.choice.wanted(game_detected);
        match self.pause {
            MarketPresencePause::Due { .. } => Some(UserStatus::Invisible),
            MarketPresencePause::Holding { standing } if standing == wanted => None,
            MarketPresencePause::Off | MarketPresencePause::Holding { .. } => wanted,
        }
    }

    pub fn status_applied(&mut self, decided: MarketPresencePause) {
        if self.pause != decided {
            return;
        }

        self.pause = match decided {
            MarketPresencePause::Due { standing } => MarketPresencePause::Holding { standing },
            MarketPresencePause::Off | MarketPresencePause::Holding { .. } => {
                MarketPresencePause::Off
            }
        };
    }
}

pub struct MarketItems {
    items: Vec<Item>,
    by_id: HashMap<String, usize>,
    fetched_at: DateTime<Utc>,
}

impl MarketItems {
    pub(crate) fn new(items: Vec<Item>, fetched_at: DateTime<Utc>) -> Self {
        let by_id = items
            .iter()
            .enumerate()
            .map(|(index, item)| (item.id.clone(), index))
            .collect();
        Self {
            items,
            by_id,
            fetched_at,
        }
    }

    fn stale(&self, now: DateTime<Utc>) -> bool {
        now.signed_duration_since(self.fetched_at) > Duration::hours(12)
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn get(&self, id: &str) -> Option<&Item> {
        self.by_id.get(id).map(|index| &self.items[*index])
    }

    pub fn listed(&self, table: &ItemTable, dialog_name: &str) -> Option<&Item> {
        self.get(table.listing_id(dialog_name)?)
    }
}

pub async fn market_items(state: &Arc<AppState>) -> Option<Arc<MarketItems>> {
    let mut cache = state.market_items.lock().await;
    let cached = cache.clone();

    let now = Utc::now();
    if let Some(items) = &cached
        && !items.stale(now)
    {
        return cached;
    }

    match state.market().items().await {
        Ok(items) => {
            let indexed = lock(&state.core).index_market(&items);
            tracing::debug!(indexed, "Market slugs indexed by game reference");
            let items = Arc::new(MarketItems::new(items, now));
            *cache = Some(Arc::clone(&items));
            Some(items)
        }
        Err(error) => {
            tracing::warn!(
                error = %error.brief(),
                stale = cached.is_some(),
                "Item list request to warframe.market failed",
            );
            cached
        }
    }
}

fn is_necramech_set(category: MarketCategory, name: &str) -> bool {
    category == MarketCategory::Sets
        && ["Bonewidow", "Voidrig", "Damaged Necramech"]
            .iter()
            .any(|necramech| name.contains(necramech))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct OrderRow {
    pub id: String,
    pub order_type: OrderType,
    pub item_id: String,
    pub item: usize,
    pub category: MarketCategory,
    pub platinum: u32,
    pub quantity: u32,
    pub per_trade: Option<u32>,
    pub rank: Option<u32>,
    pub subtype: Option<String>,
    pub visible: bool,
    pub updated_at: DateTime<Utc>,
    pub owned: i64,
    pub show_warning: bool,
    pub lowest: Option<Lowest>,
    pub pricing: Option<Pricing>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum Lowest {
    AtRank { plat: f64 },
    RankZero { plat: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum Pricing {
    Overpriced,
    Underpriced,
}

impl Pricing {
    fn of(order: &Order, lowest: Option<Lowest>, settings: &MarketSettings) -> Option<Self> {
        let Some(Lowest::AtRank { plat }) = lowest else {
            return None;
        };

        if order.order_type != OrderType::Sell || order.subtype.is_some() {
            return None;
        }

        let tolerance = plat * f64::from(settings.market_price_tolerance) / 100.0;
        let platinum = f64::from(order.platinum);
        if platinum > plat + tolerance {
            Some(Self::Overpriced)
        } else if platinum < plat - tolerance {
            Some(Self::Underpriced)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct MarketOrders {
    pub items: Vec<ItemSummary>,
    pub rows: Vec<OrderRow>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct MarketSnapshot {
    pub orders: Option<MarketOrders>,
    pub auctions: Option<Vec<Auction>>,
    pub at: DateTime<Utc>,
}

impl MarketSnapshot {
    pub fn same_listings(&self, other: &Self) -> bool {
        self.orders == other.orders && self.auctions == other.auctions
    }
}

pub fn order_summary(item: &Item, record: &ItemRecord) -> ItemSummary {
    ItemSummary {
        name: market_name(item).to_owned(),
        market_slug: Some(item.slug.clone()),
        ..ItemSummary::from(record)
    }
}

pub fn order_row(
    order: &Order,
    item: &Item,
    record: &ItemRecord,
    core: &Core,
    prices: &dyn PriceSource,
    settings: &MarketSettings,
) -> OrderRow {
    let category = MarketCategory::of(item);
    let name = market_name(item);
    let rank = order.rank.unwrap_or(0);

    let counted_rank = match category {
        MarketCategory::Mods => settings.take_rank_into_account.then_some(rank),
        MarketCategory::Arcanes => Some(rank),
        _ => None,
    };
    let owned = core.market_owned(record, name, counted_rank, order.subtype.as_deref());
    let show_warning = order.order_type == OrderType::Sell
        && owned < i64::from(order.quantity)
        && !is_necramech_set(category, name);

    let ranked = matches!(category, MarketCategory::Mods | MarketCategory::Arcanes);
    let off_rank_zero = ranked && rank > 0;
    let at_max_rank = (off_rank_zero && item.max_rank == Some(rank))
        .then(|| prices.plat_max_rank(&item.slug))
        .flatten();
    let lowest = match at_max_rank {
        Some(plat) => Some(Lowest::AtRank { plat }),
        None if off_rank_zero => prices
            .plat(&item.slug)
            .map(|plat| Lowest::RankZero { plat }),
        None => prices.plat(&item.slug).map(|plat| Lowest::AtRank { plat }),
    };
    let pricing = Pricing::of(order, lowest, settings);

    OrderRow {
        id: order.id.clone(),
        order_type: order.order_type,
        item_id: order.item_id.clone(),
        item: 0,
        category,
        platinum: order.platinum,
        quantity: order.quantity,
        per_trade: order.per_trade,
        rank: order.rank,
        subtype: order.subtype.clone(),
        visible: order.visible,
        updated_at: order.updated_at,
        owned,
        show_warning,
        lowest,
        pricing,
    }
}

#[derive(Debug, Default, Clone)]
pub struct OwnListings {
    pub orders: MarketOrders,
    pub auctions: Vec<Auction>,
}

#[derive(Default)]
pub struct Listings {
    own: RwLock<OwnListings>,
}

impl Listings {
    pub fn current(&self) -> OwnListings {
        read(&self.own).clone()
    }

    pub fn remember(
        &self,
        core: &Mutex<Core>,
        orders: Option<&MarketOrders>,
        auctions: Option<&[Auction]>,
    ) {
        let mut own = write(&self.own);
        if let Some(orders) = orders {
            own.orders = orders.clone();
        }
        if let Some(auctions) = auctions {
            own.auctions = auctions.to_vec();
        }

        let listings = MarketListings::new(
            own.orders
                .rows
                .iter()
                .map(|row| (row.item_id.as_str(), row.order_type)),
            &own.auctions,
        );
        drop(own);

        lock(core).set_market_listings(listings);
    }
}

pub async fn order_rows(state: &Arc<AppState>, orders: &[Order]) -> Option<MarketOrders> {
    let items = market_items(state).await?;
    let settings = read(&state.settings).market.clone();
    let core = lock(&state.core);

    let mut index = IndexSet::new();
    let rows = orders
        .iter()
        .filter_map(|order| {
            let item = items.get(&order.item_id)?;
            let record = core.items().by_market_id(&order.item_id)?;
            Some(OrderRow {
                item: index.insert_full(order_summary(item, record)).0,
                ..order_row(order, item, record, &core, state.prices.as_ref(), &settings)
            })
        })
        .collect();

    Some(MarketOrders {
        items: index.into_iter().collect(),
        rows,
    })
}

pub(crate) async fn market_snapshot(
    state: &Arc<AppState>,
    orders: Option<&[Order]>,
    auctions: Option<Vec<Auction>>,
) -> MarketSnapshot {
    let orders = match orders {
        Some(orders) => order_rows(state, orders).await,
        None => None,
    };
    state
        .listings
        .remember(&state.core, orders.as_ref(), auctions.as_deref());

    MarketSnapshot {
        orders,
        auctions,
        at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use wf_core::{Catalog, Core, PriceSource};
    use wf_market::{Item, Order};

    use super::*;
    use crate::test_support::{core_with, empty_catalog, market_item, moment, order};

    const INVENTORY: &str = include_str!("../../../fixtures/inventory.json");
    const ITEMS: &str = include_str!("../../../crates/wf-data/tests/fixtures/items.json");
    const UPGRADES: &str = include_str!("../../../fixtures/upgrade_items.json");
    const RELICS: &str = include_str!("../../../crates/wf-data/tests/fixtures/relics.json");
    const COMPONENTS: &str = include_str!("../../../crates/wf-data/tests/fixtures/components.json");

    const PRIMED_CONTINUITY: &str =
        "/Lotus/Upgrades/Mods/Warframe/Expert/AvatarAbilityDurationModExpert";
    const ENERGIZE: &str =
        "/Lotus/Upgrades/CosmeticEnhancers/Utility/GolemArcaneRadialEnergyOnEnergyPickup";
    const BRATON: &str = "/Lotus/Types/Recipes/Weapons/WeaponParts/BratonPrime";

    struct TestPrices;

    impl PriceSource for TestPrices {
        fn plat(&self, market_slug: &str) -> Option<f64> {
            (market_slug == "primed_continuity").then_some(120.0)
        }

        fn plat_max_rank(&self, market_slug: &str) -> Option<f64> {
            (market_slug == "primed_continuity").then_some(320.0)
        }
    }

    fn item(slug: &str, name: &str, tags: &[&str], max_rank: Option<u32>) -> Item {
        Item {
            max_rank,
            ..market_item(slug, name, tags)
        }
    }

    fn listed(
        slug: &str,
        name: &str,
        game_ref: &str,
        tags: &[&str],
        max_rank: Option<u32>,
    ) -> Item {
        Item {
            game_ref: game_ref.to_owned(),
            ..item(slug, name, tags, max_rank)
        }
    }

    fn market_list() -> Vec<Item> {
        vec![
            listed(
                "primed_continuity",
                "Primed Continuity",
                PRIMED_CONTINUITY,
                &["mod"],
                Some(10),
            ),
            listed(
                "arcane_energize",
                "Arcane Energize",
                ENERGIZE,
                &["arcane_enhancement"],
                Some(5),
            ),
            listed(
                "braton_prime_set",
                "Braton Prime Set",
                "/Lotus/Weapons/Tenno/Rifle/BratonPrime",
                &["set", "prime"],
                None,
            ),
            listed(
                "braton_prime_barrel",
                "Braton Prime Barrel",
                &format!("{BRATON}Barrel"),
                &["component", "prime"],
                None,
            ),
            item(
                "trinity_prime_systems_blueprint",
                "Trinity Prime Systems Blueprint",
                &["component", "prime", "blueprint"],
                None,
            ),
            item("axi_a1_relic", "Axi A1 Relic", &["relic"], None),
            item(
                "rifle_riven_mod_(veiled)",
                "Rifle Riven Mod (Veiled)",
                &["mod", "riven_mod"],
                None,
            ),
            item("voidrig_set", "Voidrig Set", &["set"], None),
            item(
                "legendary_fusion_core",
                "Legendary Fusion Core",
                &["fusion core"],
                None,
            ),
            item(
                "ancient_fusion_core",
                "Ancient Fusion Core",
                &["fusion core", "legendary"],
                None,
            ),
            item(
                "nihils_oubliette_key",
                "Nihil's Oubliette (Key)",
                &["key"],
                None,
            ),
            item("forma_blueprint", "Forma Blueprint", &["misc"], None),
        ]
    }

    fn catalog() -> Catalog {
        let mut items: Vec<Value> = serde_json::from_str(ITEMS).unwrap();
        items.extend(serde_json::from_str::<Vec<Value>>(UPGRADES).unwrap());
        Catalog::from_json(&Value::from(items).to_string(), RELICS, COMPONENTS).unwrap()
    }

    fn inventory() -> String {
        let mut inventory: Value = serde_json::from_str(INVENTORY).unwrap();
        inventory["RawUpgrades"] = json!([
            { "ItemType": PRIMED_CONTINUITY, "ItemCount": 3 },
            { "ItemType": ENERGIZE, "ItemCount": 4 },
        ]);
        inventory["Upgrades"] = json!([
            {
                "ItemType": PRIMED_CONTINUITY,
                "UpgradeFingerprint": "{\"lvl\":10}",
                "ItemId": { "$oid": format!("{:024x}", 0) },
            },
            {
                "ItemType": ENERGIZE,
                "UpgradeFingerprint": "{\"lvl\":5}",
                "ItemId": { "$oid": format!("{:024x}", 1) },
            },
        ]);
        for (key, item_type) in [
            ("MiscItems", format!("{BRATON}Barrel")),
            ("MiscItems", format!("{BRATON}Receiver")),
            ("MiscItems", format!("{BRATON}Stock")),
            (
                "Recipes",
                "/Lotus/Types/Recipes/Weapons/BratonPrimeBlueprint".to_owned(),
            ),
        ] {
            if let Some(held) = inventory[key].as_array_mut() {
                held.push(json!({ "ItemType": item_type, "ItemCount": 1 }));
            }
        }
        inventory.to_string()
    }

    struct Market {
        core: Core,
        items: MarketItems,
    }

    impl Market {
        fn new() -> Self {
            let mut core = core_with(catalog());
            core.ingest_inventory(&inventory(), moment()).unwrap();
            let items = market_list();
            core.index_market(&items);
            Self {
                core,
                items: MarketItems::new(items, moment()),
            }
        }

        fn row(&self, wanted: &Order, take_rank_into_account: bool) -> OrderRow {
            self.row_with(
                wanted,
                &MarketSettings {
                    take_rank_into_account,
                    ..MarketSettings::default()
                },
            )
        }

        fn row_with(&self, wanted: &Order, settings: &MarketSettings) -> OrderRow {
            let item = self.items.get(&wanted.item_id).unwrap();
            order_row(
                wanted,
                item,
                self.core.items().by_market_id(&item.id).unwrap(),
                &self.core,
                &TestPrices,
                settings,
            )
        }

        fn sell(&self, slug: &str, quantity: u32) -> OrderRow {
            self.row(&order(slug, quantity, None), true)
        }

        fn owned(&self, slug: &str, rank: Option<u32>, take_rank_into_account: bool) -> i64 {
            self.row(&order(slug, 1, rank), take_rank_into_account)
                .owned
        }

        fn summary(&self, slug: &str) -> ItemSummary {
            let item = self.items.get(&format!("id-{slug}")).unwrap();
            order_summary(item, self.core.items().by_market_id(&item.id).unwrap())
        }

        fn listed(&self, dialog_name: &str) -> Option<&str> {
            self.items
                .listed(self.core.items(), dialog_name)
                .map(|item| item.slug.as_str())
        }
    }

    #[test]
    fn market_items_lookup() {
        let first = item("braton_prime_set", "Braton Prime Set", &["set"], None);
        let second = item("primed_continuity", "Primed Continuity", &["mod"], Some(10));
        let items = MarketItems::new(vec![first, second], moment());

        assert_eq!(
            items
                .items()
                .iter()
                .map(|item| item.slug.as_str())
                .collect::<Vec<_>>(),
            ["braton_prime_set", "primed_continuity"]
        );
        assert_eq!(
            items
                .get("id-primed_continuity")
                .map(|item| item.slug.as_str()),
            Some("primed_continuity")
        );
        assert!(items.get("id-braton_prime_barrel").is_none());
    }

    #[test]
    fn presence_follows_game() {
        use wf_market::UserStatus;

        assert_eq!(MarketPresenceChoice::default().wanted(true), None);
        assert_eq!(
            MarketPresenceChoice::Online.wanted(false),
            Some(UserStatus::Online)
        );
        assert_eq!(
            MarketPresenceChoice::FollowGame.wanted(true),
            Some(UserStatus::Ingame)
        );
        assert_eq!(
            MarketPresenceChoice::FollowGame.wanted(false),
            Some(UserStatus::Invisible)
        );
    }

    #[test]
    fn last_trade_pauses_the_standing_choice() {
        use wf_market::UserStatus;

        let mut presence = MarketPresence {
            choice: MarketPresenceChoice::FollowGame,
            ..MarketPresence::default()
        };
        assert_eq!(presence.next_status(true), Some(UserStatus::Ingame));

        presence.go_offline_after_last_trade(true);
        assert_eq!(presence.next_status(true), Some(UserStatus::Invisible));
        assert_eq!(presence.next_status(true), Some(UserStatus::Invisible));

        presence.status_applied(presence.pause);
        assert_eq!(presence.next_status(true), None);
        assert_eq!(presence.next_status(false), Some(UserStatus::Invisible));

        presence.status_applied(presence.pause);
        assert_eq!(presence.pause, MarketPresencePause::Off);
        assert_eq!(presence.next_status(true), Some(UserStatus::Ingame));
    }

    #[test]
    fn last_trade_during_a_send_is_not_lost() {
        use wf_market::UserStatus;

        let mut presence = MarketPresence {
            choice: MarketPresenceChoice::Online,
            ..MarketPresence::default()
        };
        let decided = presence.pause;
        presence.go_offline_after_last_trade(false);
        presence.status_applied(decided);
        assert_eq!(presence.next_status(false), Some(UserStatus::Invisible));
    }

    #[test]
    fn last_trade_sends_offline_once_without_a_standing_choice() {
        use wf_market::UserStatus;

        let mut presence = MarketPresence::default();
        presence.go_offline_after_last_trade(false);
        assert_eq!(presence.next_status(false), Some(UserStatus::Invisible));

        presence.status_applied(presence.pause);
        assert_eq!(presence.next_status(false), None);
    }

    #[test]
    fn presence_is_online_while_the_socket_reports_it() {
        use wf_market::UserStatus;

        let presence = MarketPresence {
            live: Some(UserStatus::Ingame),
            ..MarketPresence::default()
        };
        assert!(presence.is_live_online());
        assert!(!MarketPresence::default().is_live_online());
    }

    #[test]
    fn category_from_tags() {
        assert_eq!(
            MarketCategory::of(&item(
                "braton_prime_barrel",
                "Braton Prime Barrel",
                &["component", "prime"],
                None
            )),
            MarketCategory::Parts
        );
        assert_eq!(
            MarketCategory::of(&item("lith_a1_relic", "Lith A1 Relic", &["relic"], None)),
            MarketCategory::Relics
        );
        assert_eq!(
            MarketCategory::of(&item(
                "primed_continuity",
                "Primed Continuity",
                &["mod", "rare"],
                Some(10)
            )),
            MarketCategory::Mods
        );
        assert_eq!(
            MarketCategory::of(&item(
                "arcane_energize",
                "Arcane Energize",
                &["arcane_enhancement"],
                Some(5)
            )),
            MarketCategory::Arcanes
        );
        assert_eq!(
            MarketCategory::of(&item(
                "mirage_prime_set",
                "Mirage Prime Set",
                &["set", "prime"],
                None
            )),
            MarketCategory::Sets
        );
        assert_eq!(
            MarketCategory::of(&item("forma", "Forma", &["misc"], None)),
            MarketCategory::Misc
        );
        assert_eq!(
            MarketCategory::of(&item(
                "kavasa_prime_collar_blueprint",
                "Kavasa Prime Kubrow Collar Blueprint",
                &["prime"],
                None
            )),
            MarketCategory::Parts
        );
    }

    #[test]
    fn oversold_sell_order_flagged() {
        let market = Market::new();
        let row = market.sell("braton_prime_set", 1);
        assert_eq!(row.owned, 1);
        assert!(!row.show_warning);

        let row = market.sell("braton_prime_set", 2);
        assert_eq!(row.owned, 1);
        assert!(row.show_warning);
    }

    #[test]
    fn buy_order_not_flagged() {
        let market = Market::new();
        let mut wanted = order("braton_prime_set", 9, None);
        wanted.order_type = OrderType::Buy;
        let row = market.row(&wanted, true);
        assert_eq!(row.owned, 1);
        assert!(!row.show_warning);
    }

    #[test]
    fn necramech_set_never_flagged() {
        let market = Market::new();
        let row = market.sell("voidrig_set", 1);
        assert_eq!(row.owned, 0);
        assert!(!row.show_warning);
    }

    #[test]
    fn nothing_owned_without_an_inventory() {
        let mut core = core_with(catalog());
        let barrel = listed(
            "braton_prime_barrel",
            "Braton Prime Barrel",
            &format!("{BRATON}Barrel"),
            &["component"],
            None,
        );
        core.index_market(std::slice::from_ref(&barrel));
        let record = core.items().by_market_id(&barrel.id).unwrap();
        let summary = order_summary(&barrel, record);
        assert_eq!(summary.name, "Braton Prime Barrel");
        assert_eq!(summary.market_slug.as_deref(), Some("braton_prime_barrel"));
        assert_eq!(summary.unique_name, record.unique_name);
        let row = order_row(
            &order("braton_prime_barrel", 1, None),
            &barrel,
            record,
            &core,
            &TestPrices,
            &MarketSettings::default(),
        );
        assert_eq!(row.owned, 0);
        assert!(row.show_warning);
    }

    #[test]
    fn mod_rank_counting_setting() {
        let market = Market::new();
        assert_eq!(market.owned("primed_continuity", Some(10), true), 1);
        assert_eq!(market.owned("primed_continuity", Some(0), true), 3);
        assert_eq!(
            market.owned("primed_continuity", Some(10), false),
            4,
            "without the rank setting every rank counts"
        );
        assert_eq!(market.owned("primed_continuity", Some(0), false), 4);
    }

    #[test]
    fn arcane_rank_always_counted() {
        let market = Market::new();
        assert_eq!(market.owned("arcane_energize", Some(5), false), 1);
        assert_eq!(market.owned("arcane_energize", None, false), 4);
    }

    #[test]
    fn dialog_names_find_their_listing() {
        let market = Market::new();
        assert_eq!(
            market.listed("Primed Continuity"),
            Some("primed_continuity")
        );
        assert_eq!(
            market.listed("primed continuity"),
            Some("primed_continuity")
        );
        assert_eq!(
            market.listed("Trinity Prime Systems"),
            Some("trinity_prime_systems_blueprint")
        );
        assert_eq!(
            market.listed("Axi A1 Relic [RADIANT]"),
            Some("axi_a1_relic")
        );
        assert_eq!(market.listed("Braton Prime Set"), Some("braton_prime_set"));
        assert_eq!(market.listed("Voidrig Set"), Some("voidrig_set"));
        assert_eq!(market.listed("Forma"), Some("forma_blueprint"));
        assert_eq!(
            market.listed("Rifle Riven Mod"),
            Some("rifle_riven_mod_(veiled)")
        );
        assert_eq!(
            market.listed("Enter Nihil's Oubliette"),
            Some("nihils_oubliette_key")
        );
        assert_eq!(
            market.listed("Legendary Core"),
            Some("legendary_fusion_core")
        );
        assert_eq!(market.listed("Ancient Core"), Some("ancient_fusion_core"));
        assert_eq!(market.listed("Rubico Critacan"), None);
    }

    #[test]
    fn dialog_alias_needs_listing() {
        let listing = item("legendary_core", "Legendary Core", &[], None);
        let mut table = ItemTable::build(&empty_catalog());
        table.index_market(std::slice::from_ref(&listing));
        let items = MarketItems::new(vec![listing], moment());
        assert!(
            items.listed(&table, "Legendary Core").is_none(),
            "the dialog name only ever means the fusion core listing"
        );
    }

    #[test]
    fn lowest_price_at_rank() {
        let market = Market::new();
        let lowest = |rank| {
            market
                .row(&order("primed_continuity", 1, Some(rank)), true)
                .lowest
        };
        assert_eq!(lowest(0), Some(Lowest::AtRank { plat: 120.0 }));
        assert_eq!(lowest(10), Some(Lowest::AtRank { plat: 320.0 }));
        assert_eq!(lowest(5), Some(Lowest::RankZero { plat: 120.0 }));
        assert_eq!(
            serde_json::to_value(lowest(5)).unwrap(),
            json!({ "from": "rank_zero", "plat": 120.0 })
        );
    }

    #[test]
    fn sell_price_against_lowest() {
        let market = Market::new();
        let pricing = |platinum, rank| {
            let mut wanted = order("primed_continuity", 1, Some(rank));
            wanted.platinum = platinum;
            market.row(&wanted, true).pricing
        };
        assert_eq!(pricing(133, 0), Some(Pricing::Overpriced));
        assert_eq!(pricing(132, 0), None);
        assert_eq!(pricing(108, 0), None);
        assert_eq!(pricing(107, 0), Some(Pricing::Underpriced));
        assert_eq!(pricing(400, 10), Some(Pricing::Overpriced));
        assert_eq!(pricing(400, 5), None);
    }

    #[test]
    fn price_tolerance_setting() {
        let market = Market::new();
        let mut wanted = order("primed_continuity", 1, None);
        wanted.platinum = 121;
        let settings = MarketSettings {
            market_price_tolerance: 0,
            ..MarketSettings::default()
        };
        assert_eq!(
            market.row_with(&wanted, &settings).pricing,
            Some(Pricing::Overpriced)
        );
    }

    #[test]
    fn refinement_not_priced() {
        let market = Market::new();
        let mut wanted = order("primed_continuity", 1, None);
        wanted.platinum = 400;
        wanted.subtype = Some("radiant".to_owned());
        assert_eq!(market.row(&wanted, true).pricing, None);
    }

    #[test]
    fn buy_order_not_priced() {
        let market = Market::new();
        let mut wanted = order("primed_continuity", 1, None);
        wanted.order_type = OrderType::Buy;
        wanted.platinum = 500;
        assert_eq!(market.row(&wanted, true).pricing, None);
    }

    #[test]
    fn unpriced_item() {
        let market = Market::new();
        assert_eq!(market.sell("braton_prime_set", 1).lowest, None);
    }

    #[test]
    fn listings_keep_other_side() {
        const AUCTIONS: &str =
            include_str!("../../../crates/wf-market/tests/fixtures/auctions_my.json");
        let auctions = wf_market::parse_v1_auctions(AUCTIONS).unwrap();
        let listings = Listings::default();
        assert!(listings.current().orders.rows.is_empty());
        assert!(listings.current().auctions.is_empty());

        let market = Market::new();
        let orders = MarketOrders {
            items: vec![market.summary("braton_prime_barrel")],
            rows: vec![market.sell("braton_prime_barrel", 1)],
        };
        let core = Mutex::new(market.core);
        listings.remember(&core, Some(&orders), Some(&auctions));
        assert_eq!(listings.current().orders.rows.len(), 1);
        assert_eq!(listings.current().auctions.len(), 2);
        let locked = lock(&core);
        assert!(
            locked
                .market_listings()
                .listed_slugs(OrderType::Sell, locked.items())
                .contains("braton_prime_barrel")
        );
        drop(locked);

        listings.remember(&core, Some(&MarketOrders::default()), None);
        let own = listings.current();
        assert!(own.orders.rows.is_empty());
        assert_eq!(
            own.auctions.len(),
            2,
            "an order refresh leaves the riven auctions alone"
        );
    }

    #[test]
    fn same_listings_ignores_time() {
        const AUCTIONS: &str =
            include_str!("../../../crates/wf-market/tests/fixtures/auctions_my.json");
        let market = Market::new();
        let snapshot = MarketSnapshot {
            orders: Some(MarketOrders {
                items: vec![market.summary("braton_prime_barrel")],
                rows: vec![market.sell("braton_prime_barrel", 1)],
            }),
            auctions: Some(wf_market::parse_v1_auctions(AUCTIONS).unwrap()),
            at: moment(),
        };
        let later = MarketSnapshot {
            at: moment() + Duration::minutes(1),
            ..snapshot.clone()
        };
        assert!(snapshot.same_listings(&later));

        let mut repriced = later.clone();
        repriced.orders.as_mut().unwrap().rows[0].platinum += 1;
        assert!(!snapshot.same_listings(&repriced));

        let mut closed = later.clone();
        closed.auctions.as_mut().unwrap()[0].closed = true;
        assert!(!snapshot.same_listings(&closed));

        let orders_only = MarketSnapshot {
            auctions: None,
            ..later
        };
        assert!(!snapshot.same_listings(&orders_only));
    }
}
