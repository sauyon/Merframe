use std::collections::{HashMap, HashSet};

use crate::model::{CountedItem, EquipmentItem, Inventory, Upgrade};

pub const RELIC_PREFIX: &str = "/Lotus/Types/Game/Projections/";
pub const RIVEN_MARKER: &str = "/Randomized/";

#[derive(Debug, Clone, Copy)]
pub struct UpgradeSlot<'a> {
    pub item: &'a EquipmentItem,
    pub config: usize,
}

impl Inventory {
    pub fn equipment_categories(&self) -> [&[EquipmentItem]; 14] {
        [
            &self.suits,
            &self.long_guns,
            &self.pistols,
            &self.melee,
            &self.space_suits,
            &self.space_guns,
            &self.space_melee,
            &self.sentinels,
            &self.sentinel_weapons,
            &self.mech_suits,
            &self.hoverboards,
            &self.moa_pets,
            &self.kubrow_pets,
            &self.data_knives,
        ]
    }

    pub fn counted_categories(&self) -> [&[CountedItem]; 6] {
        [
            &self.misc_items,
            &self.recipes,
            &self.raw_upgrades,
            &self.consumables,
            &self.level_keys,
            &self.fusion_treasures,
        ]
    }

    pub fn counted_items(&self) -> impl Iterator<Item = &CountedItem> {
        self.counted_categories()
            .into_iter()
            .flat_map(<[CountedItem]>::iter)
    }

    pub fn equipment(&self) -> impl Iterator<Item = &EquipmentItem> {
        self.equipment_categories()
            .into_iter()
            .flat_map(<[EquipmentItem]>::iter)
    }

    pub fn owned_item_types(&self) -> HashSet<&str> {
        self.equipment()
            .chain(&self.ships)
            .map(|item| item.item_type.as_str())
            .collect()
    }

    pub fn owns(&self, item_type: &str) -> bool {
        let cosmetics = self.weapon_skins.iter().chain(&self.flavour_items);
        self.counted(item_type) > 0
            || self
                .equipment()
                .map(|item| &item.item_type)
                .chain(cosmetics.map(|item| &item.item_type))
                .chain(self.upgrades.iter().map(|upgrade| &upgrade.item_type))
                .chain(self.ship_decorations.iter().map(|item| &item.item_type))
                .any(|owned| owned == item_type)
    }

    pub fn is_founder(&self) -> bool {
        self.accolades.is_founder()
    }

    pub fn counted_index(&self) -> HashMap<&str, i64> {
        let mut index = HashMap::new();
        for item in self.counted_items() {
            *index.entry(item.item_type.as_str()).or_insert(0) += item.item_count;
        }

        index
    }

    pub fn affinity_index(&self) -> HashMap<&str, u64> {
        let pets = self.moa_pets.iter().chain(&self.kubrow_pets);
        let mut index = max_per_key(pets.map(|pet| (pet.item_type.as_str(), pet.xp)));
        for entry in &self.xp_info {
            index.insert(entry.item_type.as_str(), entry.xp);
        }

        index
    }

    pub fn subsumed_suits(&self) -> HashSet<&str> {
        self.infested_foundry
            .consumed_suits
            .iter()
            .map(|suit| suit.suit_type.as_str())
            .collect()
    }

    pub fn archon_shard_index(&self) -> HashMap<&str, u32> {
        max_per_key(
            self.suits
                .iter()
                .map(|suit| (suit.item_type.as_str(), suit.archon_shards())),
        )
    }

    pub fn counted(&self, item_type: &str) -> i64 {
        self.counted_items()
            .filter(|item| item.item_type == item_type)
            .map(|item| item.item_count)
            .sum()
    }

    pub fn mastered_xp(&self) -> impl Iterator<Item = (&str, u64)> {
        self.xp_info
            .iter()
            .map(|entry| (entry.item_type.as_str(), entry.xp))
    }

    pub fn relics(&self) -> impl Iterator<Item = (&str, i64)> {
        self.misc_items
            .iter()
            .filter(|item| item.item_type.starts_with(RELIC_PREFIX))
            .map(|item| (item.item_type.as_str(), item.item_count))
    }

    pub fn rivens(&self) -> impl Iterator<Item = &Upgrade> {
        self.upgrades
            .iter()
            .filter(|upgrade| upgrade.item_type.contains(RIVEN_MARKER))
    }

    pub fn pre_veiled_rivens(&self) -> impl Iterator<Item = &CountedItem> {
        self.raw_upgrades
            .iter()
            .filter(|item| item.item_type.contains(RIVEN_MARKER) && item.item_count > 0)
    }

    pub fn upgrade_slots(&self) -> HashMap<&str, Vec<UpgradeSlot<'_>>> {
        let mut slots: HashMap<&str, Vec<UpgradeSlot<'_>>> = HashMap::new();
        for item in self.equipment() {
            for (config, loadout) in item.configs.iter().enumerate() {
                for upgrade in &loadout.upgrades {
                    if upgrade.is_empty() {
                        continue;
                    }
                    slots
                        .entry(upgrade.as_str())
                        .or_default()
                        .push(UpgradeSlot { item, config });
                }
            }
        }
        slots
    }

    pub fn adapted_incarnons(&self) -> HashSet<&str> {
        self.equipment()
            .filter(|item| {
                item.skill_tree
                    .as_ref()
                    .is_some_and(|tree| !tree.is_empty())
            })
            .map(|item| item.item_type.as_str())
            .collect()
    }
}

fn max_per_key<'a, T: Ord + Default + Copy>(
    entries: impl Iterator<Item = (&'a str, T)>,
) -> HashMap<&'a str, T> {
    let mut index: HashMap<&str, T> = HashMap::new();
    for (key, value) in entries {
        let entry = index.entry(key).or_default();
        *entry = (*entry).max(value);
    }

    index
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::model::Inventory;
    use chrono::Datelike;

    pub(crate) const FIXTURE: &str = include_str!("../../../fixtures/inventory.json");

    fn fixture() -> Inventory {
        Inventory::parse(FIXTURE).unwrap()
    }

    pub(crate) fn patched(field: &str) -> Inventory {
        Inventory::parse(&FIXTURE.replacen('{', &format!("{{{field},"), 1)).unwrap()
    }

    #[test]
    fn golden_fixture_scalars() {
        let inv = fixture();
        assert_eq!(inv.player_level, 14);
        assert_eq!(inv.premium_credits, 5249);
        assert_eq!(inv.premium_credits_free, 0);
        assert_eq!(inv.regular_credits, 90_814_797);
        assert_eq!(inv.fusion_points, 189_082);
        assert_eq!(inv.trades_remaining, 6);
        assert_eq!(inv.reward_seed, 1_234_567_890_123_456_789);
        assert_eq!(inv.last_inventory_sync.as_str(), "6a9eeb1f000000000000c001");
        let created = inv.created.datetime();
        assert_eq!(
            (created.year(), created.month(), created.day()),
            (2020, 1, 15)
        );
    }

    #[test]
    fn golden_fixture_player_skills() {
        let inv = fixture();
        assert_eq!(inv.player_skills.tactical, 10);
        assert_eq!(inv.player_skills.piloting, 10);
        assert_eq!(inv.player_skills.gunnery, 10);
        assert_eq!(inv.player_skills.engineering, 10);
        assert_eq!(inv.player_skills.command, 10);
        assert_eq!(inv.player_skills.railjack(), 50);
        assert_eq!(inv.player_skills.drift_combat, 10);
        assert_eq!(inv.player_skills.drift_riding, 10);
        assert_eq!(inv.player_skills.drift_opportunity, 10);
        assert_eq!(inv.player_skills.drift_endurance, 10);
        assert_eq!(inv.player_skills.duviri(), 40);
    }

    #[test]
    fn missing_player_skills() {
        let inv = Inventory::parse(
            r#"{"PlayerLevel":0,"PremiumCredits":0,"PremiumCreditsFree":0,"RegularCredits":0,"FusionPoints":0,"TradesRemaining":0,"Created":{"$date":{"$numberLong":"0"}},"LastInventorySync":{"$oid":"000000000000000000000000"},"RewardSeed":0}"#,
        )
        .unwrap();
        assert_eq!(inv.player_skills.railjack(), 0);
        assert_eq!(inv.player_skills.duviri(), 0);
    }

    #[test]
    fn golden_fixture_collections() {
        let inv = fixture();
        assert_eq!(inv.suits.len(), 22);
        assert_eq!(inv.long_guns.len(), 18);
        assert_eq!(inv.pistols.len(), 14);
        assert_eq!(inv.melee.len(), 18);
        assert_eq!(inv.misc_items.len(), 118);
        assert_eq!(inv.recipes.len(), 40);
        assert_eq!(inv.raw_upgrades.len(), 72);
        assert_eq!(inv.upgrades.len(), 35);
        assert_eq!(inv.xp_info.len(), 81);
        assert_eq!(inv.pending_recipes.len(), 4);
        assert_eq!(inv.missions.len(), 182);
        assert_eq!(inv.affiliations.len(), 34);
        assert_eq!(inv.challenge_progress.len(), 20);
        assert_eq!(inv.focus_upgrades.len(), 20);
        assert_eq!(inv.boosters.len(), 6);
        assert_eq!(inv.data_knives.len(), 1);
        assert_eq!(inv.weapon_skins.len(), 24);
        assert_eq!(inv.flavour_items.len(), 14);
        assert_eq!(inv.kubrow_pet_prints.len(), 6);
        assert!(inv.load_out_presets.is_object());
    }

    #[test]
    fn pet_print_breed() {
        let inv = fixture();
        let breeds: Vec<&str> = inv
            .kubrow_pet_prints
            .iter()
            .map(|print| print.dominant_traits.personality.as_str())
            .collect();
        assert_eq!(breeds.len(), 6);
        assert!(breeds.contains(&"/Lotus/Types/Game/CatbrowPet/VampireCatbrowPetPowerSuit"));
        assert!(breeds.iter().all(|b| b.starts_with("/Lotus/Types/")));
    }

    #[test]
    fn equipment_ids_and_xp() {
        let inv = fixture();
        assert_eq!(inv.equipment().count(), 125);
        let trinity = inv
            .suits
            .iter()
            .find(|item| item.item_type == "/Lotus/Powersuits/Trinity/TrinityPrime")
            .unwrap();
        assert_eq!(trinity.features, Some(3));
        assert_eq!(trinity.item_id.as_str().len(), 24);
        let knife = &inv.data_knives[0];
        assert_eq!(knife.xp, 752_834);
        assert_eq!(knife.upgrade_ver, Some(101));
    }

    #[test]
    fn owned_item_types() {
        let inv = fixture();
        let owned = inv.owned_item_types();
        assert!(owned.contains("/Lotus/Powersuits/Trinity/TrinityPrime"));
        assert!(owned.contains("/Lotus/Types/Sentinels/SentinelWeapons/Gremlin"));
        assert!(owned.contains("/Lotus/Types/Vehicles/Hoverboard/HoverboardSuit"));
        assert!(owned.contains("/Lotus/Types/Items/Ships/BlueSkyShip"));
        assert!(!owned.contains("/Lotus/Types/Items/MiscItems/Ferrite"));
        assert!(owned.len() <= inv.equipment().count() + inv.ships.len());
    }

    #[test]
    fn owns_across_collections() {
        let inv = fixture();
        assert!(inv.owns("/Lotus/Powersuits/Trinity/TrinityPrime"));
        assert!(inv.owns("/Lotus/Types/Items/MiscItems/Ferrite"));
        assert!(inv.owns("/Lotus/Upgrades/Skins/Sigils/BossSigilJackal"));
        assert!(inv.owns("/Lotus/Types/StoreItems/AvatarImages/AvatarImageItem1"));
        assert!(inv.owns("/Lotus/Upgrades/Mods/Warframe/AvatarShieldMaxMod"));
        assert!(!inv.owns("/Lotus/Weapons/Corpus/LongGuns/Prisma/PrismaGrakata"));
    }

    #[test]
    fn owns_ship_decorations() {
        const BOBBLE_HEAD: &str = "/Lotus/Types/Items/ShipDecos/TeshinBobbleHead";
        let decorated = patched(&format!(
            r#""ShipDecorations":[{{"ItemType":"{BOBBLE_HEAD}","ItemCount":1}}]"#
        ));
        assert!(decorated.owns(BOBBLE_HEAD));
        assert!(!fixture().owns(BOBBLE_HEAD));
    }

    #[test]
    fn subsumed_suits() {
        let inv = fixture();
        let subsumed = inv.subsumed_suits();
        assert_eq!(subsumed.len(), 14);
        assert!(subsumed.contains("/Lotus/Powersuits/Excalibur/Excalibur"));
        assert!(!subsumed.contains("/Lotus/Powersuits/Cowgirl/MesaPrime"));
    }

    #[test]
    fn archon_shard_index() {
        let inv = fixture();
        let index = inv.archon_shard_index();
        assert_eq!(
            index.get("/Lotus/Powersuits/YinYang/EquinoxPrime").copied(),
            Some(5)
        );
        assert_eq!(
            index.get("/Lotus/Powersuits/Glass/GaraPrime").copied(),
            Some(2)
        );
        assert_eq!(
            index.get("/Lotus/Powersuits/Trinity/TrinityPrime").copied(),
            Some(0)
        );
        assert_eq!(index.get("/Lotus/Powersuits/Excalibur/Excalibur2"), None);
    }

    #[test]
    fn empty_archon_slots() {
        let inv = fixture();
        let choir = inv
            .suits
            .iter()
            .find(|suit| suit.item_type == "/Lotus/Powersuits/Choir/Choir")
            .unwrap();
        assert_eq!(choir.archon_crystal_upgrades.len(), 5);
        assert_eq!(choir.archon_shards(), 4);
        let wisp = inv
            .suits
            .iter()
            .find(|suit| suit.item_type == "/Lotus/Powersuits/Wisp/WispPrime")
            .unwrap();
        assert_eq!(wisp.archon_crystal_upgrades.len(), 3);
        assert_eq!(wisp.archon_shards(), 1);
    }

    #[test]
    fn counted_across_categories() {
        let inv = fixture();
        assert_eq!(
            inv.counted("/Lotus/Types/Items/MiscItems/Ferrite"),
            51_923_112
        );
        assert_eq!(
            inv.counted("/Lotus/Types/Recipes/Components/VorBoltRemoverBlueprint"),
            3
        );
        assert_eq!(inv.counted("/Lotus/Types/Restoratives/LisetAutoHack"), 1);
        assert_eq!(inv.counted("/Lotus/Types/Items/MiscItems/DoesNotExist"), 0);
    }

    #[test]
    fn counted_index() {
        let inv = fixture();
        let index = inv.counted_index();
        assert_eq!(
            index.get("/Lotus/Types/Items/MiscItems/Ferrite").copied(),
            Some(51_923_112)
        );
        assert_eq!(
            index
                .get("/Lotus/Types/Restoratives/LisetAutoHack")
                .copied(),
            Some(1)
        );
        assert_eq!(index.get("/Lotus/Types/Items/MiscItems/DoesNotExist"), None);
    }

    #[test]
    fn affinity_index_prefers_xp_info() {
        let inv = fixture();
        let index = inv.affinity_index();
        assert_eq!(
            index.get("/Lotus/Powersuits/Excalibur/Excalibur").copied(),
            Some(73_506_288)
        );
        for pet in inv.moa_pets.iter().chain(&inv.kubrow_pets) {
            assert!(index.contains_key(pet.item_type.as_str()));
        }
    }

    #[test]
    fn founder_accolade() {
        assert!(!fixture().is_founder());
        assert!(patched(r#""Accolades":{"Founder":4}"#).is_founder());
        assert!(!patched(r#""Accolades":{"Guide":true}"#).is_founder());
    }

    #[test]
    fn mastered_xp() {
        let inv = fixture();
        let excalibur = inv
            .mastered_xp()
            .find(|(item_type, _)| *item_type == "/Lotus/Powersuits/Excalibur/Excalibur")
            .unwrap();
        assert_eq!(excalibur.1, 73_506_288);
        assert_eq!(inv.mastered_xp().count(), 81);
    }

    #[test]
    fn relics() {
        let inv = fixture();
        let relics: Vec<_> = inv.relics().collect();
        assert_eq!(relics.len(), 26);
        assert!(
            relics
                .iter()
                .all(|(item_type, _)| item_type.starts_with("/Lotus/Types/Game/Projections/"))
        );
        assert!(relics.iter().all(|(_, count)| *count > 0));
    }

    #[test]
    fn pre_veiled_rivens() {
        let inv = fixture();
        let stacks: Vec<_> = inv.pre_veiled_rivens().collect();
        assert_eq!(stacks.len(), 8);
        assert_eq!(stacks.iter().map(|item| item.item_count).sum::<i64>(), 182);
        assert!(
            stacks
                .iter()
                .all(|item| item.item_type.contains("/Randomized/"))
        );
    }

    #[test]
    fn upgrade_slots() {
        let inv = fixture();
        let slots = inv.upgrade_slots();
        let equipped = inv
            .upgrades
            .iter()
            .filter(|upgrade| slots.contains_key(upgrade.item_id.as_str()))
            .count();
        assert!(equipped > 0);
        assert!(equipped < inv.upgrades.len());
        assert!(slots.values().all(|holders| !holders.is_empty()));
        assert!(!slots.contains_key(""));
        let trinity = slots.get("5bf0583058c949d97f403a39").unwrap();
        assert!(trinity.iter().any(|slot| {
            slot.item.item_type == "/Lotus/Powersuits/Trinity/TrinityPrime" && slot.config < 6
        }));
    }

    #[test]
    fn adapted_incarnons() {
        let inv = fixture();
        let adapted = inv.adapted_incarnons();
        assert_eq!(adapted.len(), 5);
        assert!(adapted.contains("/Lotus/Weapons/Tenno/LongGuns/PrimeSybaris/PrimeSybarisRifle"));
        assert!(!adapted.contains("/Lotus/Weapons/Tenno/Rifle/BratonPrime"));
    }

    #[test]
    fn riven_fingerprints() {
        let inv = fixture();
        let rivens: Vec<_> = inv.rivens().collect();
        assert_eq!(rivens.len(), 15);
        let mut unveiled = 0;
        for riven in &rivens {
            let fp = riven.fingerprint().unwrap();
            if fp.is_unveiled() {
                unveiled += 1;
                assert!(!fp.buffs.is_empty() || fp.lvl_req.is_some());
                assert!(fp.pol.is_some());
            }
        }
        assert_eq!(unveiled, 13);
    }

    #[test]
    fn unknown_top_level_keys() {
        assert_eq!(
            patched(r#""CompletelyNewKey":{"a":[1,2]}"#).player_level,
            14
        );
    }
}
