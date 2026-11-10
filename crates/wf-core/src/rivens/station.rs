use wf_data::{Item, RivenType};
use wf_inventory::{Inventory, RivenFingerprint};
use wf_market::{RivenData as RivenTable, RivenWeapon};

use super::grading::compat_path;
use super::tab::weapon_class;
use super::{Grader, RivenRow};
use crate::catalog::Catalog;

fn contains_word(text: &str, word: &str) -> bool {
    let text = text.to_lowercase();
    let word = word.to_lowercase();
    let boundary = |byte: Option<&u8>| match byte {
        Some(byte) => !byte.is_ascii_alphanumeric(),
        None => true,
    };
    text.match_indices(&word).any(|(start, found)| {
        let before = start.checked_sub(1).and_then(|at| text.as_bytes().get(at));
        boundary(before) && boundary(text.as_bytes().get(start + found.len()))
    })
}

pub(super) fn family_named<'a>(name: &str, table: &'a RivenTable) -> Option<&'a RivenWeapon> {
    table
        .weapons
        .iter()
        .filter(|weapon| contains_word(name, &weapon.name))
        .max_by_key(|weapon| weapon.name.len())
}

fn is_variant(
    item: &Item,
    family_path: &str,
    family_name: &str,
    table: Option<&RivenTable>,
) -> bool {
    if item.unique_name == family_path {
        return true;
    }
    contains_word(&item.name, family_name)
        && match table {
            Some(table) => {
                family_named(&item.name, table).is_some_and(|family| family.game_ref == family_path)
            }
            None => true,
        }
}

fn owned_variant<'a>(
    family_path: &str,
    family_name: &str,
    inventory: &Inventory,
    catalog: &'a Catalog,
    table: Option<&RivenTable>,
) -> Option<&'a Item> {
    [
        &inventory.long_guns,
        &inventory.pistols,
        &inventory.melee,
        &inventory.space_guns,
        &inventory.sentinel_weapons,
    ]
    .into_iter()
    .flatten()
    .filter_map(|equipment| catalog.item(&equipment.item_type))
    .find(|item| is_variant(item, family_path, family_name, table))
}

impl Grader<'_> {
    pub(super) fn disposition_shown(
        &self,
        family_path: &str,
        family_name: &str,
        family_disposition: Option<f64>,
        inventory: Option<&Inventory>,
    ) -> (Option<f64>, Option<String>) {
        let variant = inventory
            .and_then(|inventory| {
                owned_variant(
                    family_path,
                    family_name,
                    inventory,
                    self.catalog,
                    self.table,
                )
            })
            .filter(|item| item.omega_attenuation.is_some());
        match variant {
            Some(item) => (item.omega_attenuation, Some(item.name.clone())),
            None => (family_disposition, None),
        }
    }

    pub(super) fn identity(
        &self,
        weapon: &RivenWeapon,
        riven_type: Option<&RivenType>,
        inventory: Option<&Inventory>,
    ) -> RivenRow {
        let item = self.catalog.item(&weapon.game_ref);
        let (disposition, disposition_weapon) = self.disposition_shown(
            &weapon.game_ref,
            &weapon.name,
            item.and_then(|item| item.omega_attenuation),
            inventory,
        );
        RivenRow {
            item_id: String::new(),
            item_type: weapon.mod_type.clone(),
            riven_type: riven_type.map(|found| found.name.clone()),
            weapon_class: riven_type.map(|found| weapon_class(&found.name)),
            name: None,
            weapon: weapon.name.clone(),
            weapon_path: Some(weapon.game_ref.clone()),
            weapon_slug: Some(weapon.slug.clone()),
            image_name: item.and_then(|item| item.image_name.clone()),
            disposition,
            disposition_weapon,
            unveiled: false,
            rank: 0,
            rank_required: None,
            rerolls: 0,
            polarity: None,
            grade: 0.0,
            grade_letter: "D",
            attributes: Vec::new(),
            splices: Vec::new(),
            good_roll: None,
            listed_in_wfm: false,
            unlisted_stat: None,
            pending: None,
        }
    }

    pub(super) fn roll(
        &self,
        identity: RivenRow,
        fingerprint: &RivenFingerprint,
        riven_type: Option<&RivenType>,
    ) -> RivenRow {
        let shown = self.pending_from(fingerprint, riven_type, identity.disposition);
        RivenRow {
            unveiled: fingerprint.is_unveiled(),
            rank: fingerprint.lvl,
            rank_required: fingerprint.lvl_req,
            ..identity.with_roll(shown)
        }
    }

    pub(crate) fn at_station(
        &self,
        item_id: &str,
        shown: &str,
        offered: Option<&str>,
        inventory: Option<&Inventory>,
    ) -> Option<RivenRow> {
        let shown = RivenFingerprint::parse(shown).ok()?;
        let offered = offered.map(RivenFingerprint::parse).transpose().ok()?;
        let weapon = compat_weapon(self.table?, shown.compat.as_deref()?)?;
        let riven_type = self.riven_type(&weapon.mod_type);
        let identity = self.identity(weapon, riven_type, inventory);
        let row = self.roll(identity, &shown, riven_type);
        Some(RivenRow {
            item_id: item_id.to_owned(),
            pending: offered
                .map(|offered| self.pending_from(&offered, riven_type, row.disposition)),
            ..row
        })
    }
}

pub(super) fn compat_weapon<'a>(table: &'a RivenTable, compat: &str) -> Option<&'a RivenWeapon> {
    let path = compat_path(compat);
    table.weapons.iter().find(|weapon| weapon.game_ref == path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::fixtures;
    use crate::listings::MarketListings;
    use crate::rivens::support::*;
    use crate::rivens::tab::display_name;

    const STATION_ITEM: &str = "19afb0d351ebb3350bab634c";

    fn at_station(shown: &str, offered: Option<&str>) -> Option<RivenRow> {
        fixture()
            .grader()
            .at_station(STATION_ITEM, shown, offered, None)
    }

    #[test]
    fn selected_riven_fingerprint() {
        let row = at_station(CURRENT_ROLL, None).unwrap();

        assert_eq!(row.item_id, STATION_ITEM);
        assert_eq!(row.weapon, "Nepheri");
        assert_eq!(row.weapon_slug.as_deref(), Some("nepheri"));
        assert_eq!(row.disposition, Some(1.0));
        assert_eq!(row.name.as_deref(), Some("Geli-toxidra"));
        assert_eq!(display_name(&row).as_deref(), Some("Nepheri Geli-toxidra"));
        assert_eq!(row.rerolls, 74);
        assert_eq!(row.rank, 8);
        assert_eq!(row.rank_required, Some(11));
        assert!(row.unveiled);
        assert!(row.pending.is_none());
        assert!(row.grade > 0.0);
    }

    #[test]
    fn cycled_station_second_roll() {
        let row = at_station(&paid_roll(), Some(PENDING_ROLL)).unwrap();

        assert_eq!(row.rerolls, 75);
        assert_eq!(row.name.as_deref(), Some("Geli-toxidra"));
        let pending = row.pending.unwrap();
        assert_eq!(pending.rerolls, 75);
        assert_eq!(pending.name.as_deref(), Some("Acri-gelipha"));
        assert_eq!(
            pending.attributes.iter().filter(|stat| stat.curse).count(),
            1
        );
        assert!(pending.grade > 0.0);
    }

    #[test]
    fn ungradable_rolls() {
        assert!(at_station("not json", None).is_none());
        assert!(at_station(CURRENT_ROLL, Some("{")).is_none());
        assert!(at_station(r#"{"rerolls":3,"buffs":[],"curses":[]}"#, None).is_none());
        let elsewhere = CURRENT_ROLL.replace(ARCHON_DAGGERS, OTHER_WEAPON);
        assert!(at_station(&elsewhere, None).is_none());
    }

    #[test]
    fn weapon_absent_from_inventory() {
        let fixture = fixture();
        let grader = fixture.grader();
        let acceltra = table_weapon_named(&fixture.table, "Acceltra");
        let roll = rolled(
            &acceltra.game_ref,
            &[
                "WeaponCritDamageMod",
                "WeaponFireIterationsMod",
                "WeaponDamageAmountMod",
            ],
            None,
        );
        assert!(
            grader
                .rows(&fixtures::inventory(), &MarketListings::default())
                .iter()
                .all(|row| row.weapon_path.as_deref() != Some(acceltra.game_ref.as_str()))
        );

        let row = grader.at_station(STATION_ITEM, &roll, None, None).unwrap();

        assert_eq!(row.weapon, "Acceltra");
        assert_eq!(row.weapon_path.as_deref(), Some(acceltra.game_ref.as_str()));
        assert_eq!(row.weapon_slug.as_deref(), Some("acceltra"));
        assert_eq!(row.item_type, acceltra.mod_type);
        assert_eq!(row.weapon_class.as_deref(), Some("Rifle"));
        assert_eq!(row.disposition, None);
        assert_eq!(row.rerolls, 1);
        assert_eq!(row.rank, 8);
        assert_eq!(row.rank_required, Some(16));
        assert!(row.unveiled);
        let view = row.good_roll.as_ref().unwrap();
        assert!(view.matches);
        assert!(view.alternatives[0].complete);
        assert_eq!(view.accepted_bad.len(), 2);
        assert!(view.accepted_bad.iter().all(|stat| !stat.matches));
    }

    #[test]
    fn compat_path_weapon() {
        let table = riven_table();
        let acceltra = table_weapon_named(&table, "Acceltra");
        assert_eq!(
            compat_weapon(&table, &acceltra.game_ref).map(|weapon| weapon.name.as_str()),
            Some("Acceltra")
        );
        assert_eq!(compat_weapon(&table, OTHER_WEAPON), None);
    }

    #[test]
    fn variant_family_matching() {
        let table = riven_table();
        assert!(contains_word("Braton Prime", "Braton"));
        assert!(contains_word("MK1-Braton", "braton"));
        assert!(!contains_word("Akbronco Prime", "Bronco"));
        assert_eq!(
            family_named("Secura Dual Cestra", &table).map(|weapon| weapon.name.as_str()),
            Some("Dual Cestra")
        );
        assert_eq!(
            family_named("Kuva Karak", &table).map(|weapon| weapon.name.as_str()),
            Some("Karak")
        );
    }
}
