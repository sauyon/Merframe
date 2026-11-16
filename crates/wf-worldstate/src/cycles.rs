use chrono::{DateTime, TimeDelta, Utc};
use serde::Serialize;

use crate::syndicate::SyndicateMission;
use crate::syndicate::expiry_for_tag;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct Timer {
    pub name: &'static str,
    pub state: &'static str,
    pub next_state: &'static str,
    pub starts: DateTime<Utc>,
    pub ends: DateTime<Utc>,
}

struct CycleWindow {
    state: &'static str,
    next_state: &'static str,
    length_ms: i64,
    expiry_ms: i64,
}

impl CycleWindow {
    fn alternating(phases: [(&'static str, i64); 2], in_first: bool, expiry_ms: i64) -> Self {
        let [(state, length_ms), (next_state, _)] = if in_first {
            phases
        } else {
            [phases[1], phases[0]]
        };

        Self {
            state,
            next_state,
            length_ms,
            expiry_ms,
        }
    }

    fn timer(&self, name: &'static str) -> Option<Timer> {
        let ends = DateTime::from_timestamp_millis(self.expiry_ms)?;
        Some(Timer {
            name,
            state: self.state,
            next_state: self.next_state,
            starts: ends - TimeDelta::milliseconds(self.length_ms),
            ends,
        })
    }
}

fn round_div(numerator: i64, denominator: i64) -> i64 {
    let quotient = numerator.div_euclid(denominator);
    let remainder = numerator.rem_euclid(denominator);
    if remainder * 2 >= denominator {
        quotient + 1
    } else {
        quotient
    }
}

const BOUNTY_ROTATION_MS: i64 = 9_000_000;

fn upcoming_bounty_expiry(bounty_expiry_ms: i64, now_ms: i64) -> i64 {
    if bounty_expiry_ms > now_ms {
        return bounty_expiry_ms;
    }
    let rotations_passed = (now_ms - bounty_expiry_ms).div_euclid(BOUNTY_ROTATION_MS) + 1;
    bounty_expiry_ms + rotations_passed * BOUNTY_ROTATION_MS
}

fn floor_minutes_ms(epoch_ms: i64) -> i64 {
    epoch_ms.div_euclid(60_000) * 60_000
}

fn round_minutes_ms(epoch_ms: i64) -> i64 {
    round_div(epoch_ms, 60_000) * 60_000
}

fn earth_window(now_ms: i64) -> CycleWindow {
    const EARTH_CYCLE_MS: i64 = 28_800_000;
    const EARTH_DAY_MS: i64 = 14_400_000;
    const EARTH_NIGHT_MS: i64 = EARTH_CYCLE_MS - EARTH_DAY_MS;
    let cycle_ms = now_ms.rem_euclid(EARTH_CYCLE_MS);
    let is_day = cycle_ms < EARTH_DAY_MS;
    let remaining_ms = if is_day {
        EARTH_DAY_MS - cycle_ms
    } else {
        EARTH_CYCLE_MS - cycle_ms
    };
    CycleWindow::alternating(
        [("day", EARTH_DAY_MS), ("night", EARTH_NIGHT_MS)],
        is_day,
        now_ms + remaining_ms,
    )
}

fn cetus_window(bounty_expiry_ms: i64, now_ms: i64) -> CycleWindow {
    const CETUS_NIGHT_SECONDS: i64 = 3000;
    const CETUS_DAY_MS: i64 = 6_000_000;
    const CETUS_NIGHT_MS: i64 = CETUS_NIGHT_SECONDS * 1000;
    let bounty_expiry_floored = upcoming_bounty_expiry(floor_minutes_ms(bounty_expiry_ms), now_ms);
    let millis_left = bounty_expiry_floored - now_ms;
    let seconds_to_night_end = round_div(millis_left, 1000);
    let is_day = seconds_to_night_end > CETUS_NIGHT_SECONDS;
    let seconds_remaining = if is_day {
        seconds_to_night_end - CETUS_NIGHT_SECONDS
    } else {
        seconds_to_night_end
    };
    let expiry_ms = round_minutes_ms(now_ms + seconds_remaining * 1000);
    CycleWindow::alternating(
        [("day", CETUS_DAY_MS), ("night", CETUS_NIGHT_MS)],
        is_day,
        expiry_ms,
    )
}

fn cambion_window(cetus: &CycleWindow) -> CycleWindow {
    CycleWindow::alternating(
        [("fass", cetus.length_ms), ("vome", cetus.length_ms)],
        cetus.state == "day",
        cetus.expiry_ms,
    )
}

fn vallis_window(now_ms: i64) -> CycleWindow {
    const VALLIS_ANCHOR_MS: i64 = 1_770_234_408_000;
    const VALLIS_LOOP_MS: i64 = 1_600_000;
    const VALLIS_WARM_MS: i64 = 400_000;
    const VALLIS_COLD_MS: i64 = VALLIS_LOOP_MS - VALLIS_WARM_MS;
    let since_last = (now_ms - VALLIS_ANCHOR_MS).rem_euclid(VALLIS_LOOP_MS);
    let to_next_full = VALLIS_LOOP_MS - since_last;
    let is_warm = to_next_full > VALLIS_COLD_MS;
    let to_next_minor = if to_next_full < VALLIS_COLD_MS {
        to_next_full
    } else {
        to_next_full - VALLIS_COLD_MS
    };
    CycleWindow::alternating(
        [("warm", VALLIS_WARM_MS), ("cold", VALLIS_COLD_MS)],
        is_warm,
        now_ms + to_next_minor,
    )
}

fn duviri_window(now_ms: i64) -> CycleWindow {
    const DUVIRI_CYCLE_SECONDS: i64 = 36_000;
    const DUVIRI_STATE_SECONDS: i64 = 7_200;
    const DUVIRI_STATES: [&str; 5] = ["sorrow", "fear", "joy", "anger", "envy"];
    const DUVIRI_EPOCH_OFFSET_SECONDS: i64 = 52;
    let now_secs = now_ms.div_euclid(1000);
    let cycle_delta = (now_secs - DUVIRI_EPOCH_OFFSET_SECONDS).rem_euclid(DUVIRI_CYCLE_SECONDS);
    let state_index = usize::try_from(cycle_delta / DUVIRI_STATE_SECONDS).unwrap_or(0);
    let state_delta = cycle_delta.rem_euclid(DUVIRI_STATE_SECONDS);
    let until_next = DUVIRI_STATE_SECONDS - state_delta;
    let expiry_ms = floor_minutes_ms(now_ms + until_next * 1000);
    CycleWindow {
        state: DUVIRI_STATES[state_index],
        next_state: DUVIRI_STATES[(state_index + 1) % DUVIRI_STATES.len()],
        length_ms: DUVIRI_STATE_SECONDS * 1000,
        expiry_ms,
    }
}

fn zariman_window(bounty_expiry_ms: i64, now_ms: i64) -> CycleWindow {
    const ZARIMAN_CORPUS_EPOCH_MS: i64 = 1_655_182_800_000;
    const ZARIMAN_FULL_CYCLE_MS: i64 = 18_000_000;
    const ZARIMAN_STATE_MS: i64 = 9_000_000;
    const ZARIMAN_BOUNTY_SKEW_MS: i64 = 5_000;
    let bounty_expiry_skewed =
        upcoming_bounty_expiry(bounty_expiry_ms - ZARIMAN_BOUNTY_SKEW_MS, now_ms);
    let millis_left = bounty_expiry_skewed - now_ms;
    let cycle_time_elapsed =
        (bounty_expiry_skewed - ZARIMAN_CORPUS_EPOCH_MS).rem_euclid(ZARIMAN_FULL_CYCLE_MS);
    let cycle_time_left = ZARIMAN_FULL_CYCLE_MS - cycle_time_elapsed;
    let is_corpus = cycle_time_left > ZARIMAN_STATE_MS;
    CycleWindow::alternating(
        [("corpus", ZARIMAN_STATE_MS), ("grineer", ZARIMAN_STATE_MS)],
        is_corpus,
        round_minutes_ms(now_ms + millis_left),
    )
}

pub fn timers(syndicate_missions: &[SyndicateMission], now: DateTime<Utc>) -> Vec<Timer> {
    let now_ms = now.timestamp_millis();
    let mut timers = Vec::with_capacity(6);
    timers.extend(earth_window(now_ms).timer("Earth"));

    if let Some(bounty_expiry_ms) = expiry_for_tag(syndicate_missions, "CetusSyndicate") {
        let cetus = cetus_window(bounty_expiry_ms, now_ms);
        timers.extend(cetus.timer("Cetus"));
        timers.extend(cambion_window(&cetus).timer("Cambion Drift"));
    }

    timers.extend(vallis_window(now_ms).timer("Orb Vallis"));
    timers.extend(duviri_window(now_ms).timer("Duviri"));

    if let Some(bounty_expiry_ms) = expiry_for_tag(syndicate_missions, "ZarimanSyndicate") {
        timers.extend(zariman_window(bounty_expiry_ms, now_ms).timer("Zariman"));
    }

    timers
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::*;
    use crate::syndicate::fixtures::syndicate_mission;
    use crate::window::fixtures::at;

    fn syndicate(tag: &str, expiry_ms: i64) -> SyndicateMission {
        syndicate_mission(tag, 0, expiry_ms, &[])
    }

    fn timer(missions: &[SyndicateMission], now: DateTime<Utc>, name: &str) -> Timer {
        timers(missions, now)
            .into_iter()
            .find(|timer| timer.name == name)
            .unwrap()
    }

    #[test]
    fn cetus_from_bounty_expiry() {
        let now = at(1_788_807_069_000);
        let missions = vec![syndicate("CetusSyndicate", 1_788_816_031_915)];
        let all = timers(&missions, now);
        let cetus = all.iter().find(|timer| timer.name == "Cetus").unwrap();
        assert!(cetus.state == "day" || cetus.state == "night");
        assert!(cetus.ends > now);
        assert!(all.iter().any(|timer| timer.name == "Cambion Drift"));
    }

    #[test]
    fn cetus_past_bounty_expiry() {
        let night_end_ms = 1_788_816_000_000;
        let missions = vec![syndicate("CetusSyndicate", night_end_ms)];
        let find = |now_ms: i64, name: &str| timer(&missions, at(now_ms), name);

        let night = find(night_end_ms - 60_000, "Cetus");
        assert_eq!(night.state, "night");
        assert_eq!(night.ends.timestamp_millis(), night_end_ms);

        let day = find(night_end_ms + 60_000, "Cetus");
        assert_eq!(day.state, "day");
        assert_eq!(day.ends.timestamp_millis(), night_end_ms + 6_000_000);
        assert_eq!(find(night_end_ms + 60_000, "Cambion Drift").state, "fass");

        let next_night = find(night_end_ms + 6_060_000, "Cetus");
        assert_eq!(next_night.state, "night");
        assert_eq!(
            next_night.ends.timestamp_millis(),
            night_end_ms + BOUNTY_ROTATION_MS
        );

        let two_rotations_on = find(night_end_ms + 2 * BOUNTY_ROTATION_MS + 60_000, "Cetus");
        assert_eq!(two_rotations_on.state, "day");
    }

    #[test]
    fn zariman_past_bounty_expiry() {
        let expiry_ms = 1_788_816_005_000;
        let missions = vec![syndicate("ZarimanSyndicate", expiry_ms)];
        let find = |now_ms: i64| timer(&missions, at(now_ms), "Zariman");

        let before = find(expiry_ms - 65_000);
        let after = find(expiry_ms + 55_000);
        assert_ne!(before.state, after.state);
        assert_eq!(after.state, before.next_state);
        assert_eq!(
            after.ends.timestamp_millis() - before.ends.timestamp_millis(),
            BOUNTY_ROTATION_MS
        );
    }

    #[test]
    fn vallis_and_duviri_without_data() {
        let now = at(1_788_807_069_000);
        let all = timers(&[], now);
        assert!(all.iter().any(|timer| timer.name == "Orb Vallis"));
        assert!(all.iter().any(|timer| timer.name == "Duviri"));
        assert!(!all.iter().any(|timer| timer.name == "Cetus"));
        assert!(!all.iter().any(|timer| timer.name == "Zariman"));
    }

    #[test]
    fn earth_window_bounded() {
        for now_ms in [
            1_788_807_069_000_i64,
            1_788_800_000_000,
            1_788_818_000_000,
            1_788_825_599_999,
            0,
        ] {
            let now = at(now_ms);
            let earth = timer(&[], now, "Earth");
            assert!(earth.state == "day" || earth.state == "night");
            assert!(earth.ends > now);
            assert!(earth.ends.timestamp_millis() - now_ms <= 14_400_000);
        }
    }

    #[test]
    fn earth_epoch_formula() {
        let day = timer(&[], at(1_788_807_069_000), "Earth");
        assert_eq!(day.state, "day");
        assert_eq!(day.ends.timestamp_millis(), 1_788_811_200_000);

        let night = timer(&[], at(1_788_818_000_000), "Earth");
        assert_eq!(night.state, "night");
        assert_eq!(night.ends.timestamp_millis(), 1_788_825_600_000);
    }

    #[test]
    fn timer_spans_now() {
        let now = at(1_788_807_069_000);
        let missions = vec![
            syndicate("CetusSyndicate", 1_788_816_031_915),
            syndicate("ZarimanSyndicate", 1_788_816_031_915),
        ];
        let all = timers(&missions, now);
        assert_eq!(all.len(), 6);
        for timer in &all {
            assert!(timer.starts <= now, "{} starts in the future", timer.name);
            assert!(timer.ends > now, "{} already ended", timer.name);
            assert_ne!(timer.state, timer.next_state, "{}", timer.name);
        }
        let earth = all.iter().find(|timer| timer.name == "Earth").unwrap();
        assert_eq!((earth.ends - earth.starts).num_milliseconds(), 14_400_000);
        let duviri = all.iter().find(|timer| timer.name == "Duviri").unwrap();
        assert_eq!((duviri.ends - duviri.starts).num_seconds(), 7200);
        let cambion = all
            .iter()
            .find(|timer| timer.name == "Cambion Drift")
            .unwrap();
        let cetus = all.iter().find(|timer| timer.name == "Cetus").unwrap();
        assert_eq!(cambion.ends, cetus.ends);
        assert_eq!(cambion.starts, cetus.starts);
    }

    #[test]
    fn earth_first() {
        let now = at(1_788_807_069_000);
        let missions = vec![syndicate("CetusSyndicate", 1_788_816_031_915)];
        assert_eq!(timers(&missions, now).first().unwrap().name, "Earth");
    }

    #[test]
    fn vallis_cold_after_400_seconds() {
        let anchor_ms = 1_770_234_408_000;
        let vallis = |now_ms: i64| timer(&[], at(now_ms), "Orb Vallis");
        assert_eq!(vallis(anchor_ms).state, "warm");
        assert_eq!(vallis(anchor_ms + 399_999).state, "warm");
        assert_eq!(vallis(anchor_ms + 400_000).state, "cold");
        assert_eq!(vallis(anchor_ms + 1_599_999).state, "cold");
        assert_eq!(vallis(anchor_ms + 1_600_000).state, "warm");
        assert_eq!(
            vallis(anchor_ms).ends.timestamp_millis(),
            anchor_ms + 400_000
        );
    }

    #[test]
    fn duviri_sorrow_offset() {
        let duviri = |now_secs: i64| timer(&[], at(now_secs * 1000), "Duviri");
        let cycle_start = 1_788_804_000 + 52;
        assert_eq!(duviri(cycle_start - 1).state, "envy");
        assert_eq!(duviri(cycle_start).state, "sorrow");
        assert_eq!(duviri(cycle_start + 7_199).state, "sorrow");
        assert_eq!(duviri(cycle_start + 7_200).state, "fear");
        assert_eq!(duviri(cycle_start + 4 * 7_200).state, "envy");
    }

    #[test]
    fn zariman_corpus_at_epoch() {
        let epoch_ms = 1_655_182_800_000;
        let zariman = |bounty_expiry_ms: i64| {
            let missions = [syndicate("ZarimanSyndicate", bounty_expiry_ms)];
            timer(&missions, at(bounty_expiry_ms - 60_000), "Zariman")
        };
        assert_eq!(zariman(epoch_ms + 5_000).state, "corpus");
        assert_eq!(zariman(epoch_ms + 4_999).state, "grineer");
        assert_eq!(zariman(epoch_ms + 5_000 + 8_999_999).state, "corpus");
        assert_eq!(zariman(epoch_ms + 5_000 + 9_000_000).state, "grineer");
        assert_eq!(zariman(epoch_ms + 5_000 + 18_000_000).state, "corpus");
    }

    #[test]
    fn zariman_from_bounty_expiry() {
        let now = at(1_788_807_069_000);
        let missions = vec![syndicate("ZarimanSyndicate", 1_788_816_031_915)];
        let zariman = timer(&missions, now, "Zariman");
        assert!(zariman.state == "corpus" || zariman.state == "grineer");
    }
}
