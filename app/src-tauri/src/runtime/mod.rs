use std::sync::Arc;

use anyhow::Context;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_clipboard_manager::ClipboardExt;
#[cfg(not(target_os = "linux"))]
use tauri_plugin_notification::NotificationExt;
use tracing::{debug, error, info, warn};
use wf_core::{ArbitrationAlert, CoreEvent};

use crate::envelope::CoreEventEnvelope;
use crate::event_feed::{self, EventFeed};
use crate::market::{MarketSnapshot, Presence};
use crate::notice::Notice;
use crate::overlay::{self, OverlayState};
pub use crate::runtime::market_loop::MarketAutoClose;
use crate::settings::{self, DiscordMirroredAlert, Settings};
use crate::state::{AppState, GameStatus, InventorySource, lock, read, write};

mod attach;
mod feeds;
mod game;
mod market_loop;

use feeds::{arbitration_task, price_task, world_state_task};
use game::{
    buffs_task, ducatering_task, inventory_task, kiosk_task, log_task, process_task, trade_task};
use market_loop::{auto_close, last_trade_done, market_presence_task, market_task};

pub(crate) use attach::{Attached, reattach};
pub use game::acquire;
pub(crate) use market_loop::trade_side;

#[derive(Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum AppEvent {
    CoreEvent(CoreEventEnvelope),
    StatusUpdated(GameStatus),
    InventoryUpdated(GameStatus),
    SettingsUpdated(Box<Settings>),
    TradesUpdated,
    WorldStateUpdated,
    RivenDataUpdated,
    PricesUpdated,
    FavouriteUpdated,
    MarketUpdated(MarketSnapshot),
    MarketAutoClosed(MarketAutoClose),
    MarketSessionRejected,
    MarketPresence(Presence),
    OverlayState(Box<OverlayState>),
}

const RIVEN_DATA_KEY: &str = "riven_data";
const APP_TITLE: &str = "Merframe";
const NOTIFICATION_SOUND: &str = if cfg!(windows) {
    "Default"
} else {
    "message-new-instant"
};

pub fn spawn<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) {
    tauri::async_runtime::spawn(bootstrap(app.clone(), Arc::clone(&state)));
    tauri::async_runtime::spawn(process_task(app.clone(), Arc::clone(&state)));
    tauri::async_runtime::spawn(log_task(app.clone(), Arc::clone(&state)));
    tauri::async_runtime::spawn(inventory_task(app.clone(), Arc::clone(&state)));
    tauri::async_runtime::spawn(ducatering_task(app.clone(), Arc::clone(&state)));
    tauri::async_runtime::spawn(trade_task(app.clone(), Arc::clone(&state)));
    tauri::async_runtime::spawn(kiosk_task(app.clone(), Arc::clone(&state)));
    tauri::async_runtime::spawn(buffs_task(app.clone(), Arc::clone(&state)));
    tauri::async_runtime::spawn(world_state_task(app.clone(), Arc::clone(&state)));
    tauri::async_runtime::spawn(arbitration_task(app.clone(), Arc::clone(&state)));
    tauri::async_runtime::spawn(market_task(app.clone(), Arc::clone(&state)));
    tauri::async_runtime::spawn(market_presence_task(app.clone(), Arc::clone(&state)));
    tauri::async_runtime::spawn(event_feed::serve(app.clone(), Arc::clone(&state)));
    tauri::async_runtime::spawn(price_task(app, state));
}

async fn bootstrap<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) {
    load_cached_inventory(&app, &state).await;
    let riven_types = lock(&state.core).catalog().data().riven_data().len();
    if riven_types == 0 {
        warn!("Game data has no riven stat tables");
    } else {
        info!(riven_types, "Riven stat tables loaded");
    }
    emit_status(&app, &state);
}

async fn load_cached_inventory<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>) {
    let owned = Arc::clone(state);
    let loaded = try_blocking(move || {
        lock(&owned.core)
            .load_snapshot()
            .context("Loading the stored inventory")
    })
    .await;
    match loaded {
        Ok(Some(snapshot)) => {
            let trades_remaining = lock(&state.core)
                .inventory()
                .map(|inventory| inventory.trades_remaining);
            info!(
                snapshot = snapshot.id.0,
                last_sync = snapshot.last_sync_oid,
                taken_at = snapshot.taken_at.to_rfc3339(),
                "Last inventory snapshot restored from the store"
            );
            update_status(app, state, |status| {
                status.source = InventorySource::Cached;
                status.remember_snapshot(&snapshot);
                status.trades_remaining = trades_remaining;
            });
            emit(app, AppEvent::InventoryUpdated(state.status_snapshot()));
            state.prices_wake.notify_one();
        }
        Ok(None) => debug!("No stored inventory snapshot yet"),
        Err(error) => warn!(?error, "Stored inventory unreadable"),
    }
}

pub async fn dispatch<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    events: Vec<CoreEvent>,
) {
    for event in events {
        debug!(event = event.label(), "Core event");
        emit(
            app,
            AppEvent::CoreEvent(CoreEventEnvelope::wrap(event.clone())),
        );
        deliver(app, state, &event).await;
    }
}

async fn deliver<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, event: &CoreEvent) {
    let settings = read(&state.settings).clone();
    match event {
        CoreEvent::NewConversation { player, .. } => {
            conversation_alert(app, state, &settings, player).await;
        }
        CoreEvent::RelicRewardScreen { relic, screen } => {
            overlay::on_relic_rewards(app, state, relic.clone(), screen);
            if settings.copy_relic_rewards {
                let line = screen.chat_line();
                if !line.is_empty()
                    && let Err(error) = app.clipboard().write_text(line)
                {
                    warn!(%error, "Relic rewards not copied to the clipboard");
                }
            }
        }
        CoreEvent::TradeCompleted { trade, .. } => {
            info!(
                offered = trade.offered.len(),
                received = trade.received.len(),
                plat = trade.plat,
                "Trade completed"
            );
            debug!(
                offered = ?trade.offered,
                received = ?trade.received,
                "Trade items"
            );
            count_trade(app, state);
            emit(app, AppEvent::TradesUpdated);
            if settings.market.market_auto_close {
                auto_close(app, state, trade).await;
            }
        }
        CoreEvent::FissureAlert { fissure } => {
            let notice = Notice::fissure(fissure);
            world_alert(
                app,
                state,
                &settings,
                &notice,
                DiscordMirroredAlert::Fissure,
            )
            .await;
        }
        CoreEvent::WorldRewardAlert { reward } => {
            let notice = Notice::reward(reward);
            world_alert(app, state, &settings, &notice, DiscordMirroredAlert::Reward).await;
        }
        CoreEvent::ArbitrationAlert { alert } => {
            arbitration_alert(app, state, &settings, alert).await;
        }
        CoreEvent::TimerAlert {
            name,
            next_state,
            remaining_secs,
            ..
        } => {
            let notice = Notice::timer(name, next_state, *remaining_secs);
            world_alert(app, state, &settings, &notice, DiscordMirroredAlert::Timer).await;
        }
        CoreEvent::InventoryUpdated(_) => {}
    }
}

async fn world_alert<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    settings: &Settings,
    notice: &Notice,
    kind: DiscordMirroredAlert,
) {
    let message = settings
        .discord
        .mirrored
        .mirrors(kind)
        .then(|| notice.webhook_message());
    alert(app, state, settings, notice, message).await;
}

async fn conversation_alert<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    settings: &Settings,
    player: &str,
) {
    if state
        .focus
        .suppresses(settings.notifications.notification_only_background)
    {
        return;
    }

    let discord = &settings.discord;
    let message = discord
        .mirrored
        .mirrors(DiscordMirroredAlert::Conversation)
        .then(|| discord.discord_message_template.replace("{tenno}", player));
    alert(app, state, settings, &Notice::conversation(player), message).await;
}

async fn arbitration_alert<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    settings: &Settings,
    alert: &ArbitrationAlert,
) {
    if alert.reminder {
        forget_arbitration_reminder(app, state, &alert.slot.id);
    }

    let notice = Notice::arbitration(alert);
    world_alert(
        app,
        state,
        settings,
        &notice,
        DiscordMirroredAlert::Arbitration,
    )
    .await;
}

fn forget_arbitration_reminder<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, id: &str) {
    let mut updated = read(&state.settings).clone();
    if !updated.alerts.arbitrations.reminders.remove(id) {
        return;
    }

    if let Err(error) = settings::apply(app, state, &updated) {
        warn!(%error, "Settings not saved after the arbitration reminder fired");
    }
    emit(app, AppEvent::SettingsUpdated(Box::new(updated)));
}

fn count_trade<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>) {
    let remaining = {
        let mut status = write(&state.status);
        let Some(left) = status.trades_remaining.filter(|left| *left > 0) else {
            return;
        };

        status.trades_remaining = Some(left - 1);
        left - 1
    };

    emit_status(app, state);
    if remaining == 0 {
        last_trade_done(app, state);
    }
}

async fn alert<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    settings: &Settings,
    notice: &Notice,
    webhook: Option<String>,
) {
    if settings.notifications.windows_notifications_enabled
        && let Err(error) = show(app, &notice.title, &notice.body, settings).await
    {
        warn!(
            ?error,
            notice.title, notice.body, "Game event notification not shown"
        );
    }

    if let Some(message) = webhook {
        post_webhook(state, settings.discord.webhook(), message).await;
    }
}

#[cfg(not(target_os = "linux"))]
fn show<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
    body: &str,
    settings: &Settings,
) -> std::future::Ready<anyhow::Result<()>> {
    let mut builder = app.notification().builder().title(title).body(body);
    if settings.notifications.sound_notifications_enabled {
        builder = builder.sound(NOTIFICATION_SOUND);
    }
    std::future::ready(builder.show().context("Sending the desktop notification"))
}

#[cfg(target_os = "linux")]
async fn show<R: Runtime>(
    _app: &AppHandle<R>,
    title: &str,
    body: &str,
    settings: &Settings,
) -> anyhow::Result<()> {
    let mut notification = notify_rust::Notification::new();
    notification.summary(title).body(body).auto_icon();
    if settings.notifications.sound_notifications_enabled {
        notification.sound_name(NOTIFICATION_SOUND);
    }
    let handle = notification
        .show_async()
        .await
        .context("Sending the desktop notification")?;
    tauri::async_runtime::spawn(async move {
        handle.wait_for_action_async(|_| {}).await;
    });
    Ok(())
}

pub async fn test_notifications<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    settings: &Settings,
) -> anyhow::Result<()> {
    let discord = &settings.discord;
    let webhook = if discord.mirrored.any() {
        Some(
            discord
                .webhook()
                .context("The Discord webhook URL is empty")?,
        )
    } else {
        None
    };
    let in_game = overlay::enabled(settings, overlay::Kind::Notification);
    if !settings.notifications.windows_notifications_enabled && webhook.is_none() && !in_game {
        anyhow::bail!("No notification channels are enabled");
    }
    if settings.notifications.windows_notifications_enabled {
        show(app, APP_TITLE, "Notifications are working", settings).await?;
    }
    if in_game {
        overlay::notify(
            app,
            state,
            APP_TITLE.to_owned(),
            "Notifications are working".to_owned(),
        );
    }
    if let Some(url) = webhook {
        send_webhook(state, url, "Merframe notifications are working").await?;
    }
    Ok(())
}

async fn post_webhook(state: &Arc<AppState>, url: Option<&str>, message: String) {
    let Some(url) = url else {
        return;
    };
    if let Err(error) = send_webhook(state, url, &message).await {
        warn!(?error, "Discord webhook post failed");
    }
}

async fn send_webhook(state: &Arc<AppState>, url: &str, message: &str) -> anyhow::Result<()> {
    let body = serde_json::json!({ "content": message });
    state
        .http
        .post(url)
        .json(&body)
        .send()
        .await
        .context("Sending the Discord webhook")?
        .error_for_status()
        .context("The Discord webhook returned an error")?;
    Ok(())
}

pub fn emit<R: Runtime>(app: &AppHandle<R>, event: AppEvent) {
    app.state::<EventFeed>().publish(&event);
    if let Err(error) = app.emit("app-event", event) {
        warn!(%error, "Webview emit failed");
    }
}

pub(crate) fn update_status<R: Runtime, T>(
    app: &AppHandle<R>,
    state: &AppState,
    change: impl FnOnce(&mut GameStatus) -> T,
) -> T {
    let mut guard = write(&state.status);
    let outcome = change(&mut guard);
    drop(guard);

    emit_status(app, state);
    outcome
}

pub(crate) fn emit_status<R: Runtime>(app: &AppHandle<R>, state: &AppState) {
    emit(app, AppEvent::StatusUpdated(state.status_snapshot()));
}

pub(crate) async fn blocking<T, F>(task: &'static str, job: F) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(job).await {
        Ok(value) => Some(value),
        Err(error) => {
            error!(task, %error, "Blocking task panicked");
            None
        }
    }
}

pub(crate) async fn try_blocking<T, F>(job: F) -> anyhow::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(anyhow::Error::from)
        .flatten()
}
