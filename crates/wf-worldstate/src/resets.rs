use chrono::{DateTime, Datelike, NaiveTime, TimeDelta, Utc};
use serde::Serialize;

use crate::window::Window;

const GAME_WEEK_ZERO_SECS: i64 = 1_391_990_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ResetWindow {
    pub activation: DateTime<Utc>,
    pub expiry: DateTime<Utc>,
}

impl From<Window> for ResetWindow {
    fn from(window: Window) -> Self {
        Self {
            activation: window.activation.datetime(),
            expiry: window.expiry.datetime(),
        }
    }
}

pub fn daily_reset_window(now: DateTime<Utc>) -> ResetWindow {
    let activation = now.date_naive().and_time(NaiveTime::MIN).and_utc();
    ResetWindow {
        activation,
        expiry: activation + TimeDelta::days(1),
    }
}

pub fn weekly_reset_window(now: DateTime<Utc>) -> ResetWindow {
    let days_since_monday = i64::from(now.weekday().num_days_from_monday());
    let activation = daily_reset_window(now).activation - TimeDelta::days(days_since_monday);
    ResetWindow {
        activation,
        expiry: activation + TimeDelta::weeks(1),
    }
}

pub fn game_week_count(now: DateTime<Utc>) -> i64 {
    let since_week_zero = weekly_reset_window(now).activation.timestamp() - GAME_WEEK_ZERO_SECS;
    TimeDelta::seconds(since_week_zero).num_weeks()
}

pub fn current_descendia_window(descents: &[Window], now: DateTime<Utc>) -> Option<ResetWindow> {
    descents
        .iter()
        .find(|window| window.contains(now))
        .map(|&window| window.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::fixtures::at;

    const MONDAY_2026_09_07_MS: i64 = 1_788_739_200_000;
    const DAY_MS: i64 = 86_400_000;

    #[test]
    fn daily_window_mid_day() {
        let window = daily_reset_window(at(MONDAY_2026_09_07_MS + 3 * DAY_MS + 68_000_000));

        assert_eq!(
            window.activation.timestamp_millis(),
            MONDAY_2026_09_07_MS + 3 * DAY_MS
        );
        assert_eq!(
            window.expiry.timestamp_millis(),
            MONDAY_2026_09_07_MS + 4 * DAY_MS
        );
    }

    #[test]
    fn daily_window_boundary() {
        let midnight = MONDAY_2026_09_07_MS + DAY_MS;

        let before = daily_reset_window(at(midnight - 1000));
        assert_eq!(before.expiry.timestamp_millis(), midnight);

        let at_midnight = daily_reset_window(at(midnight));
        assert_eq!(at_midnight.activation.timestamp_millis(), midnight);
        assert_eq!(at_midnight.expiry.timestamp_millis(), midnight + DAY_MS);
    }

    #[test]
    fn weekly_window_sunday_last_second() {
        let window = weekly_reset_window(at(MONDAY_2026_09_07_MS - 1000));

        assert_eq!(
            window.activation.timestamp_millis(),
            MONDAY_2026_09_07_MS - 7 * DAY_MS
        );
        assert_eq!(window.expiry.timestamp_millis(), MONDAY_2026_09_07_MS);
    }

    #[test]
    fn weekly_window_monday_midnight() {
        let window = weekly_reset_window(at(MONDAY_2026_09_07_MS));

        assert_eq!(window.activation.timestamp_millis(), MONDAY_2026_09_07_MS);
        assert_eq!(
            window.expiry.timestamp_millis(),
            MONDAY_2026_09_07_MS + 7 * DAY_MS
        );
    }

    #[test]
    fn weekly_window_mid_week() {
        let window = weekly_reset_window(at(MONDAY_2026_09_07_MS + 4 * DAY_MS + 1));
        assert_eq!(window.activation.timestamp_millis(), MONDAY_2026_09_07_MS);
    }

    #[test]
    fn clan_vault_weeks() {
        assert_eq!(game_week_count(at(1_788_807_069_000)), 656);
        assert_eq!(game_week_count(at(MONDAY_2026_09_07_MS - 1000)), 655);
        assert_eq!(game_week_count(at(1_794_400_303_000)), 665);
    }
}
