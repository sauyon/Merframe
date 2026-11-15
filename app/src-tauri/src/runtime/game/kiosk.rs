use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tauri::{AppHandle, Runtime};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tokio::sync::watch;
use tracing::{debug, warn};
use wf_core::{Ducatering, DucateringStep, ScannedKiosk, ScannedKioskItem};
use wf_scan::DucatKiosk;

use super::{still_open, ticks};
use crate::overlay;
use crate::runtime::attach::{Attached, attach_or_warn, read_screen};
use crate::runtime::blocking;
use crate::state::{AppState, lock, read};

const KIOSK_TICK: Duration = Duration::from_millis(100);

fn scanned_kiosk(kiosk: &DucatKiosk) -> ScannedKiosk {
    let items = |items: &[wf_scan::KioskItem]| {
        items
            .iter()
            .map(|item| ScannedKioskItem {
                item_type: item.item_type.clone(),
                name: item.name.clone(),
                count: item.count,
                ducats: item.ducats,
            })
            .collect()
    };
    ScannedKiosk {
        owned: items(&kiosk.owned),
        marked: items(&kiosk.marked),
    }
}

fn step_ducatering<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, kiosk: &ScannedKiosk) {
    let mut step = DucateringStep::Waiting;
    let moved = state.ducatering.send_if_modified(|run| {
        let Some(run) = run.as_mut() else {
            return false;
        };
        let before = run.state();
        step = run.advance(kiosk);
        run.state() != before
    });
    if !moved {
        return;
    }
    match step {
        DucateringStep::Waiting => {}
        DucateringStep::Search { position, name } => {
            debug!(position, "Ducatering moved");
            let clipboard = read(&state.settings)
                .overlays
                .ducatering
                .ducatering_clipboard;
            if clipboard && let Err(error) = app.clipboard().write_text(name) {
                warn!(%error, "Item name not copied to the clipboard");
            }
        }
        DucateringStep::Finished => debug!("Ducatering finished"),
    }
}

pub(crate) async fn kiosk_task<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) {
    let mut open = state.kiosk_open.subscribe();
    while open.changed().await.is_ok() {
        if *open.borrow_and_update() {
            watch_kiosk(&app, &state, &mut open).await;
        }
    }
}

async fn watch_kiosk<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    open: &mut watch::Receiver<bool>,
) {
    let Some(Attached { reader, lua }) = attach_or_warn(
        state,
        "Ducat Kiosk not watched, Ducatering does not advance",
    )
    .await
    else {
        return;
    };

    let filter = {
        let settings = read(&state.settings);
        if !overlay::enabled(&settings, overlay::Kind::Ducatering) {
            return;
        }
        settings.overlays.ducatering.filter()
    };
    let owned = Arc::clone(state);
    let run = blocking("build the ducatering run", move || {
        lock(&owned.core).ducatering(Utc::now(), &filter)
    })
    .await
    .flatten();
    state.ducatering.send_replace(run);
    let mut tick = ticks(KIOSK_TICK);
    let mut last = None;
    let mut script = None;
    while still_open(open, &mut tick).await {
        let kiosk = read_screen(
            "read ducat kiosk",
            &reader,
            lua,
            &mut script,
            DucatKiosk::read,
        )
        .await
        .flatten();
        if let Some(settled) = &kiosk
            && last.as_ref() == Some(settled)
        {
            step_ducatering(app, state, &scanned_kiosk(settled));
        }
        last = kiosk;
    }
    state.ducatering.send_replace(None);
}

pub(crate) async fn ducatering_task<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) {
    let mut run = state.ducatering.subscribe();
    while run.changed().await.is_ok() {
        let progress = run.borrow_and_update().as_ref().map(Ducatering::progress);
        overlay::on_ducatering(&app, &state, progress);
    }
}
