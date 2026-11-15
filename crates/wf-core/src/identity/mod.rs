mod listing;
mod naming;

use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Index;

use wf_data::{
    Component, Item, Rarity, Refinement, catch_grade, misc_item_name, store_item_to_type,
};
use wf_inventory::RIVEN_MARKER;

use crate::catalog::{
    Catalog, VaultStatus, component_image, display_name_from_path, is_fish, names_a_prime,
    part_name,
};
use crate::inventory_view::ItemSummary;

pub use listing::market_name;
pub(crate) use listing::traded_as;
use naming::held_names;
pub use naming::market_slug;
pub(crate) use naming::{
    ambassador_blueprint, blueprint_of, component_of, is_landing_craft_part, part_identity,
    part_market_slug, part_tradable,
};

pub(crate) const OUBLIETTE_KEY: &str = "/Lotus/Types/Keys/Nightwave/GlassmakerBossFightKey";
pub(crate) const LEGENDARY_CORE: &str = "/Lotus/Upgrades/Mods/Fusers/LegendaryModFuser";

const FOUNDERS_PRIMES: [&str; 3] = ["Excalibur Prime", "Lato Prime", "Skana Prime"];

pub(crate) struct UnlistedUpgrade {
    unique_name: &'static str,
    pub(crate) name: &'static str,
    rarity: Rarity,
    market_slug: &'static str,
    image_name: Option<&'static str>,
}

static UPGRADES_OUTSIDE_THE_EXPORT: [UnlistedUpgrade; 6] = [
    UnlistedUpgrade {
        unique_name: "/Lotus/Upgrades/CosmeticEnhancers/Antiques/AmmoEfficencyDuringUltimate",
        name: "Zid-An Haras",
        rarity: Rarity::Rare,
        market_slug: "zid-an-haras",
        image_name: None,
    },
    UnlistedUpgrade {
        unique_name: "/Lotus/Upgrades/CosmeticEnhancers/Antiques/HeatStatusProcOnUltimateKill",
        name: "Zid-An Uskos",
        rarity: Rarity::Rare,
        market_slug: "zid-an-uskos",
        image_name: None,
    },
    UnlistedUpgrade {
        unique_name: "/Lotus/Upgrades/CosmeticEnhancers/Antiques/StatusChanceOnUltimateHit",
        name: "Zid-An Asheir",
        rarity: Rarity::Rare,
        market_slug: "zid-an-asheir",
        image_name: None,
    },
    UnlistedUpgrade {
        unique_name: "/Lotus/Upgrades/CosmeticEnhancers/Antiques/UltimateInvisibilty",
        name: "Zid-An Sek-Eel",
        rarity: Rarity::Rare,
        market_slug: "zid-an-sek-eel",
        image_name: None,
    },
    UnlistedUpgrade {
        unique_name: "/Lotus/Upgrades/CosmeticEnhancers/Antiques/VoidSlingsOverguardStrip",
        name: "Zid-An Osbok",
        rarity: Rarity::Rare,
        market_slug: "zid-an-osbok",
        image_name: None,
    },
    UnlistedUpgrade {
        unique_name: LEGENDARY_CORE,
        name: "Legendary Core",
        rarity: Rarity::Legendary,
        market_slug: "legendary_fusion_core",
        image_name: Some("game/legendary-core.png"),
    },
];

pub(crate) fn unlisted_upgrade(unique_name: &str) -> Option<&'static UnlistedUpgrade> {
    UPGRADES_OUTSIDE_THE_EXPORT
        .iter()
        .find(|upgrade| upgrade.unique_name == unique_name)
}

pub(crate) fn is_riven(unique_name: &str) -> bool {
    unique_name.contains(RIVEN_MARKER)
}

fn starter_original(unique_name: &str) -> Option<String> {
    let (parent, leaf) = unique_name.strip_suffix("Beginner")?.rsplit_once('/')?;
    Some(format!("{}/{leaf}", parent.strip_suffix("/Beginner")?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemRef(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variant {
    Set,
    Imprint,
    Veiled,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemKind {
    Item {
        tradable: bool,
    },
    Upgrade {
        rarity: Option<Rarity>,
        max_rank: Option<u32>,
    },
    Riven,
    Set {
        parts: Vec<ItemRef>,
    },
    Part {
        set: ItemRef,
        ducats: Option<u32>,
        tradable: bool,
    },
    Relic {
        refinement: Refinement,
    },
    Imprint,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemRecord {
    pub unique_name: String,
    pub name: String,
    pub image_name: Option<String>,
    pub market_slug: Option<String>,
    pub prime: bool,
    pub vault: Option<VaultStatus>,
    pub kind: ItemKind,
}

impl ItemRecord {
    fn unknown(unique_name: &str) -> Self {
        let name = misc_item_name(unique_name)
            .map_or_else(|| display_name_from_path(unique_name), str::to_owned);
        let name = if is_riven(unique_name) {
            format!("{name} (Veiled)")
        } else {
            name
        };
        Self::new(unique_name, name, None, ItemKind::Other)
    }

    fn of(item: &Item, kind: ItemKind) -> Self {
        Self::new(
            &item.unique_name,
            item.name.clone(),
            item.image_name.clone(),
            kind,
        )
    }

    fn new(unique_name: &str, name: String, image_name: Option<String>, kind: ItemKind) -> Self {
        Self {
            unique_name: unique_name.to_owned(),
            market_slug: Some(market_slug(&name)),
            prime: names_a_prime(&name),
            vault: None,
            image_name,
            name,
            kind,
        }
    }

    fn slugged(self, slug: String) -> Self {
        Self {
            market_slug: Some(slug),
            ..self
        }
    }

    fn vaulted(self, item: &Item) -> Self {
        let vaulted = if FOUNDERS_PRIMES.contains(&item.name.as_str()) {
            Some(true)
        } else {
            item.vaulted
        };
        Self {
            vault: names_a_prime(&self.name).then(|| VaultStatus::from(vaulted)),
            ..self
        }
    }

    pub(crate) fn held_as(&self, unique_name: &str) -> Cow<'_, Self> {
        if self.unique_name == unique_name {
            Cow::Borrowed(self)
        } else {
            Cow::Owned(Self {
                unique_name: unique_name.to_owned(),
                ..self.clone()
            })
        }
    }

    pub(crate) fn tradable(&self) -> bool {
        match self.kind {
            ItemKind::Item { tradable } | ItemKind::Part { tradable, .. } => tradable,
            _ => false,
        }
    }

    pub fn ducats(&self) -> Option<u32> {
        match self.kind {
            ItemKind::Part { ducats, .. } => ducats,
            _ => None,
        }
    }

    fn takes_market_slug(&self) -> bool {
        matches!(self.kind, ItemKind::Upgrade { .. })
            && unlisted_upgrade(&self.unique_name).is_none()
    }
}

#[derive(Debug, Default)]
pub struct ItemTable {
    records: Vec<ItemRecord>,
    by_unique_name: HashMap<String, ItemRef>,
    by_held_name: HashMap<String, ItemRef>,
    variants: HashMap<(ItemRef, Variant), ItemRef>,
    by_market_id: HashMap<String, ItemRef>,
    by_market_slug: HashMap<String, ItemRef>,
    by_game_ref: HashMap<String, ItemRef>,
    listing_by_name: HashMap<String, String>,
    catalogued: usize,
}

const INVASION_WEAPONS: [&str; 7] = [
    "DeraVandal",
    "GrineerCombatKnife",
    "KarakWraith",
    "LatronWraith",
    "SnipetronVandal",
    "StrunWraith",
    "TwinVipersWraith",
];

pub(crate) const OPTICOR_VANDAL: &str = "/Lotus/Weapons/Corpus/LongGuns/CrpBFG/Vandal/VandalCrpBFG";

fn is_weapon_or_part(unique_name: &str) -> bool {
    unique_name.starts_with("/Lotus/Types/Recipes/Weapons/")
        || unique_name.starts_with("/Lotus/Weapons/")
}

fn is_invasion_weapon_part(unique_name: &str) -> bool {
    unique_name
        .strip_prefix("/Lotus/Types/Recipes/Weapons/")
        .map(|rest| rest.strip_prefix("WeaponParts/").unwrap_or(rest))
        .is_some_and(|tail| {
            INVASION_WEAPONS
                .iter()
                .any(|weapon| tail.starts_with(weapon))
        })
}

pub(crate) fn is_unusual_reward_weapon(unique_name: &str) -> bool {
    is_weapon_or_part(unique_name)
        && !is_invasion_weapon_part(unique_name)
        && unique_name != OPTICOR_VANDAL
}

impl Index<ItemRef> for ItemTable {
    type Output = ItemRecord;

    fn index(&self, item: ItemRef) -> &ItemRecord {
        &self.records[item.0]
    }
}

impl ItemTable {
    pub fn build(catalog: &Catalog) -> Self {
        let mut table = Self::default();
        let items = catalog.data().items();
        let bases: Vec<ItemRef> = items.iter().map(|item| table.add_item(item)).collect();
        for (item, base) in items.iter().zip(bases) {
            table.add_parts(catalog, item, base);
            table.add_grades(catalog, item, base);
            if item.category == "Pets" {
                let name = format!("{} Imprint", item.name);
                let imprint = table.push(ItemRecord::new(
                    &item.unique_name,
                    name,
                    item.image_name.clone(),
                    ItemKind::Imprint,
                ));
                table.variants.insert((base, Variant::Imprint), imprint);
            }
        }
        table.add_relics(catalog);
        for item in items {
            if let Some(original) = starter_original(&item.unique_name)
                && !table.by_unique_name.contains_key(&original)
            {
                let record = upgrade_record(&original, item, "");
                table.insert(record);
            }
        }
        for upgrade in &UPGRADES_OUTSIDE_THE_EXPORT {
            if !table.by_unique_name.contains_key(upgrade.unique_name) {
                let record = ItemRecord::new(
                    upgrade.unique_name,
                    upgrade.name.to_owned(),
                    upgrade.image_name.map(str::to_owned),
                    ItemKind::Upgrade {
                        rarity: Some(upgrade.rarity),
                        max_rank: None,
                    },
                )
                .slugged(upgrade.market_slug.to_owned());
                table.insert(record);
            }
        }
        table.catalogued = table.records.len();
        table.index_slugs();
        table
    }

    fn index_slugs(&mut self) {
        self.by_market_slug.clear();
        for (position, record) in self.records.iter().enumerate() {
            if let Some(slug) = &record.market_slug {
                self.by_market_slug
                    .entry(slug.clone())
                    .or_insert(ItemRef(position));
            }
        }
    }

    fn push(&mut self, record: ItemRecord) -> ItemRef {
        self.records.push(record);
        ItemRef(self.records.len() - 1)
    }

    fn insert(&mut self, record: ItemRecord) -> ItemRef {
        let unique_name = record.unique_name.clone();
        let item = self.push(record);
        self.by_unique_name.insert(unique_name, item);
        item
    }

    fn add_item(&mut self, item: &Item) -> ItemRef {
        if is_riven(&item.unique_name) {
            let veiled_slug = format!("{}_(veiled)", market_slug(&item.name));
            let base = self.insert(ItemRecord::of(item, ItemKind::Riven).slugged(veiled_slug));
            let veiled = upgrade_record(&item.unique_name, item, " (Veiled)");
            let veiled = self.push(veiled);
            self.variants.insert((base, Variant::Veiled), veiled);
            return base;
        }
        if item.category == "Mods"
            || item.category == "Arcanes"
            || unlisted_upgrade(&item.unique_name).is_some()
        {
            return self.insert(upgrade_record(&item.unique_name, item, ""));
        }
        let kind = ItemKind::Item {
            tradable: item.tradable,
        };
        self.insert(ItemRecord::of(item, kind).vaulted(item))
    }

    fn add_parts(&mut self, catalog: &Catalog, item: &Item, base: ItemRef) {
        let Some(components) = item.components.as_deref().filter(|parts| !parts.is_empty()) else {
            return;
        };
        let set = self.push(
            ItemRecord::of(item, ItemKind::Set { parts: Vec::new() })
                .slugged(format!("{}_set", market_slug(&item.name)))
                .vaulted(item),
        );
        self.variants.insert((base, Variant::Set), set);
        let parts = components
            .iter()
            .map(|component| {
                let record = ItemRecord {
                    prime: names_a_prime(&item.name),
                    ..ItemRecord::new(
                        &component.unique_name,
                        part_name(item, component),
                        component_image(item, component),
                        ItemKind::Part {
                            set,
                            ducats: component.ducats,
                            tradable: part_tradable(component),
                        },
                    )
                    .slugged(part_market_slug(item, component))
                    .vaulted(item)
                };
                let part = self.push(record);
                let first_owner = catalog
                    .component(&component.unique_name)
                    .is_some_and(|(_, found)| std::ptr::eq(found, component));
                if first_owner {
                    self.by_unique_name
                        .entry(component.unique_name.clone())
                        .or_insert(part);
                    for held in held_names(&component.unique_name) {
                        self.by_held_name.entry(held).or_insert(part);
                    }
                }
                part
            })
            .collect();
        self.records[set.0].kind = ItemKind::Set { parts };
    }

    fn add_grades(&mut self, catalog: &Catalog, item: &Item, base: ItemRef) {
        if !is_fish(&item.unique_name) {
            return;
        }
        let stem = item.unique_name.strip_suffix("Item");
        for marker in ["Large", "Medium"] {
            let graded = [
                Some(format!("{}{marker}", item.unique_name)),
                stem.map(|stem| format!("{stem}{marker}Item")),
            ];
            for unique_name in graded.into_iter().flatten() {
                let Some((_, grade)) =
                    catch_grade(&unique_name).filter(|(found, _)| *found == item.unique_name)
                else {
                    continue;
                };
                let image_name = catalog
                    .item(&unique_name)
                    .and_then(|own| own.image_name.clone());
                let slug = self[base].market_slug.clone();
                let record = ItemRecord {
                    market_slug: slug,
                    ..ItemRecord::new(
                        &unique_name,
                        format!("{} ({grade})", item.name),
                        image_name,
                        ItemKind::Item {
                            tradable: item.tradable,
                        },
                    )
                };
                self.insert(record);
            }
        }
    }

    fn add_relics(&mut self, catalog: &Catalog) {
        for relic in catalog.relics() {
            for refinement in Refinement::ALL {
                let Some(unique_name) = relic.unique_names.get(&refinement) else {
                    continue;
                };
                let record = ItemRecord {
                    market_slug: relic.market_info.as_ref().map(|info| info.url_name.clone()),
                    vault: Some(VaultStatus::from(Some(relic.vaulted))),
                    ..ItemRecord::new(
                        unique_name,
                        format!("{} Relic ({})", relic.name, refinement.name()),
                        relic.image_names.get(&refinement).cloned(),
                        ItemKind::Relic { refinement },
                    )
                };
                self.insert(record);
            }
        }
    }

    pub fn get(&self, unique_name: &str) -> Option<&ItemRecord> {
        self.by_unique_name
            .get(unique_name)
            .map(|&item| &self[item])
    }

    pub fn variant(&self, unique_name: &str, variant: Variant) -> Option<&ItemRecord> {
        let base = self.by_unique_name.get(unique_name)?;
        self.variants
            .get(&(*base, variant))
            .map(|&item| &self[item])
    }

    pub(crate) fn part(&self, item: &Item, component: &Component) -> Option<&ItemRecord> {
        let ItemKind::Set { parts } = &self.variant(&item.unique_name, Variant::Set)?.kind else {
            return None;
        };
        parts
            .iter()
            .map(|&part| &self[part])
            .find(|part| part.unique_name == component.unique_name)
    }

    pub fn invasion_weapon_parts(&self) -> Vec<&ItemRecord> {
        let mut parts: Vec<&ItemRecord> = self
            .by_unique_name
            .iter()
            .filter(|(unique_name, _)| is_invasion_weapon_part(unique_name))
            .map(|(_, &item)| &self[item])
            .collect();
        parts.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| a.unique_name.cmp(&b.unique_name))
        });
        parts
    }

    pub fn lookup(&self, item_type: &str) -> Option<&ItemRecord> {
        let unique_name = store_item_to_type(item_type);
        self.get(&unique_name)
            .or_else(|| self.by_game_ref(&unique_name))
            .or_else(|| self.by_held_name.get(&unique_name).map(|&item| &self[item]))
    }

    pub fn resolve(&self, item_type: &str) -> Cow<'_, ItemRecord> {
        let unique_name = store_item_to_type(item_type);
        match self.lookup(&unique_name) {
            Some(record) => record.held_as(&unique_name),
            None => Cow::Owned(ItemRecord::unknown(&unique_name)),
        }
    }

    pub fn summary(&self, item_type: &str) -> ItemSummary {
        ItemSummary::from(&*self.resolve(item_type))
    }

    pub fn name(&self, item_type: &str) -> String {
        self.resolve(item_type).name.clone()
    }

    pub(crate) fn known_name(&self, item_type: &str) -> Option<String> {
        self.lookup(item_type)
            .map(|record| record.name.clone())
            .or_else(|| misc_item_name(item_type).map(str::to_owned))
    }

    pub fn by_market_id(&self, id: &str) -> Option<&ItemRecord> {
        self.by_market_id.get(id).map(|&item| &self[item])
    }

    pub fn by_market_slug(&self, slug: &str) -> Option<&ItemRecord> {
        self.by_market_slug.get(slug).map(|&item| &self[item])
    }

    pub fn by_game_ref(&self, game_ref: &str) -> Option<&ItemRecord> {
        self.by_game_ref.get(game_ref).map(|&item| &self[item])
    }
}

fn upgrade_record(unique_name: &str, item: &Item, suffix: &str) -> ItemRecord {
    let unlisted = unlisted_upgrade(unique_name);
    let record = ItemRecord::new(
        unique_name,
        format!("{}{suffix}", item.name),
        item.image_name
            .clone()
            .or_else(|| unlisted.and_then(|upgrade| upgrade.image_name.map(str::to_owned))),
        ItemKind::Upgrade {
            rarity: item.rarity.or(unlisted.map(|upgrade| upgrade.rarity)),
            max_rank: Some(item.max_upgrade_rank()),
        },
    );
    match unlisted {
        Some(upgrade) => record.slugged(upgrade.market_slug.to_owned()),
        None => record,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invasion_weapon_parts_by_stem() {
        assert!(is_invasion_weapon_part(
            "/Lotus/Types/Recipes/Weapons/WeaponParts/DeraVandalBarrel"
        ));
        assert!(is_invasion_weapon_part(
            "/Lotus/Types/Recipes/Weapons/GrineerCombatKnifeSortieBlueprint"
        ));
        assert!(!is_invasion_weapon_part(
            "/Lotus/Types/Recipes/Weapons/WeaponParts/DetronBarrel"
        ));
        assert!(!is_invasion_weapon_part(
            "/Lotus/Types/Recipes/Weapons/BratonPrimeBlueprint"
        ));
    }

    #[test]
    fn unusual_reward_weapons() {
        assert!(is_unusual_reward_weapon(
            "/Lotus/Types/Recipes/Weapons/WeaponParts/ThrowingHammerHead"
        ));
        assert!(is_unusual_reward_weapon(
            "/Lotus/Weapons/Tenno/Melee/Hammer/ThrowingHammer"
        ));
        assert!(!is_unusual_reward_weapon(
            "/Lotus/Types/Recipes/Weapons/WeaponParts/DeraVandalBarrel"
        ));
        assert!(!is_unusual_reward_weapon(OPTICOR_VANDAL));
        assert!(!is_unusual_reward_weapon(
            "/Lotus/Types/Items/Research/EnergyComponent"
        ));
    }
    use crate::catalog::fixtures::{
        self, BRATON_PRIME, BRATON_PRIME_BARREL, TRINITY_PRIME_SYSTEMS,
        TRINITY_PRIME_SYSTEMS_BLUEPRINT, market_item,
    };

    const ENERGIZE: &str =
        "/Lotus/Upgrades/CosmeticEnhancers/Utility/GolemArcaneRadialEnergyOnEnergyPickup";
    const HARAS: &str = "/Lotus/Upgrades/CosmeticEnhancers/Antiques/AmmoEfficencyDuringUltimate";

    fn slug_of(table: &ItemTable, unique_name: &str) -> Option<String> {
        table
            .get(unique_name)
            .and_then(|record| record.market_slug.clone())
    }

    #[test]
    fn starter_originals() {
        assert_eq!(
            starter_original("/Lotus/Upgrades/Mods/Warframe/Beginner/AvatarSlideBoostModBeginner")
                .as_deref(),
            Some("/Lotus/Upgrades/Mods/Warframe/AvatarSlideBoostMod")
        );
        assert_eq!(
            starter_original("/Lotus/Upgrades/Mods/Warframe/AvatarSlideBoostModBeginner"),
            None
        );
    }

    #[test]
    fn parts_sets_and_relics() {
        let catalog = fixtures::catalog();
        let table = ItemTable::build(&catalog);
        let barrel = table.get(BRATON_PRIME_BARREL).unwrap();
        assert_eq!(barrel.name, "Braton Prime Barrel");
        assert_eq!(barrel.market_slug.as_deref(), Some("braton_prime_barrel"));
        assert!(barrel.prime);
        assert!(barrel.vault.is_some());
        assert_eq!(
            table
                .resolve(&format!("/Lotus/StoreItems{}", &BRATON_PRIME_BARREL[6..]))
                .as_ref(),
            barrel
        );
        assert_eq!(table.by_market_slug("braton_prime_barrel"), Some(barrel));
        let held_blueprint = table.resolve(TRINITY_PRIME_SYSTEMS_BLUEPRINT);
        assert_eq!(held_blueprint.unique_name, TRINITY_PRIME_SYSTEMS_BLUEPRINT);
        assert_eq!(held_blueprint.name, "Trinity Prime Systems");
        assert_eq!(
            held_blueprint.market_slug.as_deref(),
            Some("trinity_prime_systems_blueprint")
        );
        assert_eq!(
            slug_of(&table, TRINITY_PRIME_SYSTEMS).as_deref(),
            Some("trinity_prime_systems_blueprint")
        );
        let ItemKind::Part { set, ducats, .. } = barrel.kind else {
            unreachable!();
        };
        assert_eq!(ducats, Some(15));
        assert_eq!(barrel.ducats(), ducats);
        assert_eq!(table[set].market_slug.as_deref(), Some("braton_prime_set"));
        assert_eq!(table.variant(BRATON_PRIME, Variant::Set), Some(&table[set]));
        assert_eq!(
            slug_of(&table, BRATON_PRIME).as_deref(),
            Some("braton_prime")
        );

        let relic = table
            .get("/Lotus/Types/Game/Projections/T4VoidProjectionESilver")
            .unwrap();
        assert_eq!(relic.name, "Axi A1 Relic (Exceptional)");
        assert_eq!(relic.market_slug.as_deref(), Some("axi_a1_relic"));
        assert_eq!(
            relic.kind,
            ItemKind::Relic {
                refinement: Refinement::Exceptional
            }
        );
        assert!(relic.vault.is_some());
        assert!(table.get("/Lotus/Nope").is_none());
    }

    #[test]
    fn upgrades() {
        let table = ItemTable::build(&fixtures::upgrade_catalog());
        let energize = table.get(ENERGIZE).unwrap();
        assert_eq!(energize.market_slug.as_deref(), Some("arcane_energize"));
        assert!(matches!(
            energize.kind,
            ItemKind::Upgrade {
                max_rank: Some(5),
                ..
            }
        ));
        assert_eq!(
            slug_of(
                &table,
                "/Lotus/Weapons/Tenno/Melee/MeleeTrees/FistCmbThreeMeleeTree"
            )
            .as_deref(),
            Some("gaias_tragedy")
        );
        let maglev = table
            .get("/Lotus/Upgrades/Mods/Warframe/AvatarSlideBoostMod")
            .unwrap();
        assert_eq!(maglev.name, "Maglev");
        assert_eq!(maglev.market_slug.as_deref(), Some("maglev"));
        let core = table
            .get("/Lotus/Upgrades/Mods/Fusers/LegendaryModFuser")
            .unwrap();
        assert_eq!(core.name, "Legendary Core");
        assert_eq!(core.market_slug.as_deref(), Some("legendary_fusion_core"));
        assert_eq!(core.image_name.as_deref(), Some("game/legendary-core.png"));
        assert_eq!(slug_of(&table, HARAS).as_deref(), Some("zid-an-haras"));
    }

    #[test]
    fn rivens_fish_and_imprints() {
        let catalog = fixtures::catalog_of(
            r#"[
              {"uniqueName":"/Lotus/Upgrades/Mods/Randomized/LotusRifleRandomModRare",
               "name":"Rifle Riven Mod","category":"Mods","type":"Riven Mod","tradable":true},
              {"uniqueName":"/Lotus/Types/Items/Fish/Solaris/SolarisCoolCommonFishAItem",
               "name":"Tink","category":"Fish","type":"Fish","tradable":true},
              {"uniqueName":"/Lotus/Types/Game/CatbrowPet/VampireCatbrowPetPowerSuit",
               "name":"Vasca Kavat","category":"Pets","type":"Pets","tradable":false}
            ]"#,
        );
        let table = ItemTable::build(&catalog);
        let riven = "/Lotus/Upgrades/Mods/Randomized/LotusRifleRandomModRare";
        assert_eq!(
            slug_of(&table, riven).as_deref(),
            Some("rifle_riven_mod_(veiled)")
        );
        let veiled = table.variant(riven, Variant::Veiled).unwrap();
        assert_eq!(veiled.name, "Rifle Riven Mod (Veiled)");
        assert_eq!(
            veiled.market_slug.as_deref(),
            Some("rifle_riven_mod_veiled")
        );

        let graded = table
            .get("/Lotus/Types/Items/Fish/Solaris/SolarisCoolCommonFishAMediumItem")
            .unwrap();
        assert_eq!(graded.name, "Tink (Adorned)");
        assert_eq!(graded.market_slug.as_deref(), Some("tink"));

        let imprint = table
            .variant(
                "/Lotus/Types/Game/CatbrowPet/VampireCatbrowPetPowerSuit",
                Variant::Imprint,
            )
            .unwrap();
        assert_eq!(imprint.name, "Vasca Kavat Imprint");
        assert_eq!(imprint.market_slug.as_deref(), Some("vasca_kavat_imprint"));
    }

    #[test]
    fn market_index() {
        let catalog = fixtures::upgrade_catalog();
        let mut table = ItemTable::build(&catalog);
        let unknown = "/Lotus/Weapons/Tenno/Melee/MeleeTrees/StaffCmbOneMeleeTree";
        let items = [
            market_item("arcane\u{2019}energize", "Arcane Energize", ENERGIZE, &[]),
            market_item("clashing_forest", "Clashing Forest", unknown, &[]),
            market_item("zid_an_haras", "Zid-An Haras", HARAS, &[]),
            market_item("braton_prime_set", "Braton Prime Set", "", &["set"]),
            market_item(
                "scan_aquatic_lifeforms",
                "Scan Aquatic Lifeforms",
                "",
                &["mod"],
            ),
        ];
        assert_eq!(table.index_market(&items), 4);
        let precept = table.resolve("/Lotus/Types/Sentinels/SentinelPrecepts/LocateCreatures");
        assert_eq!(precept.name, "Scan Aquatic Lifeforms");
        assert_eq!(
            precept.market_slug.as_deref(),
            Some("scan_aquatic_lifeforms")
        );
        assert_eq!(
            slug_of(&table, ENERGIZE).as_deref(),
            Some("arcane\u{2019}energize")
        );
        assert_eq!(
            table
                .by_market_id("arcane\u{2019}energize")
                .map(|record| record.unique_name.as_str()),
            Some(ENERGIZE)
        );
        let listed = table.by_game_ref(unknown).unwrap();
        assert_eq!(listed.name, "Clashing Forest");
        assert_eq!(listed.kind, ItemKind::Other);
        assert_eq!(table.by_market_id("clashing_forest"), Some(listed));
        assert!(table.get(unknown).is_none());
        let resolved = table.resolve(unknown);
        assert_eq!(resolved.name, "Clashing Forest");
        assert_eq!(resolved.market_slug.as_deref(), Some("clashing_forest"));
        assert_eq!(table.resolve("/Lotus/Nowhere").name, "Nowhere");
        assert_eq!(
            table
                .resolve("/Lotus/Upgrades/Mods/Randomized/NowhereRandomMod")
                .name,
            "Nowhere Random Mod (Veiled)"
        );
        assert_eq!(
            table
                .by_market_slug("clashing_forest")
                .map(|record| record.name.as_str()),
            Some("Clashing Forest")
        );
        assert_eq!(slug_of(&table, HARAS).as_deref(), Some("zid-an-haras"));
        assert_eq!(
            table
                .by_market_id("braton_prime_set")
                .map(|record| &record.kind),
            Some(&ItemKind::Other)
        );

        assert_eq!(table.index_market(&[]), 0);
        assert_eq!(
            slug_of(&table, ENERGIZE).as_deref(),
            Some("arcane_energize")
        );
        assert!(table.by_game_ref(unknown).is_none());
        assert!(table.by_market_id("arcane\u{2019}energize").is_none());
    }

    #[test]
    fn set_listing() {
        let mut table = ItemTable::build(&fixtures::catalog());
        table.index_market(&[
            market_item(
                "braton_prime_set",
                "Braton Prime Set",
                BRATON_PRIME,
                &["set", "weapon"],
            ),
            market_item(
                "barrel_listing",
                "Braton Prime Barrel",
                BRATON_PRIME_BARREL,
                &[],
            ),
        ]);
        let set = table.by_market_id("braton_prime_set");
        assert_eq!(set, table.variant(BRATON_PRIME, Variant::Set));
        assert_eq!(
            slug_of(&table, BRATON_PRIME).as_deref(),
            Some("braton_prime")
        );
        assert_eq!(
            slug_of(&table, BRATON_PRIME_BARREL).as_deref(),
            Some("braton_prime_barrel")
        );
        assert_eq!(
            table.by_market_id("barrel_listing"),
            table.get(BRATON_PRIME_BARREL)
        );
    }
}
