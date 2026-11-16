use chrono::{DateTime, Utc};
use serde::Serialize;
use wf_worldstate::{ArchimedeaKind, ResetWindow, daily_reset_window, weekly_reset_window};

use crate::reset_checklist::context::{ResetChecklistContext, ResetChecklistWorld};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ResetChecklistGroup {
    Daily,
    Weekly,
    #[serde(rename = "sortie")]
    SortieReset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResetCadence {
    Daily,
    Weekly,
}

impl ResetChecklistGroup {
    pub(crate) fn reset(self, context: &ResetChecklistContext) -> Option<ResetWindow> {
        match self {
            Self::Daily => Some(ResetCadence::Daily.window(context.now)),
            Self::Weekly => Some(ResetCadence::Weekly.window(context.now)),
            Self::SortieReset => Cadence::World(WorldCadence::Sortie).window(context),
        }
    }
}

impl From<ResetCadence> for ResetChecklistGroup {
    fn from(cadence: ResetCadence) -> Self {
        match cadence {
            ResetCadence::Daily => Self::Daily,
            ResetCadence::Weekly => Self::Weekly,
        }
    }
}

impl ResetCadence {
    pub(crate) fn window(self, now: DateTime<Utc>) -> ResetWindow {
        match self {
            Self::Daily => daily_reset_window(now),
            Self::Weekly => weekly_reset_window(now),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorldCadence {
    Sortie,
    ArchonHunt,
    Archimedea(ArchimedeaKind),
    Descendia,
    Calendar,
}

impl WorldCadence {
    fn window(self, world: &ResetChecklistWorld) -> Option<ResetWindow> {
        match self {
            Self::Sortie => world.sortie.as_ref().map(|sortie| ResetWindow {
                activation: sortie.activation,
                expiry: sortie.expiry,
            }),
            Self::ArchonHunt => world.archon_hunt.as_ref().map(|hunt| ResetWindow {
                activation: hunt.activation,
                expiry: hunt.expiry,
            }),
            Self::Archimedea(kind) => world.archimedea(kind).map(|week| week.window),
            Self::Descendia => world.descendia,
            Self::Calendar => world.calendar.as_ref().map(|season| season.window),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cadence {
    Reset(ResetCadence),
    World(WorldCadence),
}

impl Cadence {
    pub(crate) fn group(self) -> ResetChecklistGroup {
        match self {
            Self::Reset(cadence) => cadence.into(),
            Self::World(WorldCadence::Sortie) => ResetChecklistGroup::SortieReset,
            Self::World(
                WorldCadence::ArchonHunt
                | WorldCadence::Archimedea(_)
                | WorldCadence::Descendia
                | WorldCadence::Calendar,
            ) => ResetChecklistGroup::Weekly,
        }
    }

    fn window(self, context: &ResetChecklistContext) -> Option<ResetWindow> {
        match self {
            Self::Reset(cadence) => Some(cadence.window(context.now)),
            Self::World(cadence) => cadence.window(context.world.as_ref()?),
        }
    }

    pub(crate) fn own_window(self, context: &ResetChecklistContext) -> Option<ResetWindow> {
        self.window(context)
            .filter(|&window| Some(window) != self.group().reset(context))
    }
}
