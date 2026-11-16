use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use wf_inventory::Inventory;
use wf_worldstate::{
    ArchimedeaKind, ArchimedeaWeek, ArchonHunt, CalendarSeason, ClanVaultBonusWeek,
    NightwaveSeason, ResetWindow, Sortie, SyndicateMissionSet, WorldState,
};

use crate::catalog::Catalog;
use crate::identity::ItemTable;
use crate::reset_checklist::cadence::ResetCadence;
use crate::reset_checklist::definitions::ResetChecklistManualRowId;

pub(crate) struct ResetChecklistWorld {
    pub(crate) sortie: Option<Sortie>,
    pub(crate) archon_hunt: Option<ArchonHunt>,
    pub(crate) archimedea: Vec<ArchimedeaWeek>,
    pub(crate) descendia: Option<ResetWindow>,
    pub(crate) calendar: Option<CalendarSeason>,
    pub(crate) syndicate_sets: Vec<SyndicateMissionSet>,
    pub(crate) nightwave: Option<NightwaveSeason>,
    pub(crate) clan_vault: Option<ClanVaultBonusWeek>,
}

impl ResetChecklistWorld {
    pub(crate) fn new(world: &WorldState, catalog: &Catalog, now: DateTime<Utc>) -> Self {
        Self {
            sortie: world.sortie(now),
            archon_hunt: world.archon_hunt(now),
            archimedea: world.archimedea_weeks(now),
            descendia: world.descendia_window(now),
            calendar: world.calendar_season(now),
            syndicate_sets: world.syndicate_mission_sets(now),
            nightwave: world.season(now, |challenge| catalog.nightwave_act_text(challenge)),
            clan_vault: world.clan_vault_bonus_week(now),
        }
    }

    pub(crate) fn archimedea(&self, kind: ArchimedeaKind) -> Option<&ArchimedeaWeek> {
        self.archimedea.iter().find(|week| week.kind == kind)
    }
}

#[derive(Default)]
pub(crate) struct ResetChecklistStored {
    pub(crate) last_mission_won: Option<DateTime<Utc>>,
    pub(crate) checked: HashSet<ResetChecklistManualRowId>,
    pub(crate) kuva_bought: HashMap<String, u32>,
}

pub(crate) struct ResetChecklistContext<'a> {
    pub(crate) inventory: &'a Inventory,
    pub(crate) catalog: &'a Catalog,
    pub(crate) items: &'a ItemTable,
    pub(crate) world: Option<ResetChecklistWorld>,
    pub(crate) now: DateTime<Utc>,
    pub(crate) synced: Option<DateTime<Utc>>,
    pub(crate) stored: ResetChecklistStored,
    pub(crate) completed_quests: HashSet<&'a str>,
}

impl<'a> ResetChecklistContext<'a> {
    pub(crate) fn new(
        inventory: &'a Inventory,
        catalog: &'a Catalog,
        items: &'a ItemTable,
        world: Option<ResetChecklistWorld>,
        now: DateTime<Utc>,
        stored: ResetChecklistStored,
    ) -> Self {
        Self {
            inventory,
            catalog,
            items,
            world,
            now,
            synced: inventory
                .last_inventory_sync
                .seconds()
                .and_then(|seconds| DateTime::from_timestamp(i64::from(seconds), 0)),
            stored,
            completed_quests: inventory.completed_quests(),
        }
    }

    pub(crate) fn synced_within(&self, window: ResetWindow) -> bool {
        self.synced
            .is_some_and(|synced| synced >= window.activation && synced < window.expiry)
    }

    pub(crate) fn synced_today(&self) -> bool {
        self.synced_within(ResetCadence::Daily.window(self.now))
    }

    pub(crate) fn left_today(&self, left: u32, cap: u32) -> u32 {
        if self.synced_today() { left } else { cap }
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use std::sync::LazyLock;

    use chrono::{DateTime, Utc};
    use wf_inventory::Inventory;
    use wf_worldstate::WorldState;

    use super::{ResetChecklistContext, ResetChecklistStored, ResetChecklistWorld};
    use crate::catalog::Catalog;
    use crate::catalog::fixtures::{WORLD_STATE, at, nightwave_catalog};
    use crate::identity::ItemTable;

    pub(crate) static CATALOG: LazyLock<Catalog> = LazyLock::new(nightwave_catalog);

    pub(crate) static ITEMS: LazyLock<ItemTable> = LazyLock::new(|| ItemTable::build(&CATALOG));

    pub(crate) fn synced_at() -> DateTime<Utc> {
        at(1_788_799_775_000)
    }

    pub(crate) fn world_at(now: DateTime<Utc>) -> ResetChecklistWorld {
        ResetChecklistWorld::new(&WorldState::parse(WORLD_STATE).unwrap(), &CATALOG, now)
    }

    pub(crate) fn context(inventory: &Inventory, now: DateTime<Utc>) -> ResetChecklistContext<'_> {
        ResetChecklistContext::new(
            inventory,
            &CATALOG,
            &ITEMS,
            Some(world_at(now)),
            now,
            ResetChecklistStored::default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use super::fixtures::{context, synced_at};
    use crate::catalog::fixtures;

    #[test]
    fn stale_sync_reads_full() {
        let inventory = fixtures::inventory();

        let same_day = context(&inventory, synced_at());
        let next_day = context(&inventory, synced_at() + TimeDelta::days(1));

        assert_eq!(same_day.synced, Some(synced_at()));
        assert!(same_day.synced_today());
        assert_eq!(same_day.left_today(6, 14), 6);
        assert!(!next_day.synced_today());
        assert_eq!(next_day.left_today(6, 14), 14);
    }
}
