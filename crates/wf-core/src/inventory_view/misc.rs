use std::collections::BTreeMap;

use wf_data::{Item, catch_grade, catch_size};
use wf_inventory::{EquipmentItem, Inventory};

use super::{ItemIndex, ItemSummary, MiscRow, SculptureStars};
use crate::catalog::{Catalog, RELIC_PREFIX, is_fish};
use crate::identity::{
    ItemRecord, ItemTable, LEGENDARY_CORE, OPTICOR_VANDAL, OUBLIETTE_KEY, Variant,
};
use crate::view::View;

const SCULPTURE_SOCKETS: [(&str, u32, u32); 11] = [
    ("A", 3, 0b010),
    ("B", 3, 0b000),
    ("C", 4, 0b0010),
    ("D", 3, 0b100),
    ("E", 3, 0b001),
    ("F", 4, 0b0101),
    ("G", 3, 0b001),
    ("H", 3, 0b010),
    ("I", 3, 0b010),
    ("J", 3, 0b010),
    ("Entrati", 5, 0b00010),
];

fn sculpture_stars(item_type: &str, sockets: Option<u32>) -> Option<SculptureStars> {
    let suffix = item_type.strip_prefix("/Lotus/Types/Items/FusionTreasures/OroFusex")?;
    let (_, sockets_total, amber_mask) = SCULPTURE_SOCKETS
        .into_iter()
        .find(|(name, _, _)| *name == suffix)?;
    let filled = sockets.unwrap_or_default();
    Some(SculptureStars {
        amber_filled: (filled & amber_mask).count_ones(),
        cyan_filled: (filled & !amber_mask).count_ones(),
        amber_sockets: amber_mask.count_ones(),
        cyan_sockets: sockets_total - amber_mask.count_ones(),
    })
}

fn is_catchable_fish(item_type: &str) -> bool {
    is_fish(item_type) && !item_type.contains("Boot")
}

fn is_arcane_helmet(item_type: &str) -> bool {
    item_type.contains("/Lotus/Upgrades/Skins/") && item_type.contains("Helmet")
}

pub(super) fn is_misc(item_type: &str) -> bool {
    if item_type.starts_with(RELIC_PREFIX) {
        return false;
    }
    item_type == "/Lotus/Upgrades/Skins/Kubrows/Collars/PrimeKubrowCollarA"
        || is_oubliette_key(item_type)
        || item_type.starts_with("/Lotus/Types/Items/FusionTreasures")
        || is_scene(item_type)
        || item_type == LEGENDARY_CORE
        || item_type.contains("/Resources/Mechs/")
        || is_catchable_fish(item_type)
        || is_arcane_helmet(item_type)
}

fn is_emote(item_type: &str) -> bool {
    item_type.starts_with("/Lotus/Types/Items/Emotes")
}

fn is_syndicate_emote(item_type: &str) -> bool {
    item_type.starts_with("/Lotus/Types/Items/Emotes/Syndicate/")
}

fn is_scene(item_type: &str) -> bool {
    item_type.starts_with("/Lotus/Types/Items/MiscItems/PhotoboothTile")
}

fn is_oubliette_key(item_type: &str) -> bool {
    item_type == OUBLIETTE_KEY
}

fn misc_source_item<'a>(catalog: &'a Catalog, unique_name: &str) -> Option<&'a Item> {
    if let Some(item) = catalog.item(unique_name) {
        return Some(item);
    }
    let (base, _) = catch_grade(unique_name)?;
    catalog.item(&base)
}

fn is_tradable_misc(catalog: &Catalog, items: &ItemTable, unique_name: &str) -> bool {
    if items.known_name(unique_name).is_none() {
        return false;
    }
    if is_emote(unique_name) {
        return is_syndicate_emote(unique_name);
    }
    if unique_name.contains("ModFuser") || is_oubliette_key(unique_name) {
        return true;
    }
    if let Some(item) = misc_source_item(catalog, unique_name) {
        if item.is_skin() {
            return item.name.contains("Arcane");
        }
        return item.tradable || is_scene(unique_name);
    }
    items.get(unique_name).is_some_and(ItemRecord::tradable) || is_scene(unique_name)
}

pub(super) fn is_spare_weapon_stock(item_type: &str) -> bool {
    if item_type.contains("/Lotus/Weapons/Syndicates/CephalonSuda/Pistols/CSDroidArray") {
        return false;
    }
    item_type.contains("/Prisma")
        || item_type.contains("/Lotus/Weapons/Syndicates/")
        || item_type.contains("/VoidTrader")
        || item_type.contains(OPTICOR_VANDAL)
        || item_type
            .contains("/Lotus/Weapons/Tenno/Pistols/ConclaveLeverPistol/ConclaveLeverPistol")
}

pub(crate) fn spare_weapons(inventory: &Inventory) -> impl Iterator<Item = &EquipmentItem> {
    [
        inventory.long_guns.as_slice(),
        &inventory.pistols,
        &inventory.melee,
        &inventory.space_guns,
        &inventory.space_melee,
        &inventory.space_suits,
        &inventory.sentinel_weapons,
    ]
    .into_iter()
    .flat_map(<[EquipmentItem]>::iter)
}

pub(crate) fn misc(view: &View, index: &mut ItemIndex) -> Vec<MiscRow> {
    let mut rows = counted_rows(view, index);
    rows.extend(cosmetic_rows(view, index));
    rows.extend(pet_print_rows(view, index));
    rows.extend(spare_equipment_rows(view, index));
    rows.sort_by(|a, b| index[a.item].name.cmp(&index[b.item].name));
    rows
}

fn record_row(view: &View, index: &mut ItemIndex, record: &ItemRecord, count: i64) -> MiscRow {
    MiscRow {
        item: index.add_marked(
            ItemSummary::from(record),
            view.favourites.contains(&record.unique_name),
        ),
        count,
        plat: record
            .market_slug
            .as_deref()
            .and_then(|slug| view.prices.plat(slug)),
        market_subtype: None,
        stars: None,
    }
}

fn counted_rows(view: &View, index: &mut ItemIndex) -> Vec<MiscRow> {
    let View {
        account,
        catalog,
        items,
        ..
    } = *view;
    let inventory = &account.inventory;
    let mut counted: BTreeMap<(&str, Option<SculptureStars>), i64> = BTreeMap::new();
    for item in inventory
        .misc_items
        .iter()
        .chain(&inventory.fusion_treasures)
        .chain(&inventory.raw_upgrades)
        .chain(&inventory.level_keys)
    {
        if !is_misc(&item.item_type) || !is_tradable_misc(catalog, items, &item.item_type) {
            continue;
        }
        let stars = sculpture_stars(&item.item_type, item.sockets);
        *counted.entry((item.item_type.as_str(), stars)).or_insert(0) += item.item_count;
    }
    counted
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .map(|((unique_name, stars), count)| MiscRow {
            market_subtype: catch_size(unique_name),
            stars,
            ..record_row(view, index, &items.resolve(unique_name), count)
        })
        .collect()
}

fn cosmetic_rows(view: &View, index: &mut ItemIndex) -> Vec<MiscRow> {
    let View {
        account,
        catalog,
        items,
        ..
    } = *view;
    let inventory = &account.inventory;
    let mut counted: BTreeMap<&str, i64> = BTreeMap::new();
    for skin in &inventory.weapon_skins {
        if is_misc(&skin.item_type) && is_tradable_misc(catalog, items, &skin.item_type) {
            *counted.entry(skin.item_type.as_str()).or_insert(0) += 1;
        }
    }
    for flavour in &inventory.flavour_items {
        if is_tradable_misc(catalog, items, &flavour.item_type) {
            *counted.entry(flavour.item_type.as_str()).or_insert(0) += 1;
        }
    }
    counted
        .into_iter()
        .map(|(unique_name, count)| record_row(view, index, &items.resolve(unique_name), count))
        .collect()
}

fn pet_print_rows(view: &View, index: &mut ItemIndex) -> Vec<MiscRow> {
    let mut counted: BTreeMap<&str, i64> = BTreeMap::new();
    for print in &view.account.inventory.kubrow_pet_prints {
        *counted
            .entry(print.dominant_traits.personality.as_str())
            .or_insert(0) += 1;
    }
    counted
        .into_iter()
        .filter_map(|(unique_name, count)| {
            let imprint = view.items.variant(unique_name, Variant::Imprint)?;
            Some(record_row(view, index, imprint, count))
        })
        .collect()
}

fn spare_equipment_rows(view: &View, index: &mut ItemIndex) -> Vec<MiscRow> {
    let mut counted: BTreeMap<&str, i64> = BTreeMap::new();
    for owned in spare_weapons(&view.account.inventory) {
        if owned.xp == 0 && is_spare_weapon_stock(&owned.item_type) {
            *counted.entry(owned.item_type.as_str()).or_insert(0) += 1;
        }
    }
    counted
        .into_iter()
        .filter_map(|(unique_name, count)| {
            let record = view.items.get(unique_name)?;
            Some(record_row(view, index, record, count))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::super::tests::priced;
    use super::*;
    use crate::catalog::display_name_from_path;
    use crate::catalog::fixtures::{self, FERRITE, catalog_of};
    use crate::prices::FixedPrices;
    use crate::view::Fixture;

    const FACTION_ITEMS: &str = r#"[
      {"uniqueName":"/Lotus/Weapons/Corpus/Pistols/CrpHandRL/PrismaAngstrum",
       "name":"Prisma Angstrum","category":"Secondary","type":"Pistols","tradable":true,
       "imageName":"prisma-angstrum.png"},
      {"uniqueName":"/Lotus/Weapons/Syndicates/SteelMeridian/LongGuns/SMHek",
       "name":"Vaykor Hek","category":"Primary","type":"Shotgun","tradable":true,
       "imageName":"vaykor-hek.png"},
      {"uniqueName":"/Lotus/Types/Game/CatbrowPet/VampireCatbrowPetPowerSuit",
       "name":"Vasca Kavat","category":"Pets","type":"Pets","tradable":false,
       "imageName":"vasca-kavat.png"},
      {"uniqueName":"/Lotus/Types/Friendly/Pets/CreaturePets/ArmoredInfestedCatbrowPetPowerSuit",
       "name":"Panzer Vulpaphyla","category":"Pets","type":"Pets","tradable":false,
       "imageName":"panzer-vulpaphyla.png"},
      {"uniqueName":"/Lotus/Types/Friendly/Pets/CreaturePets/MedjayPredatorKubrowPetPowerSuit",
       "name":"Medjay Predasite","category":"Pets","type":"Pets","tradable":false,
       "imageName":"medjay-predasite.png"}
    ]"#;

    const GATE_ITEMS: &str = r#"[
      {"uniqueName":"/Lotus/Upgrades/Skins/Rhino/RhinoHelmetAltB",
       "name":"Arcane Vanguard Helmet","category":"Skins","type":"Skin","tradable":false,
       "imageName":"RhinoSeries3Helmet.png"},
      {"uniqueName":"/Lotus/Upgrades/Skins/Rhino/RhinoHelmetAltBStatless",
       "name":"Rhino Vanguard Helmet","category":"Skins","type":"Skin","tradable":false,
       "imageName":"RhinoSeries3Helmet.png"},
      {"uniqueName":"/Lotus/Types/StoreItems/AvatarImages/AvatarImageItem1",
       "name":"Excalibur Glyph","category":"Glyphs","type":"Glyph","tradable":false,
       "imageName":"Icon01.png"},
      {"uniqueName":"/Lotus/Types/Items/Fish/Eidolon/DayUncommonFishBItem",
       "name":"Mortus Lungfish","category":"Fish","type":"Fish","tradable":true,
       "imageName":"FishItemDayUncommonB.png"},
      {"uniqueName":"/Lotus/Types/Items/Fish/Eidolon/FishParts/DayUncommonFishBPartItem",
       "name":"Mortus Horn","category":"Misc","type":"Fish Part","tradable":false,
       "imageName":"FishResourceHorn.png"},
      {"uniqueName":"/Lotus/Types/Items/Emotes/ShawzinEmote",
       "name":"Shawzin","category":"Skins","type":"Emotes","tradable":false,
       "imageName":"ShawzinEmote.png"},
      {"uniqueName":"/Lotus/Types/Items/Emotes/Syndicate/AHCombatEmote",
       "name":"Arbiters Combat Emote","category":"Skins","type":"Emotes","tradable":false,
       "imageName":"AHCombatEmote.png"},
      {"uniqueName":"/Lotus/Types/Items/Emotes/Interalpha/InteralphaPrimeNarta",
       "name":"Interalpha Prime Narta","category":"Skins","type":"Emotes","tradable":true,
       "imageName":"InteralphaPrimeNarta.png"},
      {"uniqueName":"/Lotus/Types/Items/MiscItems/PhotoboothTileDrifterCamp",
       "name":"The Drifter Camp Scene","category":"Misc","type":"Captura","tradable":true,
       "imageName":"DrifterCamp.png"}
    ]"#;

    const GAMMACOR: &str = "/Lotus/Weapons/Syndicates/CephalonSuda/Pistols/CSDroidArray";

    const ZYLOK: &str = "/Lotus/Weapons/Tenno/Pistols/ConclaveLeverPistol/ConclaveLeverPistol";

    const SPARE_WEAPONS: &str = r#"[
      {"uniqueName":"/Lotus/Weapons/Corpus/LongGuns/CrpBFG/Vandal/VandalCrpBFG",
       "name":"Opticor Vandal","category":"Primary","type":"Rifle","tradable":true,
       "imageName":"OpticorVandal.png"},
      {"uniqueName":"/Lotus/Weapons/ClanTech/Chemical/FlameThrowerWraith",
       "name":"Ignis Wraith","category":"Primary","type":"Rifle","tradable":true,
       "imageName":"IgnisWraith.png"},
      {"uniqueName":"/Lotus/Weapons/Tenno/Pistols/ConclaveLeverPistol/ConclaveLeverPistol",
       "name":"Zylok","category":"Secondary","type":"Pistol","tradable":false,
       "imageName":"ConclaveLeverPistol.png"},
      {"uniqueName":"/Lotus/Weapons/Syndicates/CephalonSuda/Pistols/CSDroidArray",
       "name":"Gammacor","category":"Secondary","type":"Pistol","tradable":false,
       "imageName":"Gammacor.png"},
      {"uniqueName":"/Lotus/Types/Sentinels/SentinelPowersuits/PrismaShadePowerSuit",
       "name":"Prisma Shade","category":"Sentinels","type":"Sentinel","tradable":true,
       "imageName":"PrismaShade.png"}
    ]"#;

    #[test]
    fn tradable_oddments() {
        let fixture = misc_fixture();
        let (rows, index) = fixture.rows(misc);
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|row| row.count > 0));
        assert!(
            !rows
                .iter()
                .any(|row| index[row.item].unique_name.starts_with(RELIC_PREFIX))
        );
        assert!(
            !rows
                .iter()
                .any(|row| index[row.item].unique_name.ends_with("MiscItems/Ferrite")),
            "plain resources belong to the resources tab, not misc"
        );
        assert!(
            rows.iter().any(|row| index[row.item]
                .unique_name
                .starts_with("/Lotus/Types/Items/FusionTreasures")),
            "ayatan sculptures are misc"
        );
        let core = rows
            .iter()
            .find(|row| {
                index[row.item].unique_name == "/Lotus/Upgrades/Mods/Fusers/LegendaryModFuser"
            })
            .expect("Legendary Core");
        assert_eq!(index[core.item].name, "Legendary Core");
        assert_eq!(
            index[core.item].image_name.as_deref(),
            Some("game/legendary-core.png")
        );
        assert_eq!(
            index[core.item].market_slug.as_deref(),
            Some("legendary_fusion_core")
        );
    }

    #[test]
    fn no_owned_cosmetic_is_tradable() {
        let fixture = misc_fixture();
        let (rows, index) = fixture.rows(misc);

        let skins: HashSet<&str> = fixture
            .account
            .inventory
            .weapon_skins
            .iter()
            .map(|skin| skin.item_type.as_str())
            .filter(|item_type| is_misc(item_type))
            .collect();
        let flavour: HashSet<&str> = fixture
            .account
            .inventory
            .flavour_items
            .iter()
            .map(|flavour| flavour.item_type.as_str())
            .collect();
        assert_eq!(skins.len(), 5);
        assert_eq!(flavour.len(), 14);

        let cosmetics: Vec<&MiscRow> = rows
            .iter()
            .filter(|row| {
                skins.contains(index[row.item].unique_name.as_str())
                    || flavour.contains(index[row.item].unique_name.as_str())
            })
            .collect();
        assert!(
            cosmetics.is_empty(),
            "the owned emotes are not syndicate emotes and the export names no other cosmetic"
        );

        let listed = |unique_name: &str| {
            rows.iter()
                .any(|row| index[row.item].unique_name == unique_name)
        };
        assert!(
            !listed("/Lotus/Upgrades/Skins/Excalibur/ExcaliburHelmet"),
            "an ordinary helmet skin cannot be traded"
        );
        assert!(
            !listed("/Lotus/Types/StoreItems/AvatarImages/AvatarImageItem1"),
            "glyphs cannot be traded"
        );
        assert!(
            !listed("/Lotus/Upgrades/Skins/Kubrows/Collars/PrimeKubrowCollarA"),
            "the collar is a skin without Arcane in its name"
        );
        assert!(
            !listed("/Lotus/Upgrades/Skins/Armor/ArbiterOfHexisArmor/ArbiterOfHexisArmorA"),
            "armour pieces are not part of the misc selection"
        );
    }

    #[test]
    fn cosmetic_names_from_export() {
        let fixture = misc_fixture();
        let (rows, index) = fixture.rows(misc);
        let row = |unique_name: &str| {
            rows.iter()
                .find(|row| index[row.item].unique_name == unique_name)
                .cloned()
                .unwrap()
        };

        let scene = row("/Lotus/Types/Items/MiscItems/PhotoboothTileDrifterCamp");
        assert_eq!(index[scene.item].name, "The Drifter Camp Scene");
        assert!(index[scene.item].image_name.is_some());

        let owned_emotes: HashSet<&str> = fixture
            .account
            .inventory
            .flavour_items
            .iter()
            .map(|flavour| flavour.item_type.as_str())
            .filter(|item_type| is_emote(item_type))
            .collect();
        let listed_emotes = rows
            .iter()
            .filter(|row| is_emote(&index[row.item].unique_name))
            .count();
        assert_eq!(owned_emotes.len(), 4);
        assert_eq!(
            listed_emotes, 0,
            "none of the owned emotes is a syndicate emote, so none is traded"
        );
    }

    #[test]
    fn tradable_gate() {
        let catalog = catalog_of(GATE_ITEMS);
        let items = ItemTable::build(&catalog);
        let tradable = |unique_name: &str| is_tradable_misc(&catalog, &items, unique_name);

        assert!(tradable("/Lotus/Upgrades/Skins/Rhino/RhinoHelmetAltB"));
        assert!(!tradable(
            "/Lotus/Upgrades/Skins/Rhino/RhinoHelmetAltBStatless"
        ));
        assert!(!tradable(
            "/Lotus/Types/StoreItems/AvatarImages/AvatarImageItem1"
        ));
        assert!(tradable(
            "/Lotus/Types/Items/Fish/Eidolon/DayUncommonFishBItem"
        ));
        assert!(tradable(
            "/Lotus/Types/Items/Fish/Eidolon/DayUncommonFishBItemLarge"
        ));
        assert!(!tradable(
            "/Lotus/Types/Items/Fish/Eidolon/FishParts/DayUncommonFishBPartItem"
        ));
        assert!(tradable("/Lotus/Upgrades/Mods/Fusers/LegendaryModFuser"));
        assert!(tradable(
            "/Lotus/Types/Keys/Nightwave/GlassmakerBossFightKey"
        ));
        assert!(
            tradable("/Lotus/Types/Items/Emotes/Syndicate/AHCombatEmote"),
            "syndicate emotes are the traded ones although the export flags none of them"
        );
        assert!(!tradable("/Lotus/Types/Items/Emotes/ShawzinEmote"));
        assert!(
            !tradable("/Lotus/Types/Items/Emotes/Interalpha/InteralphaPrimeNarta"),
            "the one emote the export flags tradable is not a syndicate emote"
        );
        assert!(!tradable("/Lotus/Types/Game/ShipScenes/CorpusShipScene"));
        assert!(tradable(
            "/Lotus/Types/Items/MiscItems/PhotoboothTileDrifterCamp"
        ));
        assert!(
            !tradable("/Lotus/Upgrades/Skins/Horse/HorseHelmetDrapery"),
            "a curated skin without Arcane in its name is not tradable"
        );
        assert!(
            !tradable("/Lotus/Types/Items/MiscItems/NothingTheExportsKnow"),
            "a candidate no data source can name is dropped"
        );
    }

    #[test]
    fn spare_weapon_stock() {
        assert!(is_spare_weapon_stock(
            "/Lotus/Weapons/Corpus/Pistols/CrpHandRL/PrismaAngstrum"
        ));
        assert!(is_spare_weapon_stock(
            "/Lotus/Weapons/Syndicates/SteelMeridian/LongGuns/SMHek"
        ));
        assert!(is_spare_weapon_stock("/Lotus/Weapons/VoidTrader/VTDetron"));
        assert!(is_spare_weapon_stock(OPTICOR_VANDAL));
        assert!(
            is_spare_weapon_stock(ZYLOK),
            "the conclave pistol trades even though the export calls it untradable"
        );
        assert!(
            !is_spare_weapon_stock(GAMMACOR),
            "the plain Gammacor shares the Cephalon Suda path with its syndicate twin"
        );
        assert!(
            !is_spare_weapon_stock("/Lotus/Weapons/ClanTech/Chemical/FlameThrowerWraith"),
            "a wraith weapon the export calls tradable is not a syndicate weapon"
        );
        assert!(!is_spare_weapon_stock("/Lotus/Weapons/Tenno/Rifle/Rifle"));
    }

    #[test]
    fn spare_weapons_from_slots() {
        let fixture = priced(
            catalog_of(SPARE_WEAPONS),
            fixtures::inventory_stocked(
                &[
                    ("LongGuns", OPTICOR_VANDAL),
                    (
                        "LongGuns",
                        "/Lotus/Weapons/ClanTech/Chemical/FlameThrowerWraith",
                    ),
                    ("Pistols", ZYLOK),
                    ("Pistols", GAMMACOR),
                    (
                        "Sentinels",
                        "/Lotus/Types/Sentinels/SentinelPowersuits/PrismaShadePowerSuit",
                    ),
                ],
                &[],
            ),
        );
        let (rows, index) = fixture.rows(misc);
        let listed = |name: &str| rows.iter().any(|row| index[row.item].name == name);
        assert!(listed("Opticor Vandal"));
        assert!(listed("Zylok"));
        assert!(
            !listed("Ignis Wraith"),
            "a tradable wraith weapon is not a spare faction weapon"
        );
        assert!(!listed("Gammacor"));
        assert!(
            !listed("Prisma Shade"),
            "a sentinel is not one of the weapon slots the tab reads"
        );
    }

    #[test]
    fn faction_weapons_and_imprints() {
        let fixture = Fixture::new(catalog_of(FACTION_ITEMS), fixtures::inventory()).with_prices(
            FixedPrices::new([("prisma_angstrum", 32.0), ("vasca_kavat_imprint", 15.0)]),
        );
        let (rows, index) = fixture.rows(misc);

        let angstrum: Vec<&MiscRow> = rows
            .iter()
            .filter(|row| index[row.item].name == "Prisma Angstrum")
            .collect();
        assert_eq!(angstrum.len(), 1);
        assert_eq!(
            angstrum[0].count, 1,
            "the account owns two, one of them ranked"
        );
        assert_eq!(
            index[angstrum[0].item].market_slug.as_deref(),
            Some("prisma_angstrum")
        );
        assert_eq!(angstrum[0].plat, Some(32.0));
        assert!(
            !rows.iter().any(|row| index[row.item].name == "Vaykor Hek"),
            "a ranked weapon can no longer be traded"
        );

        let imprints: Vec<&MiscRow> = rows
            .iter()
            .filter(|row| index[row.item].name.ends_with(" Imprint"))
            .collect();
        assert_eq!(imprints.len(), 3);
        assert_eq!(imprints.iter().map(|row| row.count).sum::<i64>(), 6);
        let vasca = imprints
            .iter()
            .find(|row| index[row.item].name == "Vasca Kavat Imprint")
            .expect("Vasca Kavat");
        assert_eq!(vasca.count, 1);
        assert_eq!(
            index[vasca.item].market_slug.as_deref(),
            Some("vasca_kavat_imprint")
        );
        assert_eq!(vasca.plat, Some(15.0));
        assert_eq!(
            index[vasca.item].image_name.as_deref(),
            Some("vasca-kavat.png")
        );
        assert!(
            imprints
                .iter()
                .any(|row| index[row.item].name == "Panzer Vulpaphyla Imprint")
        );
    }

    fn misc_fixture() -> Fixture {
        priced(fixtures::misc_catalog(), fixtures::inventory())
    }

    fn misc_catalog_without(categories: &[&str]) -> Catalog {
        let entries: Vec<serde_json::Value> = serde_json::from_str(fixtures::MISC_ITEMS).unwrap();
        let kept: Vec<serde_json::Value> = entries
            .into_iter()
            .filter(|entry| !categories.contains(&entry["category"].as_str().unwrap_or_default()))
            .collect();
        let json = serde_json::to_string(&kept).unwrap();
        Catalog::from_json(&json, fixtures::RELICS, fixtures::COMPONENTS).unwrap()
    }

    fn path_derived(catalog: &Catalog, item: &ItemSummary) -> bool {
        catalog.item(&item.unique_name).is_none()
            && catalog.component(&item.unique_name).is_none()
            && item.name == display_name_from_path(&item.unique_name)
    }

    #[test]
    fn no_path_derived_names() {
        let fixture = misc_fixture();
        let (rows, index) = fixture.rows(misc);
        let unresolved: Vec<&str> = rows
            .iter()
            .filter(|row| path_derived(&fixture.catalog, &index[row.item]))
            .map(|row| index[row.item].unique_name.as_str())
            .collect();
        assert!(
            unresolved.is_empty(),
            "path-derived rows left: {unresolved:?}"
        );

        let (narrowed, narrowed_index) =
            priced(misc_catalog_without(&["Fish"]), fixtures::inventory()).rows(misc);
        assert!(
            !narrowed
                .iter()
                .any(|row| is_catchable_fish(&narrowed_index[row.item].unique_name)),
            "a catch no export names is dropped rather than listed by its path"
        );
        assert_eq!(rows.len() - narrowed.len(), 17);
    }

    #[test]
    fn fish_sizes_share_one_market_item() {
        let fixture = misc_fixture();
        let (rows, index) = fixture.rows(misc);
        let market = |unique_name: &str| {
            rows.iter()
                .find(|row| index[row.item].unique_name == unique_name)
                .map(|row| {
                    (
                        index[row.item].market_slug.as_deref().unwrap(),
                        row.market_subtype.as_deref(),
                    )
                })
        };
        assert_eq!(
            market("/Lotus/Types/Items/Fish/Eidolon/DayUncommonFishBItem"),
            Some(("mortus_lungfish", Some("small")))
        );
        assert_eq!(
            market("/Lotus/Types/Items/Fish/Eidolon/DayUncommonFishBItemLarge"),
            Some(("mortus_lungfish", Some("large")))
        );
        assert_eq!(
            market("/Lotus/Types/Items/Fish/Deimos/HybridRareAFishItemLarge"),
            Some(("aquapulmo", Some("magnificent")))
        );
        assert_eq!(
            market("/Lotus/Types/Items/Fish/Duviri/DuviriFishAItem"),
            Some(("inaak", None)),
            "Duviri fish have one size and no subtype"
        );
    }

    #[test]
    fn fish_names() {
        let fixture = misc_fixture();
        let (rows, index) = fixture.rows(misc);
        let name = |unique_name: &str| {
            rows.iter()
                .find(|row| index[row.item].unique_name == unique_name)
                .map(|row| index[row.item].name.as_str())
        };
        assert_eq!(
            name("/Lotus/Types/Items/Fish/Eidolon/DayUncommonFishBItem"),
            Some("Mortus Lungfish")
        );
        assert_eq!(
            name("/Lotus/Types/Items/Fish/Eidolon/DayUncommonFishBItemLarge"),
            Some("Mortus Lungfish (Large)")
        );
        assert_eq!(
            name("/Lotus/Types/Items/Fish/Deimos/InfestedCommonDFishItemMedium"),
            Some("Amniophysi (Medium)")
        );
        assert_eq!(
            name("/Lotus/Types/Items/Fish/Deimos/HybridRareAFishItemLarge"),
            Some("Aquapulmo (Magnificent)")
        );
        assert_eq!(
            name("/Lotus/Types/Items/Fish/Solaris/OrokinCoolRareFishAMediumItem"),
            Some("Tromyzon (Adorned)")
        );
        assert_eq!(
            name("/Lotus/Types/Items/Fish/Duviri/DuviriFishAItem"),
            Some("Inaak")
        );
    }

    #[test]
    fn curated_rows_without_image() {
        let fixture = misc_fixture();
        let (rows, index) = fixture.rows(misc);
        let without_image: Vec<(&str, &str)> = rows
            .iter()
            .filter(|row| index[row.item].image_name.is_none())
            .map(|row| {
                (
                    index[row.item].unique_name.as_str(),
                    index[row.item].name.as_str(),
                )
            })
            .collect();
        assert_eq!(
            without_image,
            [(
                "/Lotus/Types/Keys/Nightwave/GlassmakerBossFightKey",
                "Enter Nihil's Oubliette"
            )]
        );
    }

    #[test]
    fn sculptures_split_by_filled_stars() {
        let fixture = misc_fixture();
        let (rows, index) = fixture.rows(misc);
        let sah: Vec<_> = rows
            .iter()
            .filter(|row| {
                index[row.item].unique_name == "/Lotus/Types/Items/FusionTreasures/OroFusexA"
            })
            .map(|row| (row.stars, row.count))
            .collect();
        let sah_stars = |amber_filled, cyan_filled| {
            Some(SculptureStars {
                amber_filled,
                cyan_filled,
                amber_sockets: 1,
                cyan_sockets: 2,
            })
        };
        assert_eq!(sah, vec![(sah_stars(0, 0), 9), (sah_stars(1, 2), 27)]);
        let star = rows
            .iter()
            .find(|row| index[row.item].unique_name.ends_with("OroFusexOrnamentA"))
            .expect("Ayatan Cyan Star");
        assert_eq!(star.stars, None, "loose stars have no sockets");
    }

    #[test]
    fn sculpture_sockets_from_bitmask() {
        let kitha = sculpture_stars(
            "/Lotus/Types/Items/FusionTreasures/OroFusexEntrati",
            Some(0b1_0101),
        )
        .expect("Kitha");
        assert_eq!(
            kitha,
            SculptureStars {
                amber_filled: 0,
                cyan_filled: 3,
                amber_sockets: 1,
                cyan_sockets: 4,
            }
        );
        let ayr =
            sculpture_stars("/Lotus/Types/Items/FusionTreasures/OroFusexB", None).expect("Ayr");
        assert_eq!(
            ayr,
            SculptureStars {
                amber_filled: 0,
                cyan_filled: 0,
                amber_sockets: 0,
                cyan_sockets: 3,
            }
        );
        let vaya = sculpture_stars("/Lotus/Types/Items/FusionTreasures/OroFusexD", Some(0b100))
            .expect("Vaya");
        assert_eq!((vaya.amber_filled, vaya.cyan_filled), (1, 0));
        let anasa = sculpture_stars("/Lotus/Types/Items/FusionTreasures/OroFusexF", Some(0b0110))
            .expect("Anasa");
        assert_eq!((anasa.amber_filled, anasa.cyan_filled), (1, 1));
        assert_eq!(
            sculpture_stars(
                "/Lotus/Types/Items/FusionTreasures/OroFusexOrnamentB",
                Some(1)
            ),
            None
        );
    }

    #[test]
    fn is_misc_categories() {
        assert!(is_misc(
            "/Lotus/Types/Items/FusionTreasures/Ayatan/AyatanAnasaSculpture"
        ));
        assert!(is_misc(
            "/Lotus/Types/Items/MiscItems/PhotoboothTileSomething"
        ));
        assert!(is_misc(
            "/Lotus/Upgrades/Skins/Kubrows/Collars/PrimeKubrowCollarA"
        ));
        assert!(is_misc("/Lotus/Types/Items/Fish/EidolonFishA"));
        assert!(!is_misc("/Lotus/Types/Items/Fish/BootItem"));
        assert!(!is_misc(FERRITE));
        assert!(!is_misc(
            "/Lotus/Types/Game/Projections/T1VoidProjectionABronze"
        ));
    }

    fn misc_family(inventory: &Inventory, unique_name: &str) -> &'static str {
        if unique_name.starts_with("/Lotus/Types/Items/FusionTreasures") {
            return "sculptures";
        }
        if unique_name.contains("ModFuser") {
            return "fusers";
        }
        if is_oubliette_key(unique_name) {
            return "keys";
        }
        if is_scene(unique_name) {
            return "scenes";
        }
        if is_catchable_fish(unique_name) {
            return "fish";
        }
        if unique_name.contains("/Resources/Mechs/") {
            return "necramech resources";
        }
        if inventory
            .kubrow_pet_prints
            .iter()
            .any(|print| print.dominant_traits.personality == unique_name)
        {
            return "imprints";
        }
        if inventory
            .equipment()
            .any(|owned| owned.item_type == unique_name)
        {
            return "faction weapons";
        }
        if is_arcane_helmet(unique_name) {
            return "helmet skins";
        }
        if inventory
            .weapon_skins
            .iter()
            .any(|skin| skin.item_type == unique_name)
        {
            return "other skins";
        }
        "flavour items"
    }

    fn misc_families(
        inventory: &Inventory,
        rows: &[MiscRow],
        index: &ItemIndex,
    ) -> BTreeMap<&'static str, usize> {
        let mut families: BTreeMap<&'static str, usize> = BTreeMap::new();
        for row in rows {
            *families
                .entry(misc_family(inventory, &index[row.item].unique_name))
                .or_insert(0) += 1;
        }
        families
    }

    #[test]
    fn fixture_misc_families() {
        let fixture = misc_fixture();
        let (rows, index) = fixture.rows(misc);
        assert_eq!(rows.len(), 44);
        let expected: BTreeMap<&str, usize> = [
            ("faction weapons", 1),
            ("fish", 17),
            ("fusers", 1),
            ("imprints", 3),
            ("keys", 1),
            ("necramech resources", 5),
            ("scenes", 3),
            ("sculptures", 13),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            misc_families(&fixture.account.inventory, &rows, &index),
            expected,
            "helmet skins, flavour items outside the export, non-syndicate emotes and orbiter decorations stay out"
        );
    }
}
