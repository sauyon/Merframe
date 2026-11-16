use std::sync::Arc;

use tauri::{AppHandle, Runtime};
use tracing::{debug, warn};
use wf_market::{Client, UserStatus};

use crate::market::{MarketOrders, MarketPresence, MarketPresenceChoice, MarketPresencePause};
use crate::runtime::{AppEvent, emit, update_status};
use crate::settings::{self, MarketAccount};
use crate::state::{AppState, write};

pub fn store_token<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    token: &str,
) -> anyhow::Result<()> {
    replace_token(app, state, &mut write(&state.market), token)
}

pub fn adopt_checked_session<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    checked: &Arc<Client>,
    rotated_token: Option<&str>,
) -> bool {
    let mut market = write(&state.market);
    if !Arc::ptr_eq(&market, checked) {
        return false;
    }

    if let Some(token) = rotated_token {
        debug!("Warframe.market issued a new token");
        if let Err(error) = replace_token(app, state, &mut market, token) {
            warn!(%error, "Rotated token save failed");
        }
    }

    true
}

fn replace_token<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    market: &mut Arc<Client>,
    token: &str,
) -> anyhow::Result<()> {
    settings::set_token(app, Some(token))?;
    *market = Arc::new(state.anonymous_market().with_token(token.to_owned()));
    Ok(())
}

pub fn record_account<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    account: &MarketAccount,
) -> anyhow::Result<()> {
    update_status(app, state, |status| {
        status.market_account = Some(account.clone());
    });
    settings::set_account(app, Some(account))
}

pub fn clear<R: Runtime>(app: &AppHandle<R>, state: &AppState) -> anyhow::Result<()> {
    let mut market = write(&state.market);
    *market = Arc::new(state.anonymous_market());
    let token_removed = settings::set_token(app, None);
    drop(market);

    state
        .listings
        .remember(&state.core, Some(&MarketOrders::default()), Some(&[]));

    update_status(app, state, |status| {
        status.market_account = None;
        status.market_unread = 0;
    });
    set_live(app, state, None);
    token_removed
        .and(settings::set_account(app, None))
        .and(set_presence(app, state, MarketPresenceChoice::default()).map(|_| ()))
}

pub fn set_live<R: Runtime>(app: &AppHandle<R>, state: &AppState, live: Option<UserStatus>) {
    let mut guard = write(&state.market_presence);
    guard.live = live;
    let presence = *guard;
    drop(guard);

    emit(app, AppEvent::MarketPresence(presence));
}

pub fn set_presence<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    choice: MarketPresenceChoice,
) -> anyhow::Result<MarketPresence> {
    let mut guard = write(&state.market_presence);
    guard.choice = choice;
    guard.pause = MarketPresencePause::Off;
    let presence = *guard;
    drop(guard);

    state.market_presence_wake.notify_one();
    emit(app, AppEvent::MarketPresence(presence));
    settings::set_presence(app, choice).map(|()| presence)
}

pub fn clear_rejected<R: Runtime>(app: &AppHandle<R>, state: &AppState) {
    if let Err(error) = clear(app, state) {
        warn!(%error, "Clearing the stored market session failed");
    }
    emit(app, AppEvent::MarketSessionRejected);
}
