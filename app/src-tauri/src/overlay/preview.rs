use std::sync::Arc;

use anyhow::bail;
use tauri::{AppHandle, Runtime};
use wf_core::{Ducatering, ItemSummary};

use super::relic::recommendation_trigger;
use super::riven::show_riven;
use super::{Kind, enabled, hide, on_ducatering, show, show_rewards};
use crate::state::{AppState, lock, read};

pub fn toggle_preview<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    kind: Kind,
) -> anyhow::Result<()> {
    if !enabled(&read(&state.settings), kind) {
        bail!("The overlay is turned off");
    }

    let previewed = lock(&state.overlays.state).preview;
    if let Some(previewed) = previewed {
        hide(app, state, previewed);
    }
    if previewed == Some(kind) {
        return Ok(());
    }

    lock(&state.overlays.state).preview = Some(kind);
    let shown = show_sample(app, state, kind);
    if shown.is_err() {
        lock(&state.overlays.state).preview = None;
    }

    shown
}

fn show_sample<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    kind: Kind,
) -> anyhow::Result<()> {
    match kind {
        Kind::RelicReward => {
            let rewards = sample_parts(&ducatering_run(state)?)?;
            let screen = lock(&state.core).recommend(&rewards);
            show_rewards(app, state, None, screen);
        }
        Kind::RelicRecommendation => {
            let trigger = recommendation_trigger(&read(&state.settings), None);
            show(app, state, kind, |slots| {
                slots.recommendation = Some(trigger);
            });
        }
        Kind::Riven => {
            let tab = lock(&state.core).rivens_tab().ok_or_else(no_inventory)?;
            let Some(row) = tab.unveiled.into_iter().next() else {
                bail!("No unveiled riven in the inventory");
            };
            show_riven(app, state, row);
        }
        Kind::Ducatering => {
            let progress = ducatering_run(state)?.progress();
            on_ducatering(app, state, Some(progress));
        }
        Kind::Notification => bail!("Notifications are tested from the settings page"),
        Kind::Buffs => bail!("The buff strip shows live buffs in a mission"),
    }

    Ok(())
}

fn ducatering_run(state: &AppState) -> anyhow::Result<Ducatering> {
    let filter = read(&state.settings).overlays.ducatering.filter();
    lock(&state.core)
        .ducatering(&filter)
        .ok_or_else(no_inventory)
}

fn no_inventory() -> anyhow::Error {
    anyhow::anyhow!("No inventory loaded yet")
}

fn sample_parts(run: &Ducatering) -> anyhow::Result<Vec<ItemSummary>> {
    let parts: Vec<ItemSummary> = run
        .progress()
        .items
        .into_iter()
        .take(4)
        .map(|entry| entry.item)
        .collect();
    if parts.is_empty() {
        bail!("No Prime parts match the Ducatering filters");
    }

    Ok(parts)
}
