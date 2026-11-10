use serde::Serialize;
use wf_data::RivenType;
use wf_market::{GoodRoll, StatRef};

use super::Grader;
use super::grading::AttributeGrade;
use super::station::compat_weapon;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct StatMatch {
    pub abbr: String,
    pub name: Option<String>,
    pub matches: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct AlternativeMatch {
    pub mandatory: Vec<StatMatch>,
    pub optional: Vec<StatMatch>,
    pub optional_needed: usize,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct GoodRollView {
    pub alternatives: Vec<AlternativeMatch>,
    pub accepted_bad: Vec<StatMatch>,
    pub matches: bool,
}

impl Grader<'_> {
    fn stat_match(
        &self,
        stat: &StatRef,
        present: &[&str],
        riven_type: Option<&RivenType>,
    ) -> StatMatch {
        StatMatch {
            abbr: stat.abbr.clone(),
            name: match stat.tags.as_slice() {
                [tag] => riven_type.and_then(|found| self.stat_name(found, tag)),
                _ => None,
            },
            matches: stat.tags.iter().any(|tag| present.contains(&tag.as_str())),
        }
    }

    pub(super) fn good_roll_of(&self, weapon_path: Option<&str>) -> Option<&GoodRoll> {
        let table = self.table?;
        let weapon = compat_weapon(table, weapon_path?)?;
        table.good_rolls.get(&weapon.name.to_lowercase())
    }

    pub(super) fn good_roll(
        &self,
        good: &GoodRoll,
        attributes: &[AttributeGrade],
        riven_type: Option<&RivenType>,
    ) -> GoodRollView {
        let tags = |curse: bool| -> Vec<&str> {
            attributes
                .iter()
                .filter(|attribute| attribute.curse == curse)
                .map(|attribute| attribute.tag.as_str())
                .collect()
        };
        let buffs = tags(false);
        let curses = tags(true);
        let alternatives: Vec<AlternativeMatch> = good
            .alternatives
            .iter()
            .map(|alternative| {
                let matched = |stats: &[StatRef]| -> Vec<StatMatch> {
                    stats
                        .iter()
                        .map(|stat| self.stat_match(stat, &buffs, riven_type))
                        .collect()
                };
                let mandatory = matched(&alternative.mandatory);
                let optional = matched(&alternative.optional);
                AlternativeMatch {
                    complete: mandatory.iter().all(|stat| stat.matches)
                        && optional.iter().filter(|stat| stat.matches).count()
                            >= alternative.optional_needed,
                    optional_needed: alternative.optional_needed,
                    mandatory,
                    optional,
                }
            })
            .collect();
        let accepted_bad: Vec<StatMatch> = good
            .accepted_bad
            .iter()
            .map(|stat| self.stat_match(stat, &curses, riven_type))
            .collect();
        GoodRollView {
            matches: alternatives.iter().any(|alternative| alternative.complete)
                && (curses.is_empty() || accepted_bad.iter().any(|stat| stat.matches)),
            alternatives,
            accepted_bad,
        }
    }
}

pub(super) fn desires(good: &GoodRoll, tag: &str, curse: bool) -> bool {
    let has_tag = |stats: &[StatRef]| {
        stats
            .iter()
            .any(|stat| stat.tags.iter().any(|named| named == tag))
    };

    if curse {
        return has_tag(&good.accepted_bad);
    }

    good.alternatives
        .iter()
        .any(|alternative| has_tag(&alternative.mandatory) || has_tag(&alternative.optional))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rivens::support::*;
    use wf_inventory::RivenFingerprint;

    fn desired_tags(weapon: &str, buffs: &[&str], curse: Option<&str>) -> Vec<String> {
        let fixture = fixture();
        let grader = fixture.grader();
        let weapon = table_weapon_named(&fixture.table, weapon);
        let riven_type = grader.riven_type(&weapon.mod_type);
        let fingerprint = RivenFingerprint::parse(&rolled(&weapon.game_ref, buffs, curse)).unwrap();
        let good = grader.good_roll_of(Some(weapon.game_ref.as_str()));
        grader
            .graded(
                &fingerprint,
                riven_type,
                fixture.disposition_of(&weapon.game_ref),
                good,
            )
            .into_iter()
            .filter(|attribute| attribute.desired)
            .map(|attribute| attribute.tag)
            .collect()
    }

    fn roll_view(weapon: &str, buffs: &[&str], curse: Option<&str>) -> Option<GoodRollView> {
        let fixture = fixture();
        let grader = fixture.grader();
        let weapon = table_weapon_named(&fixture.table, weapon);
        let riven_type = grader.riven_type(&weapon.mod_type);
        let fingerprint = RivenFingerprint::parse(&rolled(&weapon.game_ref, buffs, curse)).unwrap();
        let good = grader.good_roll_of(Some(weapon.game_ref.as_str()))?;
        let attributes = grader.graded(
            &fingerprint,
            riven_type,
            fixture.disposition_of(&weapon.game_ref),
            Some(good),
        );
        Some(grader.good_roll(good, &attributes, riven_type))
    }

    #[test]
    fn complete_alternative_with_kept_curse() {
        let view = roll_view(
            "Onos",
            &[
                "WeaponCritDamageMod",
                "WeaponFireIterationsMod",
                "WeaponDamageAmountMod",
            ],
            Some("WeaponArmorPiercingDamageMod"),
        )
        .unwrap();

        assert!(view.matches);
        assert_eq!(view.alternatives.len(), 1);
        let alternative = &view.alternatives[0];
        assert!(alternative.complete);
        assert_eq!(alternative.optional_needed, 2);
        assert_eq!(alternative.mandatory.len(), 1);
        assert_eq!(alternative.mandatory[0].abbr, "CD");
        assert_eq!(
            alternative.mandatory[0].name.as_deref(),
            Some("Critical Damage")
        );
        assert!(alternative.mandatory[0].matches);
        assert_eq!(
            alternative
                .optional
                .iter()
                .filter(|stat| stat.matches)
                .map(|stat| stat.abbr.as_str())
                .collect::<Vec<&str>>(),
            ["MS", "DMG"]
        );
        assert_eq!(
            view.accepted_bad
                .iter()
                .filter(|stat| stat.matches)
                .map(|stat| stat.abbr.as_str())
                .collect::<Vec<&str>>(),
            ["PUNC"]
        );
    }

    #[test]
    fn incomplete_alternatives() {
        let view = roll_view(
            "Onos",
            &["WeaponCritDamageMod", "WeaponFireIterationsMod"],
            None,
        )
        .unwrap();

        assert!(!view.matches);
        assert!(!view.alternatives[0].complete);
        assert!(view.alternatives[0].mandatory[0].matches);
        assert_eq!(
            view.alternatives[0]
                .optional
                .iter()
                .filter(|stat| stat.matches)
                .count(),
            1
        );

        let without_mandatory = roll_view(
            "Onos",
            &[
                "WeaponFireIterationsMod",
                "WeaponDamageAmountMod",
                "WeaponCritChanceMod",
            ],
            None,
        )
        .unwrap();
        assert!(!without_mandatory.alternatives[0].mandatory[0].matches);
        assert!(!without_mandatory.alternatives[0].complete);
        assert!(!without_mandatory.matches);
    }

    #[test]
    fn unaccepted_curse_spoils_roll() {
        let view = roll_view(
            "Onos",
            &[
                "WeaponCritDamageMod",
                "WeaponFireIterationsMod",
                "WeaponDamageAmountMod",
            ],
            Some("WeaponAmmoMaxMod"),
        )
        .unwrap();

        assert!(view.alternatives[0].complete);
        assert!(view.accepted_bad.iter().all(|stat| !stat.matches));
        assert!(!view.matches);
    }

    #[test]
    fn element_matches_any_tag() {
        let element = |tag: &str| {
            roll_view(
                "Ferrox",
                &["WeaponDamageAmountMod", "WeaponFireRateMod", tag],
                None,
            )
            .unwrap()
        };
        for tag in [
            "WeaponFreezeDamageMod",
            "WeaponFireDamageMod",
            "WeaponToxinDamageMod",
            "WeaponElectricityDamageMod",
        ] {
            let view = element(tag);
            assert!(view.matches, "{tag}");
            let named = view.alternatives[0]
                .mandatory
                .iter()
                .find(|stat| stat.abbr == "ELEMENT")
                .unwrap();
            assert!(named.matches, "{tag}");
            assert_eq!(named.name, None);
        }
        let without = roll_view(
            "Ferrox",
            &["WeaponDamageAmountMod", "WeaponFireRateMod"],
            None,
        )
        .unwrap();
        assert!(!without.matches);
        assert!(!without.alternatives[0].complete);
    }

    #[test]
    fn desired_attributes() {
        let buffs = [
            "WeaponCritDamageMod",
            "WeaponFireIterationsMod",
            "WeaponDamageAmountMod",
        ];
        assert_eq!(
            desired_tags("Onos", &buffs, Some("WeaponArmorPiercingDamageMod")),
            [buffs.as_slice(), &["WeaponArmorPiercingDamageMod"]].concat()
        );
        assert_eq!(
            desired_tags("Onos", &buffs, Some("WeaponAmmoMaxMod")),
            buffs
        );
        assert_eq!(
            desired_tags(
                "Ferrox",
                &["WeaponDamageAmountMod", "WeaponToxinDamageMod"],
                None
            ),
            ["WeaponDamageAmountMod", "WeaponToxinDamageMod"]
        );
        assert!(desired_tags("Dex Nikana", &["WeaponMeleeDamageMod"], None).is_empty());
    }

    #[test]
    fn weapon_without_good_rolls() {
        assert!(roll_view("Dex Nikana", &["WeaponMeleeDamageMod"], None).is_none());
    }
}
