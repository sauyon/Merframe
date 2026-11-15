use serde::Serialize;
use wf_inventory::DailyStandingPool;

use crate::reset_checklist::context::ResetChecklistContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ResetChecklistStandingPool {
    pub name: &'static str,
    pub remaining: u32,
    pub cap: u32,
}

fn pool_name(pool: DailyStandingPool) -> &'static str {
    match pool {
        DailyStandingPool::Factions => "Faction Syndicates",
        DailyStandingPool::Cetus => "Ostron",
        DailyStandingPool::Quills => "The Quills",
        DailyStandingPool::Solaris => "Solaris United",
        DailyStandingPool::Ventkids => "Ventkids",
        DailyStandingPool::Vox => "Vox Solaris",
        DailyStandingPool::Entrati => "Entrati",
        DailyStandingPool::Necraloid => "Necraloid",
        DailyStandingPool::Zariman => "The Holdfasts",
        DailyStandingPool::Kahl => "Kahl's Garrison",
        DailyStandingPool::Cavia => "Cavia",
        DailyStandingPool::Hex => "The Hex",
        DailyStandingPool::Library => "Cephalon Simaris",
        DailyStandingPool::Pvp => "Conclave",
    }
}

pub(crate) fn standing_pools(context: &ResetChecklistContext) -> Vec<ResetChecklistStandingPool> {
    let cap = 16_000 + 500 * context.inventory.player_level;
    context
        .inventory
        .daily_standing_remaining()
        .map(|(pool, remaining)| ResetChecklistStandingPool {
            name: pool_name(pool),
            remaining: context.left_today(remaining, cap),
            cap,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use super::*;
    use crate::catalog::fixtures;
    use crate::reset_checklist::context::fixtures::{context, synced_at};

    #[test]
    fn today_and_stale() {
        let inventory = fixtures::inventory();

        let today = standing_pools(&context(&inventory, synced_at()));
        let tomorrow = standing_pools(&context(&inventory, synced_at() + TimeDelta::days(1)));

        assert_eq!(today.len(), 14);
        assert_eq!(
            today[0],
            ResetChecklistStandingPool {
                name: "Faction Syndicates",
                remaining: 21_000,
                cap: 23_000,
            }
        );
        assert!(
            today
                .iter()
                .any(|pool| pool.name == "Cephalon Simaris" && pool.remaining == 0)
        );
        assert!(tomorrow.iter().all(|pool| pool.remaining == 23_000));
    }
}
