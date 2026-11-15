use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::Serialize;
use wf_mongo::ObjectId;

use crate::window::Window;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SeasonChallenge {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    #[serde(flatten)]
    pub window: Window,
    pub challenge: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SeasonInfo {
    #[serde(flatten)]
    pub window: Window,
    pub affiliation_tag: String,
    pub season: u32,
    pub phase: u32,
    #[serde(default)]
    pub active_challenges: Vec<SeasonChallenge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ChallengeKind {
    Daily,
    Weekly,
    EliteWeekly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NightwaveActText {
    pub title: String,
    pub description: String,
    pub required: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct NightwaveActDefinition {
    pub name: String,
    pub description: Option<String>,
    pub kind: ChallengeKind,
    pub required: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct NightwaveChallenge {
    pub id: String,
    pub tag: String,
    #[serde(flatten)]
    pub act: NightwaveActDefinition,
    pub activation: DateTime<Utc>,
    pub expiry: DateTime<Utc>,
}

impl NightwaveChallenge {
    pub fn counter_name(&self) -> &str {
        last_path_segment(&self.tag)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct NightwaveSeason {
    pub season: u32,
    pub phase: u32,
    pub affiliation_tag: String,
    pub name: String,
    pub activation: DateTime<Utc>,
    pub expiry: DateTime<Utc>,
    pub challenges: Vec<NightwaveChallenge>,
}

pub(crate) fn split_words(camel: &str) -> String {
    let mut words = String::with_capacity(camel.len() + 8);
    let mut previous: Option<char> = None;
    for ch in camel.chars() {
        let boundary = match previous {
            Some(before) => {
                (ch.is_ascii_uppercase() && (before.is_lowercase() || before.is_ascii_digit()))
                    || (ch.is_ascii_digit() && before.is_alphabetic())
            }
            None => false,
        };
        if boundary {
            words.push(' ');
        }
        words.push(ch);
        previous = Some(ch);
    }
    words
}

pub(crate) fn last_path_segment(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

fn challenge_kind(challenge: &str) -> ChallengeKind {
    if challenge.starts_with("SeasonDaily") {
        ChallengeKind::Daily
    } else if challenge.starts_with("SeasonWeeklyHard") {
        ChallengeKind::EliteWeekly
    } else {
        ChallengeKind::Weekly
    }
}

fn derived_challenge_name(challenge: &str) -> String {
    let stem = challenge.strip_prefix("Season").unwrap_or(challenge);
    let stem = ["WeeklyHard", "Weekly", "Daily"]
        .into_iter()
        .find_map(|segment| stem.strip_prefix(segment))
        .unwrap_or(stem);
    let stem = stem.strip_prefix("Permanent").unwrap_or(stem);
    let stem = stem.trim_end_matches(|letter: char| letter.is_ascii_digit());
    let name = split_words(stem);
    if name.is_empty() {
        challenge.to_owned()
    } else {
        name
    }
}

pub fn nightwave_act_definition(
    challenge: &str,
    text: Option<NightwaveActText>,
) -> NightwaveActDefinition {
    let kind = challenge_kind(challenge);
    match text {
        Some(text) => NightwaveActDefinition {
            name: text.title,
            description: Some(text.description),
            kind,
            required: text.required,
        },
        None => NightwaveActDefinition {
            name: derived_challenge_name(challenge),
            description: None,
            kind,
            required: 1,
        },
    }
}

fn season_name(affiliation_tag: &str, season: u32) -> String {
    let stem = affiliation_tag
        .strip_prefix("RadioLegion")
        .unwrap_or(affiliation_tag);
    let stem = stem.strip_suffix("Syndicate").unwrap_or(stem);
    if stem.is_empty() || stem.chars().all(|c| c.is_ascii_digit()) {
        return format!("Series {season}");
    }
    split_words(stem)
}

pub fn current_season(
    raw: &SeasonInfo,
    now: DateTime<Utc>,
    act_text: impl Fn(&str) -> Option<NightwaveActText>,
) -> NightwaveSeason {
    let mut challenges: Vec<NightwaveChallenge> = raw
        .active_challenges
        .iter()
        .filter(|challenge| challenge.window.contains(now))
        .map(|challenge| {
            let name = last_path_segment(&challenge.challenge);
            NightwaveChallenge {
                id: challenge.id.as_str().to_owned(),
                tag: challenge.challenge.clone(),
                act: nightwave_act_definition(name, act_text(name)),
                activation: challenge.window.activation.datetime(),
                expiry: challenge.window.expiry.datetime(),
            }
        })
        .collect();
    challenges.sort_by(|left, right| {
        left.act
            .kind
            .cmp(&right.act.kind)
            .then_with(|| left.expiry.cmp(&right.expiry))
            .then_with(|| left.act.name.cmp(&right.act.name))
    });
    NightwaveSeason {
        season: raw.season,
        phase: raw.phase,
        affiliation_tag: raw.affiliation_tag.clone(),
        name: season_name(&raw.affiliation_tag, raw.season),
        activation: raw.window.activation.datetime(),
        expiry: raw.window.expiry.datetime(),
        challenges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::fixtures::{at, window};

    fn challenge(tag: &str, activation_ms: i64, expiry_ms: i64) -> SeasonChallenge {
        SeasonChallenge {
            id: ObjectId("001900040000000000000055".to_owned()),
            window: window(activation_ms, expiry_ms),
            challenge: tag.to_owned(),
        }
    }

    fn no_mercy(challenge: &str) -> Option<NightwaveActText> {
        (challenge == "SeasonDailyMercyKill").then(|| NightwaveActText {
            title: "No Mercy".to_owned(),
            description: "Mercy Kill an Enemy".to_owned(),
            required: 1,
        })
    }

    fn season(challenges: Vec<SeasonChallenge>) -> SeasonInfo {
        SeasonInfo {
            window: window(0, 10_000),
            affiliation_tag: "RadioLegionIntermission16Syndicate".to_owned(),
            season: 18,
            phase: 0,
            active_challenges: challenges,
        }
    }

    #[test]
    fn running_challenges_only() {
        let raw = season(vec![
            challenge(
                "/Lotus/Types/Challenges/Seasons/Daily/SeasonDailyMercyKill",
                0,
                2000,
            ),
            challenge(
                "/Lotus/Types/Challenges/Seasons/Weekly/SeasonWeeklyNightAndDay",
                5000,
                6000,
            ),
        ]);
        let current = current_season(&raw, at(1000), no_mercy);
        assert_eq!(current.challenges.len(), 1);
        assert_eq!(current.challenges[0].id, "001900040000000000000055");
        assert_eq!(current.challenges[0].act.name, "No Mercy");
        assert_eq!(current.challenges[0].act.kind, ChallengeKind::Daily);
        assert_eq!(current.challenges[0].counter_name(), "SeasonDailyMercyKill");
        assert_eq!(current.challenges[0].act.required, 1);
        assert_eq!(
            current.challenges[0].act.description.as_deref(),
            Some("Mercy Kill an Enemy")
        );
    }

    #[test]
    fn challenge_order() {
        let raw = season(vec![
            challenge(
                "/Lotus/Types/Challenges/Seasons/WeeklyHard/SeasonWeeklyHardFallenAngel",
                0,
                2000,
            ),
            challenge(
                "/Lotus/Types/Challenges/Seasons/Weekly/SeasonWeeklyCompleteInvasionMissions",
                0,
                2000,
            ),
            challenge(
                "/Lotus/Types/Challenges/Seasons/Daily/SeasonDailyMercyKill",
                0,
                2000,
            ),
        ]);
        let kinds: Vec<_> = current_season(&raw, at(1000), no_mercy)
            .challenges
            .iter()
            .map(|challenge| challenge.act.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                ChallengeKind::Daily,
                ChallengeKind::Weekly,
                ChallengeKind::EliteWeekly
            ]
        );
    }

    #[test]
    fn listed_act() {
        let text = NightwaveActText {
            title: "Trampoline".to_owned(),
            description: "Bullet Jump 150 times".to_owned(),
            required: 150,
        };
        let definition = nightwave_act_definition("SeasonDailyBulletJump", Some(text));

        assert_eq!(definition.name, "Trampoline");
        assert_eq!(
            definition.description.as_deref(),
            Some("Bullet Jump 150 times")
        );
        assert_eq!(definition.kind, ChallengeKind::Daily);
        assert_eq!(definition.required, 150);
    }

    #[test]
    fn unlisted_act() {
        let definition = nightwave_act_definition("SeasonWeeklyHardSomethingNew", None);

        assert_eq!(definition.name, "Something New");
        assert_eq!(definition.description, None);
        assert_eq!(definition.kind, ChallengeKind::EliteWeekly);
        assert_eq!(definition.required, 1);
    }

    #[test]
    fn derived_challenge_names() {
        assert_eq!(
            derived_challenge_name("SeasonDailyKillEnemiesWithPoison"),
            "Kill Enemies With Poison"
        );
        assert_eq!(
            derived_challenge_name("SeasonWeeklyPermanentCompleteMissions5"),
            "Complete Missions"
        );
        assert_eq!(
            derived_challenge_name("SeasonWeeklyHardBattleHardened"),
            "Battle Hardened"
        );
    }

    #[test]
    fn wordless_name_kept() {
        assert_eq!(derived_challenge_name("SeasonDaily"), "SeasonDaily");
    }

    #[test]
    fn season_names() {
        assert_eq!(
            season_name("RadioLegionIntermission16Syndicate", 18),
            "Intermission 16"
        );
        assert_eq!(season_name("RadioLegionSyndicate", 1), "Series 1");
        assert_eq!(season_name("RadioLegion3Syndicate", 12), "Series 12");
    }
}
