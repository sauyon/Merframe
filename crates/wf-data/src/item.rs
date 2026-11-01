use serde::{Deserialize, Serialize};

pub fn store_item_to_type(path: &str) -> String {
    path.replacen("/Lotus/StoreItems/", "/Lotus/", 1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "PascalCase")]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Legendary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSlug {
    pub id: String,
    #[serde(rename = "urlName")]
    pub url_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drop {
    pub chance: Option<f64>,
    pub location: String,
    pub rarity: Rarity,
    #[serde(rename = "type")]
    pub drop_type: String,
    #[serde(rename = "uniqueName")]
    pub unique_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attack {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelStat {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    #[serde(rename = "uniqueName")]
    pub unique_name: String,
    pub name: String,
    #[serde(skip_deserializing)]
    pub item_count: u32,
    pub tradable: bool,
    pub ducats: Option<u32>,
    pub drops: Option<Vec<Drop>>,
    #[serde(rename = "imageName")]
    pub image_name: Option<String>,
}

impl Component {
    pub(crate) fn of_item(item: &Item) -> Self {
        Self {
            unique_name: item.unique_name.clone(),
            name: item.name.clone(),
            item_count: 0,
            tradable: item.tradable,
            ducats: None,
            drops: item.drops.clone(),
            image_name: item.image_name.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ComponentRef {
    #[serde(rename = "uniqueName")]
    pub(crate) unique_name: String,
    #[serde(rename = "itemCount")]
    pub(crate) item_count: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ItemRecord {
    pub(crate) components: Option<Vec<ComponentRef>>,
    #[serde(flatten)]
    pub(crate) item: Item,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    #[serde(rename = "uniqueName")]
    pub unique_name: String,
    pub name: String,
    pub category: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub tradable: bool,
    #[serde(rename = "masteryReq")]
    pub mastery_req: Option<u32>,
    #[serde(rename = "productCategory")]
    pub product_category: Option<String>,
    #[serde(skip_deserializing)]
    pub components: Option<Vec<Component>>,
    #[serde(rename = "warframeMarket")]
    pub warframe_market: Option<MarketSlug>,
    #[serde(rename = "imageName")]
    pub image_name: Option<String>,
    pub drops: Option<Vec<Drop>>,
    #[serde(rename = "wikiaUrl")]
    pub wikia_url: Option<String>,
    pub masterable: Option<bool>,
    pub rarity: Option<Rarity>,
    #[serde(rename = "maxLevelCap")]
    pub max_level_cap: Option<u32>,
    pub vaulted: Option<bool>,
    #[serde(rename = "buildQuantity")]
    pub build_quantity: Option<u32>,
    #[serde(rename = "buildPrice")]
    pub build_price: Option<u32>,
    #[serde(rename = "bpCost")]
    pub blueprint_cost: Option<u32>,
    #[serde(rename = "buildTime")]
    pub build_time: Option<u32>,
    pub tags: Option<Vec<String>>,
    pub attacks: Option<Vec<Attack>>,
    #[serde(rename = "omegaAttenuation")]
    pub omega_attenuation: Option<f64>,
    #[serde(rename = "upgradeEntries")]
    pub upgrade_entries: Option<Vec<crate::riven::UpgradeEntry>>,
    #[serde(rename = "fusionLimit")]
    pub fusion_limit: Option<u32>,
    #[serde(rename = "levelStats")]
    pub level_stats: Option<Vec<LevelStat>>,
    /// A warframe's abilities in slot order.
    pub abilities: Option<Vec<Ability>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ability {
    #[serde(rename = "uniqueName")]
    pub unique_name: String,
    pub name: String,
}

pub const DEFAULT_MAX_RANK: u32 = 30;
pub const MECH_MAX_RANK: u32 = 40;

impl Item {
    pub fn masterable(&self) -> bool {
        self.masterable.unwrap_or(false)
    }

    pub fn is_skin(&self) -> bool {
        self.category == "Skins" || self.category == "Sigils"
    }

    pub fn is_fish(&self) -> bool {
        self.category == "Fish"
    }

    pub fn is_glyph(&self) -> bool {
        self.category == "Glyphs"
    }

    pub fn is_warframe(&self) -> bool {
        self.category == "Warframes" && self.product_category.as_deref() == Some("Suits")
    }

    pub fn is_necramech(&self) -> bool {
        self.product_category.as_deref() == Some("MechSuits")
    }

    /// WFCD types the Liset, Railjack and Skaut as "Orbiter", just like the other landing crafts.
    /// Only the foundry tab ones have components.
    pub fn is_landing_craft(&self) -> bool {
        self.item_type == "Orbiter" && self.components.is_some()
    }

    pub fn is_archwing(&self) -> bool {
        self.product_category.as_deref() == Some("SpaceSuits")
    }

    pub fn is_alternate_suit_body(&self) -> bool {
        self.category == "Warframes" && self.product_category.as_deref() == Some("SpecialItems")
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "an upgrade has a handful of ranks"
    )]
    pub fn max_upgrade_rank(&self) -> u32 {
        let fusion_limit = self.fusion_limit.unwrap_or_default();
        if self
            .unique_name
            .starts_with("/Lotus/Upgrades/Mods/Railjack/")
        {
            return fusion_limit;
        }
        let ranks_above_unranked = self.level_stats.iter().flatten().skip(1).count();
        fusion_limit.max(ranks_above_unranked as u32)
    }

    pub fn mastery_per_rank(&self) -> u32 {
        if warframe_like(self) { 200 } else { 100 }
    }

    pub fn max_mastery_rank(&self) -> u32 {
        self.max_level_cap.unwrap_or(if self.is_necramech() {
            MECH_MAX_RANK
        } else {
            DEFAULT_MAX_RANK
        })
    }

    pub fn affinity_cap(&self) -> u64 {
        let rank = u64::from(self.max_mastery_rank());
        affinity_per_rank_squared(warframe_like(self)) * rank * rank
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "the affinity cap bounds the rank"
    )]
    pub fn mastery_rank_at(&self, affinity: u64) -> u32 {
        mastery_level_from_affinity(affinity, warframe_like(self), self.affinity_cap()) as u32
    }

    pub fn takes_orokin_reactor(&self) -> bool {
        warframe_like(self)
    }

    pub fn mastery_xp(&self) -> u32 {
        self.mastery_per_rank() * self.max_mastery_rank()
    }
}

fn affinity_per_rank_squared(warframe_like: bool) -> u64 {
    if warframe_like { 1000 } else { 500 }
}

pub fn mastery_level_from_affinity(affinity: u64, warframe_like: bool, cap: u64) -> u64 {
    (affinity.min(cap) / affinity_per_rank_squared(warframe_like)).isqrt()
}

fn warframe_like(item: &Item) -> bool {
    if item.product_category.as_deref() == Some("SentinelWeapons") {
        return false;
    }
    matches!(
        item.category.as_str(),
        "Warframes" | "Archwing" | "Sentinels" | "Pets"
    ) || item
        .unique_name
        .starts_with("/Lotus/Types/Vehicles/Hoverboard/HoverboardParts/")
        || item.is_necramech()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn item(category: &str, product_category: &str) -> Item {
        Item {
            unique_name: "/Lotus/Test".to_owned(),
            name: "Test".to_owned(),
            category: category.to_owned(),
            item_type: "Test".to_owned(),
            tradable: false,
            mastery_req: Some(0),
            product_category: Some(product_category.to_owned()),
            components: None,
            warframe_market: None,
            image_name: None,
            drops: None,
            wikia_url: None,
            masterable: Some(true),
            rarity: None,
            max_level_cap: None,
            vaulted: None,
            build_quantity: None,
            build_price: None,
            blueprint_cost: None,
            build_time: None,
            tags: None,
            attacks: None,
            omega_attenuation: None,
            upgrade_entries: None,
            fusion_limit: None,
            level_stats: None,
            abilities: None,
        }
    }

    pub(crate) const MASTERY_ITEMS: &str = include_str!("../../../fixtures/mastery_items.json");

    pub(crate) fn fixture_items() -> Vec<Item> {
        serde_json::from_str(MASTERY_ITEMS).unwrap()
    }

    #[test]
    fn suit_classes() {
        let items = fixture_items();
        let named = |name: &str| items.iter().find(|item| item.name == name).unwrap();

        let voidrig = named("Voidrig");
        assert!(voidrig.is_necramech());
        assert!(!voidrig.is_warframe());
        assert!(!voidrig.is_archwing());

        let excalibur = named("Excalibur");
        assert!(excalibur.is_warframe());
        assert!(!excalibur.is_necramech());
        assert!(!excalibur.is_archwing());

        let odonata = named("Odonata");
        assert!(odonata.is_archwing());
        assert!(!odonata.is_warframe());
        assert!(!odonata.is_necramech());

        let braton = named("Braton");
        assert!(!braton.is_warframe());

        assert_eq!(items.iter().filter(|item| item.is_necramech()).count(), 2);
        assert_eq!(items.iter().filter(|item| item.is_archwing()).count(), 5);
        assert_eq!(items.iter().filter(|item| item.is_warframe()).count(), 117);
    }

    #[test]
    fn dual_warframe_second_body() {
        let items = fixture_items();
        let by_unique_name = |unique_name: &str| {
            items
                .iter()
                .find(|item| item.unique_name == unique_name)
                .unwrap()
        };

        let orion = by_unique_name("/Lotus/Powersuits/SiriusOrion/OrionSuit");
        assert!(orion.is_alternate_suit_body());
        assert!(!orion.is_warframe());

        let sirius = by_unique_name("/Lotus/Powersuits/SiriusOrion/SiriusSuit");
        assert!(sirius.is_warframe());
        assert!(!sirius.is_alternate_suit_body());
    }

    #[test]
    fn orokin_reactor_or_catalyst() {
        assert!(item("Warframes", "Suits").takes_orokin_reactor());
        assert!(item("Sentinels", "Sentinels").takes_orokin_reactor());
        assert!(!item("Sentinels", "SentinelWeapons").takes_orokin_reactor());
        assert!(!item("Primary", "LongGuns").takes_orokin_reactor());
    }

    #[test]
    fn warframe_mastery_xp() {
        assert_eq!(item("Warframes", "Suits").mastery_xp(), 6000);
    }

    #[test]
    fn weapon_mastery_xp() {
        assert_eq!(item("Primary", "LongGuns").mastery_xp(), 3000);
    }

    #[test]
    fn necramech_mastery_xp() {
        assert_eq!(item("Warframes", "MechSuits").mastery_xp(), 8000);
    }

    #[test]
    fn level_cap_override() {
        let mut kuva = item("Primary", "LongGuns");
        kuva.max_level_cap = Some(40);
        assert_eq!(kuva.mastery_xp(), 4000);
        assert_eq!(kuva.affinity_cap(), 800_000);
    }

    #[test]
    fn rank_curve() {
        let warframe = item("Warframes", "Suits");
        assert_eq!(warframe.affinity_cap(), 900_000);
        assert_eq!(warframe.mastery_rank_at(0), 0);
        assert_eq!(warframe.mastery_rank_at(1000), 1);
        assert_eq!(warframe.mastery_rank_at(3999), 1);
        assert_eq!(warframe.mastery_rank_at(4000), 2);
        assert_eq!(warframe.mastery_rank_at(900_000), 30);
        assert_eq!(warframe.mastery_rank_at(20_000_000), 30);
        assert_eq!(
            warframe.mastery_per_rank() * warframe.mastery_rank_at(900_000),
            6000
        );

        let weapon = item("Melee", "Melee");
        assert_eq!(weapon.affinity_cap(), 450_000);
        assert_eq!(weapon.mastery_rank_at(500), 1);
        assert_eq!(weapon.mastery_rank_at(450_000), 30);
        assert_eq!(
            weapon.mastery_per_rank() * weapon.mastery_rank_at(450_000),
            3000
        );
    }

    #[test]
    fn level_from_affinity_matches_item() {
        let warframe = item("Warframes", "Suits");
        for affinity in [0_u64, 999, 1000, 3999, 4000, 899_999, 900_000, 5_000_000] {
            assert_eq!(
                mastery_level_from_affinity(affinity, true, warframe.affinity_cap()),
                u64::from(warframe.mastery_rank_at(affinity)),
                "affinity {affinity}"
            );
        }
    }

    #[test]
    fn arcane_rarity() {
        let arcanes: Vec<Item> =
            serde_json::from_str(include_str!("../tests/fixtures/arcanes.json")).unwrap();
        let rarity = |name: &str| {
            arcanes
                .iter()
                .find(|item| item.name == name)
                .unwrap()
                .rarity
        };
        assert_eq!(rarity("Arcane Nullifier"), Some(Rarity::Common));
        assert_eq!(rarity("Arcane Awakening"), Some(Rarity::Uncommon));
        assert_eq!(rarity("Molt Efficiency"), Some(Rarity::Rare));
        assert_eq!(rarity("Arcane Energize"), Some(Rarity::Legendary));
        assert_eq!(rarity("Arcane Survival"), None);
    }

    #[test]
    fn max_upgrade_rank() {
        let upgrades: Vec<Item> =
            serde_json::from_str(include_str!("../tests/fixtures/upgrade_ranks.json")).unwrap();
        let rank = |name: &str| {
            upgrades
                .iter()
                .find(|item| item.name == name)
                .unwrap()
                .max_upgrade_rank()
        };
        assert_eq!(rank("Serration"), 10);
        assert_eq!(rank("Primed Continuity"), 10);
        assert_eq!(rank("Corrupt Charge"), 3);
        assert_eq!(rank("Artillery Cheap Shot"), 5);
        assert_eq!(rank("Astral Autopsy"), 0);
        assert_eq!(rank("Arcane Energize"), 5);
        assert_eq!(rank("Exodia Brave"), 3);
        assert_eq!(item("Primary", "LongGuns").max_upgrade_rank(), 0);
    }

    #[test]
    fn skins_grant_no_mastery() {
        let items: Vec<Item> =
            serde_json::from_str(include_str!("../tests/fixtures/skins.json")).unwrap();
        let skins: Vec<&Item> = items.iter().filter(|item| item.is_skin()).collect();
        assert_eq!(skins.len(), 13);
        assert!(!skins.iter().any(|skin| skin.masterable()));
        assert!(skins.iter().all(|skin| skin.image_name.is_some()));
        assert!(skins.iter().all(|skin| skin.warframe_market.is_none()));
        assert!(!item("Warframes", "Suits").is_skin());
    }

    #[test]
    fn masterable_default() {
        let mut plain = item("Misc", "Misc");
        plain.masterable = None;
        assert!(!plain.masterable());
        assert!(item("Primary", "LongGuns").masterable());
    }

    #[test]
    fn store_item_to_type_path() {
        let store_path = "/Lotus/StoreItems/Types/Recipes/Weapons/WeaponParts/CedoPrimeReceiver";
        let type_path = store_item_to_type(store_path);
        assert_eq!(
            type_path,
            "/Lotus/Types/Recipes/Weapons/WeaponParts/CedoPrimeReceiver"
        );
        assert_eq!(
            store_item_to_type("/Lotus/Types/Items/MiscItems/Ferrite"),
            "/Lotus/Types/Items/MiscItems/Ferrite"
        );
    }
}
