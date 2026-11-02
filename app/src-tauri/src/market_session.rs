use std::sync::Arc;

use tauri::{AppHandle, Runtime};
use tracing::warn;
use wf_market::UserStatus;

use crate::market::{MarketOrders, MarketPresenceChoice, Presence};
use crate::runtime::{AppEvent, emit, update_status};
use crate::settings::{self, MarketAccount};
use crate::state::{AppState, write};

pub fn store_token<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    token: &str,
) -> anyhow::Result<()> {
    settings::set_token(app, Some(token))?;
    *write(&state.market) = Arc::new(state.anonymous_market().with_token(token.to_owned()));
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
    *write(&state.market) = Arc::new(state.anonymous_market());
    state
        .listings
        .remember(&state.core, Some(&MarketOrders::default()), Some(&[]));

    update_status(app, state, |status| {
        status.market_account = None;
        status.market_unread = 0;
    });
    set_live(app, state, None);
    settings::set_token(app, None)
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
) -> anyhow::Result<Presence> {
    let mut guard = write(&state.market_presence);
    guard.choice = choice;
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
