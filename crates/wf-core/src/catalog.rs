use std::collections::HashMap;

use serde::Serialize;

use wf_data::{Component, GameData, Item, Refinement, Relic};

use crate::error::Result;
use crate::identity::{blueprint_of, component_of};

pub(crate) const RECIPE_PREFIX: &str = "/Lotus/Types/Recipes/";
pub(crate) const ARCANE_PREFIX: &str = "/Lotus/Upgrades/CosmeticEnhancers/";
pub(crate) const RELIC_PREFIX: &str = wf_inventory::RELIC_PREFIX;
pub(crate) const DUCATS_ITEM: &str = "/Lotus/Types/Items/MiscItems/PrimeBucks";
pub(crate) const REQUIEM_MARKER: &str = "Requiem";
pub(crate) const FORMA_ITEM: &str = "/Lotus/Types/Items/MiscItems/Forma";
pub(crate) const FORMA_BLUEPRINT: &str = "/Lotus/Types/Recipes/Components/FormaBlueprint";

pub struct Catalog {
    data: GameData,
}

impl Catalog {
    pub fn new(data: GameData) -> Self {
        Self { data }
    }

    pub fn from_json(items_json: &str, relics_json: &str, components_json: &str) -> Result<Self> {
        Ok(Self::new(GameData::from_json(
            items_json,
            relics_json,
            components_json,
        )?))
    }

    pub fn data(&self) -> &GameData {
        &self.data
    }

    pub fn item(&self, unique_name: &str) -> Option<&Item> {
        self.data.by_unique_name(unique_name)
    }

    pub fn component(&self, unique_name: &str) -> Option<(&Item, &Component)> {
        self.data.component_by_unique_name(unique_name)
    }

    pub(crate) fn recipe(&self, unique_name: &str) -> Option<&Item> {
        self.item(unique_name)
            .filter(|item| item.components.is_some())
    }

    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.data.items().iter().filter(|item| !item.is_skin())
    }

    pub fn skins(&self) -> impl Iterator<Item = &Item> {
        self.data.items().iter().filter(|item| item.is_skin())
    }

    pub fn relics(&self) -> impl Iterator<Item = &Relic> {
        self.data.relics().iter()
    }

    pub fn component_for_reward(&self, unique_name: &str) -> Option<(&Item, &Component)> {
        if self.item(unique_name).is_some() {
            return None;
        }
        if let Some(found) = self.component(unique_name) {
            return Some(found);
        }

        self.component(&component_of(unique_name)?)
    }

    pub fn component_for_stock(&self, unique_name: &str) -> Option<(&Item, &Component)> {
        if let Some(found) = self.component_for_reward(unique_name) {
            return Some(found);
        }
        let base = unique_name.strip_suffix("Blueprint")?;
        self.component(base)
    }

    pub fn relic_by_unique_name(&self, unique_name: &str) -> Option<(&Relic, Refinement)> {
        self.data.relic_by_unique_name(unique_name)
    }

    pub fn prime_parts(&self) -> impl Iterator<Item = (&Item, &Component)> {
        self.items()
            .filter(|item| names_a_prime(&item.name))
            .flat_map(|item| {
                item.components
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .filter(|component| is_part(component))
                    .map(move |component| (item, component))
            })
    }
}

pub(crate) struct Stock {
    counts: HashMap<String, i64>,
}

impl Stock {
    pub fn new(inventory: &wf_inventory::Inventory) -> Self {
        Self {
            counts: inventory
                .counted_index()
                .into_iter()
                .map(|(item_type, count)| (item_type.to_owned(), count))
                .collect(),
        }
    }

    pub fn count(&self, unique_name: &str) -> i64 {
        resolve_count(unique_name, |item_type| {
            self.counts.get(item_type).copied().unwrap_or(0)
        })
    }
}

fn resolve_count(unique_name: &str, counted: impl Fn(&str) -> i64) -> i64 {
    if unique_name == FORMA_BLUEPRINT {
        return forma_count(counted);
    }

    let held = aliased_count(unique_name, &counted);
    if held > 0 {
        return held;
    }

    let alias = blueprint_of(unique_name)
        .or_else(|| component_of(unique_name))
        .unwrap_or_else(|| format!("{unique_name}Blueprint"));
    counted(&alias)
}

pub(crate) fn forma_count(counted: impl Fn(&str) -> i64) -> i64 {
    counted(FORMA_ITEM) + counted(FORMA_BLUEPRINT)
}

pub(crate) fn aliased_count(unique_name: &str, counted: impl Fn(&str) -> i64) -> i64 {
    let direct = counted(unique_name);
    if builds_from_its_own_blueprint(unique_name) {
        return direct + counted(&format!("{unique_name}Blueprint"));
    }

    direct
}

fn builds_from_its_own_blueprint(unique_name: &str) -> bool {
    unique_name.contains("/WeaponParts/")
        && !unique_name.contains("Prime")
        && !unique_name.ends_with("Blueprint")
        && !unique_name.ends_with("Component")
}

pub(crate) fn component_image(item: &Item, component: &Component) -> Option<String> {
    match component.image_name.as_deref() {
        Some(image) if image != "blueprint.png" => Some(image.to_owned()),
        _ => item
            .image_name
            .clone()
            .or_else(|| component.image_name.clone()),
    }
}

pub(crate) fn is_umbra(item: &Item) -> bool {
    item.name.ends_with(" Umbra")
}

pub(crate) fn names_a_prime(name: &str) -> bool {
    name.contains("Prime")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum VaultStatus {
    Vaulted,
    Available,
    Unknown,
}

impl From<Option<bool>> for VaultStatus {
    fn from(vaulted: Option<bool>) -> Self {
        match vaulted {
            Some(true) => Self::Vaulted,
            Some(false) => Self::Available,
            None => Self::Unknown,
        }
    }
}

pub(crate) fn is_fish(unique_name: &str) -> bool {
    unique_name.contains("/Items/Fish/")
}

pub(crate) fn is_part(component: &Component) -> bool {
    component.unique_name.starts_with(RECIPE_PREFIX)
}

pub(crate) fn part_name(item: &Item, component: &Component) -> String {
    let set = item.name.split(' ').next().unwrap_or(&item.name);
    if component.name.starts_with(set) {
        component.name.clone()
    } else {
        format!("{} {}", item.name, component.name)
    }
}

pub(crate) fn display_name_from_path(path: &str) -> String {
    let tail = path.rsplit_once('/').map_or(path, |(_, tail)| tail);
    let mut out = String::with_capacity(tail.len() + 8);
    for (index, character) in tail.char_indices() {
        if index > 0 && character.is_ascii_uppercase() {
            out.push(' ');
        }
        out.push(character);
    }
    out
}

#[cfg(test)]
pub mod fixtures {
    use std::path::PathBuf;

    use chrono::{DateTime, Utc};
    use serde_json::{Value, json};
    use wf_inventory::Inventory;

    use super::Catalog;

    pub const ITEMS: &str = include_str!("../../wf-data/tests/fixtures/items.json");
    pub const RELICS: &str = include_str!("../../wf-data/tests/fixtures/relics.json");
    pub const INVENTORY: &str = include_str!("../../../fixtures/inventory.json");
    pub const MASTERY_ITEMS: &str = include_str!("../../../fixtures/mastery_items.json");
    pub const FOUNDRY_ITEMS: &str = include_str!("../../wf-data/tests/fixtures/foundry_items.json");
    pub const RIVEN_ITEMS: &str = include_str!("../../wf-data/tests/fixtures/riven_items.json");
    pub const SKINS: &str = include_str!("../../wf-data/tests/fixtures/skins.json");
    pub const COMPONENTS: &str = include_str!("../../wf-data/tests/fixtures/components.json");
    pub const UPGRADE_ITEMS: &str = include_str!("../../../fixtures/upgrade_items.json");
    pub const MISC_ITEMS: &str = include_str!("../../../fixtures/misc_items.json");
    pub const WORLD_STATE: &str = include_str!("../../../fixtures/worldState.json");
    pub const ARBITRATIONS: &str = include_str!("../../../fixtures/arbitrations.txt");
    pub const ARBITRATION_TIERS: &str = include_str!("../../../fixtures/arbitration_tiers.js");
    pub const MY_AUCTIONS: &str = include_str!("../../wf-market/tests/fixtures/auctions_my.json");

    pub const BRATON_PRIME: &str = "/Lotus/Weapons/Tenno/Rifle/BratonPrime";
    pub const BRATON_PRIME_BLUEPRINT: &str = "/Lotus/Types/Recipes/Weapons/BratonPrimeBlueprint";
    pub const BRATON_PRIME_BARREL: &str =
        "/Lotus/Types/Recipes/Weapons/WeaponParts/BratonPrimeBarrel";
    pub const BRATON_PRIME_STOCK: &str =
        "/Lotus/Types/Recipes/Weapons/WeaponParts/BratonPrimeStock";
    pub const TRINITY_PRIME_SYSTEMS: &str =
        "/Lotus/Types/Recipes/WarframeRecipes/TrinityPrimeSystemsComponent";
    pub const TRINITY_PRIME_SYSTEMS_BLUEPRINT: &str =
        "/Lotus/Types/Recipes/WarframeRecipes/TrinityPrimeSystemsBlueprint";
    pub const EXCALIBUR: &str = "/Lotus/Powersuits/Excalibur/Excalibur";
    pub const DUAL_KAMAS: &str = "/Lotus/Weapons/Tenno/Melee/DualKamas/DualKamas";
    pub const SINGLE_KAMA: &str = "/Lotus/Weapons/Tenno/Melee/DualKamas/SingleKama";
    pub const OROKIN_CELL: &str = "/Lotus/Types/Items/MiscItems/OrokinCell";
    pub const FERRITE: &str = "/Lotus/Types/Items/MiscItems/Ferrite";

    pub const PRIME_PARTS: [(&str, i64); 3] = [
        (BRATON_PRIME_BARREL, 3),
        (BRATON_PRIME_STOCK, 1),
        (TRINITY_PRIME_SYSTEMS_BLUEPRINT, 1),
    ];

    const SYNC_OID: &str = "6a9eeb1f000000000000c001";

    pub fn at(millis: i64) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(millis).unwrap()
    }

    pub fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }

        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    pub fn catalog() -> Catalog {
        Catalog::from_json(ITEMS, RELICS, COMPONENTS).unwrap()
    }

    pub fn catalog_of(items_json: &str) -> Catalog {
        Catalog::from_json(items_json, RELICS, "[]").unwrap()
    }

    pub fn landing_craft_catalog() -> Catalog {
        Catalog::from_json(
            r#"[
              {"uniqueName":"/Lotus/Types/Items/Ships/InsectShip","name":"Mantis",
               "category":"Misc","type":"Orbiter","tradable":false,"imageName":"Mantis.png",
               "components":[
                 {"uniqueName":"/Lotus/Types/Recipes/LandingCraftRecipes/Mantys/MantysStarChartComponent","itemCount":1},
                 {"uniqueName":"/Lotus/Types/Recipes/LandingCraftRecipes/Mantys/MantysPowerCoreComponent","itemCount":1},
                 {"uniqueName":"/Lotus/Types/Recipes/LandingCraftRecipes/Mantys/MantysBlueprint","itemCount":1},
                 {"uniqueName":"/Lotus/Types/Items/MiscItems/OrokinCell","itemCount":1}
               ]},
              {"uniqueName":"/Lotus/Types/Items/Ships/GyroscopeShip","name":"Xiphos",
               "category":"Misc","type":"Orbiter","tradable":false,"imageName":"Xiphos.png",
               "components":[
                 {"uniqueName":"/Lotus/Types/Recipes/LandingCraftRecipes/Gyroscope/GyroscopeFuselageComponent","itemCount":1},
                 {"uniqueName":"/Lotus/Types/Items/MiscItems/OrokinCell","itemCount":1}
               ]},
              {"uniqueName":"/Lotus/Types/Items/Ships/NoraShip","name":"Nightwave",
               "category":"Misc","type":"Orbiter","tradable":false,"imageName":"Nightwave.png",
               "components":[
                 {"uniqueName":"/Lotus/Types/Recipes/LandingCraftRecipes/NightwaveShip/NoraShipFuselageComponent","itemCount":1},
                 {"uniqueName":"/Lotus/Types/Items/MiscItems/OrokinCell","itemCount":1}
               ]},
              {"uniqueName":"/Lotus/Types/Items/MiscItems/OrokinCell","name":"Orokin Cell",
               "category":"Misc","type":"Resource","tradable":false}
            ]"#,
            RELICS,
            r#"[
              {"uniqueName":"/Lotus/Types/Recipes/LandingCraftRecipes/Mantys/MantysStarChartComponent",
               "name":"Avionics","tradable":false},
              {"uniqueName":"/Lotus/Types/Recipes/LandingCraftRecipes/Mantys/MantysPowerCoreComponent",
               "name":"Engines","tradable":false},
              {"uniqueName":"/Lotus/Types/Recipes/LandingCraftRecipes/Mantys/MantysBlueprint",
               "name":"Blueprint","tradable":false},
              {"uniqueName":"/Lotus/Types/Recipes/LandingCraftRecipes/Gyroscope/GyroscopeFuselageComponent",
               "name":"Fuselage","tradable":false},
              {"uniqueName":"/Lotus/Types/Recipes/LandingCraftRecipes/NightwaveShip/NoraShipFuselageComponent",
               "name":"Fuselage","tradable":false}
            ]"#,
        )
        .unwrap()
    }

    pub fn catalog_with(items_json: &str, extra_json: &str) -> Catalog {
        let mut merged: Vec<Value> = serde_json::from_str(items_json).unwrap();
        let extra: Vec<Value> = serde_json::from_str(extra_json).unwrap();
        merged.extend(extra);
        Catalog::from_json(&Value::from(merged).to_string(), RELICS, COMPONENTS).unwrap()
    }

    pub fn with_skins(items_json: &str) -> Catalog {
        catalog_with(items_json, SKINS)
    }

    pub fn upgrade_catalog() -> Catalog {
        catalog_of(UPGRADE_ITEMS)
    }

    pub fn foundry_catalog() -> Catalog {
        Catalog::from_json(FOUNDRY_ITEMS, RELICS, COMPONENTS).unwrap()
    }

    pub fn mastery_catalog() -> Catalog {
        Catalog::from_json(MASTERY_ITEMS, RELICS, COMPONENTS).unwrap()
    }

    pub fn misc_catalog() -> Catalog {
        Catalog::from_json(MISC_ITEMS, RELICS, COMPONENTS).unwrap()
    }

    pub fn market_item(slug: &str, name: &str, game_ref: &str, tags: &[&str]) -> wf_market::Item {
        serde_json::from_value(json!({
            "id": slug,
            "slug": slug,
            "gameRef": game_ref,
            "tags": tags,
            "i18n": {"en": {"name": name, "icon": "", "thumb": ""}},
        }))
        .unwrap()
    }

    pub fn inventory() -> Inventory {
        Inventory::parse(INVENTORY).unwrap()
    }

    pub fn inventory_with(edit: impl FnOnce(&mut Value)) -> Inventory {
        let mut value: Value = serde_json::from_str(INVENTORY).unwrap();
        edit(&mut value);
        Inventory::parse(&value.to_string()).unwrap()
    }

    pub fn entries<'a>(value: &'a mut Value, key: &str) -> &'a mut Vec<Value> {
        value.get_mut(key).and_then(Value::as_array_mut).unwrap()
    }

    pub fn push_counted(value: &mut Value, key: &str, item_type: &str, count: i64) {
        entries(value, key).push(json!({ "ItemType": item_type, "ItemCount": count }));
    }

    pub fn drop_entries(value: &mut Value, keys: &[&str], item_type: &str) {
        for key in keys {
            entries(value, key)
                .retain(|entry| entry.get("ItemType").and_then(Value::as_str) != Some(item_type));
        }
    }

    pub fn synced(oid: &str) -> String {
        INVENTORY.replacen(SYNC_OID, oid, 1)
    }

    pub fn founder_inventory() -> Inventory {
        inventory_with(|value| value["Accolades"] = json!({ "Founder": 4 }))
    }

    pub fn inventory_owning(counted: &[(&str, i64)]) -> Inventory {
        inventory_with(|value| {
            for (item_type, count) in counted {
                push_counted(value, "MiscItems", item_type, *count);
            }
        })
    }

    pub fn inventory_stocked(
        equipment: &[(&str, &str)],
        counted: &[(&str, &str, i64)],
    ) -> Inventory {
        inventory_with(|value| {
            for key in [
                "Suits",
                "LongGuns",
                "Pistols",
                "Melee",
                "MiscItems",
                "Recipes",
            ] {
                value[key] = json!([]);
            }

            for (index, (category, item_type)) in equipment.iter().enumerate() {
                entries(value, category).push(json!({
                    "ItemType": item_type,
                    "ItemId": { "$oid": format!("{index:024x}") },
                    "XP": 0,
                }));
            }

            for (category, item_type, count) in counted {
                push_counted(value, category, item_type, *count);
            }
        })
    }

    pub fn inventory_without_equipment(item_type: &str) -> Inventory {
        inventory_with(|value| {
            drop_entries(value, &["Suits", "LongGuns", "Pistols", "Melee"], item_type);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::{BRATON_PRIME_BARREL, TRINITY_PRIME_SYSTEMS, TRINITY_PRIME_SYSTEMS_BLUEPRINT};
    use wf_data::Refinement;

    #[test]
    fn fixture_counts() {
        let catalog = fixtures::catalog();
        assert_eq!(catalog.items().count(), 5);
        assert_eq!(catalog.relics().count(), 2);
        assert_eq!(catalog.skins().count(), 0);
    }

    #[test]
    fn skins_kept_apart() {
        let catalog = fixtures::with_skins(fixtures::ITEMS);
        assert_eq!(catalog.items().count(), 10);
        assert_eq!(catalog.skins().count(), 13);
        assert!(catalog.items().all(|item| !item.is_skin()));
        assert!(
            catalog
                .skins()
                .any(|skin| skin.name == "Kavasa Prime Kubrow Collar")
        );

        let helmet = catalog
            .item("/Lotus/Upgrades/Skins/Brawler/BrawlerAltHelmet")
            .expect("helmet skin");
        assert_eq!(helmet.name, "Atlas Tartarus Helmet");
        assert!(helmet.components.is_some());
        assert!(!helmet.masterable());
        assert!(helmet.warframe_market.is_none());
    }

    #[test]
    fn relic_by_unique_name() {
        let catalog = fixtures::catalog();
        let (relic, refinement) = catalog
            .relic_by_unique_name(
                "/Lotus/Types/Game/Projections/T4VoidProjectionStyanaxPrimeABronze",
            )
            .unwrap();
        assert_eq!(relic.name, "Axi A21");
        assert_eq!(refinement, Refinement::Intact);
        assert!(!relic.vaulted);
    }

    #[test]
    fn prime_parts_skip_resources() {
        let catalog = fixtures::catalog();
        let parts: Vec<String> = catalog
            .prime_parts()
            .map(|(item, component)| part_name(item, component))
            .collect();
        assert!(parts.contains(&"Trinity Prime Systems".to_owned()));
        assert!(parts.contains(&"Braton Prime Barrel".to_owned()));
        assert!(!parts.iter().any(|n| n.contains("Orokin Cell")));
        assert!(!parts.iter().any(|n| n.starts_with("Excalibur")));

        let with_skins = fixtures::with_skins(fixtures::ITEMS);
        let skinned: Vec<String> = with_skins
            .prime_parts()
            .map(|(item, component)| part_name(item, component))
            .collect();
        assert_eq!(skinned, parts);
    }

    #[test]
    fn stock_matches_owned_count() {
        let inventory = fixtures::inventory_owning(&[
            (TRINITY_PRIME_SYSTEMS_BLUEPRINT, 2),
            (BRATON_PRIME_BARREL, 3),
        ]);
        let stock = Stock::new(&inventory);
        for unique_name in [
            TRINITY_PRIME_SYSTEMS,
            TRINITY_PRIME_SYSTEMS_BLUEPRINT,
            BRATON_PRIME_BARREL,
            fixtures::FERRITE,
            fixtures::OROKIN_CELL,
            "/Lotus/Types/Items/MiscItems/DoesNotExist",
        ] {
            assert_eq!(
                stock.count(unique_name),
                resolve_count(unique_name, |item_type| inventory.counted(item_type)),
                "{unique_name}"
            );
        }
        assert_eq!(stock.count(TRINITY_PRIME_SYSTEMS), 2);
    }

    #[test]
    fn item_doubling_as_ingredient() {
        let items = r#"[
            {"uniqueName": "/Lotus/Types/Items/MiscItems/Forma", "name": "Forma",
             "category": "Misc", "type": "Misc", "tradable": false, "imageName": "forma.png"},
            {"uniqueName": "/Lotus/Types/Items/MiscItems/FormaUmbra", "name": "Umbra Forma",
             "category": "Misc", "type": "Misc", "tradable": false,
             "components": [{"uniqueName": "/Lotus/Types/Items/MiscItems/Forma", "itemCount": 1}]}
        ]"#;
        let catalog = fixtures::catalog_of(items);
        let forma = "/Lotus/Types/Items/MiscItems/Forma";
        assert!(catalog.component(forma).is_some());
        assert!(catalog.component_for_reward(forma).is_none());
    }

    #[test]
    fn forma_stock() {
        let inventory = fixtures::inventory();
        let stock = Stock::new(&inventory);
        assert_eq!(inventory.counted(FORMA_ITEM), 65);
        assert_eq!(inventory.counted(FORMA_BLUEPRINT), 25);
        assert_eq!(stock.count(FORMA_BLUEPRINT), 90);
        assert_eq!(stock.count(FORMA_ITEM), 65);
        assert_eq!(
            stock.count("/Lotus/Types/Recipes/Components/FormaAuraBlueprint"),
            53
        );
    }

    #[test]
    fn non_prime_part_adds_blueprints() {
        let inventory = fixtures::inventory_owning(&[
            ("/Lotus/Types/Recipes/Weapons/WeaponParts/BoltorBarrel", 2),
            (
                "/Lotus/Types/Recipes/Weapons/WeaponParts/BoltorBarrelBlueprint",
                3,
            ),
            (BRATON_PRIME_BARREL, 4),
            (
                "/Lotus/Types/Recipes/Weapons/WeaponParts/BratonPrimeBarrelBlueprint",
                5,
            ),
        ]);
        let stock = Stock::new(&inventory);
        assert_eq!(
            stock.count("/Lotus/Types/Recipes/Weapons/WeaponParts/BoltorBarrel"),
            5
        );
        assert_eq!(
            stock.count("/Lotus/Types/Recipes/Weapons/WeaponParts/BoltorBarrelBlueprint"),
            3
        );
        assert_eq!(stock.count(BRATON_PRIME_BARREL), 4);
    }

    #[test]
    fn generic_blueprint_image() {
        let catalog = fixtures::catalog();
        let (item, blueprint) = catalog.component(fixtures::BRATON_PRIME_BLUEPRINT).unwrap();
        assert_eq!(blueprint.image_name.as_deref(), Some("blueprint.png"));
        assert_eq!(component_image(item, blueprint), item.image_name);
        let (item, barrel) = catalog.component(BRATON_PRIME_BARREL).unwrap();
        assert_eq!(component_image(item, barrel), barrel.image_name);
    }

    #[test]
    fn camel_case_display_name() {
        assert_eq!(
            display_name_from_path("/Lotus/Upgrades/Mods/Warframe/AvatarShieldMaxMod"),
            "Avatar Shield Max Mod"
        );
    }
}
