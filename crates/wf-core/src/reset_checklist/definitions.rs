use serde::{Deserialize, Serialize};
use wf_worldstate::ArchimedeaKind;

use crate::reset_checklist::cadence::ResetCadence;
use crate::reset_checklist::trackers::{InventoryTracker, Tracker, WorldTracker};
use crate::reset_checklist::unlocks::{Quest, ResetChecklistRequirement, SyndicateRank};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ResetChecklistRowId {
    Sortie,
    ArchonHunt,
    SearchPulses,
    DeepArchimedea,
    TemporalArchimedea,
    Circuit,
    CircuitSteelPath,
    Descendia,
    DescendiaSteelPath,
    Icebind,
    HelpClem,
    MaroosTreasureHunt,
    SteelPathIncursions,
    KahlsGarrison,
    ClanWeeklyInitiatives,
    HelminthInvigorations,
    DailyTribute,
    FirstWin,
    SimarisSynthesis,
    Calendar,
    NightwaveDailyActs,
    NightwaveWeeklyActs,
    NightwaveRecoveredActs,
    SyndicateMissions,
    Trades,
    Gifts,
    Focus,
    VentkidsRaces,
    OopsAllEximus,
    ConclaveChallenges,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ResetChecklistManualRowId {
    SimarisSynthesis,
    VentkidsRaces,
    ConclaveChallenges,
}

impl From<ResetChecklistManualRowId> for ResetChecklistRowId {
    fn from(id: ResetChecklistManualRowId) -> Self {
        match id {
            ResetChecklistManualRowId::SimarisSynthesis => Self::SimarisSynthesis,
            ResetChecklistManualRowId::VentkidsRaces => Self::VentkidsRaces,
            ResetChecklistManualRowId::ConclaveChallenges => Self::ConclaveChallenges,
        }
    }
}

pub(crate) struct ResetChecklistRowDefinition {
    pub(crate) id: ResetChecklistRowId,
    pub(crate) name: &'static str,
    pub(crate) requirements: &'static [ResetChecklistRequirement],
    pub(crate) tracker: Tracker,
}

pub(crate) const RESET_CHECKLIST_ROWS: [ResetChecklistRowDefinition; 30] = [
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::Sortie,
        name: "Sortie",
        requirements: &[ResetChecklistRequirement::Quest(Quest::WarWithin)],
        tracker: Tracker::World(WorldTracker::Sortie),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::SteelPathIncursions,
        name: "Steel Path Incursions",
        requirements: &[ResetChecklistRequirement::SteelPathMissionCompleted],
        tracker: Tracker::Inventory(InventoryTracker::PeriodicMissionCount {
            tag_prefix: "HardDaily",
            total: 6,
            cadence: ResetCadence::Daily,
        }),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::DailyTribute,
        name: "Daily Tribute",
        requirements: &[],
        tracker: Tracker::Inventory(InventoryTracker::DailyTribute),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::FirstWin,
        name: "First Win of the Day",
        requirements: &[],
        tracker: Tracker::Inventory(InventoryTracker::FirstWin),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::SimarisSynthesis,
        name: "Cephalon Simaris Synthesis Target",
        requirements: &[ResetChecklistRequirement::Quest(Quest::NewStrange)],
        tracker: Tracker::Manual {
            id: ResetChecklistManualRowId::SimarisSynthesis,
            cadence: ResetCadence::Daily,
        },
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::NightwaveDailyActs,
        name: "Nightwave Daily Acts",
        requirements: &[],
        tracker: Tracker::World(WorldTracker::Nightwave(ResetCadence::Daily)),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::SyndicateMissions,
        name: "Syndicate Missions",
        requirements: &[ResetChecklistRequirement::AnyFactionSyndicateRanked],
        tracker: Tracker::World(WorldTracker::SyndicateMissions),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::Trades,
        name: "Trades",
        requirements: &[],
        tracker: Tracker::Inventory(InventoryTracker::Trades),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::Gifts,
        name: "Gifts",
        requirements: &[],
        tracker: Tracker::Inventory(InventoryTracker::Gifts),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::Focus,
        name: "Focus",
        requirements: &[ResetChecklistRequirement::Quest(Quest::SecondDream)],
        tracker: Tracker::Inventory(InventoryTracker::Focus),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::VentkidsRaces,
        name: "Ventkids Races",
        requirements: &[],
        tracker: Tracker::Manual {
            id: ResetChecklistManualRowId::VentkidsRaces,
            cadence: ResetCadence::Daily,
        },
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::ConclaveChallenges,
        name: "Conclave Challenges",
        requirements: &[],
        tracker: Tracker::Manual {
            id: ResetChecklistManualRowId::ConclaveChallenges,
            cadence: ResetCadence::Daily,
        },
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::ArchonHunt,
        name: "Archon Hunt",
        requirements: &[ResetChecklistRequirement::Quest(Quest::Veilbreaker)],
        tracker: Tracker::World(WorldTracker::ArchonHunt),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::SearchPulses,
        name: "Search Pulses",
        requirements: &[ResetChecklistRequirement::Quest(Quest::WhispersInTheWalls)],
        tracker: Tracker::Inventory(InventoryTracker::SearchPulses),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::DeepArchimedea,
        name: "Deep Archimedea",
        requirements: &[ResetChecklistRequirement::SyndicateRank(
            SyndicateRank::CAVIA_5,
        )],
        tracker: Tracker::World(WorldTracker::Archimedea(ArchimedeaKind::Deep)),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::TemporalArchimedea,
        name: "Temporal Archimedea",
        requirements: &[
            ResetChecklistRequirement::Quest(Quest::TheHex),
            ResetChecklistRequirement::SyndicateRank(SyndicateRank::HEX_5),
        ],
        tracker: Tracker::World(WorldTracker::Archimedea(ArchimedeaKind::Temporal)),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::Circuit,
        name: "The Circuit",
        requirements: &[ResetChecklistRequirement::Quest(Quest::DuviriParadox)],
        tracker: Tracker::Inventory(InventoryTracker::Circuit("EXC_NORMAL")),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::CircuitSteelPath,
        name: "The Circuit (Steel Path)",
        requirements: &[
            ResetChecklistRequirement::Quest(Quest::DuviriParadox),
            ResetChecklistRequirement::SteelPathMissionCompleted,
        ],
        tracker: Tracker::Inventory(InventoryTracker::Circuit("EXC_HARD")),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::Descendia,
        name: "The Descendia",
        requirements: &[ResetChecklistRequirement::Quest(Quest::OldPeace)],
        tracker: Tracker::Inventory(InventoryTracker::Descendia("DM_COH_NORMAL")),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::DescendiaSteelPath,
        name: "The Descendia (Steel Path)",
        requirements: &[
            ResetChecklistRequirement::Quest(Quest::OldPeace),
            ResetChecklistRequirement::SteelPathMissionCompleted,
        ],
        tracker: Tracker::Inventory(InventoryTracker::Descendia("DM_COH_HARD")),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::Icebind,
        name: "The Icebind",
        requirements: &[ResetChecklistRequirement::EliteArchimedeaUnlocked],
        tracker: Tracker::Inventory(InventoryTracker::Icebind),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::HelpClem,
        name: "Help Clem",
        requirements: &[ResetChecklistRequirement::Quest(Quest::ManOfFewWords)],
        tracker: Tracker::Inventory(InventoryTracker::PeriodicMissionDone {
            tag_prefix: "GetClem",
            cadence: ResetCadence::Weekly,
        }),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::MaroosTreasureHunt,
        name: "Maroo's Ayatan Treasure Hunt",
        requirements: &[],
        tracker: Tracker::Inventory(InventoryTracker::PeriodicMissionDone {
            tag_prefix: "TreasureHunt",
            cadence: ResetCadence::Weekly,
        }),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::KahlsGarrison,
        name: "Kahl's Garrison",
        requirements: &[ResetChecklistRequirement::Quest(Quest::Veilbreaker)],
        tracker: Tracker::Inventory(InventoryTracker::KahlsGarrison),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::ClanWeeklyInitiatives,
        name: "Clan Weekly Initiatives",
        requirements: &[],
        tracker: Tracker::World(WorldTracker::ClanVault),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::HelminthInvigorations,
        name: "Helminth Invigorations",
        requirements: &[],
        tracker: Tracker::Inventory(InventoryTracker::HelminthInvigorations),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::Calendar,
        name: "1999 Calendar",
        requirements: &[ResetChecklistRequirement::Quest(Quest::TheHex)],
        tracker: Tracker::World(WorldTracker::Calendar),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::NightwaveWeeklyActs,
        name: "Nightwave Weekly Acts",
        requirements: &[],
        tracker: Tracker::World(WorldTracker::Nightwave(ResetCadence::Weekly)),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::NightwaveRecoveredActs,
        name: "Nightwave Recovered Acts",
        requirements: &[],
        tracker: Tracker::World(WorldTracker::NightwaveRecovered),
    },
    ResetChecklistRowDefinition {
        id: ResetChecklistRowId::OopsAllEximus,
        name: "Oops! All Eximus!",
        requirements: &[ResetChecklistRequirement::DialogueFlag(
            "FF1AdventureComplete",
        )],
        tracker: Tracker::Inventory(InventoryTracker::PeriodicMissionDone {
            tag_prefix: "AmirWeekly",
            cadence: ResetCadence::Weekly,
        }),
    },
];

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn ids_unique() {
        let ids: HashSet<_> = RESET_CHECKLIST_ROWS
            .iter()
            .map(|definition| definition.id)
            .collect();

        assert_eq!(ids.len(), RESET_CHECKLIST_ROWS.len());
    }

    #[test]
    fn manual_rows_match_ids() {
        let manual: Vec<_> = RESET_CHECKLIST_ROWS
            .iter()
            .filter_map(|definition| match definition.tracker {
                Tracker::Manual { id, .. } => Some((definition.id, id)),
                _ => None,
            })
            .collect();
        let ids: HashSet<_> = manual.iter().map(|&(_, id)| id).collect();

        assert_eq!(
            ids,
            HashSet::from([
                ResetChecklistManualRowId::SimarisSynthesis,
                ResetChecklistManualRowId::VentkidsRaces,
                ResetChecklistManualRowId::ConclaveChallenges,
            ])
        );
        assert_eq!(manual.len(), ids.len());
        assert!(manual.iter().all(|&(id, manual)| id == manual.into()));
    }
}
