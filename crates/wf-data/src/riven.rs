use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::item::Item;
use crate::markup::without_markup;

const ROLL_MIN: f64 = 0.9;
const ROLL_MAX: f64 = 1.1;
pub const MAX_RANK: u32 = 8;
/// DE dropped this from their public export data for some reason.
const REVERSED_STAT_TAGS: [&str; 1] = ["WeaponAmmoEfficiency"];

const SPLICE_RECIPES: [(&str, [&str; 2]); 26] = [
    (
        "WeaponWeakpointDamage",
        ["WeaponDamageAmountMod", "WeaponZoomFovMod"],
    ),
    (
        "WeaponWeakpointDamage",
        ["WeaponDamageAmountMod", "WeaponFireIterationsMod"],
    ),
    (
        "WeaponWeakpointCriticalChance",
        ["WeaponCritChanceMod", "WeaponZoomFovMod"],
    ),
    (
        "WeaponWeakpointCriticalChance",
        ["WeaponCritChanceMod", "WeaponFireIterationsMod"],
    ),
    (
        "WeaponAmmoEfficiency",
        ["WeaponClipMaxMod", "WeaponReloadSpeedMod"],
    ),
    (
        "WeaponAmmoEfficiency",
        ["WeaponRecoilReductionMod", "WeaponAmmoMaxMod"],
    ),
    (
        "WeaponMagazineReloadHolstered",
        ["WeaponAmmoMaxMod", "WeaponReloadSpeedMod"],
    ),
    (
        "WeaponMagazineReloadHolstered",
        ["WeaponAmmoMaxMod", "WeaponClipMaxMod"],
    ),
    (
        "WeaponStatusDamage",
        ["WeaponDamageAmountMod", "WeaponStunChanceMod"],
    ),
    (
        "WeaponStatusDamage",
        ["WeaponMeleeDamageMod", "WeaponStunChanceMod"],
    ),
    (
        "WeaponGasDamageMod",
        ["WeaponToxinDamageMod", "WeaponFireDamageMod"],
    ),
    (
        "WeaponCorrosiveDamageMod",
        ["WeaponToxinDamageMod", "WeaponElectricityDamageMod"],
    ),
    (
        "WeaponViralDamageMod",
        ["WeaponToxinDamageMod", "WeaponFreezeDamageMod"],
    ),
    (
        "WeaponRadiationDamageMod",
        ["WeaponFireDamageMod", "WeaponElectricityDamageMod"],
    ),
    (
        "WeaponBlastDamageMod",
        ["WeaponFireDamageMod", "WeaponFreezeDamageMod"],
    ),
    (
        "WeaponMagneticDamageMod",
        ["WeaponElectricityDamageMod", "WeaponFreezeDamageMod"],
    ),
    (
        "WeaponFactionDamageOrokin",
        ["WeaponFactionDamageGrineer", "WeaponFactionDamageCorpus"],
    ),
    (
        "WeaponFactionDamageTechrot",
        ["WeaponFactionDamageCorpus", "WeaponFactionDamageInfested"],
    ),
    (
        "WeaponFactionDamageScaldra",
        ["WeaponFactionDamageInfested", "WeaponFactionDamageGrineer"],
    ),
    (
        "WeaponFactionDamageOrokin",
        [
            "WeaponMeleeFactionDamageGrineer",
            "WeaponMeleeFactionDamageCorpus",
        ],
    ),
    (
        "WeaponFactionDamageTechrot",
        [
            "WeaponMeleeFactionDamageCorpus",
            "WeaponMeleeFactionDamageInfested",
        ],
    ),
    (
        "WeaponFactionDamageScaldra",
        [
            "WeaponMeleeFactionDamageInfested",
            "WeaponMeleeFactionDamageGrineer",
        ],
    ),
    (
        "WeaponMeleeHeavyAttackDamageMod",
        [
            "WeaponMeleeComboEfficiencyMod",
            "WeaponMeleeComboBonusOnHitMod",
        ],
    ),
    (
        "WeaponMeleeHeavyAttackChargeMod",
        ["WeaponMeleeComboEfficiencyMod", "ComboDurationMod"],
    ),
    (
        "WeaponMeleeParryAngleMod",
        ["WeaponFireRateMod", "WeaponMeleeRangeIncMod"],
    ),
    (
        "WeaponMeleeSlamDamageMod",
        ["WeaponMeleeDamageMod", "WeaponFireRateMod"],
    ),
];

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpgradeValue {
    pub value: f64,
    #[serde(rename = "locTag")]
    pub loc_tag: Option<String>,
    #[serde(rename = "reverseValueSymbol", default)]
    pub reverse_value_symbol: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpgradeEntry {
    pub tag: String,
    #[serde(rename = "prefixTag")]
    pub prefix_tag: String,
    #[serde(rename = "suffixTag")]
    pub suffix_tag: String,
    #[serde(rename = "upgradeValues")]
    pub upgrade_values: Vec<UpgradeValue>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraitMultipliers {
    pub good: f64,
    pub bad: f64,
}

pub fn trait_multipliers(buffs: usize, curses: usize) -> Option<TraitMultipliers> {
    let (good, bad) = match (buffs, curses) {
        (2, 0) => (0.99, 0.0),
        (2, 1) => (1.2375, -0.495),
        (3, 0) => (0.75, 0.0),
        (3, 1) => (0.9375, -0.75),
        _ => return None,
    };
    Some(TraitMultipliers { good, bad })
}

#[derive(Debug, Clone, PartialEq)]
pub struct RivenStat {
    pub tag: String,
    pub prefix: String,
    pub suffix: String,
    pub base_value: f64,
    pub localization: String,
    pub reversed: bool,
    pub spliced: bool,
}

impl RivenStat {
    pub fn percent(&self) -> bool {
        self.localization.contains("|val|%") || self.localization.contains("|STAT1|%")
    }

    pub fn multiplier_display(&self) -> bool {
        self.localization.contains("Damage to")
    }

    pub fn unit(&self) -> Option<&'static str> {
        if self.multiplier_display() {
            Some("multiply")
        } else if self.percent() {
            Some("percent")
        } else {
            None
        }
    }

    pub fn name(&self) -> String {
        without_markup(&self.localization)
            .replace("|val|%", "")
            .replace("|val|", "")
            .trim()
            .to_owned()
    }

    pub fn base_at_rank_nine(&self, disposition: f64, multiplier: f64) -> f64 {
        let mut base = 90.0 * self.base_value * disposition * multiplier;
        if self.reversed {
            base = -base;
        }
        if self.percent() {
            base *= 100.0;
        }
        base
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RivenType {
    pub unique_name: String,
    pub name: String,
    pub stats: HashMap<String, RivenStat>,
}

impl RivenType {
    pub fn splice_results(&self, held: &[&str]) -> Vec<&RivenStat> {
        let mut found: Vec<&RivenStat> = Vec::new();
        for (result, ingredients) in SPLICE_RECIPES {
            let complete = ingredients.iter().all(|tag| held.contains(tag));
            if !complete || found.iter().any(|stat| stat.tag == result) {
                continue;
            }

            found.extend(self.stats.get(result));
        }

        found
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RivenData {
    types: HashMap<String, RivenType>,
}

impl RivenData {
    pub fn from_items(items: &[Item]) -> Self {
        let types = items
            .iter()
            .filter_map(|item| {
                let entries = item.upgrade_entries.as_deref()?;
                let stats = entries
                    .iter()
                    .filter_map(|entry| {
                        let first = entry.upgrade_values.first()?;
                        Some((
                            entry.tag.clone(),
                            RivenStat {
                                tag: entry.tag.clone(),
                                prefix: entry.prefix_tag.clone(),
                                suffix: entry.suffix_tag.clone(),
                                base_value: first.value,
                                localization: first.loc_tag.clone().unwrap_or_default(),
                                reversed: first.reverse_value_symbol
                                    || REVERSED_STAT_TAGS.contains(&entry.tag.as_str()),
                                spliced: SPLICE_RECIPES
                                    .iter()
                                    .any(|(result, _)| *result == entry.tag),
                            },
                        ))
                    })
                    .collect();
                Some((
                    item.unique_name.clone(),
                    RivenType {
                        unique_name: item.unique_name.clone(),
                        name: item.name.clone(),
                        stats,
                    },
                ))
            })
            .collect();
        Self { types }
    }

    pub fn riven_type(&self, unique_name: &str) -> Option<&RivenType> {
        self.types.get(unique_name)
    }

    pub fn types(&self) -> impl Iterator<Item = &RivenType> {
        self.types.values()
    }

    pub fn stat(&self, riven_type: &str, tag: &str) -> Option<&RivenStat> {
        self.types.get(riven_type)?.stats.get(tag)
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "roll values are 30 bit integers"
)]
pub fn roll_multiplier(value: i64) -> f64 {
    (ROLL_MIN + value as f64 / 53_687_091.0 / 100.0).clamp(ROLL_MIN, ROLL_MAX)
}

pub fn roll_share(multiplier: f64, curse: bool) -> f64 {
    let share = (multiplier - ROLL_MIN) * 5.0;
    if curse { 1.0 - share } else { share }
}

pub fn rank_multiplier(rank: u32) -> f64 {
    f64::from(rank + 1) / 9.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: &str = include_str!("../tests/fixtures/riven_items.json");
    const RIFLE: &str = "/Lotus/Upgrades/Mods/Randomized/LotusRifleRandomModRare";
    const MELEE: &str = "/Lotus/Upgrades/Mods/Randomized/PlayerMeleeWeaponRandomModRare";

    fn data() -> RivenData {
        let items: Vec<Item> = serde_json::from_str(ITEMS).unwrap();
        RivenData::from_items(&items)
    }

    #[test]
    fn riven_types_from_items() {
        let data = data();
        assert_eq!(data.len(), 7);
        assert!(!data.is_empty());
        let rifle = data.riven_type(RIFLE).unwrap();
        assert_eq!(rifle.name, "Rifle Riven Mod");
        assert_eq!(rifle.stats.len(), 38);
        assert_eq!(data.types().count(), 7);
        assert!(data.riven_type("/Lotus/Nope").is_none());
    }

    #[test]
    fn stat_fields() {
        let data = data();
        let crit = data.stat(RIFLE, "WeaponCritChanceMod").unwrap();
        assert_eq!(crit.prefix, "crita");
        assert_eq!(crit.suffix, "cron");
        assert!((crit.base_value - 0.016_666).abs() < 1e-6);
        assert_eq!(crit.localization, "|val|% Critical Chance");
        assert!(crit.percent());
        assert!(!crit.multiplier_display());
        assert!(!crit.reversed);
        assert!(!crit.spliced);

        let corpus = data.stat(RIFLE, "WeaponFactionDamageCorpus").unwrap();
        assert!(!corpus.percent());
        assert!(corpus.multiplier_display());

        let combo = data.stat(MELEE, "WeaponMeleeComboPointsOnHitMod").unwrap();
        assert_eq!(combo.prefix, "");
        assert_eq!(combo.suffix, "");
        assert!(combo.base_value < 0.0);
    }

    #[test]
    fn spliced_stats() {
        let data = data();
        let scaldra = data.stat(RIFLE, "WeaponFactionDamageScaldra").unwrap();
        assert!(scaldra.spliced);
        assert_eq!(scaldra.prefix, "exsi");
        assert_eq!(scaldra.suffix, "llo");
        assert_eq!(scaldra.name(), "Damage to Scaldra");
        assert_eq!(scaldra.unit(), Some("multiply"));

        let gas = data.stat(RIFLE, "WeaponGasDamageMod").unwrap();
        assert_eq!(gas.localization, "|val|% <DT_GAS_COLOR>Gas");
        assert_eq!(gas.name(), "Gas");
        assert_eq!(gas.unit(), Some("percent"));

        let parry = data.stat(MELEE, "WeaponMeleeParryAngleMod").unwrap();
        assert!(parry.spliced);
        assert_eq!(parry.name(), "Parry Angle");
        assert_eq!(parry.unit(), None);
        assert_eq!(
            data.stat(MELEE, "WeaponMeleeSlamDamageMod")
                .map(|stat| (stat.prefix.as_str(), stat.suffix.as_str())),
            Some(("laci", "nus"))
        );
        assert_eq!(
            (parry.prefix.as_str(), parry.suffix.as_str()),
            ("laci", "nus")
        );

        let weakpoint = data.stat(RIFLE, "WeaponWeakpointDamage").unwrap();
        assert_eq!(weakpoint.name(), "Weak Point Damage");
        let spliced = |riven_type: &str| {
            data.riven_type(riven_type)
                .unwrap()
                .stats
                .values()
                .filter(|stat| stat.spliced)
                .count()
        };
        assert_eq!(spliced(RIFLE), 14);
        assert_eq!(spliced(MELEE), 14);
    }

    #[test]
    fn splices_from_held_stats() {
        let data = data();
        let rifle = data.riven_type(RIFLE).unwrap();
        let names = |tags: &[&str]| -> Vec<String> {
            rifle
                .splice_results(tags)
                .iter()
                .map(|stat| stat.name())
                .collect()
        };
        assert_eq!(
            names(&[
                "WeaponDamageAmountMod",
                "WeaponFireIterationsMod",
                "WeaponZoomFovMod"
            ]),
            ["Weak Point Damage"]
        );
        assert_eq!(
            names(&[
                "WeaponToxinDamageMod",
                "WeaponFireDamageMod",
                "WeaponFreezeDamageMod"
            ]),
            ["Gas", "Viral", "Blast"]
        );
        assert!(names(&["WeaponCritChanceMod", "WeaponDamageAmountMod"]).is_empty());

        let melee = data.riven_type(MELEE).unwrap();
        let slam: Vec<&str> = melee
            .splice_results(&[
                "WeaponMeleeDamageMod",
                "WeaponFireRateMod",
                "WeaponStunChanceMod",
            ])
            .iter()
            .map(|stat| stat.tag.as_str())
            .collect();
        assert_eq!(slam, ["WeaponStatusDamage", "WeaponMeleeSlamDamageMod"]);
    }

    #[test]
    fn reversed_sign_reads_as_buff() {
        let data = data();
        let efficiency = data.stat(RIFLE, "WeaponAmmoEfficiency").unwrap();
        assert!(efficiency.reversed);
        assert!(efficiency.base_value < 0.0);
        let multipliers = trait_multipliers(2, 1).unwrap();
        assert!(efficiency.base_at_rank_nine(1.0, multipliers.good) > 0.0);
        assert!(efficiency.base_at_rank_nine(1.0, multipliers.bad) < 0.0);
        assert_eq!(efficiency.name(), "Ammo Efficiency");
    }

    #[test]
    fn trait_multiplier_table() {
        assert_eq!(
            trait_multipliers(2, 0),
            Some(TraitMultipliers {
                good: 0.99,
                bad: 0.0
            })
        );
        assert_eq!(
            trait_multipliers(2, 1),
            Some(TraitMultipliers {
                good: 1.2375,
                bad: -0.495
            })
        );
        assert_eq!(
            trait_multipliers(3, 0),
            Some(TraitMultipliers {
                good: 0.75,
                bad: 0.0
            })
        );
        assert_eq!(
            trait_multipliers(3, 1),
            Some(TraitMultipliers {
                good: 0.9375,
                bad: -0.75
            })
        );
        assert_eq!(trait_multipliers(1, 0), None);
        assert_eq!(trait_multipliers(4, 1), None);
        assert_eq!(trait_multipliers(3, 2), None);
    }

    #[test]
    fn roll_multiplier_range() {
        assert!((roll_multiplier(0) - 0.9).abs() < f64::EPSILON);
        assert!((roll_multiplier(-1) - 0.9).abs() < f64::EPSILON);
        assert!((roll_multiplier(1 << 30) - 1.1).abs() < 1e-9);
        assert!((roll_multiplier(1 << 40) - 1.1).abs() < f64::EPSILON);
        let half = roll_multiplier(536_870_912);
        assert!((half - 1.0).abs() < 1e-6, "{half}");
        assert!((roll_share(1.1, false) - 1.0).abs() < 1e-9);
        assert!((roll_share(1.1, true) - 0.0).abs() < 1e-9);
        assert!((roll_share(0.9, true) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn rank_multiplier_at_max() {
        assert!((rank_multiplier(MAX_RANK) - 1.0).abs() < f64::EPSILON);
        assert!((rank_multiplier(0) - 1.0 / 9.0).abs() < f64::EPSILON);
    }

    #[test]
    fn rifle_damage_published_range() {
        let data = data();
        let damage = data.stat(RIFLE, "WeaponDamageAmountMod").unwrap();
        let multipliers = trait_multipliers(2, 1).unwrap();
        let base = damage.base_at_rank_nine(1.0, multipliers.good);
        assert!((base * 0.9 - 183.77).abs() < 0.01, "{}", base * 0.9);
        assert!((base * 1.1 - 224.61).abs() < 0.01, "{}", base * 1.1);

        let recoil = data.stat(RIFLE, "WeaponRecoilReductionMod").unwrap();
        assert!(recoil.base_value < 0.0);
        assert!(recoil.base_at_rank_nine(1.0, multipliers.good) < 0.0);
    }

    #[test]
    fn combo_points_curse() {
        let data = data();
        let combo = data.stat(MELEE, "WeaponMeleeComboPointsOnHitMod").unwrap();
        assert!(combo.reversed);
        let multipliers = trait_multipliers(2, 1).unwrap();
        let as_curse = combo.base_at_rank_nine(1.0, multipliers.bad);
        assert!(as_curse < 0.0, "{as_curse}");
        let as_buff = combo.base_at_rank_nine(1.0, multipliers.good);
        assert!(as_buff > 0.0, "{as_buff}");
    }
}
