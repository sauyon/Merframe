use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use futures_util::StreamExt;
use futures_util::pin_mut;
use tauri::{AppHandle, Runtime};
use tracing::{debug, warn};
use wf_core::Ducatering;
use wf_log::Event as LogEvent;

use super::core_events;
use super::inventory::capture;
use super::rewards::{handle_reward_screen, watch_relic_picker};
use crate::overlay;
use crate::runtime::update_status;
use crate::state::{AppState, lock, read};

pub(crate) async fn log_task<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) {
    let mut missing_reported = false;
    loop {
        let (path, selection) = {
            let settings = read(&state.settings);
            (settings.log_path(), settings.log_selection())
        };
        let Some(path) = path else {
            if !std::mem::replace(&mut missing_reported, true) {
                warn!("No EE.log found, looking again every 10 s");
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
            continue;
        };
        update_status(&app, &state, |status| {
            status.log_file = Some(path.display().to_string());
        });
        match wf_log::lines(&path, selection) {
            Ok(stream) => {
                replay_log_head(&app, &state, &path).await;
                update_status(&app, &state, |status| status.log_attached = true);
                pin_mut!(stream);
                while let Some(line) = stream.next().await {
                    match line {
                        Ok(source) => {
                            if let Some(event) = wf_log::classify(&source.line) {
                                let started = Instant::now();
                                if !matches!(event, LogEvent::InputMappingReset) {
                                    debug!(
                                        event = event.label(),
                                        origin = source.origin.label(),
                                        at = source.line.time,
                                        "Log event"
                                    );
                                }
                                handle_log_event(&app, &state, event, source.line.time).await;
                                if started.elapsed() > Duration::from_secs(1) {
                                    warn!(
                                        millis = started.elapsed().as_millis(),
                                        "Log event handled slowly"
                                    );
                                }
                            }
                        }
                        Err(error) => warn!(%error, "EE.log line was unreadable, skipped"),
                    }
                }
                update_status(&app, &state, |status| status.log_attached = false);
                let finished = core_events(&app, &state, "log end", |core| {
                    core.finish_arbitration_run(Utc::now())
                })
                .await;
                if let Err(error) = finished {
                    warn!(%error, "Core rejected the log end");
                }
                warn!("Log stream ended, reattaching in 10 s");
            }
            Err(error) => warn!(%error, "Opening EE.log failed, next attempt in 10 s"),
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

async fn replay_log_head<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, path: &Path) {
    let lines = match wf_log::read_all(path) {
        Ok(lines) => lines,
        Err(error) => {
            warn!(%error, "Reading the existing EE.log failed");
            return;
        }
    };
    let clock = lines.iter().find_map(|line| match wf_log::classify(line) {
        Some(event @ LogEvent::ClockAnchor { .. }) => Some((event, line.time)),
        _ => None,
    });
    if let Some((event, at)) = clock {
        handle_log_event(app, state, event, at).await;
    }
    let monitor = lines
        .iter()
        .rev()
        .filter(|line| line.message.starts_with("Monitor Info ("))
        .find_map(|line| match wf_log::classify(line) {
            Some(event @ LogEvent::GameMonitor(_)) => Some((event, line.time)),
            _ => None,
        });
    if let Some((event, at)) = monitor {
        overlay::on_log_event(app, state, &event, at);
    }
}

async fn handle_log_event<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    event: LogEvent,
    at: f64,
) {
    if let LogEvent::WindowFocus { focused } = &event {
        state.focus.changed(*focused);
    }
    overlay::on_log_event(app, state, &event, at);
    if matches!(
        event,
        LogEvent::InventorySynced | LogEvent::InventoryCommitted
    ) {
        capture(app, state);
    }
    if matches!(event, LogEvent::RelicSelectScreenLoaded) {
        watch_relic_picker(state);
    }
    if let LogEvent::TradeScreen { visible } = event {
        state.trade_screen_open.send_replace(visible);
    }
    if let LogEvent::DucatKiosk { visible } = event {
        state.kiosk_open.send_replace(visible);
    }
    if matches!(event, LogEvent::DucatSale)
        && state
            .ducatering
            .send_if_modified(|run| run.as_mut().is_some_and(Ducatering::sold))
    {
        debug!("Ducatering sale recorded");
    }
    let scan_rewards = matches!(
        event,
        LogEvent::SquadRewardInfoComplete | LogEvent::RelicRewardsShown
    );
    let handled = core_events(app, state, "log event", move |core| {
        core.handle_log_event(&event, Utc::now(), at)
    })
    .await;
    if let Err(error) = handled {
        warn!(%error, "Core rejected the log event");
    }

    let started = scan_rewards && lock(&state.core).start_reward_scan();
    if started {
        let app = app.clone();
        let state = Arc::clone(state);
        tauri::async_runtime::spawn(async move { handle_reward_screen(&app, &state).await });
    }
}
