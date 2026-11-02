use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use tracing::{debug, info, warn};
use wf_core::{DucateringProgress, RewardScreen, RivenRow};
use wf_log::{DialogButtons, Event as LogEvent, MonitorRect};
use wf_worldstate::RelicTier;

use crate::runtime::{self, AppEvent, emit};
use crate::settings::{
    Motion, OVERLAY_OPACITY_DEFAULT, OverlayPlacement, RecommendationRefinement, Settings, TextSize,
};
use crate::state::{AppState, lock, read};

mod preview;
mod relic;
mod riven;
mod screens;
mod session;
mod window;

pub use preview::toggle_preview;
pub use riven::riven_item_type;
#[cfg(target_os = "linux")]
pub use session::adopt_xwayland;
use session::uses_windows;
pub use session::{OverlaySupport, probe};

use relic::scan_recommendation;
use riven::{arm_station_answer, on_dialog_answer, show_linked_riven, show_station_selection};
use screens::Screens;
use window::{open, park, place, retire, reveal, screen_of, window_bounds};

pub const KINDS: [Kind; 6] = [
    Kind::RelicReward,
    Kind::RelicRecommendation,
    Kind::Riven,
    Kind::Ducatering,
    Kind::Notification,
    Kind::Buffs,
];

const NOTIFICATION_LIFETIME: Duration = Duration::from_millis(6500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "bindings",
    derive(ts_rs::TS),
    ts(export, rename = "OverlayKind")
)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    RelicReward,
    RelicRecommendation,
    Riven,
    Ducatering,
    Notification,
    Buffs,
}

impl Kind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::RelicReward => "overlay-relic",
            Self::RelicRecommendation => "overlay-recommend",
            Self::Riven => "overlay-riven",
            Self::Ducatering => "overlay-ducatering",
            Self::Notification => "overlay-notify",
            Self::Buffs => "overlay-buffs",
        }
    }

    const fn permanent(self) -> bool {
        matches!(self, Self::Notification | Self::Buffs)
    }

    pub(super) const fn route(self) -> &'static str {
        match self {
            Self::RelicReward => "index.html#/overlay/relic",
            Self::RelicRecommendation => "index.html#/overlay/recommend",
            Self::Riven => "index.html#/overlay/riven",
            Self::Ducatering => "index.html#/overlay/ducatering",
            Self::Notification => "index.html#/overlay/notify",
            Self::Buffs => "index.html#/overlay/buffs",
        }
    }

    pub(super) const fn title(self) -> &'static str {
        match self {
            Self::RelicReward => "Merframe relic rewards",
            Self::RelicRecommendation => "Merframe relic recommendation",
            Self::Riven => "Merframe riven",
            Self::Ducatering => "Merframe Ducatering",
            Self::Notification => "Merframe notification",
            Self::Buffs => "Merframe buffs",
        }
    }
}

pub fn enabled(settings: &Settings, kind: Kind) -> bool {
    settings.overlays.overlays_enabled
        && match kind {
            Kind::RelicReward => settings.overlays.shown.overlay_relic_reward,
            Kind::RelicRecommendation => settings.overlays.shown.overlay_relic_recommendation,
            Kind::Riven => settings.overlays.shown.overlay_riven,
            Kind::Ducatering => settings.overlays.ducatering.overlay_ducatering,
            Kind::Notification => settings.toasts.toasts_in_game,
            Kind::Buffs => settings.overlays.shown.overlay_buffs,
        }
}

fn window_shows(only_while_game_active: bool, focused: Option<bool>, due: bool) -> bool {
    due && !(only_while_game_active && focused == Some(false))
}

fn windowed(state: &AppState, settings: &Settings) -> bool {
    uses_windows(
        settings.overlays.overlay_mode,
        &read(&state.status).overlay_support,
    )
}

fn placement_of(settings: &Settings, kind: Kind) -> OverlayPlacement {
    let placements = &settings.overlays.placements;
    match kind {
        Kind::RelicReward => placements.overlay_relic_reward_placement,
        Kind::RelicRecommendation => placements.overlay_relic_recommendation_placement,
        Kind::Riven => placements.overlay_riven_placement,
        Kind::Ducatering => settings.overlays.ducatering.overlay_ducatering_placement,
        Kind::Notification => settings.toasts.toasts_in_game_position.into(),
        Kind::Buffs => OverlayPlacement::Centre,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct RewardTrigger {
    pub relic: Option<String>,
    pub screen: RewardScreen,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct RecommendationTrigger {
    pub tier: Option<&'static str>,
    pub refinement: RecommendationRefinement,
    pub count: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct RivenTrigger {
    pub item_type: String,
    pub before: Option<RivenRow>,
    pub linked: Option<RivenRow>,
    pub attribution: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct NotificationTrigger {
    pub id: u64,
    pub title: String,
    pub body: String,
}

/// One buff on the countdown strip under the ability dots.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct BuffTimer {
    pub id: String,
    pub label: String,
    /// The ability slot, counted from 1, the buff comes from; `None` for other buffs.
    pub slot: Option<u8>,
    /// The game's icon for a buff outside the ability slots, as an item image name.
    pub icon: Option<String>,
    /// Seconds the buff lasted when it was last applied.
    pub total: f64,
    /// Unix milliseconds the buff runs out, `None` once it has dropped off.
    pub ends_at: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct OverlayState {
    pub seq: u64,
    pub opacity: u8,
    pub text_size: TextSize,
    pub motion: Motion,
    pub ducatering_clipboard: bool,
    pub reward: Option<RewardTrigger>,
    pub recommendation: Option<RecommendationTrigger>,
    pub riven: Option<RivenTrigger>,
    pub ducatering: Option<DucateringProgress>,
    pub notification: Option<NotificationTrigger>,
    pub buffs: Option<Vec<BuffTimer>>,
    pub preview: Option<Kind>,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            seq: 0,
            opacity: OVERLAY_OPACITY_DEFAULT,
            text_size: TextSize::default(),
            motion: Motion::default(),
            ducatering_clipboard: true,
            reward: None,
            recommendation: None,
            riven: None,
            ducatering: None,
            notification: None,
            buffs: None,
            preview: None,
        }
    }
}

impl OverlayState {
    fn shows(&self, state: &AppState, only_while_game_active: bool, kind: Kind) -> bool {
        let focused = if self.preview == Some(kind) {
            None
        } else {
            state.focus.focused()
        };
        window_shows(only_while_game_active, focused, self.occupied(kind))
    }

    fn occupied(&self, kind: Kind) -> bool {
        match kind {
            Kind::RelicReward => self.reward.is_some(),
            Kind::RelicRecommendation => self.recommendation.is_some(),
            Kind::Riven => self.riven.is_some(),
            Kind::Ducatering => self.ducatering.is_some(),
            Kind::Notification => self.notification.is_some(),
            Kind::Buffs => self.buffs.is_some(),
        }
    }

    fn clear(&mut self, kind: Kind) {
        self.preview.take_if(|preview| *preview == kind);
        match kind {
            Kind::RelicReward => self.reward = None,
            Kind::RelicRecommendation => self.recommendation = None,
            Kind::Riven => self.riven = None,
            Kind::Ducatering => self.ducatering = None,
            Kind::Notification => self.notification = None,
            Kind::Buffs => self.buffs = None,
        }
    }
}

#[derive(Default)]
pub struct Overlays {
    game: Mutex<Option<runtime::Attached>>,
    station_script: Mutex<Option<wf_scan::LuaTable>>,
    dialog_script: Mutex<Option<wf_scan::LuaTable>>,
    state: Mutex<OverlayState>,
    screens: Mutex<Screens>,
    notification_id: AtomicU64,
    notification_queue: Mutex<VecDeque<NotificationTrigger>>,
    windows: Mutex<HashMap<&'static str, WindowLife>>,
    game_monitor: Mutex<Option<MonitorRect>>,
}

const IDLE_LIFETIME: Duration = Duration::from_secs(300);

struct WindowLife {
    requested: Instant,
    ready: bool,
    idle_since: Option<Instant>,
}

impl Overlays {
    pub fn snapshot(&self) -> OverlayState {
        lock(&self.state).clone()
    }
}

pub fn on_relic_rewards<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    relic: Option<String>,
    screen: &RewardScreen,
) {
    if !lock(&state.overlays.screens).reward_shows() {
        return;
    }

    show_rewards(app, state, relic, screen.clone());
}

fn show_rewards<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    relic: Option<String>,
    mut screen: RewardScreen,
) {
    if !read(&state.settings).overlays.overlay_account_balance {
        screen.account = None;
    }

    let trigger = RewardTrigger { relic, screen };
    show(app, state, Kind::RelicReward, |slots| {
        slots.reward = Some(trigger);
    });
}

pub fn on_log_event<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    event: &LogEvent,
    at: f64,
) {
    match event {
        LogEvent::SquadRewardInfoComplete | LogEvent::RelicRewardsShown => {
            lock(&state.overlays.screens).reward.tiles_shown(at);
        }
        LogEvent::ConsoleOpened => {
            lock(&state.overlays.screens).game.console_opened(at);
            preload(app, state, Kind::Riven);
        }
        LogEvent::StarChartOpened => preload(app, state, Kind::RelicRecommendation),
        LogEvent::RelicRewardScreenOpened => {
            preload(app, state, Kind::RelicReward);
            preload(app, state, Kind::RelicRecommendation);
        }
        LogEvent::MissionCleared => lock(&state.overlays.screens).game.set_mission(None),
        LogEvent::MissionSet { void_tier, .. } => lock(&state.overlays.screens)
            .game
            .set_mission(void_tier.as_deref().and_then(RelicTier::from_modifier)),
        LogEvent::RelicSelectScreenLoaded => {
            let opened = lock(&state.overlays.screens).relic_select_loaded(at);
            if let Some(generation) = opened {
                scan_recommendation(app, state, generation);
            }
        }
        LogEvent::InputMappingReset => {
            let (reward_closed, recommendation_closed) = {
                let mut screens = lock(&state.overlays.screens);
                (
                    screens.reward.input_reset(at),
                    screens.recommendation.input_reset(at),
                )
            };
            if reward_closed {
                hide(app, state, Kind::RelicReward);
            }

            if recommendation_closed {
                hide(app, state, Kind::RelicRecommendation);
            }
        }
        LogEvent::PurchaseDialogHudVisibility { count } => {
            let closed = lock(&state.overlays.screens).chat_link_layers(*count, at);
            if closed {
                hide(app, state, Kind::Riven);
            }
        }
        LogEvent::RivenDialog { item_path } => {
            let Some(item_type) = riven_item_type(item_path) else {
                return;
            };
            let shown = lock(&state.overlays.screens).riven_line(at);
            if let Some(generation) = shown {
                show_linked_riven(app, state, item_type, generation);
            }
        }
        LogEvent::RivenRerollScreenLoaded => {
            lock(&state.overlays.screens).cycling.screen_loaded(at);
            show_station_selection(app, state);
        }
        LogEvent::ConfirmDialog {
            buttons: DialogButtons::YesNo,
        } => {
            if lock(&state.overlays.screens).cycling.is_open() {
                arm_station_answer(state, at);
            }
        }
        LogEvent::WindowFocus { .. } => follow_game_window(app, state),
        LogEvent::GameMonitor(rect) => follow_game_monitor(app, state, *rect),
        LogEvent::DialogAnswered { accepted: true } => on_dialog_answer(app, state, at),
        LogEvent::DialogAnswered { accepted: false } => {
            lock(&state.overlays.screens).cycling.reject_answer();
        }
        LogEvent::SceneTornDown => {
            let closed = lock(&state.overlays.screens).cycling.scene_torn_down(at);
            if closed {
                hide(app, state, Kind::Riven);
            }
        }
        _ => {}
    }
}

fn lua_read<T>(
    state: &Arc<AppState>,
    screen: impl FnOnce(&(dyn wf_mem::MemoryReader + 'static), &wf_scan::LuaState) -> Option<T>,
) -> Option<T> {
    let mut game = lock(&state.overlays.game);
    let attached = match runtime::reattach(state, game.take()) {
        Ok(attached) => game.insert(attached),
        Err(_) if read(&state.status).pid.is_none() => {
            debug!("Screen not read, the game is not running");
            return None;
        }
        Err(error) => {
            warn!(?error, "Screen not read from game memory");
            return None;
        }
    };

    screen(attached.reader.as_ref(), &attached.lua)
}

pub fn notify<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, title: String, body: String) {
    if !enabled(&read(&state.settings), Kind::Notification) {
        return;
    }
    let id = state
        .overlays
        .notification_id
        .fetch_add(1, Ordering::Relaxed)
        + 1;
    let idle = {
        let mut queue = lock(&state.overlays.notification_queue);
        queue.push_back(NotificationTrigger { id, title, body });
        queue.len() == 1
    };
    if idle {
        tauri::async_runtime::spawn(show_queued_notifications(app.clone(), Arc::clone(state)));
    }
}

async fn show_queued_notifications<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) {
    loop {
        let next = lock(&state.overlays.notification_queue).front().cloned();
        show(&app, &state, Kind::Notification, |slots| {
            slots.notification = next;
        });
        tokio::time::sleep(NOTIFICATION_LIFETIME).await;
        let mut queue = lock(&state.overlays.notification_queue);
        queue.pop_front();
        if queue.is_empty() {
            hide(&app, &state, Kind::Notification);
            break;
        }
    }
}

pub fn on_ducatering<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    progress: Option<DucateringProgress>,
) {
    match progress {
        Some(progress) => show(app, state, Kind::Ducatering, |slots| {
            slots.ducatering = Some(progress);
        }),
        None => hide(app, state, Kind::Ducatering),
    }
}

pub fn on_buffs<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, buffs: Vec<BuffTimer>) {
    if buffs.is_empty() {
        hide(app, state, Kind::Buffs);
    } else {
        show(app, state, Kind::Buffs, |slots| slots.buffs = Some(buffs));
    }
}

pub fn on_inventory_updated<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>) {
    update_slots(app, state, |slots| {
        slots
            .riven
            .as_ref()
            .is_some_and(|trigger| trigger.before.is_some())
    });
}

pub fn apply_from<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>) {
    let screen = screen_of(app, *lock(&state.overlays.game_monitor));
    let (wanted, windowed, only_while_game_active) = {
        let settings = read(&state.settings);
        let mut slots = lock(&state.overlays.state);
        slots.opacity = settings.overlays.overlay_opacity;
        slots.text_size = settings.text_size;
        slots.motion = settings.motion;
        slots.ducatering_clipboard = settings.overlays.ducatering.ducatering_clipboard;
        drop(slots);
        (
            KINDS.map(|kind| {
                (
                    enabled(&settings, kind),
                    window_bounds(screen, kind, &settings),
                )
            }),
            windowed(state, &settings),
            settings.overlays.overlay_only_while_game_active,
        )
    };

    for (kind, (on, bounds)) in KINDS.into_iter().zip(wanted) {
        let keep = windowed && on;
        match (app.get_webview_window(kind.label()), keep) {
            (Some(_), false) => {
                lock(&state.overlays.windows).remove(kind.label());
                retire(app, kind);
            }
            (Some(window), true) => place(&window, bounds, screen.scale),
            (None, _) => {}
        }

        if !on {
            hide(app, state, kind);
        } else if keep {
            if kind.permanent() {
                load(app, state, kind);
            }

            let shows = lock(&state.overlays.state).shows(state, only_while_game_active, kind);
            settle(app, state, kind, shows);
        }
    }

    broadcast(app, state);
}

fn update_slots<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    change: impl FnOnce(&mut OverlayState) -> bool,
) -> bool {
    let mut slots = lock(&state.overlays.state);
    let changed = change(&mut slots);
    if changed {
        slots.seq += 1;
    }
    drop(slots);

    if changed {
        broadcast(app, state);
    }

    changed
}

fn show<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    kind: Kind,
    fill: impl FnOnce(&mut OverlayState),
) {
    let (windowed, only_while_game_active) = {
        let settings = read(&state.settings);
        if !enabled(&settings, kind) {
            return;
        }

        (
            windowed(state, &settings),
            settings.overlays.overlay_only_while_game_active,
        )
    };

    update_slots(app, state, |slots| {
        fill(slots);
        true
    });
    if windowed && lock(&state.overlays.state).shows(state, only_while_game_active, kind) {
        surface(app, state, kind);
    }
}

fn load<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, kind: Kind) {
    if app.get_webview_window(kind.label()).is_some() {
        return;
    }

    let screen = screen_of(app, *lock(&state.overlays.game_monitor));
    let bounds = window_bounds(screen, kind, &read(&state.settings));
    lock(&state.overlays.windows).insert(
        kind.label(),
        WindowLife {
            requested: Instant::now(),
            ready: false,
            idle_since: None,
        },
    );
    open(app, kind, bounds, screen.scale);
}

fn surface<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, kind: Kind) {
    load(app, state, kind);
    let ready = lock(&state.overlays.windows)
        .get_mut(kind.label())
        .is_some_and(|life| {
            life.idle_since = None;
            life.ready
        });
    if ready {
        reveal(app, kind);
    }
}

fn settle<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, kind: Kind, shows: bool) {
    if shows {
        surface(app, state, kind);
    } else {
        rest(app, state, kind);
    }
}

fn rest<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, kind: Kind) {
    park(app, kind);
    if kind.permanent() {
        return;
    }
    let idle_since = Instant::now();
    match lock(&state.overlays.windows).get_mut(kind.label()) {
        Some(life) if life.idle_since.is_none() => life.idle_since = Some(idle_since),
        _ => return,
    }
    let app = app.clone();
    let owned = Arc::clone(state);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(IDLE_LIFETIME).await;
        let expired = {
            let mut windows = lock(&owned.overlays.windows);
            let expired = windows
                .get(kind.label())
                .is_some_and(|life| life.idle_since == Some(idle_since));
            if expired {
                windows.remove(kind.label());
            }
            expired
        };
        if expired {
            debug!(label = kind.label(), "Idle overlay window closed");
            retire(&app, kind);
        }
    });
}

fn preload<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, kind: Kind) {
    let windowed = {
        let settings = read(&state.settings);
        enabled(&settings, kind) && windowed(state, &settings)
    };
    if !windowed || app.get_webview_window(kind.label()).is_some() {
        return;
    }
    load(app, state, kind);
    rest(app, state, kind);
}

pub fn on_page_ready<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, label: &str) {
    let Some(kind) = KINDS.into_iter().find(|kind| kind.label() == label) else {
        return;
    };
    let waited = {
        let mut windows = lock(&state.overlays.windows);
        let Some(life) = windows.get_mut(label) else {
            return;
        };
        life.ready = true;
        life.requested.elapsed()
    };
    info!(label, millis = waited.as_millis(), "Overlay page ready");
    let settings = read(&state.settings);
    let shows = enabled(&settings, kind)
        && lock(&state.overlays.state).shows(
            state,
            settings.overlays.overlay_only_while_game_active,
            kind,
        );
    if shows {
        reveal(app, kind);
    }
}

pub fn on_content_shrank<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    if !session::keeps_vacated_pixels() {
        return;
    }
    let label = window.label();
    let nudged = window.outer_size().and_then(|size| {
        let height = if size.height % 2 == 0 {
            size.height + 1
        } else {
            size.height - 1
        };
        window.set_size(tauri::PhysicalSize::new(size.width, height))
    });
    match nudged {
        Ok(()) => debug!(label, "Overlay window resized after its content shrank"),
        Err(error) => warn!(label, %error, "Overlay window resize failed"),
    }
}

fn hide<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, kind: Kind) {
    let cleared = update_slots(app, state, |slots| {
        let occupied = slots.occupied(kind);
        slots.clear(kind);
        occupied
    });
    if cleared {
        rest(app, state, kind);
    }
}

fn follow_game_window<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>) {
    let shown = {
        let settings = read(&state.settings);
        if !windowed(state, &settings) {
            return;
        }

        let only_while_game_active = settings.overlays.overlay_only_while_game_active;
        let slots = lock(&state.overlays.state);
        KINDS.map(|kind| {
            enabled(&settings, kind) && slots.shows(state, only_while_game_active, kind)
        })
    };

    for (kind, shows) in KINDS.into_iter().zip(shown) {
        settle(app, state, kind, shows);
    }
}

fn follow_game_monitor<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, rect: MonitorRect) {
    if lock(&state.overlays.game_monitor).replace(rect) == Some(rect) {
        return;
    }
    info!(
        left = rect.left,
        top = rect.top,
        width = rect.width,
        height = rect.height,
        "Game monitor read from EE.log"
    );
    apply_from(app, state);
}

fn broadcast<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>) {
    emit(
        app,
        AppEvent::OverlayState(Box::new(state.overlays.snapshot())),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn enabled_needs_both_switches() {
        let settings = Settings::default();
        assert!(enabled(&settings, Kind::RelicReward));
        assert!(enabled(&settings, Kind::RelicRecommendation));
        assert!(enabled(&settings, Kind::Riven));
        assert!(enabled(&settings, Kind::Ducatering));
        assert!(!enabled(&settings, Kind::Notification));

        let notification_on: Settings = serde_json::from_str(r#"{"toasts_in_game":true}"#).unwrap();
        assert!(enabled(&notification_on, Kind::Notification));

        let master_off: Settings = serde_json::from_str(r#"{"overlays_enabled":false}"#).unwrap();
        assert!(!enabled(&master_off, Kind::RelicReward));
        assert!(!enabled(&master_off, Kind::RelicRecommendation));
        assert!(!enabled(&master_off, Kind::Riven));
        assert!(!enabled(&master_off, Kind::Ducatering));
        assert!(!enabled(&master_off, Kind::Notification));

        let one_toggle_off: Settings = serde_json::from_str(r#"{"overlay_riven":false}"#).unwrap();
        assert!(!enabled(&one_toggle_off, Kind::Riven));
        assert!(enabled(&one_toggle_off, Kind::RelicReward));
        assert!(enabled(&one_toggle_off, Kind::Ducatering));
    }

    #[test]
    fn background_hide_setting() {
        for due in [false, true] {
            assert_eq!(window_shows(true, None, due), due);
            assert_eq!(window_shows(true, Some(true), due), due);
            assert!(!window_shows(true, Some(false), due));

            assert_eq!(window_shows(false, None, due), due);
            assert_eq!(window_shows(false, Some(true), due), due);
            assert_eq!(window_shows(false, Some(false), due), due);
        }
    }

    #[test]
    fn clearing_a_slot_ends_its_preview() {
        let mut slots = OverlayState {
            preview: Some(Kind::Riven),
            ..OverlayState::default()
        };
        slots.clear(Kind::Ducatering);
        assert_eq!(slots.preview, Some(Kind::Riven));

        slots.clear(Kind::Riven);
        assert_eq!(slots.preview, None);
    }

    #[test]
    fn kind_label_and_route() {
        let labels: Vec<&str> = super::KINDS.iter().map(|kind| kind.label()).collect();
        assert_eq!(
            labels,
            vec![
                "overlay-relic",
                "overlay-recommend",
                "overlay-riven",
                "overlay-ducatering",
                "overlay-notify",
                "overlay-buffs"
            ]
        );
        assert!(Kind::Notification.permanent());
        assert!(!Kind::Riven.permanent());
        for kind in super::KINDS {
            assert!(kind.route().starts_with("index.html#/overlay/"));
        }
    }
}
