use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Runtime};
use wf_core::{ArbitrationRun, ArbitrationRunStatus};

use super::{Kind, hide, show};
use crate::state::{AppState, lock};

const RUN_SUMMARY_LIFETIME: Duration = Duration::from_secs(20);

pub fn show_arbitration_run<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    run: Box<ArbitrationRun>,
) {
    show(app, state, Kind::ArbitrationLive, |slots| {
        slots.arbitration = Some(run);
    });
}

pub fn on_arbitration_run_updated<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    run: Box<ArbitrationRun>,
) {
    if run.status == ArbitrationRunStatus::Recording {
        show_arbitration_run(app, state, run);
    } else {
        hide(app, state, Kind::ArbitrationLive);
    }
}

pub async fn on_arbitration_run_recorded<R: Runtime>(
    app: AppHandle<R>,
    state: Arc<AppState>,
    run: Box<ArbitrationRun>,
) {
    let id = run.id.clone();
    show_arbitration_run(&app, &state, run);
    tokio::time::sleep(RUN_SUMMARY_LIFETIME).await;
    let same_run = lock(&state.overlays.state)
        .arbitration
        .as_ref()
        .is_some_and(|shown| shown.id == id);
    if same_run {
        hide(&app, &state, Kind::ArbitrationLive);
    }
}
