use std::collections::{HashMap, HashSet};

use crate::error::{DataError, Result};
use crate::item::{Component, Item, ItemRecord, Rarity};
use crate::relic::{Refinement, Relic, parse_relics};
use crate::riven::RivenData;

pub struct GameData {
    items: Vec<Item>,
    relics: Vec<Relic>,
    unique_name_index: HashMap<String, usize>,
    name_index: HashMap<String, usize>,
    component_index: HashMap<String, (usize, usize)>,
    relic_name_index: HashMap<String, usize>,
    relic_unique_name_index: HashMap<String, (usize, Refinement)>,
    riven_data: RivenData,
}

impl GameData {
    pub fn from_json(items: &str, relics: &str, components: &str) -> Result<Self> {
        Self::build(
            serde_json::from_str(items).map_err(|source| DataError::Parse("item", source))?,
            relics,
            components,
        )
    }

    pub fn from_json_parts(items: &[String], relics: &str, components: &str) -> Result<Self> {
        let mut records = Vec::new();
        for part in items {
            records.append(
                &mut serde_json::from_str(part)
                    .map_err(|source| DataError::Parse("item", source))?,
            );
        }
        Self::build(records, relics, components)
    }

    fn build(records: Vec<ItemRecord>, relics: &str, components: &str) -> Result<Self> {
        let components: Vec<Component> = serde_json::from_str(components)
            .map_err(|source| DataError::Parse("component", source))?;
        let parsed = resolve_components(records, components)?;
        let parsed: Vec<Item> = parsed
            .into_iter()
            .filter(|item| !item.is_alternate_suit_body())
            .collect();
        let crafted: HashSet<(&str, &str)> = parsed
            .iter()
            .filter(|item| item.components.is_some())
            .map(|item| (item.category.as_str(), item.name.as_str()))
            .collect();
        let kept: Vec<bool> = parsed
            .iter()
            .map(|item| {
                item.components.is_some()
                    || !item.masterable()
                    || !crafted.contains(&(item.category.as_str(), item.name.as_str()))
            })
            .collect();
        let items: Vec<Item> = parsed
            .into_iter()
            .zip(kept)
            .filter_map(|(item, kept)| kept.then_some(item))
            .collect();
        let relics = parse_relics(relics)?;

        let mut unique_name_index = HashMap::with_capacity(items.len());
        let mut name_index = HashMap::with_capacity(items.len());
        let mut component_index = HashMap::new();
        for (item_index, item) in items.iter().enumerate() {
            unique_name_index.insert(item.unique_name.clone(), item_index);
            if item.is_skin() || item.is_fish() || item.is_glyph() {
                continue;
            }
            name_index.entry(item.name.clone()).or_insert(item_index);
            if let Some(components) = &item.components {
                for (component_index_in_item, component) in components.iter().enumerate() {
                    component_index.insert(
                        component.unique_name.clone(),
                        (item_index, component_index_in_item),
                    );
                }
            }
        }
        for (item_index, item) in items.iter().enumerate() {
            if !item.is_skin() {
                continue;
            }
            for (component_index_in_item, component) in item.components.iter().flatten().enumerate()
            {
                component_index
                    .entry(component.unique_name.clone())
                    .or_insert((item_index, component_index_in_item));
            }
        }

        let mut relic_name_index = HashMap::with_capacity(relics.len());
        let mut relic_unique_name_index = HashMap::new();
        for (relic_index, relic) in relics.iter().enumerate() {
            relic_name_index.insert(relic.name.clone(), relic_index);
            for (&refinement, relic_unique_name) in &relic.unique_names {
                relic_unique_name_index
                    .insert(relic_unique_name.clone(), (relic_index, refinement));
            }
        }

        let riven_data = RivenData::from_items(&items);

        Ok(Self {
            items,
            relics,
            unique_name_index,
            name_index,
            component_index,
            relic_name_index,
            relic_unique_name_index,
            riven_data,
        })
    }

    pub fn riven_data(&self) -> &RivenData {
        &self.riven_data
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn relics(&self) -> &[Relic] {
        &self.relics
    }

    pub fn by_unique_name(&self, unique_name: &str) -> Option<&Item> {
        self.unique_name_index
            .get(unique_name)
            .map(|&index| &self.items[index])
    }

    pub fn by_name(&self, name: &str) -> Option<&Item> {
        self.name_index.get(name).map(|&index| &self.items[index])
    }

    pub fn component_by_unique_name(&self, unique_name: &str) -> Option<(&Item, &Component)> {
        let &(item_index, component_index) = self.component_index.get(unique_name)?;
        let item = &self.items[item_index];
        let component = item.components.as_ref()?.get(component_index)?;
        Some((item, component))
    }

    pub fn relic_by_name(&self, name: &str) -> Option<&Relic> {
        self.relic_name_index
            .get(name)
            .map(|&index| &self.relics[index])
    }

    pub fn relic_by_unique_name(&self, unique_name: &str) -> Option<(&Relic, Refinement)> {
        let &(relic_index, refinement) = self.relic_unique_name_index.get(unique_name)?;
        Some((&self.relics[relic_index], refinement))
    }

    pub fn relics_dropping(&self, component_unique_name: &str) -> Vec<(&Relic, Rarity)> {
        let Some((_, component)) = self.component_by_unique_name(component_unique_name) else {
            return Vec::new();
        };
        let Some(drops) = &component.drops else {
            return Vec::new();
        };
        let mut seen_relics = HashSet::new();
        let mut hits = Vec::new();
        for drop in drops {
            let Some(relic_unique_name) = &drop.unique_name else {
                continue;
            };
            let Some(&(relic_index, _)) = self.relic_unique_name_index.get(relic_unique_name)
            else {
                continue;
            };
            if seen_relics.insert(relic_index) {
                hits.push((&self.relics[relic_index], drop.rarity));
            }
        }
        hits
    }
}

fn resolve_components(records: Vec<ItemRecord>, components: Vec<Component>) -> Result<Vec<Item>> {
    let mut described: HashMap<String, Component> = components
        .into_iter()
        .map(|component| (component.unique_name.clone(), component))
        .collect();
    let ingredients: HashMap<&str, &Item> = records
        .iter()
        .map(|record| (record.item.unique_name.as_str(), &record.item))
        .collect();
    for record in &records {
        for component_ref in record.components.iter().flatten() {
            if described.contains_key(&component_ref.unique_name) {
                continue;
            }
            let ingredient = ingredients
                .get(component_ref.unique_name.as_str())
                .ok_or_else(|| DataError::UnknownComponent {
                    item: record.item.name.clone(),
                    component: component_ref.unique_name.clone(),
                })?;
            described.insert(
                component_ref.unique_name.clone(),
                Component::of_item(ingredient),
            );
        }
    }
    Ok(records
        .into_iter()
        .map(|record| {
            let mut item = record.item;
            item.components = record.components.map(|refs| {
                refs.into_iter()
                    .map(|component_ref| Component {
                        item_count: component_ref.item_count,
                        ..described[&component_ref.unique_name].clone()
                    })
                    .collect()
            });
            item
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::tests::MASTERY_ITEMS;
    use crate::relic::Refinement;

    const ITEMS: &str = include_str!("../tests/fixtures/items.json");
    const SKINS: &str = include_str!("../tests/fixtures/skins.json");
    const RELICS: &str = include_str!("../tests/fixtures/relics.json");
    const MISC_ITEMS: &str = include_str!("../../../fixtures/misc_items.json");
    const COMPONENTS: &str = include_str!("../tests/fixtures/components.json");

    fn fixture() -> GameData {
        GameData::from_json(ITEMS, RELICS, COMPONENTS).unwrap()
    }

    fn fixture_with_skins() -> GameData {
        GameData::from_json_parts(&[ITEMS.to_owned(), SKINS.to_owned()], RELICS, COMPONENTS)
            .unwrap()
    }

    #[test]
    fn doppelganger_grimoire_dropped() {
        let data = GameData::from_json(MASTERY_ITEMS, RELICS, COMPONENTS).unwrap();
        let grimoire = data
            .by_unique_name("/Lotus/Weapons/Tenno/Grimoire/TnGrimoire")
            .unwrap();
        assert_eq!(grimoire.name, "Grimoire");
        assert!(
            data.by_unique_name("/Lotus/Weapons/Tenno/Grimoire/TnDoppelgangerGrimoire")
                .is_none()
        );
        assert_eq!(
            data.items()
                .iter()
                .filter(|item| item.name == "Grimoire")
                .count(),
            1
        );
    }

    #[test]
    fn dual_warframe_single_suit() {
        let data = GameData::from_json(MASTERY_ITEMS, RELICS, COMPONENTS).unwrap();
        let sirius = data
            .by_unique_name("/Lotus/Powersuits/SiriusOrion/SiriusSuit")
            .unwrap();
        assert_eq!(sirius.name, "Sirius & Orion");
        assert!(sirius.is_warframe());
        assert!(
            data.by_unique_name("/Lotus/Powersuits/SiriusOrion/OrionSuit")
                .is_none()
        );
        assert!(data.by_name("Orion & Sirius").is_none());
        assert_eq!(
            data.items()
                .iter()
                .filter(|item| item.unique_name.contains("SiriusOrion"))
                .count(),
            1
        );
    }

    #[test]
    fn parses_fixtures() {
        let data = fixture();
        assert_eq!(data.items.len(), 5);
        assert!(!data.relics.is_empty());
    }

    #[test]
    fn by_unique_name() {
        let data = fixture();
        let item = data
            .by_unique_name("/Lotus/Powersuits/Excalibur/Excalibur")
            .unwrap();
        assert_eq!(item.name, "Excalibur");
    }

    #[test]
    fn skins_by_unique_name_only() {
        let data = fixture_with_skins();
        assert_eq!(data.items().len(), 23);
        let helmet = data
            .by_unique_name("/Lotus/Upgrades/Skins/Excalibur/ExcaliburHelmet")
            .unwrap();
        assert_eq!(helmet.name, "Excalibur Helmet");
        assert_eq!(helmet.image_name.as_deref(), Some("ExcaliburHelmet.png"));
        assert!(helmet.is_skin());
        assert!(data.by_name("Excalibur Helmet").is_none());
        assert_eq!(
            data.by_name("Excalibur").map(|item| item.category.as_str()),
            Some("Warframes")
        );
    }

    #[test]
    fn catches_and_glyphs_by_unique_name_only() {
        let data = GameData::from_json(MISC_ITEMS, RELICS, COMPONENTS).unwrap();
        let fish = data
            .by_unique_name("/Lotus/Types/Items/Fish/Eidolon/DayUncommonFishBItem")
            .unwrap();
        assert_eq!(fish.name, "Mortus Lungfish");
        assert!(fish.is_fish());
        assert!(data.by_name("Mortus Lungfish").is_none());

        let glyph = data
            .by_unique_name("/Lotus/Types/StoreItems/AvatarImages/Factions/GlyphFactionCorpus")
            .unwrap();
        assert_eq!(glyph.name, "Corpus Glyph");
        assert!(glyph.is_glyph());
        assert!(data.by_name("Corpus Glyph").is_none());
    }

    #[test]
    fn component_described_by_the_component_file() {
        const ITEMS: &str = r#"[
            {"uniqueName":"/Lotus/Weapons/Tenno/Rifle/BratonPrime","name":"Braton Prime",
             "category":"Primary","type":"LongGuns","tradable":true,
             "components":[
                {"uniqueName":"/Lotus/Types/Recipes/Weapons/WeaponParts/BratonPrimeBarrel","itemCount":1},
                {"uniqueName":"/Lotus/Types/Items/MiscItems/OrokinCell","itemCount":10}
             ]},
            {"uniqueName":"/Lotus/Types/Items/MiscItems/OrokinCell","name":"Orokin Cell",
             "category":"Misc","type":"Resource","tradable":false,"imageName":"cell.png",
             "drops":[{"location":"Saturn/Helene (Defense), Rotation C","type":"Orokin Cell",
                       "chance":null,"rarity":"Rare"}]}
        ]"#;
        const COMPONENTS: &str = r#"[
            {"uniqueName":"/Lotus/Types/Recipes/Weapons/WeaponParts/BratonPrimeBarrel",
             "name":"Barrel","tradable":true,"ducats":45,"imageName":"barrel.png"}
        ]"#;
        let data = GameData::from_json(ITEMS, RELICS, COMPONENTS).unwrap();
        let (_, barrel) = data
            .component_by_unique_name("/Lotus/Types/Recipes/Weapons/WeaponParts/BratonPrimeBarrel")
            .unwrap();
        assert_eq!(barrel.name, "Barrel");
        assert_eq!(barrel.item_count, 1);
        assert_eq!(barrel.ducats, Some(45));
        let (_, cell) = data
            .component_by_unique_name("/Lotus/Types/Items/MiscItems/OrokinCell")
            .unwrap();
        assert_eq!(cell.name, "Orokin Cell");
        assert_eq!(cell.item_count, 10);
        assert_eq!(cell.image_name.as_deref(), Some("cell.png"));
        assert_eq!(cell.drops.as_ref().map(Vec::len), Some(1));
        assert_eq!(cell.drops.as_ref().unwrap()[0].chance, None);
    }

    #[test]
    fn undescribed_component_fails_to_load() {
        const ITEMS: &str = r#"[
            {"uniqueName":"/Lotus/Weapons/Tenno/Rifle/BratonPrime","name":"Braton Prime",
             "category":"Primary","type":"LongGuns","tradable":true,
             "components":[{"uniqueName":"/Lotus/Types/Items/MiscItems/Alertium","itemCount":1}]}
        ]"#;
        let Err(error) = GameData::from_json(ITEMS, RELICS, "[]") else {
            panic!("an undescribed component must not load");
        };
        assert_eq!(
            error.to_string(),
            "Braton Prime needs a part the game data does not describe: /Lotus/Types/Items/MiscItems/Alertium"
        );
    }

    #[test]
    fn skin_component_does_not_shadow() {
        let data = fixture_with_skins();
        let (item, component) = data
            .component_by_unique_name("/Lotus/Types/Items/MiscItems/OrokinCell")
            .unwrap();
        assert_eq!(component.name, "Orokin Cell");
        assert!(!item.is_skin());
        let (helmet, blueprint) = data
            .component_by_unique_name("/Lotus/Types/Recipes/Helmets/BrawlerAltHelmetBlueprint")
            .unwrap();
        assert!(helmet.is_skin());
        assert_eq!(blueprint.name, "Blueprint");
    }

    #[test]
    fn craftable_skin_parts() {
        let data = fixture_with_skins();
        let (collar, band) = data
            .component_by_unique_name(
                "/Lotus/Types/Recipes/Kubrow/Collars/PrimeKubrowCollarABandComponent",
            )
            .unwrap();
        assert_eq!(collar.name, "Kavasa Prime Kubrow Collar");
        assert_eq!(band.name, "Kavasa Prime Band");
        assert_eq!(band.ducats, Some(45));
    }

    #[test]
    fn by_name() {
        let data = fixture();
        let item = data.by_name("Braton Prime").unwrap();
        assert_eq!(item.category, "Primary");
    }

    #[test]
    fn component_by_unique_name() {
        let data = fixture();
        let (item, component) = data
            .component_by_unique_name(
                "/Lotus/Types/Recipes/WarframeRecipes/TrinityPrimeSystemsComponent",
            )
            .unwrap();
        assert_eq!(item.name, "Trinity Prime");
        assert_eq!(component.name, "Systems");
    }

    #[test]
    fn reward_chances_sum_to_100() {
        let data = fixture();
        let relic = data.relic_by_name("Axi A1").unwrap();
        for refinement in [
            Refinement::Intact,
            Refinement::Exceptional,
            Refinement::Flawless,
            Refinement::Radiant,
        ] {
            let rewards = relic.rewards_for(refinement);
            assert_eq!(rewards.len(), 6);
            let total: f64 = rewards.iter().map(|r| r.chance).sum();
            assert!(
                (total - 100.0).abs() < 0.5,
                "refinement {refinement:?} summed to {total}"
            );
        }
    }

    #[test]
    fn relic_by_unique_name() {
        let data = fixture();
        let (relic, refinement) = data
            .relic_by_unique_name("/Lotus/Types/Game/Projections/T4VoidProjectionEBronze")
            .unwrap();
        assert_eq!(relic.name, "Axi A1");
        assert_eq!(refinement, Refinement::Intact);

        assert!(
            data.relic_by_unique_name("/Lotus/Types/Game/Projections/NotARealRelic")
                .is_none()
        );
    }

    #[test]
    fn relics_dropping_component() {
        let data = fixture();
        let hits = data
            .relics_dropping("/Lotus/Types/Recipes/WarframeRecipes/TrinityPrimeSystemsComponent");
        assert!(!hits.is_empty());
        assert!(hits.iter().any(|(relic, _)| relic.name == "Axi A1"));

        let hits =
            data.relics_dropping("/Lotus/Types/Recipes/Weapons/WeaponParts/BratonPrimeStock");
        assert!(hits.iter().any(|(relic, _)| relic.name == "Axi A1"));
    }

    #[test]
    fn mastery_xp_from_fixture() {
        let data = fixture();
        let warframe = data
            .by_unique_name("/Lotus/Powersuits/Excalibur/Excalibur")
            .unwrap();
        assert_eq!(warframe.mastery_xp(), 6000);
        let weapon = data.by_name("Braton Prime").unwrap();
        assert_eq!(weapon.mastery_xp(), 3000);
    }
}
