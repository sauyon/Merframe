use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::resets::ResetWindow;
use crate::window::Window;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RawCalendarSeason {
    #[serde(flatten)]
    pub window: Window,
    pub season: String,
    #[serde(default)]
    pub days: Vec<IgnoredAny>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalendarSeason {
    pub season: String,
    pub window: ResetWindow,
    pub days: Vec<IgnoredAny>,
}

pub fn current_calendar_season(
    seasons: &[RawCalendarSeason],
    now: DateTime<Utc>,
) -> Option<CalendarSeason> {
    let season = seasons.iter().find(|season| season.window.contains(now))?;

    Some(CalendarSeason {
        season: season.season.clone(),
        window: season.window.into(),
        days: season.days.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::fixtures::at;

    const SEASONS: &str = r#"[{
        "Activation":{"$date":{"$numberLong":"1000"}},
        "Expiry":{"$date":{"$numberLong":"2000"}},
        "Season":"CST_SUMMER","YearIteration":22,"Version":19,
        "Days":[
            {"day":185,"events":[{"type":"CET_CHALLENGE","challenge":"/Lotus/Types/Challenges/Calendar1999/CalendarKillEximusHard"}]},
            {"day":190,"events":[{"type":"CET_REWARD","reward":"/Lotus/StoreItems/Types/BoosterPacks/CalendarArtifactPack"},{"type":"CET_PLOT","dialogueName":"/Lotus/Types/Gameplay/1999Wf/Dialogue/KayaDialogue_rom.dialogue","dialogueConvo":"KayaBirthdayConvo"}]},
            {"day":191,"events":[]},
            {"day":212,"events":[{"type":"CET_UPGRADE","upgrade":"/Lotus/Upgrades/Calendar/MeleeCritChance"},{"type":"CET_NEW_KIND"}]}
        ]}]"#;

    #[test]
    fn running_season_days() {
        let seasons: Vec<RawCalendarSeason> = serde_json::from_str(SEASONS).unwrap();
        let season = current_calendar_season(&seasons, at(1500)).unwrap();

        assert_eq!(season.season, "CST_SUMMER");
        assert_eq!(season.window.expiry.timestamp_millis(), 2000);
        assert_eq!(season.days.len(), 4);
    }

    #[test]
    fn no_running_season() {
        let seasons: Vec<RawCalendarSeason> = serde_json::from_str(SEASONS).unwrap();

        assert!(current_calendar_season(&seasons, at(2000)).is_none());
    }
}
