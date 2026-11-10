use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use chrono::Utc;
use tauri::{AppHandle, Runtime};
use tracing::{debug, warn};
use wf_core::{CoreEvent, MissionEndReward};
use wf_mem::MemoryReader;
use wf_scan::{HttpClients, InventoryBuffer};

use crate::overlay;
use crate::runtime::attach::open_game;
use crate::runtime::{AppEvent, dispatch, emit, market_loop, try_blocking, update_status};
use crate::state::{AppState, InventorySource, QueueSession, lock, read};

pub(crate) async fn inventory_task<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) {
    loop {
        state.rescan.notified().await;
        if read(&state.status).game_detected {
            acquire(&app, &state).await;
        }
    }
}

pub(super) fn capture<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>) {
    if state.capturing.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let state = Arc::clone(state);
    tauri::async_runtime::spawn(async move {
        acquire_with(&app, &state, |state| {
            ingest_window(state, Duration::from_secs(10))
        })
        .await;
        state.capturing.store(false, Ordering::SeqCst);
    });
}

pub async fn acquire<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>) -> bool {
    acquire_with(app, state, |state| ingest_window(state, Duration::ZERO)).await
}

async fn acquire_with<R, F>(app: &AppHandle<R>, state: &Arc<AppState>, ingest: F) -> bool
where
    R: Runtime,
    F: FnOnce(&Arc<AppState>) -> anyhow::Result<Ingested> + Send + 'static,
{
    update_status(app, state, |status| status.scanning = true);

    let owned = Arc::clone(state);
    let ingested = match try_blocking(move || ingest(&owned)).await {
        Ok(ingested) => ingested,
        Err(error) => {
            let cause = format!("{error:#}");
            warn!(cause, "Inventory not ingested");
            update_status(app, state, |status| {
                status.scanning = false;
                status.last_scan_error = Some(cause);
            });
            return false;
        }
    };

    let (trades_remaining, snapshot) = {
        let core = lock(&state.core);
        (
            core.inventory().map(|inventory| inventory.trades_remaining),
            core.latest_snapshot().cloned(),
        )
    };
    let last_trade_done = update_status(app, state, |status| {
        status.scanning = false;
        status.last_scan_at = Some(Utc::now());
        status.last_scan_error = None;
        status.source = InventorySource::Live;
        if let Some(snapshot) = &snapshot {
            status.remember_snapshot(snapshot);
        }
        let done =
            status.trades_remaining.is_some_and(|left| left > 0) && trades_remaining == Some(0);
        status.trades_remaining = trades_remaining;
        done
    });
    if last_trade_done {
        market_loop::last_trade_done(app, state);
    }

    match ingested {
        Ingested::Newer {
            events,
            rewards_attached,
        } => {
            emit(app, AppEvent::InventoryUpdated(state.status_snapshot()));
            if rewards_attached {
                emit(app, AppEvent::ArbitrationRunsUpdated);
            }
            overlay::on_inventory_updated(app, state);
            state.prices_wake.notify_one();
            dispatch(app, state, events).await;
        }
        Ingested::Unchanged => {}
    }
    true
}

enum Ingested {
    Newer {
        events: Vec<CoreEvent>,
        rewards_attached: bool,
    },
    Unchanged,
}

fn needs_resolve(session: Option<&QueueSession>, pid: u32) -> bool {
    session.is_none_or(|session| session.pid != pid || session.clients.is_none())
}

fn http_clients(
    state: &Arc<AppState>,
    pid: u32,
    reader: &dyn MemoryReader,
) -> anyhow::Result<HttpClients> {
    let mut session = lock(&state.http_clients);
    if needs_resolve(session.as_ref(), pid) {
        let clients = match HttpClients::locate(reader) {
            Ok(clients) => clients,
            Err(error) => {
                warn!(pid, %error, "Game HTTP client lookup failed");
                None
            }
        };
        *session = Some(QueueSession { pid, clients });
    }
    session
        .as_ref()
        .and_then(|session| session.clients.clone())
        .context("The game's HTTP client was not found")
}

fn ingest_window(state: &Arc<AppState>, window: Duration) -> anyhow::Result<Ingested> {
    let (pid, reader) = open_game(state)?;
    let clients = http_clients(state, pid, reader.as_ref())?;
    let Some(buffer) = clients.await_inventory(reader.as_ref(), window) else {
        debug!(
            window_millis = window.as_millis(),
            "No inventory response passed through the HTTP queue"
        );
        return Ok(Ingested::Unchanged);
    };
    ingest_buffer(state, &buffer)
}

fn ingest_buffer(state: &Arc<AppState>, buffer: &InventoryBuffer) -> anyhow::Result<Ingested> {
    debug!(
        addr = format!("{:#x}", buffer.addr),
        len = buffer.len,
        last_sync = buffer.last_sync,
        "Inventory document accepted"
    );
    if let Some(current) = read(&state.status).last_sync_oid.clone()
        && buffer.last_sync <= current
    {
        debug!(
            last_sync = buffer.last_sync,
            current, "Captured inventory is not newer, skipped"
        );
        return Ok(Ingested::Unchanged);
    }

    let now = Utc::now();
    let mut core = lock(&state.core);
    let events = core
        .ingest_inventory(&buffer.json, now)
        .context("Ingesting the inventory")?;
    let rewards_attached = match &buffer.mission_rewards {
        Some(rewards) => {
            let rewards: Vec<MissionEndReward> = rewards
                .iter()
                .map(|reward| MissionEndReward {
                    store_item: reward.store_item.clone(),
                    item_count: reward.item_count,
                })
                .collect();
            core.record_mission_end(&rewards)
        }
        None => false,
    };
    Ok(Ingested::Newer {
        events,
        rewards_attached,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(pid: u32) -> QueueSession {
        QueueSession { pid, clients: None }
    }

    #[test]
    fn resolve_without_clients() {
        let unresolved = session(15556);
        assert!(needs_resolve(None, 15556));
        assert!(needs_resolve(Some(&unresolved), 15556));
        assert!(needs_resolve(Some(&unresolved), 280));
    }
}
