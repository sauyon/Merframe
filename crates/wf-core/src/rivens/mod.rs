use std::collections::HashMap;

use serde::Serialize;
use wf_data::{RivenStat, RivenType};
use wf_market::{Polarity, RivenAttribute, RivenData as RivenTable};

use crate::catalog::Catalog;
use crate::identity::ItemTable;

mod dialog;
mod good_roll;
mod grading;
mod listing;
mod reroll;
mod station;
mod tab;

pub use good_roll::{AlternativeMatch, GoodRollView, StatMatch};
pub(crate) use grading::traded_riven_name;
pub use grading::{AttributeGrade, fingerprint_weapon};
pub use listing::{ListingChoices, listing_payload, listing_update};
pub use reroll::{KeptRoll, kept_roll};
pub use tab::display_name;

const RIVEN_MOD_SUFFIX: &str = " Riven Mod";

type AttributeIndex<'a> = HashMap<&'a str, HashMap<&'a str, &'a RivenAttribute>>;

pub(crate) struct Grader<'a> {
    catalog: &'a Catalog,
    items: &'a ItemTable,
    attributes: AttributeIndex<'a>,
    table: Option<&'a RivenTable>,
}

fn market_attribute<'a>(
    stat: &RivenStat,
    riven_type: &RivenType,
    by_game_ref: &HashMap<&str, &'a RivenAttribute>,
    by_affixes: &HashMap<(String, String), &'a RivenAttribute>,
) -> Option<&'a RivenAttribute> {
    if let Some(attribute) = by_game_ref.get(stat.tag.as_str()) {
        return Some(attribute);
    }
    if stat.prefix.is_empty() {
        return None;
    }
    let shared = by_affixes.get(&(stat.prefix.to_lowercase(), stat.suffix.to_lowercase()))?;
    let owned_by_another_stat = riven_type.stats.contains_key(&shared.game_ref);
    (!owned_by_another_stat).then_some(shared)
}

fn attribute_index<'a>(
    riven_data: &'a wf_data::RivenData,
    table: Option<&'a RivenTable>,
) -> AttributeIndex<'a> {
    let attributes = table.map_or(&[][..], |table| table.attributes.as_slice());
    let by_game_ref: HashMap<&str, &RivenAttribute> = attributes
        .iter()
        .map(|attribute| (attribute.game_ref.as_str(), attribute))
        .collect();
    let by_affixes: HashMap<(String, String), &RivenAttribute> = attributes
        .iter()
        .map(|attribute| {
            let affixes = (
                attribute.prefix.to_lowercase(),
                attribute.suffix.to_lowercase(),
            );
            (affixes, attribute)
        })
        .collect();
    riven_data
        .types()
        .map(|riven_type| {
            let joined = riven_type
                .stats
                .values()
                .filter_map(|stat| {
                    let attribute = market_attribute(stat, riven_type, &by_game_ref, &by_affixes)?;
                    Some((stat.tag.as_str(), attribute))
                })
                .collect();
            (riven_type.unique_name.as_str(), joined)
        })
        .collect()
}

impl<'a> Grader<'a> {
    pub(crate) fn new(
        catalog: &'a Catalog,
        items: &'a ItemTable,
        table: Option<&'a RivenTable>,
    ) -> Self {
        Self {
            catalog,
            items,
            attributes: attribute_index(catalog.data().riven_data(), table),
            table,
        }
    }

    fn riven_type(&self, unique_name: &str) -> Option<&'a RivenType> {
        self.catalog.data().riven_data().riven_type(unique_name)
    }

    fn attribute(&self, riven_type: &RivenType, tag: &str) -> Option<&'a RivenAttribute> {
        self.attributes
            .get(riven_type.unique_name.as_str())?
            .get(tag)
            .copied()
    }

    fn stat_name(&self, riven_type: &RivenType, tag: &str) -> Option<String> {
        let modifier = riven_type.stats.get(tag)?;
        Some(match self.attribute(riven_type, tag) {
            Some(attribute) => attribute.name.clone(),
            None => modifier.name(),
        })
    }

    fn open_splice_names(&self, riven_type: &RivenType, graded: &[AttributeGrade]) -> Vec<String> {
        if graded.iter().any(|attribute| attribute.spliced) {
            return Vec::new();
        }

        let tags: Vec<&str> = graded
            .iter()
            .map(|attribute| attribute.tag.as_str())
            .collect();
        riven_type
            .splice_results(&tags)
            .into_iter()
            .filter_map(|stat| self.stat_name(riven_type, &stat.tag))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct PendingRoll {
    pub name: Option<String>,
    pub rerolls: u32,
    pub polarity: Option<Polarity>,
    pub grade: f64,
    pub grade_letter: &'static str,
    pub attributes: Vec<AttributeGrade>,
    pub splices: Vec<String>,
    pub good_roll: Option<GoodRollView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct RivenRow {
    pub item_id: String,
    pub item_type: String,
    pub riven_type: Option<String>,
    pub weapon_class: Option<String>,
    pub name: Option<String>,
    pub weapon: String,
    pub weapon_path: Option<String>,
    pub weapon_slug: Option<String>,
    pub image_name: Option<String>,
    pub disposition: Option<f64>,
    pub disposition_weapon: Option<String>,
    pub unveiled: bool,
    pub rank: u32,
    pub rank_required: Option<u32>,
    pub rerolls: u32,
    pub polarity: Option<Polarity>,
    pub grade: f64,
    pub grade_letter: &'static str,
    pub attributes: Vec<AttributeGrade>,
    pub splices: Vec<String>,
    pub good_roll: Option<GoodRollView>,
    pub listed_in_wfm: bool,
    pub unlisted_stat: Option<String>,
    pub pending: Option<PendingRoll>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct VeiledRiven {
    pub riven_id: String,
    pub item_type: String,
    pub name: String,
    pub weapon_class: String,
    pub market_slug: Option<String>,
    pub image_name: Option<String>,
    pub count: i64,
    pub progress: i64,
    pub required: i64,
    pub pre_veiled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct VeiledGroup {
    pub challenge_id: String,
    pub challenge: String,
    pub complication: Option<String>,
    pub count: i64,
    pub rivens: Vec<VeiledRiven>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct RivensTab {
    pub veiled: Vec<VeiledGroup>,
    pub unveiled: Vec<RivenRow>,
    pub attribution: Option<String>,
}

#[cfg(test)]
pub(crate) mod support {
    use super::*;
    use crate::catalog::{Catalog, fixtures};
    use crate::listings::MarketListings;
    use wf_market::{RivenData as RivenTable, RivenWeapon};

    pub(crate) const ARCHON_DAGGERS: &str =
        "/Lotus/Weapons/Archon/Melee/DualDaggers/ArchonDualDaggersPlayerWep";

    pub(crate) const OTHER_WEAPON: &str = "/Lotus/Weapons/Tenno/Unknown/NoSuchWeapon";

    const RIVEN_DATA: &str = include_str!("../../../../fixtures/riven_data.json");

    pub(crate) const CURRENT_ROLL: &str = r#"{"compat":"/Lotus/Weapons/Archon/Melee/DualDaggers/ArchonDualDaggersPlayerWep","lim":294146365,"lvlReq":11,"lvl":8,"rerolls":74,"pol":"AP_DEFENSE","buffs":[{"Tag":"WeaponFreezeDamageMod","Value":708669601},{"Tag":"WeaponToxinDamageMod","Value":526133492},{"Tag":"WeaponFireRateMod","Value":397284473}],"curses":[]}"#;

    pub(crate) const PENDING_ROLL: &str = r#"{"compat":"/Lotus/Weapons/Archon/Melee/DualDaggers/ArchonDualDaggersPlayerWep","lim":294146365,"lvlReq":11,"lvl":8,"rerolls":75,"pol":"AP_DEFENSE","buffs":[{"Tag":"WeaponFreezeDamageMod","Value":741320000},{"Tag":"WeaponCritDamageMod","Value":903450000},{"Tag":"WeaponFireDamageMod","Value":512780000}],"curses":[{"Tag":"SlideAttackCritChanceMod","Value":268940000}]}"#;

    pub(crate) const PRE_VEILED_ITEMS: &str = r#"[
      {
            "uniqueName": "/Lotus/Upgrades/Mods/Randomized/RawArchgunRandomMod",
            "name": "Archgun Riven Mod",
            "category": "Mods",
            "type": "Archgun Riven Mod",
            "tradable": false,
            "imageName": "OmegaMod.png"
      },
      {
            "uniqueName": "/Lotus/Upgrades/Mods/Randomized/RawSentinelWeaponRandomMod",
            "name": "Companion Weapon Riven Mod",
            "category": "Mods",
            "type": "Companion Weapon Riven Mod",
            "tradable": false,
            "imageName": "OmegaMod.png"
      },
      {
            "uniqueName": "/Lotus/Upgrades/Mods/Randomized/RawModularPistolRandomMod",
            "name": "Kitgun Riven Mod",
            "category": "Mods",
            "type": "Kitgun Riven Mod",
            "tradable": false,
            "imageName": "OmegaMod.png"
      },
      {
            "uniqueName": "/Lotus/Upgrades/Mods/Randomized/RawMeleeRandomMod",
            "name": "Melee Riven Mod",
            "category": "Mods",
            "type": "Melee Riven Mod",
            "tradable": false,
            "imageName": "OmegaMod.png"
      },
      {
            "uniqueName": "/Lotus/Upgrades/Mods/Randomized/RawPistolRandomMod",
            "name": "Pistol Riven Mod",
            "category": "Mods",
            "type": "Pistol Riven Mod",
            "tradable": false,
            "imageName": "OmegaMod.png"
      },
      {
            "uniqueName": "/Lotus/Upgrades/Mods/Randomized/RawRifleRandomMod",
            "name": "Rifle Riven Mod",
            "category": "Mods",
            "type": "Rifle Riven Mod",
            "tradable": false,
            "imageName": "OmegaMod.png"
      },
      {
            "uniqueName": "/Lotus/Upgrades/Mods/Randomized/RawShotgunRandomMod",
            "name": "Shotgun Riven Mod",
            "category": "Mods",
            "type": "Shotgun Riven Mod",
            "tradable": false,
            "imageName": "OmegaMod.png"
      },
      {
            "uniqueName": "/Lotus/Upgrades/Mods/Randomized/RawModularMeleeRandomMod",
            "name": "Zaw Riven Mod",
            "category": "Mods",
            "type": "Zaw Riven Mod",
            "tradable": false,
            "imageName": "OmegaMod.png"
      }
    ]"#;

    pub(crate) const RIFLE_RIVEN: &str = "/Lotus/Upgrades/Mods/Randomized/LotusRifleRandomModRare";

    pub(crate) const MELEE_RIVEN: &str =
        "/Lotus/Upgrades/Mods/Randomized/PlayerMeleeWeaponRandomModRare";

    pub(crate) const MIDDLE_ROLL: i64 = 536_870_910;

    pub(crate) const BEST_ROLL: i64 = 1_073_741_820;

    pub(crate) fn paid_roll() -> String {
        CURRENT_ROLL.replace("\"rerolls\":74", "\"rerolls\":75")
    }

    pub(crate) fn riven_table() -> RivenTable {
        serde_json::from_str::<wf_market::RivenData>(RIVEN_DATA).expect("riven data")
    }

    pub(crate) fn table_weapon_named(table: &RivenTable, name: &str) -> RivenWeapon {
        table
            .weapons
            .iter()
            .find(|weapon| weapon.name == name)
            .unwrap()
            .clone()
    }

    pub(crate) fn rolled(compat: &str, buffs: &[&str], curse: Option<&str>) -> String {
        let stats = |tags: &[&str]| {
            tags.iter()
                .map(|tag| format!(r#"{{"Tag":"{tag}","Value":{MIDDLE_ROLL}}}"#))
                .collect::<Vec<String>>()
                .join(",")
        };
        let curses = curse.map(|tag| stats(&[tag])).unwrap_or_default();
        format!(
            r#"{{"compat":"{compat}","lvlReq":16,"lvl":8,"rerolls":1,"pol":"AP_DEFENSE","buffs":[{}],"curses":[{curses}]}}"#,
            stats(buffs)
        )
    }

    pub(crate) struct Fixture {
        pub(crate) catalog: Catalog,
        pub(crate) items: ItemTable,
        pub(crate) table: RivenTable,
    }

    impl Fixture {
        pub(crate) fn grader(&self) -> Grader<'_> {
            Grader::new(&self.catalog, &self.items, Some(&self.table))
        }

        pub(crate) fn disposition_of(&self, weapon_path: &str) -> Option<f64> {
            self.catalog
                .item(weapon_path)
                .and_then(|item| item.omega_attenuation)
        }
    }

    pub(crate) fn fixture() -> Fixture {
        let catalog = fixtures::catalog_with(fixtures::RIVEN_ITEMS, PRE_VEILED_ITEMS);
        Fixture {
            items: ItemTable::build(&catalog),
            catalog,
            table: riven_table(),
        }
    }

    pub(crate) fn riven_tab() -> RivensTab {
        fixture()
            .grader()
            .tab(&fixtures::inventory(), &MarketListings::default())
    }

    pub(crate) fn by_weapon<'a>(rows: &'a [RivenRow], weapon: &str) -> &'a RivenRow {
        rows.iter().find(|row| row.weapon == weapon).unwrap()
    }
}
