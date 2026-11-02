use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::{Store, StoreExt};
use wf_core::{AlertSettings, DucateringFilter, DucateringHidden, MasteryOptions, MasteryOrdering};
use wf_market::{OrderType, Reach, Session, TraderStatus};

use crate::market::MarketPresenceChoice;
use crate::state::{AppState, lock, write};

pub const STORE_FILE: &str = "merframe.json";
const TOKEN_FILE: &str = "market_token";
const SETTINGS_KEY: &str = "settings";
const LEGACY_TOKEN_KEY: &str = "market_token";
const ACCOUNT_KEY: &str = "market_account";
const PRESENCE_KEY: &str = "market_presence";

pub const DISCORD_TEMPLATE: &str = "{tenno} started a conversation with you in Warframe.";

pub const OVERLAY_OPACITY_DEFAULT: u8 = 100;
pub const RECOMMENDATION_COUNT_DEFAULT: u8 = 6;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum OverlayMode {
    #[default]
    Auto,
    Windows,
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum OverlayPlacement {
    TopLeft,
    TopCentre,
    TopRight,
    BottomLeft,
    BottomCentre,
    BottomRight,
    Centre,
}

impl From<ToastPosition> for OverlayPlacement {
    fn from(position: ToastPosition) -> Self {
        match position {
            ToastPosition::TopLeft => Self::TopLeft,
            ToastPosition::TopCentre => Self::TopCentre,
            ToastPosition::TopRight => Self::TopRight,
            ToastPosition::BottomLeft => Self::BottomLeft,
            ToastPosition::BottomCentre => Self::BottomCentre,
            ToastPosition::BottomRight => Self::BottomRight,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ToastPosition {
    TopLeft,
    TopCentre,
    TopRight,
    BottomLeft,
    BottomCentre,
    #[default]
    BottomRight,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum TextSize {
    #[default]
    Small,
    Medium,
    Large,
}

impl TextSize {
    pub fn root_px(self) -> u8 {
        match self {
            Self::Small => 16,
            Self::Medium => 18,
            Self::Large => 20,
        }
    }

    pub fn factor(self) -> f64 {
        f64::from(self.root_px()) / 16.0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum Motion {
    #[default]
    System,
    Reduced,
    Full,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum RecommendationRefinement {
    #[default]
    Radiant,
    Owned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct Settings {
    pub alerts: AlertSettings,
    #[serde(flatten)]
    pub notifications: NotificationSettings,
    #[serde(flatten)]
    pub discord: DiscordSettings,
    #[serde(flatten)]
    pub market: MarketSettings,
    pub world_state_interval_minutes: u32,
    #[serde(flatten)]
    pub inventory: InventorySettings,
    #[serde(flatten)]
    pub overlays: OverlaySettings,
    pub copy_relic_rewards: bool,
    #[serde(flatten)]
    pub toasts: ToastSettings,
    pub check_for_updates: bool,
    pub force_log_file: bool,
    pub log_file_path: Option<PathBuf>,
    pub text_size: TextSize,
    pub motion: Motion,
    #[serde(flatten)]
    pub event_feed: EventFeedSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct EventFeedSettings {
    pub event_feed_enabled: bool,
    pub event_feed_port: u16,
}

impl EventFeedSettings {
    pub const PORT_MIN: u16 = 1024;

    pub fn port(self) -> Option<u16> {
        self.event_feed_enabled.then_some(self.event_feed_port)
    }

    pub fn check(self) -> anyhow::Result<()> {
        if self.event_feed_enabled && self.event_feed_port < Self::PORT_MIN {
            anyhow::bail!("The event feed port has to be {} or higher", Self::PORT_MIN);
        }

        Ok(())
    }
}

impl Default for EventFeedSettings {
    fn default() -> Self {
        Self {
            event_feed_enabled: false,
            event_feed_port: 28005,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum InGameToast {
    Inventory,
    Trade,
    Fissure,
    Reward,
    Arbitration,
    Timer,
    MarketClose,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct ToastSettings {
    pub toasts_enabled: bool,
    pub toast_position: ToastPosition,
    pub toasts_in_game: bool,
    pub toasts_in_game_position: ToastPosition,
    pub toasts_in_game_muted: Vec<InGameToast>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct NotificationSettings {
    pub windows_notifications_enabled: bool,
    pub sound_notifications_enabled: bool,
    pub notification_only_background: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct DiscordSettings {
    pub discord_webhook: Option<String>,
    pub discord_message_template: String,
    #[serde(flatten)]
    pub mirrored: DiscordMirroredAlerts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum DiscordMirroredAlert {
    Conversation,
    Fissure,
    Reward,
    Arbitration,
    Timer,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct DiscordMirroredAlerts {
    pub discord_mirror_enabled: bool,
    pub discord_mirrored_alerts: BTreeSet<DiscordMirroredAlert>,
}

impl DiscordMirroredAlerts {
    pub fn any(&self) -> bool {
        self.discord_mirror_enabled && !self.discord_mirrored_alerts.is_empty()
    }

    pub fn mirrors(&self, alert: DiscordMirroredAlert) -> bool {
        self.discord_mirror_enabled && self.discord_mirrored_alerts.contains(&alert)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredDiscordMirroredAlerts {
    Set {
        #[serde(default)]
        discord_mirror_enabled: bool,
        discord_mirrored_alerts: BTreeSet<DiscordMirroredAlert>,
    },
    Toggles {
        #[serde(default)]
        discord_notifications_enabled: bool,
        #[serde(default)]
        discord_fissure_alerts: bool,
        #[serde(default)]
        discord_reward_alerts: bool,
        #[serde(default)]
        discord_timer_alerts: bool,
    },
}

impl<'de> Deserialize<'de> for DiscordMirroredAlerts {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(
            match StoredDiscordMirroredAlerts::deserialize(deserializer)? {
                StoredDiscordMirroredAlerts::Set {
                    discord_mirror_enabled,
                    discord_mirrored_alerts,
                } => Self {
                    discord_mirror_enabled,
                    discord_mirrored_alerts,
                },
                StoredDiscordMirroredAlerts::Toggles {
                    discord_notifications_enabled,
                    discord_fissure_alerts,
                    discord_reward_alerts,
                    discord_timer_alerts,
                } => {
                    let discord_mirrored_alerts: BTreeSet<DiscordMirroredAlert> = [
                        (
                            discord_notifications_enabled,
                            DiscordMirroredAlert::Conversation,
                        ),
                        (discord_fissure_alerts, DiscordMirroredAlert::Fissure),
                        (discord_reward_alerts, DiscordMirroredAlert::Reward),
                        (discord_timer_alerts, DiscordMirroredAlert::Timer),
                    ]
                    .into_iter()
                    .filter(|(on, _)| *on)
                    .map(|(_, alert)| alert)
                    .collect();
                    Self {
                        discord_mirror_enabled: !discord_mirrored_alerts.is_empty(),
                        discord_mirrored_alerts,
                    }
                }
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct MarketSettings {
    pub market_poll_minutes: u32,
    pub market_auto_close: bool,
    #[serde(flatten)]
    pub last_trade: LastTradeSettings,
    pub take_rank_into_account: bool,
    pub market_price_check: bool,
    pub market_price_tolerance: u32,
    pub market_trader_status: TraderStatus,
    pub market_trader_locale: Option<String>,
    pub set_part_click: OrderType,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct LastTradeSettings {
    pub market_offline_after_last_trade: bool,
    pub market_hide_after_last_trade: bool,
    pub market_hide_auctions_after_last_trade: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct InventorySettings {
    pub include_founders_items: Option<bool>,
    #[serde(skip_serializing)]
    hide_founders_items: Option<bool>,
    pub include_forma_ranks: bool,
    pub stats_tab_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct OverlaySettings {
    pub overlays_enabled: bool,
    #[serde(flatten)]
    pub shown: OverlayToggles,
    #[serde(flatten)]
    pub placements: OverlayPlacements,
    pub overlay_recommendation_refinement: RecommendationRefinement,
    pub overlay_opacity: u8,
    pub overlay_recommendation_count: u8,
    pub overlay_mode: OverlayMode,
    pub overlay_only_while_game_active: bool,
    pub overlay_account_balance: bool,
    #[serde(flatten)]
    pub ducatering: DucateringSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct DucateringSettings {
    pub overlay_ducatering: bool,
    pub overlay_ducatering_placement: OverlayPlacement,
    pub ducatering_clipboard: bool,
    pub ducatering_most_ducats_first: bool,
    pub ducatering_max_plat: u32,
    pub ducatering_hidden: Vec<DucateringHidden>,
    pub ducatering_hidden_set_plat: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct OverlayToggles {
    pub overlay_relic_reward: bool,
    pub overlay_relic_recommendation: bool,
    pub overlay_riven: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct OverlayPlacements {
    pub overlay_relic_reward_placement: OverlayPlacement,
    pub overlay_relic_recommendation_placement: OverlayPlacement,
    pub overlay_riven_placement: OverlayPlacement,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            alerts: AlertSettings::default(),
            notifications: NotificationSettings::default(),
            discord: DiscordSettings::default(),
            market: MarketSettings::default(),
            world_state_interval_minutes: 5,
            inventory: InventorySettings::default(),
            overlays: OverlaySettings::default(),
            copy_relic_rewards: false,
            toasts: ToastSettings::default(),
            check_for_updates: true,
            force_log_file: false,
            log_file_path: None,
            text_size: TextSize::default(),
            motion: Motion::default(),
            event_feed: EventFeedSettings::default(),
        }
    }
}

impl Default for ToastSettings {
    fn default() -> Self {
        Self {
            toasts_enabled: true,
            toast_position: ToastPosition::BottomRight,
            toasts_in_game: false,
            toasts_in_game_position: ToastPosition::BottomRight,
            toasts_in_game_muted: Vec::new(),
        }
    }
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            windows_notifications_enabled: true,
            sound_notifications_enabled: true,
            notification_only_background: true,
        }
    }
}

impl DiscordSettings {
    pub fn webhook(&self) -> Option<&str> {
        self.discord_webhook
            .as_deref()
            .filter(|url| !url.is_empty())
    }
}

impl Default for DiscordSettings {
    fn default() -> Self {
        Self {
            discord_webhook: None,
            discord_message_template: DISCORD_TEMPLATE.to_owned(),
            mirrored: DiscordMirroredAlerts::default(),
        }
    }
}

impl Default for MarketSettings {
    fn default() -> Self {
        Self {
            market_poll_minutes: 5,
            market_auto_close: false,
            last_trade: LastTradeSettings::default(),
            take_rank_into_account: true,
            market_price_check: true,
            market_price_tolerance: 10,
            market_trader_status: TraderStatus::Ingame,
            market_trader_locale: Some("en".to_owned()),
            set_part_click: OrderType::Sell,
        }
    }
}

impl Default for InventorySettings {
    fn default() -> Self {
        Self {
            include_founders_items: None,
            hide_founders_items: None,
            include_forma_ranks: true,
            stats_tab_enabled: true,
        }
    }
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            overlays_enabled: true,
            shown: OverlayToggles::default(),
            placements: OverlayPlacements::default(),
            overlay_recommendation_refinement: RecommendationRefinement::Radiant,
            overlay_opacity: OVERLAY_OPACITY_DEFAULT,
            overlay_recommendation_count: RECOMMENDATION_COUNT_DEFAULT,
            overlay_mode: OverlayMode::Auto,
            overlay_only_while_game_active: true,
            overlay_account_balance: true,
            ducatering: DucateringSettings::default(),
        }
    }
}

impl DucateringSettings {
    pub fn filter(&self) -> DucateringFilter {
        DucateringFilter {
            most_ducats_first: self.ducatering_most_ducats_first,
            max_plat: self.ducatering_max_plat,
            hidden: self.ducatering_hidden.clone(),
            hidden_set_plat: self.ducatering_hidden_set_plat,
        }
    }
}

impl Default for DucateringSettings {
    fn default() -> Self {
        Self {
            overlay_ducatering: true,
            overlay_ducatering_placement: OverlayPlacement::TopRight,
            ducatering_clipboard: true,
            ducatering_most_ducats_first: true,
            ducatering_max_plat: 10,
            ducatering_hidden: Vec::new(),
            ducatering_hidden_set_plat: 0,
        }
    }
}

impl Default for OverlayToggles {
    fn default() -> Self {
        Self {
            overlay_relic_reward: true,
            overlay_relic_recommendation: true,
            overlay_riven: true,
        }
    }
}

impl Default for OverlayPlacements {
    fn default() -> Self {
        Self {
            overlay_relic_reward_placement: OverlayPlacement::Centre,
            overlay_relic_recommendation_placement: OverlayPlacement::TopRight,
            overlay_riven_placement: OverlayPlacement::TopLeft,
        }
    }
}

impl Settings {
    pub fn log_path(&self) -> Option<PathBuf> {
        self.log_file_path.clone().or_else(wf_log::default_log_path)
    }

    pub fn trader_reach(&self) -> Reach {
        Reach {
            status: self.market.market_trader_status,
            locale: self.market.market_trader_locale.clone(),
        }
    }

    pub fn log_selection(&self) -> wf_log::Selection {
        if self.force_log_file {
            wf_log::Selection::File
        } else {
            wf_log::Selection::Auto
        }
    }

    pub fn world_state_interval(&self) -> Duration {
        let minutes = self.world_state_interval_minutes.clamp(5, 10);
        Duration::from_secs(u64::from(minutes) * 60)
    }

    pub fn market_poll_interval(&self) -> Duration {
        let minutes = self.market.market_poll_minutes.clamp(2, 60);
        Duration::from_secs(u64::from(minutes) * 60)
    }

    pub fn mastery_options(&self, ordering: MasteryOrdering) -> MasteryOptions {
        MasteryOptions {
            ordering,
            include_founders_items: self.inventory.founders_choice(),
            include_forma_ranks: self.inventory.include_forma_ranks,
        }
    }
}

impl InventorySettings {
    pub fn founders_choice(&self) -> Option<bool> {
        self.include_founders_items
            .or_else(|| self.hide_founders_items.map(|hidden| !hidden))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct MarketAccount {
    pub ingame_name: String,
    pub slug: String,
    pub tier: String,
    pub mastery_rank: u32,
}

impl From<&Session> for MarketAccount {
    fn from(session: &Session) -> Self {
        Self {
            ingame_name: session.user.ingame_name.clone(),
            slug: session.user.slug.clone(),
            tier: session.user.tier.clone(),
            mastery_rank: session.user.mastery_rank,
        }
    }
}

#[derive(Deserialize)]
struct StoredDocument {
    settings: Settings,
}

pub fn wants_overlay_windows(document: &[u8]) -> bool {
    let settings = serde_json::from_slice::<StoredDocument>(document)
        .map(|stored| stored.settings)
        .unwrap_or_default();
    settings.overlays.overlays_enabled && settings.overlays.overlay_mode != OverlayMode::Tab
}

fn store<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<Arc<Store<R>>> {
    app.store(crate::state::data_dir(app)?.join(STORE_FILE))
        .context("Opening the settings store")
}

fn stored<R: Runtime, T: DeserializeOwned>(
    app: &AppHandle<R>,
    key: &str,
    unreadable: &'static str,
) -> anyhow::Result<Option<T>> {
    store(app)?
        .get(key)
        .map(serde_json::from_value)
        .transpose()
        .context(unreadable)
}

fn put<R: Runtime>(
    app: &AppHandle<R>,
    key: &str,
    value: Option<serde_json::Value>,
) -> anyhow::Result<()> {
    let store = store(app)?;
    match value {
        Some(value) => store.set(key, value),
        None => {
            store.delete(key);
        }
    }

    store.save().context("Writing the settings store")
}

fn unless_missing<T>(result: std::io::Result<T>) -> std::io::Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn load<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<Settings> {
    let settings = stored(app, SETTINGS_KEY, "Stored settings are not readable")?;
    Ok(settings.unwrap_or_default())
}

pub fn save<R: Runtime>(app: &AppHandle<R>, settings: &Settings) -> anyhow::Result<()> {
    let value = serde_json::to_value(settings).context("Serializing the settings")?;
    put(app, SETTINGS_KEY, Some(value))
}

pub fn apply<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    settings: &Settings,
) -> anyhow::Result<Settings> {
    let previous = std::mem::replace(&mut *write(&state.settings), settings.clone());
    lock(&state.core).set_alert_settings(settings.alerts.clone());
    save(app, settings)?;
    Ok(previous)
}

fn token_path<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<PathBuf> {
    Ok(crate::state::data_dir(app)?.join(TOKEN_FILE))
}

pub fn token<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<Option<String>> {
    let path = token_path(app)?;
    let token = unless_missing(std::fs::read_to_string(&path))
        .with_context(|| format!("Reading {}", path.display()))?;
    match token {
        Some(token) => Ok(Some(token)),
        None => moved_token(app, &path),
    }
}

fn moved_token<R: Runtime>(app: &AppHandle<R>, path: &Path) -> anyhow::Result<Option<String>> {
    let Some(token) = store(app)?
        .get(LEGACY_TOKEN_KEY)
        .and_then(|value| value.as_str().map(str::to_owned))
    else {
        return Ok(None);
    };

    write_token(path, &token)?;
    put(app, LEGACY_TOKEN_KEY, None)?;
    tracing::info!("Session token moved to its own file");
    Ok(Some(token))
}

pub fn set_token<R: Runtime>(app: &AppHandle<R>, token: Option<&str>) -> anyhow::Result<()> {
    let path = token_path(app)?;
    match token {
        Some(token) => write_token(&path, token),
        None => unless_missing(std::fs::remove_file(&path))
            .map(|_| ())
            .with_context(|| format!("Removing {}", path.display())),
    }
}

fn write_token(path: &Path, token: &str) -> anyhow::Result<()> {
    std::fs::write(path, token).with_context(|| format!("Writing {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("Restricting {}", path.display()))?;
    }
    Ok(())
}

pub fn account<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<Option<MarketAccount>> {
    stored(app, ACCOUNT_KEY, "Stored market account is not readable")
}

pub fn set_account<R: Runtime>(
    app: &AppHandle<R>,
    account: Option<&MarketAccount>,
) -> anyhow::Result<()> {
    let value = account
        .map(serde_json::to_value)
        .transpose()
        .context("Serializing the market account")?;
    put(app, ACCOUNT_KEY, value)
}

pub fn presence<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<MarketPresenceChoice> {
    let choice = stored(app, PRESENCE_KEY, "Stored market presence is not readable")?;
    Ok(choice.unwrap_or_default())
}

pub fn set_presence<R: Runtime>(
    app: &AppHandle<R>,
    choice: MarketPresenceChoice,
) -> anyhow::Result<()> {
    let value = serde_json::to_value(choice).context("Serializing the market presence")?;
    put(app, PRESENCE_KEY, Some(value))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use wf_core::TimerAlerts;

    use super::*;

    #[test]
    fn discord_mirrored_alerts_read_both_shapes() {
        let legacy: Settings =
            serde_json::from_str(r#"{"discord_fissure_alerts":true,"discord_timer_alerts":true}"#)
                .unwrap();
        let mirrored = &legacy.discord.mirrored;
        assert!(mirrored.discord_mirror_enabled);
        assert!(mirrored.mirrors(DiscordMirroredAlert::Fissure));
        assert!(!mirrored.mirrors(DiscordMirroredAlert::Reward));
        assert!(mirrored.mirrors(DiscordMirroredAlert::Timer));
        let json = serde_json::to_string(&legacy).unwrap();
        assert!(json.contains(r#""discord_mirror_enabled":true"#));
        assert!(json.contains(r#""discord_mirrored_alerts":["fissure","timer"]"#));
        assert!(!json.contains("discord_fissure_alerts"));
        let parsed: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.discord.mirrored, legacy.discord.mirrored);

        let switched_off: Settings = serde_json::from_str(
            r#"{"discord_mirror_enabled":false,"discord_mirrored_alerts":["conversation"]}"#,
        )
        .unwrap();
        assert!(!switched_off.discord.mirrored.any());
        let empty: Settings = serde_json::from_str("{}").unwrap();
        assert!(!empty.discord.mirrored.any());
    }

    #[test]
    fn shipped_defaults() {
        let settings = Settings::default();
        assert!(settings.notifications.windows_notifications_enabled);
        assert!(settings.notifications.sound_notifications_enabled);
        assert_eq!(settings.discord.discord_webhook, None);
        assert_eq!(settings.discord.discord_message_template, DISCORD_TEMPLATE);
        assert!(settings.notifications.notification_only_background);
        assert_eq!(settings.world_state_interval_minutes, 5);
        assert_eq!(settings.market.market_poll_minutes, 5);
        assert!(!settings.market.market_auto_close);
        assert_eq!(settings.market.last_trade, LastTradeSettings::default());
        assert_eq!(settings.inventory.include_founders_items, None);
        assert!(settings.inventory.include_forma_ranks);
        assert!(settings.inventory.stats_tab_enabled);
        assert!(settings.toasts.toasts_enabled);
        assert_eq!(settings.toasts.toast_position, ToastPosition::BottomRight);
        assert!(!settings.toasts.toasts_in_game);
        assert_eq!(
            settings.toasts.toasts_in_game_position,
            ToastPosition::BottomRight
        );
        assert!(settings.toasts.toasts_in_game_muted.is_empty());
        assert!(settings.overlays.overlays_enabled);
        assert!(settings.overlays.shown.overlay_relic_reward);
        assert!(settings.overlays.shown.overlay_relic_recommendation);
        assert!(settings.overlays.shown.overlay_riven);
        assert!(settings.overlays.ducatering.overlay_ducatering);
        assert_eq!(
            settings.overlays.ducatering.overlay_ducatering_placement,
            OverlayPlacement::TopRight
        );
        assert!(settings.overlays.ducatering.ducatering_clipboard);
        assert!(settings.overlays.ducatering.ducatering_most_ducats_first);
        assert_eq!(settings.overlays.ducatering.ducatering_max_plat, 10);
        assert!(settings.overlays.ducatering.ducatering_hidden.is_empty());
        assert_eq!(settings.overlays.ducatering.ducatering_hidden_set_plat, 0);
        assert_eq!(
            settings.overlays.placements.overlay_relic_reward_placement,
            OverlayPlacement::Centre
        );
        assert_eq!(
            settings
                .overlays
                .placements
                .overlay_relic_recommendation_placement,
            OverlayPlacement::TopRight
        );
        assert_eq!(
            settings.overlays.placements.overlay_riven_placement,
            OverlayPlacement::TopLeft
        );
        assert_eq!(
            settings.overlays.overlay_recommendation_refinement,
            RecommendationRefinement::Radiant
        );
        assert_eq!(settings.overlays.overlay_opacity, 100);
        assert_eq!(settings.text_size, TextSize::Small);
        assert_eq!(settings.motion, Motion::System);
        assert_eq!(settings.overlays.overlay_recommendation_count, 6);
        assert_eq!(settings.overlays.overlay_mode, OverlayMode::Auto);
        assert!(settings.overlays.overlay_only_while_game_active);
        assert!(settings.overlays.overlay_account_balance);
        assert!(!settings.copy_relic_rewards);
        assert!(settings.check_for_updates);
        assert!(!settings.discord.mirrored.any());
        assert!(!settings.alerts.arbitrations.enabled);
        assert!(settings.alerts.arbitrations.notify_on_start);
        assert!(settings.alerts.arbitrations.show_on_world_page);
        assert_eq!(settings.alerts.arbitrations.lead_minutes, 10);
        assert_eq!(settings.market.market_trader_status, TraderStatus::Ingame);
        assert_eq!(settings.market.market_trader_locale.as_deref(), Some("en"));
        assert_eq!(settings.log_file_path, None);
        assert!(!settings.event_feed.event_feed_enabled);
        assert_eq!(settings.event_feed.event_feed_port, 28005);
        assert_eq!(settings.event_feed.port(), None);
        assert!(settings.event_feed.check().is_ok());
        assert_eq!(
            settings.trader_reach(),
            Reach {
                status: TraderStatus::Ingame,
                locale: Some("en".to_owned()),
            }
        );
        assert!(!settings.alerts.fissure_notifications_enabled);
        assert!(settings.alerts.fissure_filters.is_empty());
        assert_eq!(settings.alerts.timers, TimerAlerts::default());
        assert_eq!(settings.alerts.timer_lead_secs, 180);
    }

    #[test]
    fn event_feed_port_below_the_user_range_is_rejected() {
        let low = EventFeedSettings {
            event_feed_enabled: true,
            event_feed_port: 80,
        };
        assert!(low.check().is_err());

        let off = EventFeedSettings {
            event_feed_enabled: false,
            ..low
        };
        assert!(off.check().is_ok());
    }

    #[test]
    fn empty_document_default() {
        let stored: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(stored, Settings::default());
    }

    #[test]
    fn flat_document_round_trip() {
        let stored: Settings = serde_json::from_str(
            r#"{"alerts":{"fissure_notifications_enabled":true,"fissure_filters":[],"timers":[],"timer_lead_secs":120},
                "windows_notifications_enabled":false,"sound_notifications_enabled":true,
                "discord_notifications_enabled":true,"discord_webhook":"https://discord.example/hook",
                "discord_message_template":"{tenno} says hi",
                "notification_only_background":false,
                "world_state_interval_minutes":7,"market_poll_minutes":10,"market_auto_close":true,
                "take_rank_into_account":false,"set_part_click":"buy",
                "market_price_check":false,"market_price_tolerance":25,
                "include_founders_items":false,"include_forma_ranks":false,
                "stats_tab_enabled":false,"overlays_enabled":true,
                "overlay_relic_reward":false,"overlay_relic_recommendation":true,"overlay_riven":false,
                "overlay_relic_reward_placement":"bottom_left",
                "overlay_relic_recommendation_placement":"centre","overlay_riven_placement":"bottom_right",
                "overlay_recommendation_refinement":"owned","overlay_opacity":55,
                "overlay_recommendation_count":9,"overlay_mode":"windows",
                "overlay_only_while_game_active":false,"force_log_file":true,
                "toasts_enabled":false,"toast_position":"top_centre","toasts_in_game":true,
                "toasts_in_game_position":"top_left",
                "toasts_in_game_muted":["inventory","market_close"]}"#,
        )
        .unwrap();
        assert!(stored.alerts.fissure_notifications_enabled);
        assert_eq!(stored.alerts.timer_lead_secs, 120);
        assert!(!stored.notifications.windows_notifications_enabled);
        assert!(stored.notifications.sound_notifications_enabled);
        assert!(!stored.notifications.notification_only_background);
        assert!(
            stored
                .discord
                .mirrored
                .mirrors(DiscordMirroredAlert::Conversation)
        );
        assert_eq!(
            stored.discord.discord_webhook.as_deref(),
            Some("https://discord.example/hook")
        );
        assert_eq!(stored.discord.discord_message_template, "{tenno} says hi");
        assert_eq!(stored.market.market_poll_minutes, 10);
        assert!(stored.market.market_auto_close);
        assert!(!stored.market.take_rank_into_account);
        assert!(!stored.market.market_price_check);
        assert_eq!(stored.market.market_price_tolerance, 25);
        assert_eq!(stored.market.set_part_click, OrderType::Buy);
        assert_eq!(stored.world_state_interval_minutes, 7);
        assert_eq!(stored.inventory.include_founders_items, Some(false));
        assert!(!stored.inventory.include_forma_ranks);
        assert!(!stored.inventory.stats_tab_enabled);
        assert!(!stored.toasts.toasts_enabled);
        assert_eq!(stored.toasts.toast_position, ToastPosition::TopCentre);
        assert!(stored.toasts.toasts_in_game);
        assert_eq!(
            stored.toasts.toasts_in_game_position,
            ToastPosition::TopLeft
        );
        assert_eq!(
            stored.toasts.toasts_in_game_muted,
            [InGameToast::Inventory, InGameToast::MarketClose]
        );
        assert!(stored.overlays.overlays_enabled);
        assert!(!stored.overlays.shown.overlay_relic_reward);
        assert!(stored.overlays.shown.overlay_relic_recommendation);
        assert!(!stored.overlays.shown.overlay_riven);
        assert_eq!(
            stored.overlays.placements.overlay_relic_reward_placement,
            OverlayPlacement::BottomLeft
        );
        assert_eq!(
            stored
                .overlays
                .placements
                .overlay_relic_recommendation_placement,
            OverlayPlacement::Centre
        );
        assert_eq!(
            stored.overlays.placements.overlay_riven_placement,
            OverlayPlacement::BottomRight
        );
        assert_eq!(
            stored.overlays.overlay_recommendation_refinement,
            RecommendationRefinement::Owned
        );
        assert_eq!(stored.overlays.overlay_opacity, 55);
        assert_eq!(stored.overlays.overlay_recommendation_count, 9);
        assert_eq!(stored.overlays.overlay_mode, OverlayMode::Windows);
        assert!(!stored.overlays.overlay_only_while_game_active);
        assert!(stored.force_log_file);

        let saved: serde_json::Value = serde_json::to_value(&stored).unwrap();
        let keys: Vec<&str> = saved
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert!(keys.iter().all(|key| !key.contains('.')));
        assert!(saved.get("notifications").is_none());
        assert!(saved.get("overlays").is_none());
        assert_eq!(saved["overlay_riven"], false);
        assert_eq!(saved["market_poll_minutes"], 10);
    }

    #[test]
    fn flipped_defaults_stay_off() {
        let stored: Settings = serde_json::from_str(
            r#"{"windows_notifications_enabled":false,"sound_notifications_enabled":false,
                "notification_only_background":false,"stats_tab_enabled":false,
                "overlays_enabled":false,"overlay_relic_reward":false,
                "overlay_relic_recommendation":false,"overlay_riven":false}"#,
        )
        .unwrap();
        assert!(!stored.notifications.windows_notifications_enabled);
        assert!(!stored.notifications.sound_notifications_enabled);
        assert!(!stored.notifications.notification_only_background);
        assert!(!stored.inventory.stats_tab_enabled);
        assert!(!stored.overlays.overlays_enabled);
        assert!(!stored.overlays.shown.overlay_relic_reward);
        assert!(!stored.overlays.shown.overlay_relic_recommendation);
        assert!(!stored.overlays.shown.overlay_riven);
    }

    #[test]
    fn placements_round_trip() {
        let stored: Settings = serde_json::from_str(
            r#"{"overlay_relic_reward_placement":"bottom_left",
                "overlay_relic_recommendation_placement":"bottom_right",
                "overlay_riven_placement":"centre",
                "overlay_recommendation_refinement":"owned",
                "overlay_opacity":70,
                "overlay_recommendation_count":9}"#,
        )
        .unwrap();
        assert_eq!(
            stored.overlays.placements.overlay_relic_reward_placement,
            OverlayPlacement::BottomLeft
        );
        assert_eq!(
            stored
                .overlays
                .placements
                .overlay_relic_recommendation_placement,
            OverlayPlacement::BottomRight
        );
        assert_eq!(
            stored.overlays.placements.overlay_riven_placement,
            OverlayPlacement::Centre
        );
        assert_eq!(
            stored.overlays.overlay_recommendation_refinement,
            RecommendationRefinement::Owned
        );
        assert_eq!(stored.overlays.overlay_opacity, 70);
        assert_eq!(stored.overlays.overlay_recommendation_count, 9);

        let saved = serde_json::to_string(&stored).unwrap();
        assert!(saved.contains(r#""overlay_relic_reward_placement":"bottom_left""#));
        assert!(saved.contains(r#""overlay_riven_placement":"centre""#));
        assert!(saved.contains(r#""overlay_recommendation_refinement":"owned""#));

        let top: Settings =
            serde_json::from_str(r#"{"overlay_riven_placement":"top_right"}"#).unwrap();
        assert_eq!(
            top.overlays.placements.overlay_riven_placement,
            OverlayPlacement::TopRight
        );
    }

    #[test]
    fn only_while_game_active_opt_out() {
        let off: Settings =
            serde_json::from_str(r#"{"overlay_only_while_game_active":false}"#).unwrap();
        assert!(!off.overlays.overlay_only_while_game_active);

        let saved = serde_json::to_string(&off).unwrap();
        assert!(saved.contains(r#""overlay_only_while_game_active":false"#));
    }

    #[test]
    fn overlay_mode_round_trip() {
        let stored: Settings = serde_json::from_str(
            r#"{"overlay_riven":false,"overlay_relic_reward":false,"overlay_mode":"tab"}"#,
        )
        .unwrap();
        assert!(!stored.overlays.shown.overlay_riven);
        assert!(!stored.overlays.shown.overlay_relic_reward);
        assert!(stored.overlays.shown.overlay_relic_recommendation);
        assert_eq!(stored.overlays.overlay_mode, OverlayMode::Tab);

        let saved = serde_json::to_string(&stored).unwrap();
        assert!(saved.contains(r#""overlay_mode":"tab""#));

        let windows: Settings = serde_json::from_str(r#"{"overlay_mode":"windows"}"#).unwrap();
        assert_eq!(windows.overlays.overlay_mode, OverlayMode::Windows);
    }

    #[test]
    fn log_selection_default() {
        let settings = Settings::default();
        assert!(!settings.force_log_file);
        assert_eq!(settings.log_selection(), wf_log::Selection::Auto);

        let forced: Settings = serde_json::from_str(r#"{"force_log_file":true}"#).unwrap();
        assert_eq!(forced.log_selection(), wf_log::Selection::File);
    }

    #[test]
    fn discord_opt_in() {
        let settings = Settings::default();
        assert!(!settings.discord.mirrored.any());
        assert_eq!(settings.discord.discord_webhook, None);
        assert!(
            settings
                .discord
                .discord_message_template
                .contains("{tenno}")
        );

        let chosen: Settings =
            serde_json::from_str(r#"{"discord_message_template":"{tenno}?"}"#).unwrap();
        assert_eq!(chosen.discord.discord_message_template, "{tenno}?");
    }

    #[test]
    fn last_trade_actions_opt_in() {
        let enabled: Settings =
            serde_json::from_str(r#"{"market_hide_after_last_trade":true}"#).unwrap();
        assert!(!enabled.market.last_trade.market_offline_after_last_trade);
        assert!(enabled.market.last_trade.market_hide_after_last_trade);
        assert!(
            !enabled
                .market
                .last_trade
                .market_hide_auctions_after_last_trade
        );
        let saved = serde_json::to_value(&enabled).unwrap();
        assert_eq!(saved["market_hide_after_last_trade"], true);
    }

    #[test]
    fn auto_close_opt_in() {
        assert!(!Settings::default().market.market_auto_close);
        let enabled: Settings = serde_json::from_str(r#"{"market_auto_close":true}"#).unwrap();
        assert!(enabled.market.market_auto_close);
    }

    #[test]
    fn partial_document() {
        let stored: Settings = serde_json::from_str(r#"{"market_poll_minutes":10}"#).unwrap();
        assert!(stored.notifications.windows_notifications_enabled);
        assert!(stored.inventory.stats_tab_enabled);
        assert_eq!(stored.discord.discord_message_template, DISCORD_TEMPLATE);
    }

    #[test]
    fn world_state_interval_clamp() {
        let interval = |minutes: u32| {
            Settings {
                world_state_interval_minutes: minutes,
                ..Settings::default()
            }
            .world_state_interval()
        };
        assert_eq!(interval(0), Duration::from_secs(300));
        assert_eq!(interval(4), Duration::from_secs(300));
        assert_eq!(interval(5), Duration::from_secs(300));
        assert_eq!(interval(7), Duration::from_secs(420));
        assert_eq!(interval(10), Duration::from_secs(600));
        assert_eq!(interval(600), Duration::from_secs(600));
    }

    #[test]
    fn market_poll_clamp() {
        let settings = Settings::default();
        assert_eq!(settings.market.market_poll_minutes, 5);
        assert_eq!(settings.market_poll_interval(), Duration::from_secs(300));

        let interval = |minutes: u32| {
            Settings {
                market: MarketSettings {
                    market_poll_minutes: minutes,
                    ..MarketSettings::default()
                },
                ..Settings::default()
            }
            .market_poll_interval()
        };
        assert_eq!(interval(0), Duration::from_secs(120));
        assert_eq!(interval(1), Duration::from_secs(120));
        assert_eq!(interval(2), Duration::from_secs(120));
        assert_eq!(interval(15).as_secs(), 900);
        assert_eq!(interval(600).as_secs(), 3600);
    }

    #[test]
    fn founders_choice_default() {
        let settings = Settings::default();
        assert_eq!(settings.inventory.founders_choice(), None);
        assert!(settings.inventory.include_forma_ranks);

        let options = settings.mastery_options(wf_core::MasteryOrdering::FromRelics);
        assert_eq!(options.ordering, wf_core::MasteryOrdering::FromRelics);
        assert_eq!(options.include_founders_items, None);
        assert!(options.include_forma_ranks);
        assert_eq!(
            options,
            wf_core::MasteryOptions {
                ordering: wf_core::MasteryOrdering::FromRelics,
                ..wf_core::MasteryOptions::default()
            }
        );

        let stored: Settings =
            serde_json::from_str(r#"{"include_founders_items":true,"include_forma_ranks":false}"#)
                .unwrap();
        let options = stored.mastery_options(wf_core::MasteryOrdering::Closest);
        assert_eq!(options.include_founders_items, Some(true));
        assert!(!options.include_forma_ranks);
    }

    #[test]
    fn legacy_hide_founders_flag() {
        let stored: Settings = serde_json::from_str(r#"{"hide_founders_items":true}"#).unwrap();
        assert_eq!(stored.inventory.founders_choice(), Some(false));
        assert_eq!(stored.inventory.include_founders_items, None);

        let shown: Settings = serde_json::from_str(r#"{"hide_founders_items":false}"#).unwrap();
        assert_eq!(shown.inventory.founders_choice(), Some(true));

        let chosen: Settings =
            serde_json::from_str(r#"{"hide_founders_items":true,"include_founders_items":true}"#)
                .unwrap();
        assert_eq!(chosen.inventory.founders_choice(), Some(true));

        let saved = serde_json::to_string(&stored).unwrap();
        assert!(!saved.contains("hide_founders_items"));
    }

    #[test]
    fn stylesheet_sets_the_same_root_sizes() {
        let css = include_str!("../../src/globals.css").replace("\r\n", "\n");
        for (size, name) in [
            (TextSize::Small, "small"),
            (TextSize::Medium, "medium"),
            (TextSize::Large, "large"),
        ] {
            let rule = format!(
                "html[data-text-size=\"{name}\"] {{\n    font-size: {}px;",
                size.root_px()
            );
            assert!(css.contains(&rule), "{rule}");
        }
        let fallback = format!(
            "html {{\n    font-size: {}px;",
            TextSize::default().root_px()
        );
        assert!(css.contains(&fallback), "{fallback}");
    }

    #[test]
    fn wants_overlay_windows_from_bytes() {
        let shipped = serde_json::json!({ "settings": Settings::default() }).to_string();
        assert!(wants_overlay_windows(shipped.as_bytes()));
        assert!(wants_overlay_windows(
            br#"{"settings":{"overlay_mode":"windows"}}"#
        ));
        assert!(!wants_overlay_windows(
            br#"{"settings":{"overlays_enabled":false}}"#
        ));
        assert!(!wants_overlay_windows(
            br#"{"settings":{"overlay_mode":"tab"}}"#
        ));
        assert!(wants_overlay_windows(b""));
        assert!(wants_overlay_windows(b"{\"settings\":"));
    }
}
