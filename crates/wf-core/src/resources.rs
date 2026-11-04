use std::collections::{BTreeMap, BTreeSet, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use wf_data::{VendorOffer, currency_vendors, vendor_currencies};
use wf_inventory::{ArchonCrystalSlot, Inventory};

use crate::catalog::{Catalog, Stock, component_image};
use crate::foundry::{self, CraftNode, FoundryItem, farmed_resource, is_blueprint};
use crate::identity::{ItemTable, part_identity};
use crate::view::View;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ResourceSource {
    Held,
    Craftable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ResourceScope {
    Mastery,
    All,
    Starred,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ResourceQuery {
    pub source: ResourceSource,
    pub scope: ResourceScope,
    pub kind: Option<String>,
    pub prime: Option<bool>,
    pub owned: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ResourceUse {
    pub unique_name: String,
    pub name: String,
    pub image_name: Option<String>,
    pub wiki_url: Option<String>,
    pub amount: i64,
    pub favourite: bool,
    pub ready_to_build: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ResourceRow {
    pub unique_name: String,
    pub name: String,
    pub image_name: Option<String>,
    pub owned: i64,
    pub required: i64,
    pub deficit: i64,
    pub used_by: Vec<ResourceUse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ShardHolder {
    pub item_id: String,
    pub name: String,
    pub image_name: Option<String>,
    pub normal: usize,
    pub tauforged: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ShardRow {
    pub unique_name: String,
    pub name: String,
    pub image_name: Option<String>,
    pub owned_normal: i64,
    pub owned_tauforged: i64,
    pub holders: Vec<ShardHolder>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct OfferCost {
    pub unique_name: String,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct CurrencyOffer {
    pub unique_name: String,
    pub name: String,
    pub image_name: Option<String>,
    pub quantity: i64,
    pub price: i64,
    pub also_costs: Vec<OfferCost>,
    pub credits: Option<i64>,
    pub affordable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct CurrencyVendor {
    pub id: String,
    pub vendor: String,
    pub location: Option<String>,
    pub wiki_url: Option<String>,
    pub offers: Vec<CurrencyOffer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct HeldResource {
    pub unique_name: String,
    pub name: String,
    pub image_name: Option<String>,
    pub owned: i64,
    pub vendors: Vec<CurrencyVendor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ResourcesTab {
    pub resources: Vec<ResourceRow>,
    pub holdings: Vec<HeldResource>,
    pub shards: Vec<ShardRow>,
    pub items: usize,
    pub credits: i64,
}

pub(crate) fn tab(
    view: &View<'_>,
    query: &ResourceQuery,
    include_founders: Option<bool>,
    now: DateTime<Utc>,
) -> ResourcesTab {
    let inventory = &view.account.inventory;
    let stock = &view.account.stock;
    let held = held_recipes(inventory);
    let items = foundry::items(view, None, include_founders, now);
    let mut rows: BTreeMap<String, ResourceRow> = BTreeMap::new();
    let mut counted = 0;
    let mut credits = 0;
    for item in items.iter().filter(|item| selected(item, &held, query)) {
        let Some(details) = foundry::details(view, &item.unique_name) else {
            continue;
        };
        let mut needed: BTreeMap<&str, (&CraftNode, i64)> = BTreeMap::new();
        for node in &details.tree {
            gather(view.catalog, node, 1, &mut needed);
        }
        for (node, amount) in needed.into_values() {
            let row = rows
                .entry(node.unique_name.clone())
                .or_insert_with(|| ResourceRow {
                    unique_name: node.unique_name.clone(),
                    name: node.name.clone(),
                    image_name: node.image_name.clone(),
                    owned: stock.count(&node.unique_name),
                    required: 0,
                    deficit: 0,
                    used_by: Vec::new(),
                });
            row.required += amount;
            row.used_by.push(ResourceUse {
                unique_name: item.unique_name.clone(),
                name: item.name.clone(),
                image_name: item.image_name.clone(),
                wiki_url: item.wiki_url.clone(),
                amount,
                favourite: item.favourite,
                ready_to_build: item.progress.ready_to_build,
            });
        }
        counted += 1;
        credits += details.summary.credits;
    }

    let mut resources: Vec<ResourceRow> = rows
        .into_values()
        .map(|mut row| {
            row.deficit = (row.required - row.owned).max(0);
            row.used_by.sort_by(|left, right| {
                right
                    .amount
                    .cmp(&left.amount)
                    .then_with(|| left.name.cmp(&right.name))
            });
            row
        })
        .collect();
    resources.sort_by(|left, right| left.name.cmp(&right.name));
    ResourcesTab {
        resources,
        holdings: holdings(inventory, view.catalog, stock),
        shards: shards(inventory, view.catalog, view.items, stock),
        items: counted,
        credits,
    }
}

// Misc also files captura, ship segments and weapon parts, so only these types
// count as resources there.
const HELD_TYPES: &[&str] = &[
    "Resource",
    "Gem",
    "Cut Gem",
    "Alloy",
    "Pet Resource",
    "Misc",
    "Fish Part",
    "Fish Bait",
    "Eidolon Shard",
    "Focus Lens",
    "Medallion",
    "Currency",
    "Ayatan Sculpture",
    "Ayatan Star",
    "Conservation Tag",
];

fn holdings<'a>(inventory: &'a Inventory, catalog: &Catalog, stock: &Stock) -> Vec<HeldResource> {
    let mut names: BTreeSet<&'a str> = inventory
        .misc_items
        .iter()
        .filter(|item| item.item_count > 0)
        .map(|item| item.item_type.as_str())
        .filter(|unique_name| {
            catalog.item(unique_name).is_some_and(|item| {
                item.category == "Resources" || HELD_TYPES.contains(&item.item_type.as_str())
            })
        })
        .collect();
    names.extend(
        vendor_currencies()
            .filter(|currency| stock.count(currency) > 0)
            .map(|currency| -> &'a str { currency }),
    );
    let mut rows: Vec<HeldResource> = names
        .into_iter()
        .filter_map(|unique_name| {
            let item = catalog.item(unique_name)?;
            Some(HeldResource {
                unique_name: unique_name.to_owned(),
                name: item.name.clone(),
                image_name: item.image_name.clone(),
                owned: stock.count(unique_name),
                vendors: currency_vendors(unique_name)
                    .map(|vendor| CurrencyVendor {
                        id: vendor.id.clone(),
                        vendor: vendor.name.clone(),
                        location: vendor.location.clone(),
                        wiki_url: vendor.wiki.clone(),
                        offers: vendor
                            .offers
                            .iter()
                            .filter_map(|offer| {
                                currency_offer(inventory, catalog, stock, offer, unique_name)
                            })
                            .collect(),
                    })
                    .collect(),
            })
        })
        .collect();
    rows.sort_by(|left, right| left.name.cmp(&right.name));
    rows
}

fn currency_offer(
    inventory: &Inventory,
    catalog: &Catalog,
    stock: &Stock,
    offer: &VendorOffer,
    currency: &str,
) -> Option<CurrencyOffer> {
    let price = offer.price(currency)?;
    let affordable = offer
        .prices
        .iter()
        .all(|(item_type, count)| stock.count(item_type) >= *count)
        && offer
            .credits
            .is_none_or(|credits| inventory.regular_credits >= credits);
    Some(CurrencyOffer {
        unique_name: offer.item.clone(),
        name: offer.name.clone(),
        image_name: image_of(catalog, &offer.item)
            .or_else(|| image_of(catalog, offer.builds.as_deref()?)),
        quantity: offer.quantity,
        price,
        also_costs: offer
            .prices
            .iter()
            .filter(|(item_type, _)| item_type != currency)
            .map(|(item_type, count)| OfferCost {
                unique_name: item_type.clone(),
                name: catalog.item(item_type).map_or_else(
                    || item_type.rsplit('/').next().unwrap_or(item_type).to_owned(),
                    |item| item.name.clone(),
                ),
                count: *count,
            })
            .collect(),
        credits: offer.credits,
        affordable,
    })
}

fn image_of(catalog: &Catalog, unique_name: &str) -> Option<String> {
    match catalog.item(unique_name) {
        Some(item) => item.image_name.clone(),
        None => catalog
            .component(unique_name)
            .and_then(|(item, component)| component_image(item, component)),
    }
}

const SHARD_COLOURS: [(&str, &str); 6] = [
    ("Amar", "ACC_RED"),
    ("Nira", "ACC_YELLOW"),
    ("Boreal", "ACC_BLUE"),
    ("Green", "ACC_GREEN"),
    ("Orange", "ACC_ORANGE"),
    ("Violet", "ACC_PURPLE"),
];

fn shards(
    inventory: &Inventory,
    catalog: &Catalog,
    items: &ItemTable,
    stock: &Stock,
) -> Vec<ShardRow> {
    SHARD_COLOURS
        .iter()
        .filter_map(|(tail, colour)| {
            let unique_name = format!("/Lotus/Types/Gameplay/NarmerSorties/ArchonCrystal{tail}");
            let item = catalog.item(&unique_name)?;
            let tauforged_colour = format!("{colour}_MYTHIC");
            let mut holders: Vec<ShardHolder> = inventory
                .suits
                .iter()
                .map(|suit| {
                    let installed = |wanted: &str| {
                        suit.archon_crystal_upgrades
                            .iter()
                            .filter(|slot| match slot {
                                ArchonCrystalSlot::Installed { color, .. } => color == wanted,
                                ArchonCrystalSlot::Empty(_) => false,
                            })
                            .count()
                    };
                    let record = items.resolve(&suit.item_type);
                    ShardHolder {
                        item_id: suit.item_id.as_str().to_owned(),
                        name: record.name.clone(),
                        image_name: record.image_name.clone(),
                        normal: installed(colour),
                        tauforged: installed(&tauforged_colour),
                    }
                })
                .filter(|holder| holder.normal + holder.tauforged > 0)
                .collect();
            holders.sort_by(|left, right| left.name.cmp(&right.name));
            Some(ShardRow {
                name: item.name.rsplit("> ").next()?.to_owned(),
                image_name: item.image_name.clone(),
                owned_normal: stock.count(&unique_name),
                owned_tauforged: stock.count(&format!("{unique_name}Mythic")),
                holders,
                unique_name,
            })
        })
        .collect()
}

fn gather<'a>(
    catalog: &Catalog,
    node: &'a CraftNode,
    runs: i64,
    needed: &mut BTreeMap<&'a str, (&'a CraftNode, i64)>,
) {
    let stocked_resource =
        node.stocked && catalog.item(&node.unique_name).is_some_and(farmed_resource);
    if !node.children.is_empty() && !stocked_resource {
        if node.stocked {
            return;
        }
        let missing = (node.short_by + node.crafts_queued) * runs;
        let crafts = (missing + node.per_craft - 1) / node.per_craft;
        for child in &node.children {
            gather(catalog, child, crafts, needed);
        }
        return;
    }
    if is_blueprint(&node.unique_name) {
        return;
    }
    needed
        .entry(node.unique_name.as_str())
        .or_insert((node, 0))
        .1 += node.required * runs;
}

fn held_recipes(inventory: &Inventory) -> HashSet<&str> {
    inventory
        .recipes
        .iter()
        .filter(|recipe| recipe.item_count > 0)
        .map(|recipe| part_identity(&recipe.item_type))
        .collect()
}

fn selected(item: &FoundryItem, held: &HashSet<&str>, query: &ResourceQuery) -> bool {
    if item.progress.pending {
        return false;
    }
    let in_scope = match query.scope {
        ResourceScope::Mastery => !item.progress.owned && item.mastered == Some(false),
        ResourceScope::All => true,
        ResourceScope::Starred => item.favourite,
    };
    in_scope
        && query.kind.as_deref().is_none_or(|kind| item.kind == kind)
        && query
            .prime
            .is_none_or(|prime| item.prime.is_some() == prime)
        && query.owned.is_none_or(|owned| item.progress.owned == owned)
        && (query.source == ResourceSource::Craftable || holds_a_recipe(held, item))
}

fn holds_a_recipe(held: &HashSet<&str>, item: &FoundryItem) -> bool {
    held.contains(part_identity(&item.unique_name))
        || item
            .components
            .iter()
            .any(|component| held.contains(part_identity(&component.unique_name)))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::{Value, json};

    use super::*;
    use crate::catalog::fixtures::{
        self, BRATON_PRIME, BRATON_PRIME_BARREL, BRATON_PRIME_BLUEPRINT, DUAL_KAMAS, EXCALIBUR,
        OROKIN_CELL, SINGLE_KAMA, at, drop_entries, entries, inventory_with, push_counted,
    };
    use crate::favourites::Favourites;
    use crate::view::Fixture;

    const NOW_MS: i64 = 1_788_807_069_000;
    const CERATA_BLUEPRINT: &str = "/Lotus/Weapons/ClanTech/Bio/PunctureGlaiveWeaponBlueprint";
    const MUTAGEN_MASS: &str = "/Lotus/Types/Items/Research/BioComponent";
    const MUTAGEN_MASS_BLUEPRINT: &str = "/Lotus/Types/Items/Research/BioComponentBlueprint";
    const MUTAGEN_SAMPLE: &str = "/Lotus/Types/Items/Research/BioFragment";

    fn resources(fixture: &Fixture, source: ResourceSource, scope: ResourceScope) -> ResourcesTab {
        tab(
            &fixture.view(),
            &ResourceQuery {
                source,
                scope,
                kind: None,
                prime: None,
                owned: None,
            },
            None,
            at(NOW_MS),
        )
    }

    fn holding(recipe: &str, count: i64) -> Inventory {
        inventory_with(|value| push_counted(value, "Recipes", recipe, count))
    }

    fn unbuild_braton_prime(value: &mut Value) {
        drop_entries(value, &["LongGuns", "XPInfo"], BRATON_PRIME);
    }

    fn names(tab: &ResourcesTab) -> Vec<&str> {
        tab.resources
            .iter()
            .flat_map(|row| &row.used_by)
            .map(|used| used.name.as_str())
            .collect::<BTreeSet<&str>>()
            .into_iter()
            .collect()
    }

    fn amounts<'a>(tab: &'a ResourcesTab, item: &str) -> Vec<(&'a str, i64)> {
        tab.resources
            .iter()
            .flat_map(|row| {
                row.used_by
                    .iter()
                    .filter(|used| used.unique_name == item)
                    .map(|used| (row.name.as_str(), used.amount))
            })
            .collect()
    }

    fn row<'a>(tab: &'a ResourcesTab, unique_name: &str) -> &'a ResourceRow {
        tab.resources
            .iter()
            .find(|row| row.unique_name == unique_name)
            .expect("resource row")
    }

    fn used_by(row: &ResourceRow) -> Vec<(&str, i64)> {
        row.used_by
            .iter()
            .map(|used| (used.name.as_str(), used.amount))
            .collect()
    }

    #[test]
    fn nothing_held_towards_mastery() {
        let tab = resources(
            &Fixture::new(fixtures::catalog(), fixtures::inventory()),
            ResourceSource::Held,
            ResourceScope::Mastery,
        );
        assert!(tab.resources.is_empty());
        assert_eq!(tab.items, 0);
        assert_eq!(tab.credits, 0);
    }

    #[test]
    fn holdings_ignore_demand() {
        let tab = resources(
            &Fixture::new(fixtures::catalog(), fixtures::inventory()),
            ResourceSource::Held,
            ResourceScope::Mastery,
        );
        let cell = tab
            .holdings
            .iter()
            .find(|held| held.unique_name == OROKIN_CELL)
            .expect("held resource");
        assert_eq!(cell.name, "Orokin Cell");
        assert_eq!(cell.owned, 3319);
        assert!(
            cell.vendors
                .iter()
                .flat_map(|vendor| &vendor.offers)
                .all(|offer| offer.price > 0
                    && offer
                        .also_costs
                        .iter()
                        .all(|cost| cost.unique_name != OROKIN_CELL)),
            "offers are priced in the held resource, other costs listed apart"
        );
    }

    #[test]
    fn braton_prime_requirements() {
        let tab = resources(
            &Fixture::new(fixtures::catalog(), holding(BRATON_PRIME_BLUEPRINT, 3)),
            ResourceSource::Held,
            ResourceScope::All,
        );

        assert_eq!(
            amounts(&tab, BRATON_PRIME),
            [("Orokin Cell", 10)],
            "the parts drop from relics and stay out of the resource list"
        );

        let cell = row(&tab, OROKIN_CELL);
        assert_eq!(cell.name, "Orokin Cell");
        assert_eq!(cell.image_name.as_deref(), Some("ComponentCell.png"));
        assert!(!cell.used_by[0].ready_to_build);
        assert_eq!(tab.items, 2);
        assert_eq!(cell.owned, 3319);
        assert_eq!(cell.required, 11);
        assert_eq!(cell.deficit, 0);
        assert_eq!(used_by(cell), [("Braton Prime", 10), ("Excalibur", 1)]);
        assert_eq!(tab.credits, 15_000 + 25_000);

        let stacked = resources(
            &Fixture::new(fixtures::catalog(), holding(BRATON_PRIME_BLUEPRINT, 1000)),
            ResourceSource::Held,
            ResourceScope::All,
        );
        assert_eq!(
            stacked, tab,
            "a stack of blueprints is still one item to build"
        );

        let with_skins = resources(
            &Fixture::new(
                fixtures::with_skins(fixtures::ITEMS),
                holding(BRATON_PRIME_BLUEPRINT, 3),
            ),
            ResourceSource::Held,
            ResourceScope::All,
        );
        assert_eq!(
            (
                &with_skins.resources,
                &with_skins.shards,
                with_skins.items,
                with_skins.credits
            ),
            (&tab.resources, &tab.shards, tab.items, tab.credits),
            "skins add held resources to the catalog but no demand"
        );
    }

    #[test]
    fn deficit_without_stock() {
        let inventory = fixtures::inventory_stocked(&[], &[("Recipes", BRATON_PRIME_BLUEPRINT, 1)]);
        let tab = resources(
            &Fixture::new(fixtures::catalog(), inventory),
            ResourceSource::Held,
            ResourceScope::All,
        );
        assert_eq!(names(&tab), ["Braton Prime"]);
        let cell = row(&tab, OROKIN_CELL);
        assert_eq!(cell.owned, 0);
        assert_eq!(cell.required, 10);
        assert_eq!(cell.deficit, 10);
    }

    #[test]
    fn scope_filters() {
        let scoped = |scope, favourites: Favourites| {
            resources(
                &Fixture {
                    favourites,
                    ..Fixture::new(fixtures::catalog(), inventory_with(unbuild_braton_prime))
                },
                ResourceSource::Craftable,
                scope,
            )
        };

        let mastery = scoped(ResourceScope::Mastery, Favourites::default());
        assert_eq!(names(&mastery), ["Braton Prime"]);
        assert_eq!(mastery.credits, 15_000);

        let all = scoped(ResourceScope::All, Favourites::default());
        assert_eq!(names(&all), ["Braton Prime", "Excalibur", "Trinity Prime"]);
        assert_eq!(row(&all, OROKIN_CELL).required, 12);

        let starred = scoped(
            ResourceScope::Starred,
            [EXCALIBUR.to_owned()].into_iter().collect(),
        );
        assert_eq!(names(&starred), ["Excalibur"]);
        assert!(row(&starred, OROKIN_CELL).used_by[0].favourite);
        assert_eq!(row(&starred, OROKIN_CELL).required, 1);
    }

    #[test]
    fn landing_craft_outside_mastery_scope() {
        let catalog = fixtures::landing_craft_catalog();
        let fixture = Fixture::new(catalog, fixtures::inventory());
        let scoped = |scope| resources(&fixture, ResourceSource::Craftable, scope);

        assert!(names(&scoped(ResourceScope::Mastery)).is_empty());
        assert_eq!(
            names(&scoped(ResourceScope::All)),
            ["Mantis", "Nightwave", "Xiphos"]
        );
    }

    #[test]
    fn kind_prime_and_owned_filters() {
        let fixture = Fixture::new(fixtures::catalog(), inventory_with(unbuild_braton_prime));
        let filtered = |kind: Option<&str>, prime, owned| {
            tab(
                &fixture.view(),
                &ResourceQuery {
                    source: ResourceSource::Craftable,
                    scope: ResourceScope::All,
                    kind: kind.map(str::to_owned),
                    prime,
                    owned,
                },
                None,
                at(NOW_MS),
            )
        };

        assert_eq!(
            names(&filtered(None, None, None)),
            ["Braton Prime", "Excalibur", "Trinity Prime"]
        );
        assert_eq!(
            names(&filtered(Some("warframe"), None, None)),
            ["Excalibur", "Trinity Prime"]
        );
        assert_eq!(
            names(&filtered(Some("primary"), None, None)),
            ["Braton Prime"]
        );
        assert_eq!(
            names(&filtered(None, Some(true), None)),
            ["Braton Prime", "Trinity Prime"]
        );
        assert_eq!(names(&filtered(None, Some(false), None)), ["Excalibur"]);
        assert_eq!(names(&filtered(None, None, Some(false))), ["Braton Prime"]);
        assert_eq!(
            names(&filtered(None, None, Some(true))),
            ["Excalibur", "Trinity Prime"]
        );
        assert!(names(&filtered(Some("warframe"), Some(true), Some(false))).is_empty());
    }

    #[test]
    fn source_filters() {
        let sourced = |inventory: &Inventory, source| {
            resources(
                &Fixture::new(fixtures::catalog(), inventory.clone()),
                source,
                ResourceScope::Mastery,
            )
        };

        let bare = inventory_with(unbuild_braton_prime);
        assert!(names(&sourced(&bare, ResourceSource::Held)).is_empty());
        assert_eq!(
            names(&sourced(&bare, ResourceSource::Craftable)),
            ["Braton Prime"]
        );

        let held = inventory_with(|value| {
            unbuild_braton_prime(value);
            push_counted(value, "Recipes", BRATON_PRIME_BLUEPRINT, 1);
        });
        let tab = sourced(&held, ResourceSource::Held);
        assert_eq!(names(&tab), ["Braton Prime"]);
    }

    #[test]
    fn a_part_recipe_selects_the_item() {
        let tab = resources(
            &Fixture::new(fixtures::catalog(), holding(BRATON_PRIME_BARREL, 2)),
            ResourceSource::Held,
            ResourceScope::All,
        );
        assert_eq!(
            amounts(&tab, BRATON_PRIME),
            [("Orokin Cell", 10)],
            "a part recipe brings the item in without a blueprint of its own"
        );
    }

    #[test]
    fn pending_builds_are_left_out() {
        let inventory = inventory_with(|value| {
            entries(value, "PendingRecipes").push(json!({
                "ItemType": BRATON_PRIME_BLUEPRINT,
                "CompletionDate": { "$date": { "$numberLong": "1789000000000" } },
                "ItemId": { "$oid": "000000000000000000000001" },
            }));
        });
        let tab = resources(
            &Fixture::new(fixtures::catalog(), inventory),
            ResourceSource::Craftable,
            ResourceScope::All,
        );
        assert_eq!(names(&tab), ["Excalibur", "Trinity Prime"]);
    }

    #[test]
    fn every_item_that_needs_a_resource_is_listed() {
        let tab = resources(
            &Fixture::new(fixtures::foundry_catalog(), fixtures::inventory()),
            ResourceSource::Craftable,
            ResourceScope::All,
        );
        assert_eq!(
            names(&tab),
            ["Banshee Prime", "Boar", "Dual Kamas", "Kama", "Skana"]
        );
        assert_eq!(tab.items, 5);
        assert!(
            tab.resources
                .windows(2)
                .all(|pair| pair[0].name <= pair[1].name)
        );

        let cell = row(&tab, OROKIN_CELL);
        assert_eq!(used_by(cell), [("Banshee Prime", 5), ("Dual Kamas", 1)]);
        assert_eq!(cell.required, 6);

        let circuits = tab
            .resources
            .iter()
            .find(|row| row.name == "Circuits")
            .expect("Circuits");
        assert_eq!(circuits.owned, 0);
        assert_eq!(circuits.required, 2700);
        assert_eq!(circuits.deficit, 2700);
        assert_eq!(
            used_by(circuits),
            [("Dual Kamas", 1800), ("Kama", 900)],
            "both Kama slots of the Dual Kamas recipe add up"
        );
        assert!(
            tab.resources
                .iter()
                .all(|row| !is_blueprint(&row.unique_name))
        );
    }

    #[test]
    fn a_built_part_costs_nothing() {
        let inventory = fixtures::inventory_stocked(&[("Melee", SINGLE_KAMA)], &[]);
        let tab = resources(
            &Fixture::new(fixtures::foundry_catalog(), inventory),
            ResourceSource::Craftable,
            ResourceScope::All,
        );
        assert!(amounts(&tab, DUAL_KAMAS).contains(&("Circuits", 900)));
    }

    #[test]
    fn a_built_part_is_not_a_resource() {
        let inventory =
            fixtures::inventory_stocked(&[("Melee", SINGLE_KAMA), ("Melee", SINGLE_KAMA)], &[]);
        let tab = resources(
            &Fixture::new(fixtures::foundry_catalog(), inventory),
            ResourceSource::Craftable,
            ResourceScope::All,
        );
        assert!(
            tab.resources
                .iter()
                .all(|row| row.unique_name != SINGLE_KAMA)
        );
        assert!(!amounts(&tab, DUAL_KAMAS).contains(&("Circuits", 900)));
    }

    #[test]
    fn a_stocked_intermediate_stays_listed() {
        let cerata = |masses| {
            resources(
                &Fixture::new(
                    fixtures::mastery_catalog(),
                    fixtures::inventory_stocked(
                        &[],
                        &[
                            ("Recipes", CERATA_BLUEPRINT, 1),
                            ("Recipes", MUTAGEN_MASS_BLUEPRINT, 1),
                            ("MiscItems", MUTAGEN_MASS, masses),
                        ],
                    ),
                ),
                ResourceSource::Held,
                ResourceScope::All,
            )
        };

        let stocked = cerata(60);
        let mass = row(&stocked, MUTAGEN_MASS);
        assert_eq!((mass.owned, mass.required, mass.deficit), (60, 4, 0));
        assert_eq!(used_by(mass), [("Cerata", 4)]);
        assert!(
            stocked
                .resources
                .iter()
                .all(|row| row.unique_name != MUTAGEN_SAMPLE)
        );

        let short = cerata(0);
        assert!(
            short
                .resources
                .iter()
                .all(|row| row.unique_name != MUTAGEN_MASS)
        );
        let sample = row(&short, MUTAGEN_SAMPLE);
        assert_eq!(sample.required, 40);
        assert_eq!(used_by(sample), [("Cerata", 40)]);
    }

    #[test]
    fn archon_shards_by_colour() {
        const AMBER: &str = "/Lotus/Types/Gameplay/NarmerSorties/ArchonCrystalNira";
        const TAUFORGED_AMBER: &str = "/Lotus/Types/Gameplay/NarmerSorties/ArchonCrystalNiraMythic";

        let tab = resources(
            &Fixture::new(
                fixtures::misc_catalog(),
                fixtures::inventory_owning(&[(AMBER, 7), (TAUFORGED_AMBER, 2)]),
            ),
            ResourceSource::Held,
            ResourceScope::Mastery,
        );
        let equipped = |row: &ShardRow| -> Vec<(String, usize, usize)> {
            row.holders
                .iter()
                .map(|holder| (holder.name.clone(), holder.normal, holder.tauforged))
                .collect()
        };

        let [crimson, amber, violet] = tab.shards.as_slice() else {
            panic!("three shard colours in the fixture export");
        };
        assert_eq!(amber.name, "Amber Archon Shard");
        assert_eq!(amber.image_name.as_deref(), Some("ArchonShardNira.png"));
        assert_eq!((amber.owned_normal, amber.owned_tauforged), (7, 2));
        assert_eq!(amber.holders.len(), 9);
        assert_eq!(
            equipped(violet),
            [("Dagath".to_owned(), 0, 4)],
            "the Violet shard sits in purple slots"
        );
        assert_eq!((crimson.owned_normal, crimson.owned_tauforged), (0, 0));
        assert_eq!(
            crimson
                .holders
                .iter()
                .map(|holder| holder.tauforged)
                .sum::<usize>(),
            13
        );
    }

    #[test]
    fn the_whole_catalog_is_walked() {
        let tab = resources(
            &Fixture::new(fixtures::mastery_catalog(), fixtures::inventory()),
            ResourceSource::Craftable,
            ResourceScope::All,
        );
        assert_eq!(names(&tab), ["Cerata", "Grimoire"]);
        assert_eq!(
            amounts(&tab, "/Lotus/Weapons/Tenno/Grimoire/TnGrimoire"),
            [
                ("Echo Voca", 15),
                ("Entrati Lanthorn", 10),
                ("Entrati Obols", 2000),
                ("Necracoil", 50),
            ]
        );
    }
}
