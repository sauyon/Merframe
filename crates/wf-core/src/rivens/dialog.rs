use wf_inventory::{Inventory, RivenFingerprint};

use super::station::compat_weapon;
use super::{Grader, RivenRow};

impl Grader<'_> {
    pub(crate) fn in_dialog(
        &self,
        mod_type: &str,
        fingerprint: &str,
        inventory: Option<&Inventory>,
    ) -> Option<RivenRow> {
        let fingerprint = RivenFingerprint::parse(fingerprint).ok()?;
        let weapon = compat_weapon(self.table?, fingerprint.compat.as_deref()?)?;
        let riven_type = self.riven_type(mod_type);
        let identity = RivenRow {
            item_type: mod_type.to_owned(),
            ..self.identity(weapon, riven_type, inventory)
        };
        Some(self.roll(identity, &fingerprint, riven_type))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Catalog, fixtures};
    use crate::identity::ItemTable;
    use crate::listings::MarketListings;
    use crate::rivens::grading::riven_name;
    use crate::rivens::support::*;
    use crate::rivens::tab::display_name;

    const BRATONS: &str = r#"[
        {"uniqueName":"/Lotus/Weapons/Tenno/Rifle/Rifle","name":"Braton","category":"Primary","productCategory":"LongGuns","type":"Rifle","tradable":false,"omegaAttenuation":1.35},
        {"uniqueName":"/Lotus/Weapons/Tenno/Rifle/BratonPrime","name":"Braton Prime","category":"Primary","productCategory":"LongGuns","type":"Rifle","tradable":false,"omegaAttenuation":1.25},
        {"uniqueName":"/Lotus/Weapons/Tenno/Rifle/StartingRifle","name":"MK1-Braton","category":"Primary","productCategory":"LongGuns","type":"Rifle","tradable":false,"omegaAttenuation":1.35}
    ]"#;

    fn generated_name(catalog: &Catalog, mod_type: &str, weapon: &str, roll: &str) -> String {
        let riven_type = catalog
            .data()
            .riven_data()
            .riven_type(mod_type)
            .expect("riven type");
        let fingerprint = RivenFingerprint::parse(roll).unwrap();
        let name = riven_name(riven_type, &fingerprint.buffs).unwrap();
        format!("{weapon} {name}")
    }

    #[test]
    fn unowned_weapon_link() {
        let fixture = fixture();
        let acceltra = table_weapon_named(&fixture.table, "Acceltra");
        let roll = rolled(
            &acceltra.game_ref,
            &["WeaponCritDamageMod", "WeaponFireIterationsMod"],
            Some("WeaponAmmoMaxMod"),
        );
        let expected = generated_name(&fixture.catalog, &acceltra.mod_type, "Acceltra", &roll);

        let row = fixture
            .grader()
            .in_dialog(&acceltra.mod_type, &roll, None)
            .unwrap();

        assert!(row.item_id.is_empty());
        assert_eq!(row.item_type, acceltra.mod_type);
        assert_eq!(row.weapon, "Acceltra");
        assert_eq!(row.weapon_path.as_deref(), Some(acceltra.game_ref.as_str()));
        assert_eq!(row.weapon_slug.as_deref(), Some("acceltra"));
        assert_eq!(row.weapon_class.as_deref(), Some("Rifle"));
        assert_eq!(display_name(&row).as_deref(), Some(expected.as_str()));
        assert_eq!(row.rerolls, 1);
        assert_eq!(row.rank, 8);
        assert_eq!(row.rank_required, Some(16));
        assert!(row.unveiled);
        assert_eq!(row.attributes.len(), 3);
        assert!(row.attributes[2].curse);
        assert!(row.good_roll.is_some());
        assert!(row.pending.is_none());
    }

    #[test]
    fn stranger_link_grade() {
        let fixture = fixture();
        let grader = fixture.grader();
        let inventory = fixtures::inventory();
        let rows = grader.rows(&inventory, &MarketListings::default());
        let owned = rows
            .iter()
            .find(|row| {
                fixture
                    .table
                    .weapons
                    .iter()
                    .any(|weapon| Some(weapon.game_ref.as_str()) == row.weapon_path.as_deref())
            })
            .expect("unveiled riven of a table weapon");
        let fingerprint = inventory
            .rivens()
            .find(|upgrade| upgrade.item_id.as_str() == owned.item_id)
            .and_then(|upgrade| upgrade.upgrade_fingerprint.clone())
            .expect("fingerprint");

        let row = grader
            .in_dialog(&owned.item_type, &fingerprint, Some(&inventory))
            .unwrap();

        assert!(row.item_id.is_empty());
        assert_eq!(display_name(&row), display_name(owned));
        assert_eq!(row.attributes, owned.attributes);
        assert_eq!(row.disposition, owned.disposition);
        assert_eq!(row.rerolls, owned.rerolls);
    }

    #[test]
    fn owned_variant_disposition() {
        let table = riven_table();
        let braton = table_weapon_named(&table, "Braton");
        let roll = rolled(
            &braton.game_ref,
            &[
                "WeaponCritDamageMod",
                "WeaponCritChanceMod",
                "WeaponFireRateMod",
            ],
            None,
        );
        let catalog = fixtures::catalog_with(fixtures::RIVEN_ITEMS, BRATONS);
        let inventory = fixtures::inventory();
        let first_braton = inventory
            .long_guns
            .iter()
            .map(|weapon| weapon.item_type.as_str())
            .find(|item_type| {
                item_type.ends_with("/Rifle/Rifle")
                    || item_type.ends_with("/Rifle/BratonPrime")
                    || item_type.ends_with("/Rifle/StartingRifle")
            })
            .unwrap();
        assert!(first_braton.ends_with("BratonPrime"));

        let items = ItemTable::build(&catalog);
        let grader = Grader::new(&catalog, &items, Some(&table));
        let owned = grader
            .in_dialog(&braton.mod_type, &roll, Some(&inventory))
            .unwrap();
        assert_eq!(owned.weapon, "Braton");
        assert_eq!(owned.disposition, Some(1.25));
        assert_eq!(owned.disposition_weapon.as_deref(), Some("Braton Prime"));

        let bare = grader.in_dialog(&braton.mod_type, &roll, None).unwrap();
        assert_eq!(bare.disposition, Some(1.35));
        assert_eq!(bare.disposition_weapon, None);

        let crit = |row: &RivenRow| row.attributes[0].display.unwrap();
        assert!((crit(&owned) / crit(&bare) - 1.25 / 1.35).abs() < 1e-2);
    }

    #[test]
    fn ungradable_dialogs() {
        let fixture = fixture();
        let grader = fixture.grader();
        assert!(grader.in_dialog(MELEE_RIVEN, "not json", None).is_none());
        assert!(
            grader
                .in_dialog(MELEE_RIVEN, r#"{"rerolls":3,"buffs":[]}"#, None)
                .is_none()
        );
        let elsewhere = rolled(OTHER_WEAPON, &["WeaponCritDamageMod"], None);
        assert!(grader.in_dialog(MELEE_RIVEN, &elsewhere, None).is_none());
    }
}
