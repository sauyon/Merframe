use serde::Serialize;
use wf_data::{
    MAX_RANK, RivenStat as StatDefinition, RivenType, TraitMultipliers, rank_multiplier,
    roll_multiplier, roll_share, trait_multipliers,
};
use wf_inventory::{RivenFingerprint, RivenStat};
use wf_market::{GoodRoll, Polarity};

use super::Grader;
use super::good_roll::desires;
use crate::catalog::Catalog;

const GRADE_BANDS: [(f64, &str); 10] = [
    (0.025, "F"),
    (0.125, "C-"),
    (0.225, "C"),
    (0.325, "C+"),
    (0.425, "B-"),
    (0.575, "B"),
    (0.675, "B+"),
    (0.775, "A-"),
    (0.875, "A"),
    (0.975, "A+"),
];

const ROLL_BANDS: [(f64, &str); 4] = [(0.9, "S"), (0.75, "A"), (0.6, "B"), (0.4, "C")];

const COMPAT_FIXUPS: [(&str, &str); 2] = [
    (
        "/Lotus/Weapons/Tenno/Melee/Dagger/DarkDaggerBase",
        "/Lotus/Weapons/Tenno/Melee/Dagger/DarkDagger",
    ),
    (
        "/Lotus/Weapons/Tenno/Shotgun/QuadShotgunBase",
        "/Lotus/Weapons/Tenno/Shotgun/QuadShotgun",
    ),
];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct AttributeGrade {
    pub tag: String,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub unit: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub localization: Option<String>,
    pub value: i64,
    pub percentile: f64,
    pub grade: &'static str,
    pub multiplier: f64,
    pub rolled: Option<f64>,
    pub display: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub curse: bool,
    pub spliced: bool,
    pub desired: bool,
}

pub(super) fn compat_path(compat: &str) -> &str {
    match COMPAT_FIXUPS.iter().find(|(from, _)| *from == compat) {
        Some((_, renamed)) => renamed,
        None => compat,
    }
}

impl Grader<'_> {
    pub(super) fn graded(
        &self,
        fingerprint: &RivenFingerprint,
        riven_type: Option<&RivenType>,
        disposition: Option<f64>,
        good: Option<&GoodRoll>,
    ) -> Vec<AttributeGrade> {
        let roll = Roll {
            grader: self,
            riven_type,
            disposition,
            good,
            multipliers: trait_multipliers(fingerprint.buffs.len(), fingerprint.curses.len()),
        };
        fingerprint
            .buffs
            .iter()
            .map(|stat| (stat, false))
            .chain(fingerprint.curses.iter().map(|stat| (stat, true)))
            .map(|(stat, curse)| roll.grade(stat, curse))
            .collect()
    }
}

struct Roll<'a> {
    grader: &'a Grader<'a>,
    riven_type: Option<&'a RivenType>,
    disposition: Option<f64>,
    good: Option<&'a GoodRoll>,
    multipliers: Option<TraitMultipliers>,
}

impl Roll<'_> {
    fn grade(&self, stat: &RivenStat, curse: bool) -> AttributeGrade {
        let Roll {
            grader,
            riven_type,
            disposition,
            good,
            multipliers,
        } = *self;
        let modifier: Option<&StatDefinition> =
            riven_type.and_then(|found| found.stats.get(&stat.tag));
        let attribute = riven_type.and_then(|found| grader.attribute(found, &stat.tag));
        let multiplier = roll_multiplier(stat.value);
        let rank_scale = rank_multiplier(MAX_RANK);
        let base = match (modifier, disposition, multipliers) {
            (Some(modifier), Some(disposition), Some(multipliers)) => {
                let share = if curse {
                    multipliers.bad
                } else {
                    multipliers.good
                };
                Some(modifier.base_at_rank_nine(disposition, share) * rank_scale)
            }
            _ => None,
        };
        let (worst, best) = if curse { (1.1, 0.9) } else { (0.9, 1.1) };
        let rolled = base.map(|base| base * multiplier);
        let displayed = |factor: f64| {
            base.zip(modifier)
                .map(|(base, modifier)| display_value(base * factor, modifier.multiplier_display()))
        };
        let percentile = roll_share(multiplier, curse);
        AttributeGrade {
            name: riven_type.and_then(|found| grader.stat_name(found, &stat.tag)),
            slug: attribute.map(|attribute| attribute.slug.clone()),
            unit: match attribute {
                Some(attribute) => attribute.unit.clone(),
                None => modifier.and_then(|modifier| modifier.unit().map(str::to_owned)),
            },
            prefix: modifier.map(|modifier| modifier.prefix.clone()),
            suffix: modifier.map(|modifier| modifier.suffix.clone()),
            localization: modifier.map(|modifier| modifier.localization.clone()),
            value: stat.value,
            percentile,
            grade: letter(percentile),
            multiplier,
            display: displayed(multiplier),
            rolled,
            min: displayed(worst),
            max: displayed(best),
            tag: stat.tag.clone(),
            curse,
            spliced: modifier.is_some_and(|modifier| modifier.spliced),
            desired: good.is_some_and(|good| desires(good, &stat.tag, curse)),
        }
    }
}

pub(super) fn roll_grade(mean: f64) -> &'static str {
    ROLL_BANDS
        .iter()
        .find(|(floor, _)| mean >= *floor)
        .map_or("D", |(_, grade)| grade)
}

fn letter(percentile: f64) -> &'static str {
    match GRADE_BANDS
        .iter()
        .find(|(threshold, _)| percentile < *threshold)
    {
        Some((_, band)) => band,
        None => "S",
    }
}

pub(super) fn display_value(rolled: f64, multiplier_display: bool) -> f64 {
    if multiplier_display {
        round_to(rolled + 1.0, 2)
    } else {
        round_to(rolled, 1)
    }
}

pub(super) fn round_to(value: f64, places: i32) -> f64 {
    let scale = 10f64.powi(places);
    (value * scale).round() / scale
}

pub(super) fn perfectness(attributes: &[AttributeGrade]) -> f64 {
    if attributes.is_empty() {
        return 0.0;
    }
    let total: f64 = attributes
        .iter()
        .map(|attribute| attribute.percentile)
        .sum();
    #[allow(clippy::cast_precision_loss, reason = "a riven has at most four stats")]
    let count = attributes.len() as f64;
    total / count
}

fn capitalised(text: &str) -> String {
    let mut characters = text.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

pub(super) fn riven_name(riven_type: &RivenType, buffs: &[RivenStat]) -> Option<String> {
    let mut ordered: Vec<&RivenStat> = buffs.iter().collect();
    ordered.sort_by_key(|stat| std::cmp::Reverse(stat.value));
    let parts: Vec<&StatDefinition> = ordered
        .iter()
        .filter_map(|stat| riven_type.stats.get(&stat.tag))
        .collect();
    if parts.len() != ordered.len() {
        return None;
    }
    match parts.as_slice() {
        [first, second] => Some(format!(
            "{}{}",
            capitalised(&first.prefix),
            second.suffix.to_lowercase()
        )),
        [first, second, third] => Some(format!(
            "{}-{}{}",
            capitalised(&first.prefix),
            second.prefix.to_lowercase(),
            third.suffix.to_lowercase()
        )),
        _ => None,
    }
}

pub(super) fn polarity(pol: &str) -> Option<Polarity> {
    match pol {
        "AP_ATTACK" => Some(Polarity::Madurai),
        "AP_DEFENSE" => Some(Polarity::Vazarin),
        "AP_TACTIC" => Some(Polarity::Naramon),
        "AP_WARD" => Some(Polarity::Unairu),
        "AP_POWER" => Some(Polarity::Zenurik),
        "AP_PRECEPT" => Some(Polarity::Penjaga),
        "AP_UMBRA" => Some(Polarity::Umbra),
        _ => None,
    }
}

pub fn fingerprint_weapon(fingerprint: &str) -> Option<String> {
    let parsed = RivenFingerprint::parse(fingerprint).ok()?;
    Some(compat_path(parsed.compat.as_deref()?).to_owned())
}

pub(crate) fn traded_riven_name(
    catalog: &Catalog,
    item_type: &str,
    fingerprint: &str,
) -> Option<String> {
    let riven_type = catalog.data().riven_data().riven_type(item_type)?;
    let fingerprint = RivenFingerprint::parse(fingerprint).ok()?;
    let weapon = catalog.item(compat_path(fingerprint.compat.as_deref()?))?;
    let name = riven_name(riven_type, &fingerprint.buffs)?;
    Some(format!("{} {name}", weapon.name))
}

#[allow(
    clippy::cast_precision_loss,
    reason = "the game compares rolls as 32 bit floats"
)]
fn game_float(value: i64) -> f32 {
    value as f32
}

pub(super) fn same_roll(attributes: &[AttributeGrade], fingerprint: &RivenFingerprint) -> bool {
    let rolled = fingerprint.buffs.iter().chain(&fingerprint.curses);
    attributes.len() == fingerprint.buffs.len() + fingerprint.curses.len()
        && attributes.iter().zip(rolled).all(|(attribute, stat)| {
            attribute.tag == stat.tag
                && game_float(attribute.value).to_bits() == game_float(stat.value).to_bits()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::fixtures;
    use crate::rivens::support::*;

    const DAMAGE_TAG: &str = "WeaponDamageAmountMod";

    const WORST_ROLL: i64 = 0;

    fn rolled_attribute(tag: &str, value: i64, curse: bool) -> AttributeGrade {
        rolled_of_type(RIFLE_RIVEN, tag, value, curse)
    }

    fn rolled_of_type(riven_type: &str, tag: &str, value: i64, curse: bool) -> AttributeGrade {
        let fixture = fixture();
        let grader = fixture.grader();
        let riven_type = grader.riven_type(riven_type).expect("riven type");
        let roll = Roll {
            grader: &grader,
            riven_type: Some(riven_type),
            disposition: Some(1.0),
            good: None,
            multipliers: trait_multipliers(2, 1),
        };
        roll.grade(
            &RivenStat {
                tag: tag.to_owned(),
                value,
            },
            curse,
        )
    }

    #[test]
    fn generated_names() {
        let tab = riven_tab();
        assert_eq!(
            by_weapon(&tab.unveiled, "Proboscis Cernos").name.as_deref(),
            Some("Sci-cronicron")
        );
        assert_eq!(
            by_weapon(&tab.unveiled, "Soma").name.as_deref(),
            Some("Toxidex")
        );
        assert_eq!(
            by_weapon(&tab.unveiled, "Buzlok").name.as_deref(),
            Some("Acrides")
        );
        assert_eq!(
            by_weapon(&tab.unveiled, "Zylok").name.as_deref(),
            Some("Pura-vexinak")
        );
        assert_eq!(
            by_weapon(&tab.unveiled, "Harmony").name.as_deref(),
            Some("Acri-plecium")
        );
        assert_eq!(
            by_weapon(&tab.unveiled, "Akstiletto").name.as_deref(),
            Some("Crita-concimag")
        );
        assert!(
            tab.unveiled
                .iter()
                .filter(|row| row.attributes.iter().filter(|a| !a.curse).count() == 2)
                .all(|row| !row.name.as_deref().unwrap_or_default().contains('-')),
            "two-buff names carry no dash"
        );
        assert!(
            tab.unveiled
                .iter()
                .filter(|row| row.attributes.iter().filter(|a| !a.curse).count() == 3)
                .all(|row| row.name.as_deref().unwrap_or_default().contains('-')),
            "three-buff names carry a dash"
        );
    }

    #[test]
    fn name_order_by_roll_value() {
        let tab = riven_tab();
        let cernos = by_weapon(&tab.unveiled, "Proboscis Cernos");
        let mut buffs: Vec<&AttributeGrade> =
            cernos.attributes.iter().filter(|a| !a.curse).collect();
        assert_eq!(buffs.len(), 3);
        buffs.sort_by_key(|attribute| std::cmp::Reverse(attribute.value));
        let name = format!(
            "{}-{}{}",
            capitalised(buffs[0].prefix.as_deref().unwrap()),
            buffs[1].prefix.as_deref().unwrap().to_lowercase(),
            buffs[2].suffix.as_deref().unwrap().to_lowercase()
        );
        assert_eq!(cernos.name.as_deref(), Some(name.as_str()));

        let reordered = format!(
            "{}-{}{}",
            capitalised(buffs[2].prefix.as_deref().unwrap()),
            buffs[1].prefix.as_deref().unwrap().to_lowercase(),
            buffs[0].suffix.as_deref().unwrap().to_lowercase()
        );
        assert_ne!(cernos.name.as_deref(), Some(reordered.as_str()));
    }

    #[test]
    fn slash_roll_range() {
        let tab = riven_tab();
        let cernos = by_weapon(&tab.unveiled, "Proboscis Cernos");
        assert_eq!(cernos.rank, 8);
        assert!((cernos.disposition.unwrap() - 0.9).abs() < 1e-6);
        let slash = cernos
            .attributes
            .iter()
            .find(|attribute| attribute.tag == "WeaponSlashDamageMod")
            .expect("slash roll");
        let rolled = slash.rolled.unwrap();
        assert!((rolled - 108.74).abs() < 0.01, "{rolled}");
        assert_eq!(slash.min, Some(91.1));
        assert_eq!(slash.max, Some(111.4));
        assert_eq!(slash.display, Some(108.7));
        assert_eq!(slash.slug.as_deref(), Some("slash_damage"));
        assert_eq!(slash.name.as_deref(), Some("Slash"));
        assert_eq!(slash.unit.as_deref(), Some("percent"));
    }

    #[test]
    fn inverted_curse_range() {
        let tab = riven_tab();
        let cernos = by_weapon(&tab.unveiled, "Proboscis Cernos");
        let reload = cernos
            .attributes
            .iter()
            .find(|attribute| attribute.curse)
            .expect("a curse");
        assert_eq!(reload.tag, "WeaponReloadSpeedMod");
        let (rolled, min, max) = (
            reload.rolled.unwrap(),
            reload.min.unwrap(),
            reload.max.unwrap(),
        );
        assert!(rolled < 0.0 && min < 0.0 && max < 0.0);
        assert!(min < max, "the worst case is the more negative one");
        assert_eq!((min, max), (-37.1, -30.4));
        assert!((reload.percentile - (1.0 - (reload.multiplier - 0.9) * 5.0)).abs() < 1e-12);
    }

    #[test]
    fn faction_damage_multiplier() {
        let tab = riven_tab();
        let faction = tab
            .unveiled
            .iter()
            .flat_map(|row| row.attributes.iter())
            .find(|attribute| attribute.tag.starts_with("WeaponFactionDamage") && !attribute.curse)
            .expect("faction roll");
        let rolled = faction.rolled.unwrap();
        let display = faction.display.unwrap();
        assert!(rolled.abs() < 1.0, "{rolled}");
        assert!(display > 1.0, "{display}");
        assert!((display - round_to(rolled + 1.0, 2)).abs() < f64::EPSILON);
    }

    #[test]
    fn grade_is_mean_share() {
        let tab = riven_tab();
        for row in &tab.unveiled {
            let expected = perfectness(&row.attributes);
            assert!((row.grade - expected).abs() < f64::EPSILON);
            assert!((0.0..=1.0).contains(&row.grade), "{}", row.grade);
        }
        assert!(tab.unveiled[0].grade >= tab.unveiled[tab.unveiled.len() - 1].grade);
    }

    #[test]
    fn roll_grade_bands() {
        assert_eq!(roll_grade(0.95), "S");
        assert_eq!(roll_grade(0.9), "S");
        assert_eq!(roll_grade(0.75), "A");
        assert_eq!(roll_grade(0.6), "B");
        assert_eq!(roll_grade(0.4), "C");
        assert_eq!(roll_grade(0.39), "D");
    }

    #[test]
    fn letter_bands() {
        assert_eq!(letter(0.0), "F");
        assert_eq!(letter(0.024), "F");
        assert_eq!(letter(0.025), "C-");
        assert_eq!(letter(0.124), "C-");
        assert_eq!(letter(0.125), "C");
        assert_eq!(letter(0.224), "C");
        assert_eq!(letter(0.225), "C+");
        assert_eq!(letter(0.324), "C+");
        assert_eq!(letter(0.325), "B-");
        assert_eq!(letter(0.424), "B-");
        assert_eq!(letter(0.425), "B");
        assert_eq!(letter(0.574), "B");
        assert_eq!(letter(0.575), "B+");
        assert_eq!(letter(0.674), "B+");
        assert_eq!(letter(0.675), "A-");
        assert_eq!(letter(0.774), "A-");
        assert_eq!(letter(0.775), "A");
        assert_eq!(letter(0.874), "A");
        assert_eq!(letter(0.875), "A+");
        assert_eq!(letter(0.974), "A+");
        assert_eq!(letter(0.975), "S");
        assert_eq!(letter(1.0), "S");
    }

    #[test]
    fn worst_roll_grades_f() {
        let damage = rolled_attribute(DAMAGE_TAG, WORST_ROLL, false);
        assert!(damage.percentile.abs() < 1e-9, "{}", damage.percentile);
        assert_eq!(damage.grade, "F");
        assert_eq!(damage.display, damage.min);
    }

    #[test]
    fn best_roll_grades_s() {
        let damage = rolled_attribute(DAMAGE_TAG, BEST_ROLL, false);
        assert!(
            (damage.percentile - 1.0).abs() < 1e-9,
            "{}",
            damage.percentile
        );
        assert_eq!(damage.grade, "S");
        assert_eq!(damage.display, damage.max);
    }

    #[test]
    fn middle_roll_grades_b() {
        let damage = rolled_attribute(DAMAGE_TAG, MIDDLE_ROLL, false);
        assert!(
            (damage.percentile - 0.5).abs() < 1e-9,
            "{}",
            damage.percentile
        );
        assert_eq!(damage.grade, "B");
    }

    #[test]
    fn curse_grades_invert() {
        let weakest = rolled_attribute(DAMAGE_TAG, WORST_ROLL, true);
        assert!(
            (weakest.percentile - 1.0).abs() < 1e-9,
            "{}",
            weakest.percentile
        );
        assert_eq!(weakest.grade, "S");
        let (rolled, min, max) = (
            weakest.rolled.unwrap(),
            weakest.min.unwrap(),
            weakest.max.unwrap(),
        );
        assert!(rolled < 0.0 && min < max);
        assert_eq!(weakest.display, weakest.max);

        let strongest = rolled_attribute(DAMAGE_TAG, BEST_ROLL, true);
        assert!(
            strongest.percentile.abs() < 1e-9,
            "{}",
            strongest.percentile
        );
        assert_eq!(strongest.grade, "F");
    }

    #[test]
    fn fixture_letters_match_percentile() {
        let tab = riven_tab();
        for row in &tab.unveiled {
            for attribute in &row.attributes {
                assert_eq!(
                    attribute.grade,
                    letter(attribute.percentile),
                    "{} {}",
                    row.item_id,
                    attribute.tag
                );
            }
        }
        let letters: Vec<&str> = tab
            .unveiled
            .iter()
            .flat_map(|row| row.attributes.iter())
            .map(|attribute| attribute.grade)
            .collect();
        assert!(letters.contains(&"A+"));
        assert!(
            letters
                .iter()
                .all(|band| *band == "S" || GRADE_BANDS.iter().any(|(_, known)| known == band))
        );
    }

    #[test]
    fn spliced_gun_stat_without_market_attribute() {
        let scaldra = rolled_attribute("WeaponFactionDamageScaldra", MIDDLE_ROLL, false);
        assert!(scaldra.spliced);
        assert_eq!(scaldra.slug, None);
        assert_eq!(scaldra.name.as_deref(), Some("Damage to Scaldra"));
        assert_eq!(scaldra.unit.as_deref(), Some("multiply"));
        assert_eq!(scaldra.prefix.as_deref(), Some("exsi"));
        assert!(scaldra.min.unwrap() > 1.0);
        assert!(scaldra.min < scaldra.display && scaldra.display < scaldra.max);
        assert_eq!(scaldra.grade, "B");

        let gas = rolled_attribute("WeaponGasDamageMod", BEST_ROLL, false);
        assert_eq!(gas.name.as_deref(), Some("Gas"));
        assert_eq!(gas.unit.as_deref(), Some("percent"));
        assert_eq!(gas.grade, "S");

        let damage = rolled_attribute(DAMAGE_TAG, MIDDLE_ROLL, false);
        assert!(!damage.spliced);
        assert_eq!(damage.slug.as_deref(), Some("base_damage_/_melee_damage"));
    }

    #[test]
    fn melee_splices_share_affixes_with_combo_bonus() {
        let parry = rolled_of_type(MELEE_RIVEN, "WeaponMeleeParryAngleMod", MIDDLE_ROLL, false);
        assert!(parry.spliced);
        assert_eq!(parry.slug, None);
        assert_eq!(parry.name.as_deref(), Some("Parry Angle"));
        assert_eq!(parry.unit, None);
        assert_eq!(parry.prefix.as_deref(), Some("laci"));
        assert_eq!(parry.suffix.as_deref(), Some("nus"));

        let combo = rolled_of_type(
            MELEE_RIVEN,
            "WeaponMeleeComboBonusOnHitMod",
            MIDDLE_ROLL,
            false,
        );
        assert_eq!(
            combo.slug.as_deref(),
            Some("chance_to_gain_extra_combo_count")
        );
        assert_eq!(combo.prefix.as_deref(), Some("laci"));

        let melee_damage = rolled_of_type(MELEE_RIVEN, "WeaponMeleeDamageMod", MIDDLE_ROLL, false);
        assert_eq!(
            melee_damage.slug.as_deref(),
            Some("base_damage_/_melee_damage")
        );
        let corpus = rolled_of_type(
            MELEE_RIVEN,
            "WeaponMeleeFactionDamageCorpus",
            MIDDLE_ROLL,
            false,
        );
        assert_eq!(corpus.slug.as_deref(), Some("damage_vs_corpus"));
    }

    #[test]
    fn ammo_efficiency_reads_as_buff() {
        let efficiency = rolled_attribute("WeaponAmmoEfficiency", BEST_ROLL, false);
        assert_eq!(efficiency.name.as_deref(), Some("Ammo Efficiency"));
        assert_eq!(efficiency.unit.as_deref(), Some("percent"));
        assert!(efficiency.rolled.unwrap() > 0.0);
        assert!(efficiency.min.unwrap() < efficiency.max.unwrap());
        assert_eq!(efficiency.grade, "S");

        let cursed = rolled_attribute("WeaponAmmoEfficiency", WORST_ROLL, true);
        assert!(cursed.rolled.unwrap() < 0.0);
        assert_eq!(cursed.grade, "S");

        let recoil = rolled_attribute("WeaponRecoilReductionMod", BEST_ROLL, false);
        assert!(recoil.rolled.unwrap() < 0.0);
    }

    #[test]
    fn game_float_equivalence() {
        let alike =
            |left: i64, right: i64| game_float(left).to_bits() == game_float(right).to_bits();
        assert!(alike(708_669_601, 708_669_663));
        assert!(alike(903_450_000, 903_450_016));
        assert!(!alike(708_669_601, 741_320_000));
        assert!(!alike(1, 2));
    }

    #[test]
    fn traded_riven_names() {
        let catalog = fixtures::catalog_with(fixtures::RIVEN_ITEMS, PRE_VEILED_ITEMS);
        let inventory = fixtures::inventory();
        let soma = inventory
            .rivens()
            .find(|upgrade| {
                fingerprint_weapon(upgrade.upgrade_fingerprint.as_deref().unwrap_or_default())
                    .is_some_and(|weapon| weapon.ends_with("/Rifle/TennoAR"))
            })
            .unwrap();
        let fingerprint = soma.upgrade_fingerprint.as_deref().unwrap();
        assert_eq!(
            traded_riven_name(&catalog, &soma.item_type, fingerprint).as_deref(),
            Some("Soma Toxidex")
        );
        assert_eq!(
            traded_riven_name(&catalog, RIFLE_RIVEN, "{\"lvl\":0}"),
            None
        );
    }

    #[test]
    fn fingerprint_weapon_path() {
        assert_eq!(
            fingerprint_weapon(CURRENT_ROLL).as_deref(),
            Some(ARCHON_DAGGERS)
        );
        assert_eq!(fingerprint_weapon("{"), None);
    }

    #[test]
    fn renamed_compat_paths() {
        assert_eq!(
            compat_path("/Lotus/Weapons/Tenno/Melee/Dagger/DarkDaggerBase"),
            "/Lotus/Weapons/Tenno/Melee/Dagger/DarkDagger"
        );
        assert_eq!(
            compat_path("/Lotus/Weapons/Tenno/Shotgun/QuadShotgunBase"),
            "/Lotus/Weapons/Tenno/Shotgun/QuadShotgun"
        );
        assert_eq!(
            compat_path("/Lotus/Weapons/Tenno/Rifle/Soma"),
            "/Lotus/Weapons/Tenno/Rifle/Soma"
        );
    }
}
