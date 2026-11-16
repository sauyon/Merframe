use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use anyhow::Context;
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tracing::{debug, info, warn};
use wf_core::{Core, CoreEvent};
use wf_market::{Etagged, PriceTable, RivenData, bulk_prices, bulk_riven_data};
use wf_worldstate::{
    ARBITRATION_FORECAST_URL, ARBITRATION_TIERS_URL, ArbitrationForecast, ArbitrationTiers,
    WorldState, fetch_text,
};

use super::{AppEvent, RIVEN_DATA_KEY, dispatch, emit, emit_status, try_blocking};
use crate::event_feed;
use crate::market;
use crate::state::{AppState, lock, read, write};

const PRICE_RETRY: Duration = Duration::from_secs(60);
const PRICE_INTERVAL: Duration = Duration::from_mins(15);
const RIVEN_INTERVAL: Duration = Duration::from_hours(24);
const ARBITRATION_FORECAST_KEY: &str = "arbitration_forecast";
const ARBITRATION_INTERVAL: TimeDelta = TimeDelta::hours(6);
const ARBITRATION_TIERS_MAX_AGE: TimeDelta = TimeDelta::days(7);
const ARBITRATION_RETRY_CAP: Duration = Duration::from_hours(1);
const ARBITRATION_RETRY_STEPS: u32 = 6;

#[derive(Serialize, Deserialize)]
struct StoredRivenData {
    etag: Option<String>,
    data: RivenData,
}

pub(super) async fn world_state_task<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) -> ! {
    let mut fetched_at: Option<Instant> = None;
    loop {
        let interval = read(&state.settings).world_state_interval();
        if fetched_at.is_none_or(|at| at.elapsed() >= interval) {
            fetched_at = Some(Instant::now());
            match refresh_world_state(&app, &state).await {
                Ok(events) => dispatch(&app, &state, events).await,
                Err(error) => warn!(?error, "World state refresh failed"),
            }
        } else {
            let events = lock(&state.core).evaluate_world_state(Utc::now());
            dispatch(&app, &state, events).await;
        }

        event_feed::publish_reset_checklist(&app, &state);
        tokio::time::sleep(Duration::from_secs(15)).await;
    }
}

async fn refresh_world_state<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
) -> anyhow::Result<Vec<CoreEvent>> {
    let body = fetch_text(&state.http, wf_worldstate::WORLD_STATE_URL)
        .await
        .context("Requesting the world state")?;
    let now = Utc::now();
    let world = WorldState::parse(&body).context("Parsing the world state")?;
    write(&state.status).world_state_at = Some(now);
    let events = lock(&state.core).ingest_world_state(world, now);
    emit(app, AppEvent::WorldStateUpdated);
    Ok(events)
}

#[derive(Serialize, Deserialize)]
struct StoredArbitrationForecast {
    schedule: String,
    fetched_at: DateTime<Utc>,
    tiers: Option<String>,
    tiers_fetched_at: Option<DateTime<Utc>>,
}

impl StoredArbitrationForecast {
    fn load_into(&self, core: &mut Core, now: DateTime<Utc>) -> anyhow::Result<()> {
        let forecast = ArbitrationForecast::parse(&self.schedule)
            .context("Parsing the arbitration forecast")?;
        let tiers_current = self
            .tiers_fetched_at
            .is_some_and(|at| now.signed_duration_since(at) <= ARBITRATION_TIERS_MAX_AGE);
        let tiers = match self.tiers.as_deref().filter(|_| tiers_current) {
            Some(text) => ArbitrationTiers::parse(text).context("Parsing the arbitration tiers")?,
            None => ArbitrationTiers::default(),
        };
        info!(
            hours = forecast.hours(),
            nodes = forecast.unique_node_ids().len(),
            tiers = tiers.len(),
            fetched_at = %self.fetched_at,
            "Arbitration forecast loaded"
        );
        core.set_arbitration_forecast(forecast, tiers, self.fetched_at);
        Ok(())
    }
}

pub(super) async fn arbitration_task<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) -> ! {
    let mut stored = stored_arbitration_forecast(&state).await;
    let mut failures: u32 = 0;
    loop {
        let now = Utc::now();
        let due = stored.as_ref().is_none_or(|stored| {
            now.signed_duration_since(stored.fetched_at) >= ARBITRATION_INTERVAL
        });
        if due {
            match refresh_arbitration_forecast(&app, &state, stored.as_ref()).await {
                Ok(fresh) => {
                    failures = 0;
                    stored = Some(fresh);
                }
                Err(error) => {
                    failures = (failures + 1).min(ARBITRATION_RETRY_STEPS);
                    warn!(?error, "Arbitration forecast not refreshed");
                }
            }
            state
                .arbitration_refresh_failed
                .store(failures > 0, Ordering::Relaxed);
        }

        let wait = if failures > 0 {
            Duration::from_mins(1 << (failures - 1)).min(ARBITRATION_RETRY_CAP)
        } else {
            let next =
                stored.as_ref().map_or(now, |stored| stored.fetched_at) + ARBITRATION_INTERVAL;
            next.signed_duration_since(Utc::now())
                .to_std()
                .unwrap_or(Duration::ZERO)
                .max(Duration::from_mins(1))
        };
        tokio::time::sleep(wait).await;
    }
}

async fn stored_arbitration_forecast(state: &Arc<AppState>) -> Option<StoredArbitrationForecast> {
    let owned = Arc::clone(state);
    let loaded = try_blocking(move || {
        let core = lock(&owned.core);
        let stored = core
            .store()
            .setting::<StoredArbitrationForecast>(ARBITRATION_FORECAST_KEY)
            .context("Reading the stored arbitration forecast")?;
        drop(core);
        Ok(stored)
    })
    .await;
    let stored = match loaded {
        Ok(Some(stored)) => stored,
        Ok(None) => {
            debug!("No stored arbitration forecast yet");
            return None;
        }
        Err(error) => {
            warn!(?error, "Stored arbitration forecast unreadable");
            return None;
        }
    };
    let loaded = stored.load_into(&mut lock(&state.core), Utc::now());
    match loaded {
        Ok(()) => Some(stored),
        Err(error) => {
            warn!(?error, "Stored arbitration forecast unusable");
            None
        }
    }
}

async fn refresh_arbitration_forecast<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    previous: Option<&StoredArbitrationForecast>,
) -> anyhow::Result<StoredArbitrationForecast> {
    let schedule = fetch_text(&state.http, ARBITRATION_FORECAST_URL)
        .await
        .context("Requesting the arbitration forecast")?;
    let forecast =
        ArbitrationForecast::parse(&schedule).context("Parsing the arbitration forecast")?;
    let now = Utc::now();
    if forecast.starts() > now || forecast.ends() <= now + TimeDelta::hours(1) {
        anyhow::bail!("The arbitration forecast does not cover the next hour");
    }

    let fetched_tiers = fetch_text(&state.http, ARBITRATION_TIERS_URL)
        .await
        .and_then(|text| ArbitrationTiers::parse(&text).map(|_| text));
    let tiers = match fetched_tiers {
        Ok(text) => Some((text, now)),
        Err(error) => {
            warn!(%error, "Arbitration tiers not refreshed, keeping the stored ones");
            previous.and_then(|previous| previous.tiers.clone().zip(previous.tiers_fetched_at))
        }
    };
    let stored = StoredArbitrationForecast {
        schedule,
        fetched_at: now,
        tiers: tiers.as_ref().map(|(text, _)| text.clone()),
        tiers_fetched_at: tiers.map(|(_, at)| at),
    };

    let mut core = lock(&state.core);
    stored.load_into(&mut core, now)?;
    if let Err(error) = core.store().set_setting(ARBITRATION_FORECAST_KEY, &stored) {
        warn!(%error, "Arbitration forecast store write failed");
    }
    drop(core);

    emit(app, AppEvent::WorldStateUpdated);
    Ok(stored)
}

pub(super) async fn price_task<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) -> ! {
    let mut client = bulk_prices(state.http.clone());

    let mut rivens = bulk_riven_data(state.http.clone());
    if let Some(stored) = stored_riven_data(&state).await {
        info!(
            weapons = stored.data.weapons.len(),
            good_rolls = stored.data.good_rolls.len(),
            updated_at = stored.data.updated_at,
            "Riven weapon and roll data restored from the store"
        );
        lock(&state.core).set_riven_data(stored.data);
        rivens = rivens.with_etag(stored.etag);
    }

    let mut checked_at: Option<Instant> = None;
    let mut rivens_checked_at: Option<Instant> = None;
    loop {
        let mut wait = PRICE_INTERVAL;

        if checked_at.is_none_or(|at| at.elapsed() >= PRICE_INTERVAL) {
            match load_price_table(&app, &state, &mut client).await {
                Ok(_) => checked_at = Some(Instant::now()),
                Err(error) => {
                    checked_at = None;
                    wait = PRICE_RETRY;
                    warn!(
                        url = client.url(),
                        error = %error.brief(),
                        "Price table not fetched, next attempt in a minute"
                    );
                }
            }
        }

        if rivens_checked_at.is_none_or(|at| at.elapsed() >= RIVEN_INTERVAL) {
            match load_riven_data(&app, &state, &mut rivens).await {
                Ok(()) => rivens_checked_at = Some(Instant::now()),
                Err(error) => {
                    rivens_checked_at = None;
                    wait = wait.min(PRICE_RETRY);
                    warn!(
                        url = rivens.url(),
                        error = %error.brief(),
                        "Riven weapon and roll data not fetched, next attempt in a minute"
                    );
                }
            }
        }

        tokio::select! {
            () = state.prices_wake.notified() => {}
            () = tokio::time::sleep(wait) => {}
        }
    }
}

async fn stored_riven_data(state: &Arc<AppState>) -> Option<StoredRivenData> {
    let owned = Arc::clone(state);
    let loaded = try_blocking(move || {
        lock(&owned.core)
            .store()
            .setting::<StoredRivenData>(RIVEN_DATA_KEY)
            .map_err(Into::into)
    })
    .await;
    let Ok(stored) = loaded else {
        info!("Stored riven data has an older shape, downloading again");
        return None;
    };

    if stored.is_none() {
        debug!("No stored riven data yet");
    }

    stored
}

async fn load_riven_data<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    client: &mut Etagged<RivenData>,
) -> wf_market::Result<()> {
    let Some(data) = client.fetch().await? else {
        debug!("Riven data etag matches, nothing to load");
        return Ok(());
    };

    info!(
        weapons = data.weapons.len(),
        attributes = data.attributes.len(),
        good_rolls = data.good_rolls.len(),
        updated_at = data.updated_at,
        "Riven weapon and roll data downloaded"
    );

    let stored = StoredRivenData {
        etag: client.etag().map(str::to_owned),
        data,
    };
    let mut core = lock(&state.core);
    if let Err(error) = core.store().set_setting(RIVEN_DATA_KEY, &stored) {
        warn!(%error, "Riven data store write failed");
    }
    core.set_riven_data(stored.data);
    drop(core);

    emit(app, AppEvent::RivenDataUpdated);
    Ok(())
}

async fn load_price_table<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    client: &mut Etagged<PriceTable>,
) -> wf_market::Result<usize> {
    market::market_items(state).await;

    let now = Utc::now();
    let Some(table) = client.fetch().await? else {
        state.prices.checked(now);
        debug!("Bulk price table not modified since the last fetch");
        emit_status(app, state);
        return Ok(0);
    };

    let loaded = state.prices.load(&table, now);
    info!(
        loaded,
        count = table.count,
        updated_at = table.updated_at,
        "Prices refreshed from the bulk table"
    );

    emit(app, AppEvent::PricesUpdated);
    emit_status(app, state);
    Ok(loaded)
}
