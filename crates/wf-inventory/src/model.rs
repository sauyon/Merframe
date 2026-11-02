use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
use wf_mongo::{MongoDate, ObjectId};

use crate::riven::RivenFingerprint;
use crate::{InventoryError, Result};

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ArchonCrystalSlot {
    #[serde(rename_all = "PascalCase")]
    Installed {
        color: String,
        upgrade_type: String,
    },
    Empty([u8; 0]),
}

impl ArchonCrystalSlot {
    pub fn is_installed(&self) -> bool {
        matches!(self, Self::Installed { .. })
    }
}

/// A Helminth ability put in place of one of the warframe's own.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AbilityOverride {
    pub ability: String,
    /// Zero-based ability slot.
    pub index: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LoadoutConfig {
    #[serde(default)]
    pub upgrades: Vec<String>,
    #[serde(default)]
    pub ability_override: Option<AbilityOverride>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EquipmentItem {
    pub item_type: String,
    pub item_id: ObjectId,
    pub item_name: Option<String>,
    #[serde(default, rename = "XP")]
    pub xp: u64,
    pub features: Option<u32>,
    pub polarized: Option<u32>,
    pub upgrade_ver: Option<u32>,
    pub skill_tree: Option<String>,
    #[serde(default)]
    pub modular_parts: Vec<String>,
    #[serde(default)]
    pub configs: Vec<LoadoutConfig>,
    #[serde(default)]
    pub archon_crystal_upgrades: Vec<ArchonCrystalSlot>,
}

const OROKIN_UPGRADE: u32 = 1;
const EXILUS_ADAPTER: u32 = 2;

fn names_a_modular_item(part: &str) -> bool {
    part.contains("/Barrel")
        || part.contains("/Tip/")
        || part.ends_with("Deck")
        || part.contains("PetHead")
        || part.contains("PetPartHead")
}

impl EquipmentItem {
    pub fn identity_type(&self) -> &str {
        self.modular_parts
            .iter()
            .find(|part| names_a_modular_item(part))
            .unwrap_or(&self.item_type)
    }

    pub fn custom_name(&self) -> Option<&str> {
        self.item_name
            .as_deref()
            .and_then(|name| name.rsplit('|').next())
            .filter(|name| !name.is_empty())
    }

    pub fn has_orokin_upgrade(&self) -> bool {
        self.features.unwrap_or_default() & OROKIN_UPGRADE != 0
    }

    pub fn has_exilus_adapter(&self) -> bool {
        self.features.unwrap_or_default() & EXILUS_ADAPTER != 0
    }

    pub fn archon_shards(&self) -> u32 {
        self.archon_crystal_upgrades
            .iter()
            .map(|slot| u32::from(slot.is_installed()))
            .sum()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CountedItem {
    pub item_type: String,
    pub item_count: i64,
    #[serde(default)]
    pub sockets: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CosmeticItem {
    pub item_type: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PetTraits {
    pub personality: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PetPrint {
    pub dominant_traits: PetTraits,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Upgrade {
    pub item_type: String,
    pub item_id: ObjectId,
    pub upgrade_fingerprint: Option<String>,
}

impl Upgrade {
    pub fn fingerprint(&self) -> Option<RivenFingerprint> {
        RivenFingerprint::parse(self.upgrade_fingerprint.as_deref()?).ok()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PendingRecipe {
    pub item_type: String,
    pub completion_date: MongoDate,
    pub item_id: ObjectId,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XpInfo {
    pub item_type: String,
    #[serde(rename = "XP")]
    pub xp: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Mission {
    pub tag: String,
    pub completes: u32,
    pub tier: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlayerSkills {
    #[serde(default, rename = "LPS_TACTICAL")]
    pub tactical: u32,
    #[serde(default, rename = "LPS_PILOTING")]
    pub piloting: u32,
    #[serde(default, rename = "LPS_GUNNERY")]
    pub gunnery: u32,
    #[serde(default, rename = "LPS_ENGINEERING")]
    pub engineering: u32,
    #[serde(default, rename = "LPS_COMMAND")]
    pub command: u32,
    #[serde(default, rename = "LPS_DRIFT_COMBAT")]
    pub drift_combat: u32,
    #[serde(default, rename = "LPS_DRIFT_RIDING")]
    pub drift_riding: u32,
    #[serde(default, rename = "LPS_DRIFT_OPPORTUNITY")]
    pub drift_opportunity: u32,
    #[serde(default, rename = "LPS_DRIFT_ENDURANCE")]
    pub drift_endurance: u32,
}

impl PlayerSkills {
    pub fn railjack(&self) -> u32 {
        self.tactical + self.piloting + self.gunnery + self.engineering + self.command
    }

    pub fn duviri(&self) -> u32 {
        self.drift_combat + self.drift_riding + self.drift_opportunity + self.drift_endurance
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Affiliation {
    pub tag: String,
    #[serde(default)]
    pub standing: i64,
    #[serde(default)]
    pub title: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChallengeProgress {
    pub name: String,
    pub progress: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FocusUpgrade {
    pub item_type: String,
    #[serde(default)]
    pub level: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Booster {
    pub item_type: String,
    pub expiry_date: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConsumedSuit {
    #[serde(rename = "s")]
    pub suit_type: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct InfestedFoundry {
    #[serde(default)]
    pub consumed_suits: Vec<ConsumedSuit>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Accolades {
    pub founder: Option<serde_json::Value>,
}

impl Accolades {
    pub fn is_founder(&self) -> bool {
        match &self.founder {
            None | Some(serde_json::Value::Null) => false,
            Some(serde_json::Value::Bool(flag)) => *flag,
            Some(serde_json::Value::Number(level)) => level.as_f64().is_some_and(|v| v > 0.0),
            Some(serde_json::Value::String(text)) => !text.is_empty(),
            Some(_) => true,
        }
    }
}

fn typed_entries<'de, D, T>(deserializer: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    Vec::<Map<String, Value>>::deserialize(deserializer)?
        .into_iter()
        .filter(|entry| {
            let typed = entry.contains_key("ItemType");
            if !typed {
                tracing::warn!(
                    entry = std::any::type_name::<T>(),
                    fields = entry
                        .keys()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", "),
                    "Inventory entry without ItemType skipped"
                );
            }
            typed
        })
        .map(|entry| T::deserialize(Value::Object(entry)).map_err(D::Error::custom))
        .collect()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Inventory {
    pub player_level: u32,
    pub premium_credits: i64,
    pub premium_credits_free: i64,
    pub regular_credits: i64,
    pub fusion_points: i64,
    pub trades_remaining: u32,
    pub created: MongoDate,
    pub last_inventory_sync: ObjectId,
    pub reward_seed: i64,
    #[serde(default, deserialize_with = "typed_entries")]
    pub suits: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub long_guns: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub pistols: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub melee: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub space_suits: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub space_guns: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub space_melee: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub sentinels: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub sentinel_weapons: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub mech_suits: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub hoverboards: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub moa_pets: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub kubrow_pets: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub data_knives: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub crew_ship_harnesses: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub ships: Vec<EquipmentItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub misc_items: Vec<CountedItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub recipes: Vec<CountedItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub raw_upgrades: Vec<CountedItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub consumables: Vec<CountedItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub level_keys: Vec<CountedItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub fusion_treasures: Vec<CountedItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub ship_decorations: Vec<CountedItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub weapon_skins: Vec<CosmeticItem>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub flavour_items: Vec<CosmeticItem>,
    #[serde(default)]
    pub kubrow_pet_prints: Vec<PetPrint>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub upgrades: Vec<Upgrade>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub pending_recipes: Vec<PendingRecipe>,
    #[serde(default, rename = "XPInfo", deserialize_with = "typed_entries")]
    pub xp_info: Vec<XpInfo>,
    #[serde(default)]
    pub missions: Vec<Mission>,
    #[serde(default)]
    pub player_skills: PlayerSkills,
    #[serde(default)]
    pub affiliations: Vec<Affiliation>,
    #[serde(default)]
    pub challenge_progress: Vec<ChallengeProgress>,
    #[serde(default)]
    pub load_out_presets: serde_json::Value,
    #[serde(default)]
    pub current_load_out_ids: Vec<ObjectId>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub focus_upgrades: Vec<FocusUpgrade>,
    #[serde(default, deserialize_with = "typed_entries")]
    pub boosters: Vec<Booster>,
    #[serde(default)]
    pub accolades: Accolades,
    #[serde(default)]
    pub infested_foundry: InfestedFoundry,
}

impl Inventory {
    pub fn parse(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(InventoryError::from)
    }

    /// The warframe in the active loadout, with the index of the config it uses.
    pub fn equipped_suit(&self) -> Option<(&EquipmentItem, usize)> {
        let oid = |value: &Value| value.get("$oid").and_then(Value::as_str).map(str::to_owned);
        let current = self.current_load_out_ids.first()?;
        let suit = self
            .load_out_presets
            .get("NORMAL")?
            .as_array()?
            .iter()
            .find(|preset| preset.get("ItemId").and_then(oid).as_deref() == Some(current.as_str()))?
            .get("s")?;
        let id = suit.get("ItemId").and_then(oid)?;
        let config = suit
            .get("mod")
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
            .unwrap_or_default();
        let item = self.suits.iter().find(|item| item.item_id.as_str() == id)?;
        Some((item, config))
    }
}

#[cfg(test)]
mod tests {
    use super::EquipmentItem;
    use crate::views::tests::patched;

    #[test]
    fn equipped_suit_follows_the_current_preset() {
        let mut value: serde_json::Value =
            serde_json::from_str(crate::views::tests::FIXTURE).unwrap();
        let fields = serde_json::json!({
            "CurrentLoadOutIds": [{"$oid": "000000000000000000000002"}],
            "LoadOutPresets": {"NORMAL": [
                {"ItemId": {"$oid": "000000000000000000000001"},
                 "s": {"ItemId": {"$oid": "aaaaaaaaaaaaaaaaaaaaaaaa"}, "mod": 0}},
                {"ItemId": {"$oid": "000000000000000000000002"},
                 "s": {"ItemId": {"$oid": "bbbbbbbbbbbbbbbbbbbbbbbb"}, "mod": 1}}
            ]},
            "Suits": [
                {"ItemType": "/Lotus/Powersuits/Saryn/SarynPrime",
                 "ItemId": {"$oid": "aaaaaaaaaaaaaaaaaaaaaaaa"}},
                {"ItemType": "/Lotus/Powersuits/Fairy/TitaniaPrime",
                 "ItemId": {"$oid": "bbbbbbbbbbbbbbbbbbbbbbbb"},
                 "Configs": [{}, {"AbilityOverride": {
                     "Ability": "/Lotus/Powersuits/BrokenFrame/Abilities/BrokenRotAbility",
                     "Index": 2}}]}
            ]
        });
        for (key, field) in fields.as_object().unwrap() {
            value[key] = field.clone();
        }
        let inventory = super::Inventory::parse(&value.to_string()).unwrap();
        let (suit, config) = inventory.equipped_suit().unwrap();
        assert_eq!(suit.item_type, "/Lotus/Powersuits/Fairy/TitaniaPrime");
        assert_eq!(config, 1);
        let helminth = suit.configs[config].ability_override.as_ref().unwrap();
        assert_eq!(helminth.index, 2);
    }

    #[test]
    fn entries_without_an_item_type_are_skipped() {
        let inventory = patched(
            r#""ShipDecorations":[{"ItemCount":2},{"ItemType":"/Lotus/Types/Items/ShipDecos/TeshinBobbleHead","ItemCount":3}]"#,
        );
        assert_eq!(inventory.ship_decorations.len(), 1);
        assert_eq!(inventory.ship_decorations[0].item_count, 3);
    }

    fn equipment(fields: &str) -> EquipmentItem {
        serde_json::from_str(&format!(
            r#"{{"ItemType":"/Lotus/Weapons/Tenno/Rifle/BratonPrime","ItemId":{{"$oid":"5bf0583058c949d97f403a39"}}{fields}}}"#
        ))
        .unwrap()
    }

    #[test]
    fn custom_name() {
        assert_eq!(equipment("").custom_name(), None);
        assert_eq!(equipment(r#","ItemName":"""#).custom_name(), None);
        assert_eq!(
            equipment(r#","ItemName":"Test Rifle""#).custom_name(),
            Some("Test Rifle")
        );
        assert_eq!(
            equipment(r#","ItemName":"/Lotus/Language/Weapons/KuvaKohm|TEST NAME""#).custom_name(),
            Some("TEST NAME")
        );
    }

    #[test]
    fn identity_type() {
        assert_eq!(
            equipment("").identity_type(),
            "/Lotus/Weapons/Tenno/Rifle/BratonPrime"
        );
        let modular = [
            (
                "/Lotus/Weapons/Infested/Pistols/InfKitGun/Barrels/InfBarrelEgg/InfModularBarrelEggPart",
                "/Lotus/Weapons/SolarisUnited/Secondary/SUModularSecondarySet1/Clip/SUModularCritIICapIClipPart",
            ),
            (
                "/Lotus/Weapons/Ostron/Melee/ModularMelee02/Tip/TipNine",
                "/Lotus/Weapons/Ostron/Melee/ModularMelee01/Handle/HandleOne",
            ),
            (
                "/Lotus/Weapons/Corpus/OperatorAmplifiers/Set1/Barrel/CorpAmpSet1BarrelPartC",
                "/Lotus/Weapons/Corpus/OperatorAmplifiers/Set1/Grip/CorpAmpSet1GripPartC",
            ),
            (
                "/Lotus/Types/Vehicles/Hoverboard/HoverboardParts/PartComponents/HoverboardCorpusC/HoverboardCorpusCDeck",
                "/Lotus/Types/Vehicles/Hoverboard/HoverboardParts/PartComponents/HoverboardCorpusA/HoverboardCorpusAEngine",
            ),
            (
                "/Lotus/Types/Friendly/Pets/MoaPets/MoaPetParts/MoaPetHeadMelee",
                "/Lotus/Types/Friendly/Pets/MoaPets/MoaPetParts/MoaPetLegD",
            ),
            (
                "/Lotus/Types/Friendly/Pets/ZanukaPets/ZanukaPetParts/ZanukaPetPartHeadA",
                "/Lotus/Types/Friendly/Pets/ZanukaPets/ZanukaPetParts/ZanukaPetPartBodyA",
            ),
        ];
        for (naming, other) in modular {
            let item = equipment(&format!(r#","ModularParts":["{other}","{naming}"]"#));
            assert_eq!(item.identity_type(), naming);
        }
        let creature = equipment(
            r#","ModularParts":["/Lotus/Types/Friendly/Pets/CreaturePets/CreaturePetParts/Deimos/InfestedCritterAntigenB"]"#,
        );
        assert_eq!(
            creature.identity_type(),
            "/Lotus/Weapons/Tenno/Rifle/BratonPrime"
        );
    }

    #[test]
    fn feature_flags() {
        let bare = equipment("");
        assert!(!bare.has_orokin_upgrade() && !bare.has_exilus_adapter());
        let catalyst = equipment(r#","Features":33"#);
        assert!(catalyst.has_orokin_upgrade() && !catalyst.has_exilus_adapter());
        let exilus = equipment(r#","Features":2"#);
        assert!(!exilus.has_orokin_upgrade() && exilus.has_exilus_adapter());
        let both = equipment(r#","Features":547"#);
        assert!(both.has_orokin_upgrade() && both.has_exilus_adapter());
    }
}
