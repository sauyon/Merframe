use chrono::TimeDelta;
use serde::Serialize;
use wf_inventory::{CalendarSeasonProgress, Inventory};
use wf_worldstate::{
    ArchimedeaKind, ArchimedeaWeek, CalendarSeason, ClanVaultBonusWeek, game_week_count,
};

use crate::reset_checklist::ResetChecklistRowPart;
use crate::reset_checklist::cadence::{Cadence, ResetCadence, WorldCadence};
use crate::reset_checklist::context::{ResetChecklistContext, ResetChecklistWorld};
use crate::reset_checklist::definitions::ResetChecklistManualRowId;
use crate::reset_checklist::nightwave::{active_acts, recovered_acts};
use crate::reset_checklist::syndicate_missions::syndicate_missions;

const SEARCH_PULSES: u32 = 5;
const DESCENDIA_FLOORS: u32 = 21;
const CIRCUIT_REWARD_TIERS: u32 = 10;
const ICEBIND_CHOICES: u32 = 3;
const ICEBIND_CRYOBELLS_HELD: usize = 6;
const HELMINTH_INVIGORATIONS: u32 = 3;
const FOUNDER_EXTRA_TRADES: u32 = 2;
const HELMINTH_INVIGORATION_DAYS: i64 = 7;
const FOCUS_CAP_BASE: u32 = 250_000;
const FOCUS_CAP_PER_RANK: u32 = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResetChecklistProgress {
    Counted {
        done: u32,
        total: u32,
    },
    Remaining {
        left: u32,
        cap: u32,
    },
    YesNo,
    Manual {
        id: ResetChecklistManualRowId,
        checked: bool,
    },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Tracked {
    pub(crate) progress: ResetChecklistProgress,
    pub(crate) done: bool,
    pub(crate) detail: Option<String>,
    pub(crate) parts: Vec<ResetChecklistRowPart>,
}

impl Tracked {
    fn new(progress: ResetChecklistProgress, done: bool) -> Self {
        Self {
            progress,
            done,
            detail: None,
            parts: Vec::new(),
        }
    }

    pub(crate) fn counted(done: u32, total: u32) -> Self {
        Self::new(
            ResetChecklistProgress::Counted { done, total },
            total > 0 && done >= total,
        )
    }

    pub(crate) fn finished(done: bool) -> Self {
        Self::new(ResetChecklistProgress::YesNo, done)
    }

    fn remaining(left: u32, cap: u32) -> Self {
        Self::new(ResetChecklistProgress::Remaining { left, cap }, left == 0)
    }

    fn manual(id: ResetChecklistManualRowId, checked: bool) -> Self {
        Self::new(ResetChecklistProgress::Manual { id, checked }, checked)
    }

    fn unknown() -> Self {
        Self::new(ResetChecklistProgress::Unknown, false)
    }

    fn with_done(self, done: bool) -> Self {
        Self { done, ..self }
    }

    pub(crate) fn with_detail(self, detail: Option<String>) -> Self {
        Self { detail, ..self }
    }

    pub(crate) fn with_parts(self, parts: Vec<ResetChecklistRowPart>) -> Self {
        Self { parts, ..self }
    }
}

pub(crate) fn tally<T>(items: impl IntoIterator<Item = T>) -> u32 {
    items.into_iter().map(|_| 1).sum()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tracker {
    Inventory(InventoryTracker),
    World(WorldTracker),
    Manual {
        id: ResetChecklistManualRowId,
        cadence: ResetCadence,
    },
}

impl Tracker {
    pub(crate) fn cadence(self) -> Cadence {
        match self {
            Self::Inventory(tracker) => tracker.cadence(),
            Self::World(tracker) => tracker.cadence(),
            Self::Manual { cadence, .. } => Cadence::Reset(cadence),
        }
    }

    pub(crate) fn track(self, context: &ResetChecklistContext) -> Tracked {
        match self {
            Self::Inventory(tracker) => tracker.track(context),
            Self::World(tracker) => context
                .world
                .as_ref()
                .and_then(|world| tracker.track(context, world))
                .unwrap_or_else(Tracked::unknown),
            Self::Manual { id, .. } => Tracked::manual(id, context.stored.checked.contains(&id)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InventoryTracker {
    SearchPulses,
    Circuit(&'static str),
    Descendia(&'static str),
    Icebind,
    PeriodicMissionCount {
        tag_prefix: &'static str,
        total: u32,
        cadence: ResetCadence,
    },
    PeriodicMissionDone {
        tag_prefix: &'static str,
        cadence: ResetCadence,
    },
    KahlsGarrison,
    HelminthInvigorations,
    DailyTribute,
    FirstWin,
    Trades,
    Gifts,
    Focus,
}

impl InventoryTracker {
    fn cadence(self) -> Cadence {
        match self {
            Self::Descendia(_) => Cadence::World(WorldCadence::Descendia),
            Self::PeriodicMissionCount { cadence, .. }
            | Self::PeriodicMissionDone { cadence, .. } => Cadence::Reset(cadence),
            Self::DailyTribute | Self::FirstWin | Self::Trades | Self::Gifts | Self::Focus => {
                Cadence::Reset(ResetCadence::Daily)
            }
            Self::SearchPulses
            | Self::Circuit(_)
            | Self::Icebind
            | Self::KahlsGarrison
            | Self::HelminthInvigorations => Cadence::Reset(ResetCadence::Weekly),
        }
    }

    fn track(self, context: &ResetChecklistContext) -> Tracked {
        let inventory = context.inventory;
        let now = context.now;
        match self {
            Self::SearchPulses => {
                Tracked::counted(inventory.search_pulses_spent(now), SEARCH_PULSES)
            }
            Self::Circuit(category) => circuit(context, category),
            Self::Descendia(category) => Tracked::counted(
                inventory
                    .descendia_progress(now)
                    .find(|descent| descent.category == category)
                    .map_or(0, |descent| descent.floor_claimed),
                DESCENDIA_FLOORS,
            ),
            Self::Icebind => icebind(context),
            Self::PeriodicMissionCount {
                tag_prefix,
                total,
                cadence,
            } => Tracked::counted(periodic_completions(context, tag_prefix, cadence), total),
            Self::PeriodicMissionDone {
                tag_prefix,
                cadence,
            } => Tracked::finished(periodic_completions(context, tag_prefix, cadence) > 0),
            Self::KahlsGarrison => Tracked::finished(kahls_garrison_done(context)),
            Self::HelminthInvigorations => {
                Tracked::counted(helminth_invigorations(context), HELMINTH_INVIGORATIONS)
            }
            Self::DailyTribute => Tracked::finished(context.synced_today()),
            Self::FirstWin => Tracked::finished(
                context
                    .stored
                    .last_mission_won
                    .is_some_and(|won| won >= ResetCadence::Daily.window(context.now).activation),
            ),
            Self::Trades => remaining_today(
                context,
                Some(inventory.trades_remaining),
                daily_trade_cap(inventory),
            ),
            Self::Gifts => remaining_today(
                context,
                inventory.gifts_remaining,
                daily_trade_cap(inventory),
            ),
            Self::Focus => remaining_today(
                context,
                inventory.daily_focus,
                FOCUS_CAP_BASE + FOCUS_CAP_PER_RANK * inventory.player_level,
            ),
        }
    }
}

fn daily_trade_cap(inventory: &Inventory) -> u32 {
    let founder_trades = if inventory.is_founder() {
        FOUNDER_EXTRA_TRADES
    } else {
        0
    };
    inventory.player_level + founder_trades
}

fn remaining_today(context: &ResetChecklistContext, left: Option<u32>, cap: u32) -> Tracked {
    left.map_or_else(Tracked::unknown, |left| {
        Tracked::remaining(context.left_today(left, cap), cap)
    })
}

fn periodic_completions(
    context: &ResetChecklistContext,
    tag_prefix: &str,
    cadence: ResetCadence,
) -> u32 {
    tally(
        context
            .inventory
            .periodic_missions_since(cadence.window(context.now).activation)
            .filter(|completed| completed.starts_with(tag_prefix)),
    )
}

fn circuit(context: &ResetChecklistContext, category: &str) -> Tracked {
    let Some(progress) = context
        .inventory
        .circuit_progress(context.now)
        .find(|circuit| circuit.category == category)
    else {
        return Tracked::counted(0, CIRCUIT_REWARD_TIERS);
    };

    Tracked::counted(
        tally(progress.reward_tiers_earned()),
        tally(progress.reward_tiers()),
    )
    .with_detail(Some(progress.choices.join(" and ")))
}

fn icebind(context: &ResetChecklistContext) -> Tracked {
    let inventory = context.inventory;
    let claimed = inventory
        .kuva_keys_rewards
        .as_ref()
        .map_or(0, |rewards| tally(rewards.claimed_choices(context.now)));
    let held = inventory.cryobells_held();

    Tracked::counted(claimed, ICEBIND_CHOICES).with_detail(Some(format!(
        "{held}/{ICEBIND_CRYOBELLS_HELD} in inventory"
    )))
}

fn kahls_garrison_done(context: &ResetChecklistContext) -> bool {
    let this_week = game_week_count(context.now);
    context
        .inventory
        .affiliations
        .iter()
        .filter(|affiliation| affiliation.tag == "KahlSyndicate")
        .flat_map(|affiliation| &affiliation.weekly_missions)
        .any(|mission| mission.week_count > this_week)
}

fn helminth_invigorations(context: &ResetChecklistContext) -> u32 {
    let applied_since = ResetCadence::Weekly.window(context.now).activation
        + TimeDelta::days(HELMINTH_INVIGORATION_DAYS);
    tally(
        context
            .inventory
            .suits
            .iter()
            .filter_map(|suit| suit.helminth_invigoration_expiry(context.now))
            .filter(|expiry| expiry.datetime() >= applied_since),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorldTracker {
    Sortie,
    ArchonHunt,
    Archimedea(ArchimedeaKind),
    ClanVault,
    Calendar,
    Nightwave(ResetCadence),
    NightwaveRecovered,
    SyndicateMissions,
}

impl WorldTracker {
    fn cadence(self) -> Cadence {
        match self {
            Self::Sortie | Self::SyndicateMissions => Cadence::World(WorldCadence::Sortie),
            Self::ArchonHunt => Cadence::World(WorldCadence::ArchonHunt),
            Self::Archimedea(kind) => Cadence::World(WorldCadence::Archimedea(kind)),
            Self::Calendar => Cadence::World(WorldCadence::Calendar),
            Self::ClanVault | Self::NightwaveRecovered => Cadence::Reset(ResetCadence::Weekly),
            Self::Nightwave(cadence) => Cadence::Reset(cadence),
        }
    }

    fn track(
        self,
        context: &ResetChecklistContext,
        world: &ResetChecklistWorld,
    ) -> Option<Tracked> {
        let inventory = context.inventory;
        match self {
            Self::Sortie => world.sortie.as_ref().map(|sortie| {
                claimed_sortie(
                    inventory,
                    inventory.last_sortie_id(),
                    &sortie.id,
                    tally(&sortie.missions),
                )
            }),
            Self::ArchonHunt => world.archon_hunt.as_ref().map(|hunt| {
                claimed_sortie(
                    inventory,
                    inventory.last_archon_hunt_id(),
                    &hunt.id,
                    tally(&hunt.missions),
                )
            }),
            Self::Archimedea(kind) => world.archimedea(kind).map(|week| archimedea(context, week)),
            Self::ClanVault => world
                .clan_vault
                .as_ref()
                .map(|week| clan_vault(inventory, week)),
            Self::Calendar => world
                .calendar
                .as_ref()
                .map(|season| calendar(inventory, season)),
            Self::Nightwave(cadence) => world
                .nightwave
                .as_ref()
                .map(|season| active_acts(context, season, cadence)),
            Self::NightwaveRecovered => world
                .nightwave
                .as_ref()
                .and_then(|season| recovered_acts(context, season)),
            Self::SyndicateMissions => syndicate_missions(inventory, &world.syndicate_sets),
        }
    }
}

fn claimed_sortie(
    inventory: &Inventory,
    claimed_id: Option<&str>,
    id: &str,
    missions: u32,
) -> Tracked {
    Tracked::counted(tally(inventory.sortie_nodes_completed(id)), missions)
        .with_done(claimed_id == Some(id))
}

fn archimedea(context: &ResetChecklistContext, week: &ArchimedeaWeek) -> Tracked {
    let inventory = context.inventory;
    let score = match week.kind {
        ArchimedeaKind::Deep => inventory.entrati_lab_conquest_cache_score_mission,
        ArchimedeaKind::Temporal => inventory.echoes_hex_conquest_cache_score_mission,
    };

    Tracked::finished(score > 0 && context.synced_within(week.window))
}

fn clan_vault(inventory: &Inventory, week: &ClanVaultBonusWeek) -> Tracked {
    let claimed = inventory
        .weekly_guild_vault_bonus_progress
        .iter()
        .find(|progress| progress.week_count == week.week_count)
        .and_then(|progress| progress.max_reward_claimed)
        .map_or(0, |highest| highest + 1);

    Tracked::counted(claimed, tally(&week.rewards))
}

fn calendar(inventory: &Inventory, season: &CalendarSeason) -> Tracked {
    let days_done = inventory
        .calendar_progress
        .as_ref()
        .map(|progress| &progress.season_progress)
        .filter(|progress| progress.season_type == season.season)
        .and_then(CalendarSeasonProgress::last_completed_day)
        .map_or(0, |last| last + 1);

    Tracked::counted(days_done, tally(&season.days))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn done_rules() {
        assert!(Tracked::counted(6, 6).done);
        assert!(!Tracked::counted(5, 6).done);
        assert!(!Tracked::counted(0, 0).done);
        assert!(Tracked::finished(true).done);
        assert!(!Tracked::finished(false).done);
        assert!(Tracked::remaining(0, 14).done);
        assert!(!Tracked::remaining(1, 14).done);
        assert!(Tracked::manual(ResetChecklistManualRowId::VentkidsRaces, true).done);
        assert!(!Tracked::unknown().done);
    }
}
