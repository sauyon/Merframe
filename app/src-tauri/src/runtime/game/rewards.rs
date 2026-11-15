use std::mem::DropGuard;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use chrono::Utc;
use tauri::{AppHandle, Runtime};
use tracing::{debug, warn};
use wf_core::ScannedRewards;
use wf_mem::MemoryReader;
use wf_scan::{LuaState, LuaTable};

use super::core_events;
use crate::runtime::attach::{Attached, attach_or_warn, read_screen, reattach};
use crate::runtime::blocking;
use crate::state::{AppState, lock};

const REWARD_TICK: Duration = Duration::from_millis(100);
const REWARD_TICKS: usize = 50;
const PICKER_TICK: Duration = Duration::from_millis(100);
const PICKER_LIFETIME: Duration = Duration::from_secs(600);
const PICKER_GRACE: Duration = Duration::from_secs(2);

pub(super) async fn handle_reward_screen<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>) {
    let generation = lock(&state.core).reward_screen_generation();
    let Some(screen) = scan_reward_screen(state).await else {
        return;
    };

    let handled = core_events(app, state, "scanned rewards", move |core| {
        core.handle_reward_screen(Utc::now(), generation, screen)
    })
    .await;
    if let Err(error) = handled {
        warn!(%error, generation, "Core rejected the scanned rewards");
    }
}

pub(super) fn watch_relic_picker(state: &Arc<AppState>) {
    if state.watching_picker.swap(true, Ordering::SeqCst) {
        return;
    }

    let state = DropGuard::new(Arc::clone(state), |state| {
        state.watching_picker.store(false, Ordering::SeqCst);
    });
    tauri::async_runtime::spawn(async move {
        let started = Instant::now();
        let mut closed_since = None;
        let mut attached = None;
        while started.elapsed() < PICKER_LIFETIME {
            let owned = Arc::clone(&state);
            let cached = attached.take();
            let (picker, kept) =
                blocking("read relic picker", move || relic_picker(&owned, cached))
                    .await
                    .unzip();
            attached = kept.flatten();
            match picker {
                Some(Picker::Open(pick)) => {
                    closed_since = None;
                    if let Some(pick) = pick
                        && lock(&state.relic_picks)
                            .insert(pick.handle, pick.relic_type.clone())
                            .is_none()
                    {
                        debug!(relic_type = pick.relic_type, "Relic selected in the picker");
                    }
                }
                Some(Picker::Closed) | None => {
                    let since = *closed_since.get_or_insert_with(Instant::now);
                    if since.elapsed() >= PICKER_GRACE {
                        break;
                    }
                }
            }
            tokio::time::sleep(PICKER_TICK).await;
        }
    });
}

enum Picker {
    Closed,
    Open(Option<wf_scan::RelicPick>),
}

fn relic_picker(state: &AppState, cached: Option<Attached>) -> (Picker, Option<Attached>) {
    let Ok(attached) = reattach(state, cached) else {
        return (Picker::Closed, None);
    };

    let Attached { reader, lua } = &attached;
    let open = wf_scan::relic_picker_open(reader.as_ref(), lua)
        .then(|| wf_scan::relic_pick(reader.as_ref(), lua));
    (open.map_or(Picker::Closed, Picker::Open), Some(attached))
}

fn reward_screen(
    state: &AppState,
    reader: &dyn MemoryReader,
    lua: &LuaState,
    script: &mut Option<LuaTable>,
) -> ScannedRewards {
    let tiles = wf_scan::reward_screen(reader, lua, script);
    let own_relic_type = wf_scan::confirmed_relic(reader, lua)
        .and_then(|handle| lock(&state.relic_picks).get(&handle).cloned());
    ScannedRewards {
        own_relic_type,
        rewards: tiles.into_iter().map(|tile| tile.store_item).collect(),
    }
}

async fn scan_reward_screen(state: &Arc<AppState>) -> Option<ScannedRewards> {
    let Attached { reader, lua } =
        attach_or_warn(state, "Reward screen read from game memory failed").await?;
    let mut seen = ScannedRewards::default();
    let mut script = None;
    for _ in 0..REWARD_TICKS {
        let owned = Arc::clone(state);
        let screen = read_screen(
            "read reward screen",
            &reader,
            lua,
            &mut script,
            move |reader, lua, script| reward_screen(&owned, reader, lua, script),
        )
        .await?;
        if !screen.rewards.is_empty() && screen == seen {
            if screen.own_relic_type.is_none() {
                debug!("Reward screen settled without a known relic for the local player");
            }
            return Some(screen);
        }
        seen = screen;
        tokio::time::sleep(REWARD_TICK).await;
    }
    debug!("Reward screen held no settled tiles");
    None
}
