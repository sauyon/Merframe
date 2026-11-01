use std::collections::{BTreeMap, HashSet};

use wf_data::{Component, Item};

use super::{ItemIndex, ItemSummary, PartRow, SetComponent, SetLink, SetRow};
use crate::catalog::{Stock, names_a_prime};
use crate::identity::{ItemKind, ItemRecord, ItemTable, Variant, is_landing_craft_part};
use crate::prices::Prices;
use crate::view::View;

fn is_warframe_part(item_type: &str) -> bool {
    item_type.contains("/WarframeRecipes/")
        && !(item_type.contains("Beacon") && item_type.contains("Component"))
}

fn is_weapon_part(item_type: &str) -> bool {
    if item_type.contains("/Weapons/Ostron")
        || item_type.contains("/Weapons/Corpus")
        || item_type.contains("TeamAmmoBlueprint")
        || item_type.contains("/Weapons/Skins/")
    {
        return false;
    }
    if item_type.contains("/WeaponParts/") {
        return true;
    }
    item_type.contains("Blueprint")
        && (item_type.contains("/SentinelRecipes/") || item_type.contains("/Weapons/"))
}

pub(super) fn is_part_stock(item_type: &str) -> bool {
    let assembly = is_warframe_part(item_type)
        || item_type.contains("/Lotus/Types/Recipes/ArchwingRecipes/")
        || is_landing_craft_part(item_type);
    if assembly && item_type.to_lowercase().contains("blueprint") {
        return true;
    }
    is_weapon_part(item_type) || item_type.contains("/Archwing/Primary/")
}

pub(crate) fn parts(view: &View, index: &mut ItemIndex) -> Vec<PartRow> {
    let View {
        account,
        catalog,
        items,
        prices,
        favourites,
        ..
    } = *view;
    let inventory = &account.inventory;
    let building: HashSet<&str> = inventory
        .pending_recipes
        .iter()
        .map(|recipe| recipe.item_type.as_str())
        .collect();
    let mut counted: BTreeMap<&str, i64> = BTreeMap::new();
    for entry in inventory.misc_items.iter().chain(&inventory.recipes) {
        if is_part_stock(&entry.item_type) {
            *counted.entry(entry.item_type.as_str()).or_insert(0) += entry.item_count;
        }
    }
    let mut rows: Vec<PartRow> = counted
        .into_iter()
        .filter_map(|(unique_name, held)| {
            let record = items.lookup(unique_name)?;
            let ItemKind::Part { set, .. } = record.kind else {
                return None;
            };
            if !record.tradable() {
                return None;
            }

            let mut count = held;
            if building.contains(unique_name) {
                count -= 1;
            }
            if count <= 0 {
                return None;
            }

            let set = &items[set];
            let item = catalog.item(&set.unique_name)?;
            let part = record.held_as(unique_name);
            let favourite = favourites.any([unique_name, &record.unique_name, &set.unique_name]);
            Some(PartRow {
                item: index.add_marked(ItemSummary::from(&*part), favourite),
                count,
                prices: Prices::of(prices, part.market_slug.as_deref()),
                set: SetLink {
                    item: index.add(ItemSummary::from(set)),
                    complete: set_is_complete(items, &account.stock, item),
                },
                status: account.status(item),
            })
        })
        .collect();
    rows.sort_by(|a, b| index[a.item].name.cmp(&index[b.item].name));
    rows
}

fn set_parts<'a>(items: &'a ItemTable, item: &'a Item) -> impl Iterator<Item = &'a Component> {
    let prime = names_a_prime(&item.name);
    item.components.iter().flatten().filter(move |component| {
        crate::catalog::is_part(component)
            && (prime
                || items
                    .part(item, component)
                    .is_some_and(ItemRecord::tradable))
    })
}

fn set_is_complete(items: &ItemTable, stock: &Stock, item: &Item) -> bool {
    let mut parts = set_parts(items, item).peekable();
    parts.peek().is_some()
        && parts
            .all(|component| stock.count(&component.unique_name) >= i64::from(component.item_count))
}

pub(crate) struct HeldPart<'a> {
    record: &'a ItemRecord,
    owned: i64,
    required: i64,
}

impl HeldPart<'_> {
    fn enough(&self) -> bool {
        self.owned >= self.required
    }
}

pub(crate) fn held_parts<'a>(
    items: &'a ItemTable,
    item: &'a Item,
    stock: &Stock,
) -> Vec<HeldPart<'a>> {
    set_parts(items, item)
        .filter_map(|component| {
            Some(HeldPart {
                record: items.part(item, component)?,
                owned: stock.count(&component.unique_name),
                required: i64::from(component.item_count),
            })
        })
        .collect()
}

pub(crate) fn complete_sets(parts: &[HeldPart]) -> i64 {
    if !parts.iter().all(HeldPart::enough) {
        return 0;
    }
    parts
        .iter()
        .map(|part| part.owned / part.required.max(1))
        .min()
        .unwrap_or(0)
}

fn set_ducats(parts: &[HeldPart]) -> u32 {
    parts
        .iter()
        .filter_map(|part| {
            let ducats = part.record.ducats()?;
            u32::try_from(part.required)
                .ok()
                .map(|required| ducats * required)
        })
        .sum()
}

pub(crate) fn sets(view: &View, index: &mut ItemIndex) -> Vec<SetRow> {
    let View {
        account,
        catalog,
        items,
        prices,
        favourites,
        ..
    } = *view;
    let mut rows: Vec<SetRow> = catalog
        .items()
        .filter_map(|item| {
            let parts = held_parts(items, item, &account.stock);
            if parts.len() <= 1 {
                return None;
            }
            let owned_parts = parts.iter().filter(|part| part.enough()).count();
            if owned_parts == 0 {
                return None;
            }
            let complete = owned_parts == parts.len();
            let count = complete_sets(&parts);
            let set = items.variant(&item.unique_name, Variant::Set)?;
            Some(SetRow {
                item: index.add_marked(
                    ItemSummary::from(set),
                    favourites.contains(&item.unique_name),
                ),
                owned_parts,
                total_parts: parts.len(),
                count,
                complete,
                status: account.status(item),
                prices: Prices::of(prices, set.market_slug.as_deref()),
                ducats: set_ducats(&parts),
                components: parts
                    .iter()
                    .map(|part| SetComponent {
                        item: index.add(ItemSummary::from(part.record)),
                        owned: part.owned,
                        required: part.required,
                        enough: part.enough(),
                    })
                    .collect(),
            })
        })
        .collect();
    rows.sort_by(|a, b| index[a.item].name.cmp(&index[b.item].name));
    rows
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::tests::{holding, priced};
    use super::*;
    use crate::account::Account;
    use crate::catalog::Catalog;
    use crate::catalog::fixtures::{
        self, BRATON_PRIME, BRATON_PRIME_BARREL, BRATON_PRIME_STOCK, FERRITE, PRIME_PARTS,
        TRINITY_PRIME_SYSTEMS_BLUEPRINT, drop_entries, entries, inventory_with, push_counted,
    };
    use crate::identity::blueprint_of;
    use crate::inventory_view::tab;
    use crate::listings::MarketListings;
    use crate::view::Fixture;
    use wf_market::OrderType;

    #[test]
    fn owned_parts_only() {
        let fixture = priced(fixtures::catalog(), fixtures::inventory());
        let (rows, _) = fixture.rows(parts);
        assert!(
            rows.is_empty(),
            "the fixture account holds no tradable prime parts"
        );

        let stocked = holding(&[
            (BRATON_PRIME_BARREL, 3),
            (TRINITY_PRIME_SYSTEMS_BLUEPRINT, 1),
        ]);
        let (rows, index) = stocked.rows(parts);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.count > 0));
        assert!(
            !rows
                .iter()
                .any(|row| index[row.item].name.contains("Orokin Cell"))
        );
        assert!(
            !rows
                .iter()
                .any(|row| index[row.set.item].name == "Excalibur")
        );
        assert_eq!(index[rows[0].item].name, "Braton Prime Barrel");
        assert_eq!(index[rows[1].item].name, "Trinity Prime Systems");

        let barrel = &rows[0];
        assert_eq!(barrel.count, 3);
        assert_eq!(barrel.prices.sell, Some(8.0));
        assert_eq!(index[barrel.set.item].name, "Braton Prime");
        assert!(index[barrel.item].prime);
        assert!(index[barrel.item].image_name.is_some());
    }

    #[test]
    fn pending_part_not_stock() {
        let inventory = inventory_with(|value| {
            push_counted(value, "MiscItems", BRATON_PRIME_BARREL, 1);
            entries(value, "PendingRecipes").push(json!({
                "ItemType": BRATON_PRIME_BARREL,
                "CompletionDate": { "$date": { "$numberLong": "1700000000000" } },
                "ItemId": { "$oid": "000000000000000000000000" }
            }));
        });
        let fixture = priced(fixtures::catalog(), inventory);
        assert!(fixture.rows(parts).0.is_empty());
    }

    #[test]
    fn mastered_flag() {
        let mut fixture = holding(&PRIME_PARTS);
        let braton_set = |fixture: &Fixture| {
            let (found, index) = fixture.rows(sets);
            found
                .into_iter()
                .find(|row| index[row.item].name == "Braton Prime")
                .expect("Braton Prime")
        };
        let (rows, _) = fixture.rows(parts);
        assert!(
            rows.iter().all(|row| row.status.mastered),
            "the fixture account has Braton Prime and Trinity Prime at max rank"
        );
        assert!(braton_set(&fixture).status.mastered);

        fixture.account = Account::new(inventory_with(|value| {
            for entry in entries(value, "XPInfo") {
                if entry["ItemType"] == BRATON_PRIME {
                    entry["XP"] = json!(1_000);
                }
            }

            for (item_type, count) in PRIME_PARTS {
                push_counted(value, "MiscItems", item_type, count);
            }
        }));
        let (rows, index) = fixture.rows(parts);
        for row in &rows {
            assert_eq!(
                row.status.mastered,
                index[row.set.item].name == "Trinity Prime",
                "{} tracks the rank of {}",
                index[row.item].name,
                index[row.set.item].name
            );
        }
        let braton_set = braton_set(&fixture);
        assert!(!braton_set.status.mastered);
        assert!(
            braton_set.status.built,
            "the rifle is still owned, just not maxed"
        );
    }

    #[test]
    fn untradable_prime_part_stays_in_set() {
        let original = fixtures::COMPONENTS.replace("\r\n", "\n");
        let components = original.replacen(
            "\"uniqueName\": \"/Lotus/Types/Recipes/Weapons/WeaponParts/BratonPrimeBarrel\",\n    \"name\": \"Barrel\",\n    \"tradable\": true,",
            "\"uniqueName\": \"/Lotus/Types/Recipes/Weapons/WeaponParts/BratonPrimeBarrel\",\n    \"name\": \"Barrel\",\n    \"tradable\": false,",
            1,
        );
        assert_ne!(components, original);
        let catalog = Catalog::from_json(fixtures::ITEMS, fixtures::RELICS, &components).unwrap();
        let stocked = priced(
            catalog,
            fixtures::inventory_owning(&[(BRATON_PRIME_STOCK, 1)]),
        );
        let (rows, index) = stocked.rows(sets);
        let braton = &rows[0];
        assert_eq!(braton.total_parts, 4);
        let barrel = braton
            .components
            .iter()
            .find(|part| index[part.item].market_slug.as_deref() == Some("braton_prime_barrel"))
            .unwrap();
        assert_eq!(index[barrel.item].name, "Braton Prime Barrel");
        assert!(!barrel.enough);
    }

    #[test]
    fn landing_craft_set() {
        const MANTIS_AVIONICS: &str =
            "/Lotus/Types/Recipes/LandingCraftRecipes/Mantys/MantysStarChartComponent";
        const MANTIS_ENGINES: &str =
            "/Lotus/Types/Recipes/LandingCraftRecipes/Mantys/MantysPowerCoreComponent";
        let catalog = fixtures::landing_craft_catalog();
        let avionics_blueprint = blueprint_of(MANTIS_AVIONICS).unwrap();
        let engines_blueprint = blueprint_of(MANTIS_ENGINES).unwrap();
        let stocked = priced(
            catalog,
            fixtures::inventory_owning(&[(&avionics_blueprint, 2), (&engines_blueprint, 1)]),
        );
        let (rows, index) = stocked.rows(parts);
        assert_eq!(rows.len(), 2);
        assert_eq!(index[rows[0].item].name, "Mantis Avionics");
        assert_eq!(index[rows[0].set.item].name, "Mantis");
        assert!(rows[0].set.complete);

        let (sets, index) = stocked.rows(sets);
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].total_parts, 2);
        assert!(sets[0].complete);
        assert_eq!(index[sets[0].item].name, "Mantis");
    }

    #[test]
    fn partial_braton_set() {
        let stocked_inventory =
            fixtures::inventory_owning(&[(BRATON_PRIME_BARREL, 3), (BRATON_PRIME_STOCK, 1)]);
        assert!(
            priced(fixtures::catalog(), fixtures::inventory())
                .rows(sets)
                .0
                .is_empty()
        );

        let (rows, index) = priced(fixtures::catalog(), stocked_inventory.clone()).rows(sets);
        assert_eq!(rows.len(), 1);
        let braton = &rows[0];
        assert_eq!(index[braton.item].name, "Braton Prime");
        assert_eq!(braton.total_parts, 4);
        assert_eq!(braton.owned_parts, 2);
        assert!(!braton.complete);
        assert_eq!(braton.count, 0);
        assert_eq!(braton.prices.sell, Some(45.0));
        assert!(braton.status.built);
        assert_eq!(braton.components.len(), 4);
        assert_eq!(
            braton.components.iter().filter(|part| part.enough).count(),
            2
        );
        assert!(index[braton.item].image_name.is_some());

        let with_skins = || fixtures::with_skins(fixtures::ITEMS);
        assert!(
            priced(with_skins(), fixtures::inventory())
                .rows(sets)
                .0
                .is_empty()
        );
        assert_eq!(priced(with_skins(), stocked_inventory).rows(sets).0, rows);
    }

    #[test]
    fn set_ducats() {
        let stocked = holding(&[(BRATON_PRIME_BARREL, 3), (BRATON_PRIME_STOCK, 1)]);
        let (rows, _) = stocked.rows(sets);
        assert_eq!(rows[0].ducats, 110);
    }

    #[test]
    fn is_part_stock_paths() {
        assert!(is_part_stock(
            "/Lotus/Types/Recipes/WarframeRecipes/MagChassisBlueprint"
        ));
        assert!(
            !is_part_stock("/Lotus/Types/Recipes/WarframeRecipes/IronframeChassisComponent"),
            "a built warframe part is not a blueprint"
        );
        assert!(!is_part_stock(
            "/Lotus/Types/Recipes/WarframeRecipes/ChromaBeaconCComponent"
        ));
        assert!(is_part_stock(BRATON_PRIME_BARREL));
        assert!(is_part_stock(
            "/Lotus/Types/Recipes/LandingCraftRecipes/Mantys/MantysStarChartBlueprint"
        ));
        assert!(is_part_stock(
            "/Lotus/Types/Recipes/ArchwingRecipes/OdonataHarnessBlueprint"
        ));
        assert!(is_part_stock(
            "/Lotus/Types/Recipes/Weapons/Archwing/Primary/ArchGunBarrel"
        ));
        assert!(is_part_stock(
            "/Lotus/Types/Recipes/SentinelRecipes/HelperCarrierBlueprint"
        ));
        assert!(!is_part_stock(
            "/Lotus/Types/Recipes/Weapons/Ostron/OstronMeleeBlueprint"
        ));
        assert!(!is_part_stock(
            "/Lotus/Types/Recipes/Weapons/Corpus/CorpusRifleBlueprint"
        ));
        assert!(!is_part_stock(
            "/Lotus/Types/Recipes/Weapons/TeamAmmoBlueprintLarge"
        ));
        assert!(!is_part_stock(
            "/Lotus/Types/Recipes/Weapons/Skins/SkinBlueprint"
        ));
        assert!(!is_part_stock(FERRITE));

        let candidates = fixtures::inventory();
        let listed = candidates
            .misc_items
            .iter()
            .chain(&candidates.recipes)
            .filter(|item| is_part_stock(&item.item_type))
            .count();
        assert_eq!(listed, 32);
    }

    #[test]
    fn built_and_mastered_flags() {
        let stocked = holding(&[(BRATON_PRIME_BARREL, 3)]);
        let (found, index) = stocked.rows(parts);
        let row = found
            .into_iter()
            .find(|row| index[row.item].name == "Braton Prime Barrel")
            .unwrap();
        assert!(row.status.built);
        assert!(row.status.mastered);
        assert!(row.status.built || row.status.mastered);
        assert!(index[row.item].vault.is_some(), "the part names a prime");

        let inventory = inventory_with(|value| {
            drop_entries(value, &["LongGuns", "XPInfo"], BRATON_PRIME);
            push_counted(value, "MiscItems", BRATON_PRIME_BARREL, 3);
        });
        let never_built = priced(fixtures::catalog(), inventory);
        let (found, index) = never_built.rows(parts);
        let row = found
            .into_iter()
            .find(|row| index[row.item].name == "Braton Prime Barrel")
            .unwrap();
        assert!(!row.status.built);
        assert!(!row.status.mastered);
        assert!(!row.status.built && !row.status.mastered);
    }

    #[test]
    fn order_placed() {
        let mut fixture = Fixture {
            listings: MarketListings::new(
                [
                    ("braton_prime_barrel", OrderType::Sell),
                    ("trinity_prime_systems", OrderType::Buy),
                ],
                &[],
            ),
            ..holding(&[
                (BRATON_PRIME_BARREL, 3),
                (TRINITY_PRIME_SYSTEMS_BLUEPRINT, 1),
            ])
        }
        .with_market(&[
            fixtures::market_item(
                "braton_prime_barrel",
                "Braton Prime Barrel",
                BRATON_PRIME_BARREL,
                &["component"],
            ),
            fixtures::market_item(
                "trinity_prime_systems",
                "Trinity Prime Systems Blueprint",
                TRINITY_PRIME_SYSTEMS_BLUEPRINT,
                &["component", "blueprint"],
            ),
        ]);
        let listed = tab(&fixture.view());
        let ordered: Vec<(Option<&str>, bool, bool)> = listed
            .parts
            .iter()
            .map(|row| {
                (
                    listed.items[row.item].market_slug.as_deref(),
                    listed.selling.contains(&row.item),
                    listed.buying.contains(&row.item),
                )
            })
            .filter(|(_, sell, buy)| *sell || *buy)
            .collect();
        assert_eq!(
            ordered,
            [
                (Some("braton_prime_barrel"), true, false),
                (Some("trinity_prime_systems_blueprint"), false, true)
            ],
            "orders follow the item id"
        );
        fixture.listings = MarketListings::default();
        let unlisted = tab(&fixture.view());
        assert!(
            unlisted.selling.is_empty() && unlisted.buying.is_empty(),
            "without a market session no row claims an order"
        );
    }

    #[test]
    fn set_order_reaches_every_part() {
        let fixture = Fixture {
            listings: MarketListings::new([("braton_prime_set", OrderType::Sell)], &[]),
            ..holding(&[(BRATON_PRIME_BARREL, 3), (BRATON_PRIME_STOCK, 1)])
        }
        .with_market(&[fixtures::market_item(
            "braton_prime_set",
            "Braton Prime Set",
            BRATON_PRIME,
            &["set"],
        )]);
        let listed = tab(&fixture.view());

        assert_eq!(listed.parts.len(), 2, "only the two stocked parts survive");
        for row in &listed.parts {
            let name = &listed.items[row.item].name;
            assert_eq!(listed.items[row.set.item].name, "Braton Prime");
            assert!(
                listed.selling.contains(&row.set.item) && !listed.buying.contains(&row.set.item),
                "{name} reports the whole set as selling"
            );
            assert!(
                !listed.selling.contains(&row.item) && !listed.buying.contains(&row.item),
                "{name} itself is not listed"
            );
        }
    }
}
