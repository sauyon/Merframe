use serde::Serialize;
use wf_core::{ArbitrationRun, DucateringProgress, RewardScreen, RivenRow};

use super::{Kind, window_shows};
use crate::settings::{Motion, OVERLAY_OPACITY_DEFAULT, RecommendationRefinement, TextSize};
use crate::state::AppState;

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

/// How the buff strip draws, from the buff settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct BuffStyle {
    /// Seconds left at which an ability's buff turns amber.
    pub warn_seconds: u8,
    /// Ability numbers sit on discs covering the game's dots, or else just under them.
    pub cover_dots: bool,
    /// The strip's size, in percent.
    pub scale: u8,
    /// How far below the game's dots the strip sits, in pixels at 1080p.
    pub offset: i8,
}

impl BuffStyle {
    pub(super) fn from(settings: &crate::settings::BuffSettings) -> Self {
        Self {
            warn_seconds: settings.buffs_warn_seconds.min(60),
            cover_dots: settings.buffs_cover_dots,
            scale: settings.buffs_scale.clamp(50, 200),
            offset: settings.buffs_offset.clamp(-60, 60),
        }
    }
}

impl Default for BuffStyle {
    fn default() -> Self {
        Self::from(&crate::settings::BuffSettings::default())
    }
}

/// Where a buff comes from, which decides whether the buff settings show it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum BuffKind {
    /// One of the equipped warframe's abilities.
    Ability,
    /// The player's own arcanes, mods and the like.
    Own,
    /// A squadmate's ability.
    Squad,
}

/// A buff seen on the strip, for the settings to offer hiding it by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct SeenBuff {
    pub name: String,
    pub kind: BuffKind,
    pub icon: Option<String>,
}

/// One buff on the countdown strip under the ability dots.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct BuffTimer {
    pub id: String,
    pub label: String,
    /// The ability slot, counted from 1, the buff comes from; `None` for other buffs.
    pub slot: Option<u8>,
    /// For an ability that draws its buffs in its own widget, like Tribute, where that widget
    /// puts this one: 0 for the one it shows largest, which cycling in game changes.
    pub rank: Option<u8>,
    /// The game's icon for a buff outside the ability slots, as an item image name.
    pub icon: Option<String>,
    /// Seconds the buff lasted when it was last applied.
    pub total: f64,
    /// Unix milliseconds the buff runs out, `None` once it has dropped off.
    pub ends_at: Option<f64>,
}

/// Where the game's HUD draws the ability dots and how large, read from the HUD, so the strip
/// follows the HUD scale and borders. Stage units, of the stage the HUD is laid out on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct BuffLayout {
    pub stage_width: f32,
    pub stage_height: f32,
    /// The dots' clip, as the HUD placed it.
    pub x: f32,
    pub y: f32,
    /// 1 at the default HUD size.
    pub hud_scale: f32,
}

/// The player's health, shields, overguard and energy, as the HUD shows them, whole numbers
/// as it prints them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct Vitals {
    pub health: u32,
    pub max_health: u32,
    pub shield: u32,
    pub max_shield: u32,
    /// How far over their maximum shields can go.
    pub max_overshield: u32,
    pub overguard: u32,
    pub max_overguard: u32,
    pub energy: u32,
    pub max_energy: u32,
    /// The colour the HUD draws the energy bar in, `0xRRGGBB`.
    pub energy_colour: Option<u32>,
    /// The colours the HUD draws the other bars in, `0xRRGGBB`.
    pub health_colour: Option<u32>,
    pub shield_colour: Option<u32>,
    pub overshield_colour: Option<u32>,
    pub overguard_colour: Option<u32>,
}

impl From<wf_scan::Vitals> for Vitals {
    fn from(read: wf_scan::Vitals) -> Self {
        // Saturating, as the HUD prints them; energy is a float the HUD rounds down.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let whole = |value: f32| value.floor() as u32;
        Self {
            health: read.health,
            max_health: read.max_health,
            shield: read.shield,
            max_shield: read.max_shield,
            max_overshield: read.max_overshield,
            overguard: read.overguard,
            max_overguard: read.max_overguard,
            energy: whole(read.energy),
            max_energy: whole(read.max_energy),
            energy_colour: read.energy_colour,
            health_colour: read.health_colour,
            shield_colour: read.shield_colour,
            overshield_colour: read.overshield_colour,
            overguard_colour: read.overguard_colour,
        }
    }
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
    pub arbitration: Option<Box<ArbitrationRun>>,
    pub buffs: Option<Vec<BuffTimer>>,
    pub buffs_style: BuffStyle,
    /// `None` until the HUD's layout has been read.
    pub buffs_layout: Option<BuffLayout>,
    /// Shown with the buff strip, while the HUD shows them and the settings want them.
    pub vitals: Option<Vitals>,
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
            arbitration: None,
            buffs: None,
            buffs_style: BuffStyle::default(),
            buffs_layout: None,
            vitals: None,
            preview: None,
        }
    }
}

impl OverlayState {
    pub(super) fn shows(&self, state: &AppState, only_while_game_active: bool, kind: Kind) -> bool {
        let focused = if self.preview == Some(kind) {
            None
        } else {
            state.focus.focused()
        };
        window_shows(only_while_game_active, focused, self.occupied(kind))
    }

    pub(super) fn occupied(&self, kind: Kind) -> bool {
        match kind {
            Kind::RelicReward => self.reward.is_some(),
            Kind::RelicRecommendation => self.recommendation.is_some(),
            Kind::Riven => self.riven.is_some(),
            Kind::Ducatering => self.ducatering.is_some(),
            Kind::Notification => self.notification.is_some(),
            Kind::ArbitrationLive => self.arbitration.is_some(),
            Kind::Buffs => self.buffs.is_some(),
        }
    }

    pub(super) fn clear(&mut self, kind: Kind) {
        self.preview.take_if(|preview| *preview == kind);
        match kind {
            Kind::RelicReward => self.reward = None,
            Kind::RelicRecommendation => self.recommendation = None,
            Kind::Riven => self.riven = None,
            Kind::Ducatering => self.ducatering = None,
            Kind::Notification => self.notification = None,
            Kind::ArbitrationLive => self.arbitration = None,
            Kind::Buffs => {
                self.buffs = None;
                self.vitals = None;
            }
        }
    }
}
