use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::resets::game_week_count;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RawClanVaultBonusWeek {
    pub week_count: i64,
    #[serde(default)]
    pub rewards: Vec<IgnoredAny>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClanVaultBonusWeek {
    pub week_count: i64,
    pub rewards: Vec<IgnoredAny>,
}

pub fn current_clan_vault_bonus_week(
    weeks: &[RawClanVaultBonusWeek],
    now: DateTime<Utc>,
) -> Option<ClanVaultBonusWeek> {
    let week_count = game_week_count(now);
    let week = weeks.iter().find(|week| week.week_count == week_count)?;

    Some(ClanVaultBonusWeek {
        week_count,
        rewards: week.rewards.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::fixtures::at;

    const WEEKS: &str = r#"[
        {"WeekCount":656,"BonusRegion":"/Lotus/Language/Locations/Saturn","Rewards":[
            {"RewardClaimed":false,"PointThreshold":500,"ItemCount":12,"Reward":"/Lotus/StoreItems/Types/Items/MiscItems/NeuralSensor"},
            {"RewardClaimed":false,"PointThreshold":2000,"ItemCount":1,"Reward":"/Lotus/StoreItems/Types/Items/MiscItems/Forma"}]},
        {"WeekCount":657,"BonusRegion":"/Lotus/Language/Locations/Void","Rewards":[]}
    ]"#;

    #[test]
    fn current_week() {
        let weeks: Vec<RawClanVaultBonusWeek> = serde_json::from_str(WEEKS).unwrap();
        let week = current_clan_vault_bonus_week(&weeks, at(1_788_807_069_000)).unwrap();

        assert_eq!(week.week_count, 656);
        assert_eq!(week.rewards.len(), 2);
    }

    #[test]
    fn week_not_listed() {
        let weeks: Vec<RawClanVaultBonusWeek> = serde_json::from_str(WEEKS).unwrap();

        assert!(current_clan_vault_bonus_week(&weeks, at(1_800_000_000_000)).is_none());
    }
}
