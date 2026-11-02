use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Runtime};
use tokio::sync::watch;
use wf_core::{Core, CoreEvent};

use super::{blocking, dispatch};
use crate::state::{AppState, lock};

mod buffs;
mod inventory;
mod kiosk;
mod log;
mod process;
mod rewards;
mod trade;

pub use inventory::acquire;
pub(super) use buffs::buffs_task;
pub(super) use inventory::inventory_task;
pub(super) use kiosk::{ducatering_task, kiosk_task};
pub(super) use log::log_task;
pub(super) use process::process_task;
pub(super) use trade::trade_task;

async fn core_events<R, F>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    input: &'static str,
    call: F,
) -> wf_core::Result<()>
where
    R: Runtime,
    F: FnOnce(&mut Core) -> wf_core::Result<Vec<CoreEvent>> + Send + 'static,
{
    let owned = Arc::clone(state);
    if let Some(events) = blocking(input, move || call(&mut lock(&owned.core))).await {
        dispatch(app, state, events?).await;
    }

    Ok(())
}

fn ticks(period: Duration) -> tokio::time::Interval {
    let mut tick = tokio::time::interval(period);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tick
}

async fn still_open(open: &mut watch::Receiver<bool>, tick: &mut tokio::time::Interval) -> bool {
    tokio::select! {
        _ = open.wait_for(|open| !*open) => false,
        _ = tick.tick() => true,
    }
}
