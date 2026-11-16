use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use wf_data::{Component, Item, base_warframe_name, helminth_ability};
use wf_inventory::Inventory;
use wf_worldstate::WorldState;

use crate::catalog::{Catalog, is_umbra, names_a_prime, part_name};
use crate::identity::ItemTable;
use crate::mastery::{includes_founders, kind_of, masterable};
use crate::resurgence::prime_resurgence;
use crate::view::View;

use super::acquisition::{owned_relics, wiki_url};
use super::{
    FoundryComponent, FoundryItem, FoundryTab, Helminth, MasteryGate, PendingBuild, Prime,
    Progress, WorldTimer,
};

const CRAFTABLE_INTO_KINDS: [&str; 4] = ["primary", "secondary", "melee", "arch"];
const INCARNON_KINDS: [&str; 3] = ["primary", "secondary", "melee"];

pub(crate) fn tab(
    view: &View,
    world: Option<&WorldState>,
    include_founders: Option<bool>,
    now: DateTime<Utc>,
) -> FoundryTab {
    FoundryTab {
        pending: pending(&view.account.inventory, view.items, now),
        items: items(view, world, include_founders, now),
        timers: world.map_or_default(|state| timers(state, now)),
    }
}

pub(crate) fn pending(
    inventory: &Inventory,
    items: &ItemTable,
    now: DateTime<Utc>,
) -> Vec<PendingBuild> {
    let mut builds: Vec<PendingBuild> = inventory
        .pending_recipes
        .iter()
        .map(|recipe| {
            let completes_at = recipe.completion_date.datetime();
            let remaining_secs = completes_at.signed_duration_since(now).num_seconds();
            let record = items.resolve(&recipe.item_type);
            PendingBuild {
                name: record.name.clone(),
                image_name: record.image_name.clone(),
                item_type: recipe.item_type.clone(),
                completes_at,
                remaining_secs,
                ready: remaining_secs <= 0,
            }
        })
        .collect();
    builds.sort_by_key(|build| build.completes_at);
    builds
}

pub(crate) fn items(
    view: &View,
    world: Option<&WorldState>,
    include_founders: Option<bool>,
    now: DateTime<Utc>,
) -> Vec<FoundryItem> {
    let View {
        account,
        catalog,
        items,
        favourites,
        ..
    } = *view;
    let inventory = &account.inventory;
    let resurgence = prime_resurgence(world, now);
    let adapted = inventory.adapted_incarnons();
    let crafts_into = crafting_parents(catalog);
    let subsumed = subsumed_warframes(inventory, catalog);
    let shards = inventory.archon_shard_index();
    let pending: HashSet<&str> = inventory
        .pending_recipes
        .iter()
        .map(|recipe| {
            catalog
                .component_for_stock(&recipe.item_type)
                .map_or(recipe.item_type.as_str(), |(_, component)| {
                    component.unique_name.as_str()
                })
        })
        .collect();
    let landing_craft = catalog.items().filter(|item| item.is_landing_craft());
    let mut rows: Vec<FoundryItem> =
        masterable(catalog, includes_founders(inventory, include_founders))
            .chain(landing_craft)
            .map(|item| {
                let components = foundry_components(view, item);
                let pending_here = components
                    .iter()
                    .any(|component| pending.contains(component.unique_name.as_str()));
                let owned = account.built(&item.unique_name) || pending_here;
                let favourite = favourites.contains(&item.unique_name);
                let kind = kind_of(item);
                FoundryItem {
                    unique_name: item.unique_name.clone(),
                    name: item.name.clone(),
                    kind,
                    type_name: item.item_type.clone(),
                    image_name: item.image_name.clone(),
                    wiki_url: wiki_url(catalog, &item.unique_name),
                    prime: items
                        .get(&item.unique_name)
                        .and_then(|record| record.vault)
                        .map(|vault| Prime {
                            vault,
                            resurgence: resurgence.contains(item.unique_name.as_str()),
                        }),
                    mastered: item.masterable().then(|| account.mastered(item)),
                    progress: Progress {
                        owned,
                        pending: pending_here,
                        ready_to_build: !components.is_empty()
                            && components.iter().all(|component| component.enough),
                    },
                    crafts_into: CRAFTABLE_INTO_KINDS.contains(&kind).then(|| {
                        crafts_into
                            .get(item.unique_name.as_str())
                            .cloned()
                            .unwrap_or_default()
                    }),
                    mastery: MasteryGate {
                        required: item.mastery_req,
                        met: inventory.player_level >= item.mastery_req.unwrap_or_default(),
                    },
                    incarnon: INCARNON_KINDS
                        .contains(&kind)
                        .then(|| adapted.contains(item.unique_name.as_str())),
                    helminth: helminth(item, &subsumed),
                    archon_shards: (kind == "warframe").then(|| {
                        shards
                            .get(item.unique_name.as_str())
                            .copied()
                            .unwrap_or_default()
                    }),
                    favourite,
                    any_favourite: favourite
                        || components.iter().any(|component| component.favourite),
                    components,
                }
            })
            .collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

fn foundry_components(view: &View, item: &Item) -> Vec<FoundryComponent> {
    let View {
        account,
        catalog,
        items,
        favourites,
        ..
    } = *view;
    let recipe = item.components.as_deref().unwrap_or_default();
    account
        .foundry
        .fill_slots(recipe)
        .into_iter()
        .zip(recipe)
        .map(|(slot, component)| FoundryComponent {
            favourite: favourites.contains(&component.unique_name),
            unique_name: component.unique_name.clone(),
            name: component_name(catalog, item, component),
            image_name: items
                .part(item, component)
                .and_then(|part| part.image_name.clone()),
            owned: slot.filled,
            required: slot.required,
            enough: slot.satisfied,
            owned_relics: owned_relics(
                catalog,
                &account.stock,
                component.drops.as_deref().unwrap_or_default(),
            ),
        })
        .collect()
}

fn subsumed_warframes<'a>(inventory: &Inventory, catalog: &'a Catalog) -> HashSet<&'a str> {
    inventory
        .subsumed_suits()
        .into_iter()
        .filter_map(|item_type| catalog.item(item_type))
        .map(|item| base_warframe_name(&item.name))
        .collect()
}

fn helminth(item: &Item, subsumed: &HashSet<&str>) -> Option<Helminth> {
    if names_a_prime(&item.name) || is_umbra(item) {
        return None;
    }

    helminth_ability(item).map(|ability| Helminth {
        ability: ability.to_owned(),
        subsumed: subsumed.contains(base_warframe_name(&item.name)),
    })
}

fn crafting_parents(catalog: &Catalog) -> HashMap<&str, Vec<String>> {
    let mut parents: HashMap<&str, Vec<String>> = HashMap::new();
    for item in catalog.items() {
        for component in item.components.iter().flatten() {
            let names = parents.entry(component.unique_name.as_str()).or_default();
            if !names.contains(&item.name) {
                names.push(item.name.clone());
            }
        }
    }
    for names in parents.values_mut() {
        names.sort_unstable();
    }
    parents
}

fn is_farmed_stock(unique_name: &str, name: &str) -> bool {
    ["/Types/Items/", "/Resources/", "/Resource/"]
        .iter()
        .any(|marker| unique_name.contains(marker))
        || name == "Orokin Cell"
        || name.contains("Kavasa Prime")
}

pub(super) fn component_name(catalog: &Catalog, parent: &Item, component: &Component) -> String {
    if component.name == "Forma" {
        return "Forma Blueprint".to_owned();
    }
    if is_farmed_stock(&component.unique_name, &component.name) {
        return component.name.clone();
    }
    if catalog
        .item(&component.unique_name)
        .is_some_and(|item| CRAFTABLE_INTO_KINDS.contains(&kind_of(item)))
    {
        return component.name.clone();
    }
    part_name(parent, component)
}

pub(crate) fn timers(world: &WorldState, now: DateTime<Utc>) -> Vec<WorldTimer> {
    world
        .timers(now)
        .into_iter()
        .map(|timer| WorldTimer {
            name: timer.name.to_owned(),
            state: timer.state.to_owned(),
            ends_at: timer.ends,
            remaining_secs: timer.ends.signed_duration_since(now).num_seconds(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::catalog::fixtures::{
        self, DUAL_KAMAS, EXCALIBUR, SINGLE_KAMA, at, drop_entries, entries, inventory_with,
        push_counted,
    };
    use crate::foundry::details;
    use crate::view::Fixture;

    const BANSHEE_PRIME: &str = "/Lotus/Powersuits/Banshee/BansheePrime";
    const BOAR: &str = "/Lotus/Weapons/Tenno/Shotgun/FullAutoShotgun";
    const SKANA: &str = "/Lotus/Weapons/Tenno/Melee/LongSword/LongSword";
    const VARZIA_TRADING_MS: i64 = 1_788_807_069_000;

    fn foundry_fixture() -> Fixture {
        Fixture::new(fixtures::foundry_catalog(), fixtures::inventory())
    }

    fn foundry_rows(fixture: &Fixture) -> Vec<FoundryItem> {
        items(&fixture.view(), None, None, at(VARZIA_TRADING_MS))
    }

    fn world_state() -> WorldState {
        WorldState::parse(fixtures::WORLD_STATE).expect("world state")
    }

    fn row<'a>(rows: &'a [FoundryItem], unique_name: &str) -> &'a FoundryItem {
        rows.iter()
            .find(|row| row.unique_name == unique_name)
            .unwrap()
    }

    #[test]
    fn pending_builds() {
        let inventory = fixtures::inventory();
        let items = ItemTable::build(&fixtures::catalog());
        let builds = pending(&inventory, &items, at(1_787_000_000_000));
        assert_eq!(builds.len(), 4);
        assert_eq!(builds[0].name, "Bio Component Blueprint");
        assert_eq!(builds[0].remaining_secs, 0);
        assert!(builds[0].ready);
        assert_eq!(builds[1].remaining_secs, 6);
        assert!(!builds[1].ready);

        let forma = builds
            .iter()
            .find(|build| build.item_type.ends_with("FormaBlueprint"))
            .unwrap();
        assert_eq!(forma.completes_at, at(1_793_557_501_000));
        assert!(!forma.ready);
    }

    #[test]
    fn pending_part_blueprint_resolves_to_its_component() {
        let inventory = inventory_with(|value| {
            drop_entries(value, &["Suits"], EXCALIBUR);
            value["PendingRecipes"] = json!([{
                "ItemType": "/Lotus/Types/Recipes/WarframeRecipes/ExcaliburHelmetBlueprint",
                "CompletionDate": { "$date": { "$numberLong": "1787000100000" } },
                "ItemId": { "$oid": "0123456789abcdef01234567" }
            }]);
        });
        let fixture = Fixture::new(fixtures::catalog(), inventory);
        let now = at(1_787_000_000_000);

        let builds = pending(&fixture.account.inventory, &fixture.items, now);
        assert_eq!(builds[0].name, "Excalibur Neuroptics");
        assert_eq!(
            builds[0].image_name.as_deref(),
            Some("GenericWarframeHelmet.png")
        );

        let rows = items(&fixture.view(), None, None, now);
        let excalibur = row(&rows, EXCALIBUR);
        assert!(excalibur.progress.pending);
        assert!(excalibur.progress.owned);
    }

    #[test]
    fn favourite_flags() {
        let mut fixture = Fixture::new(fixtures::catalog(), fixtures::inventory());
        let braton_of = |rows: &[FoundryItem]| {
            rows.iter()
                .find(|row| row.name == "Braton Prime")
                .expect("Braton Prime")
                .clone()
        };

        let braton = braton_of(&foundry_rows(&fixture));
        assert!(!braton.favourite);
        assert!(!braton.any_favourite);
        assert!(
            braton
                .components
                .iter()
                .all(|component| !component.favourite)
        );

        let barrel = braton.components[0].unique_name.clone();
        fixture.favourites = [barrel.clone()].into_iter().collect();
        let child_starred = braton_of(&foundry_rows(&fixture));
        assert!(!child_starred.favourite);
        assert!(child_starred.any_favourite);

        fixture.favourites = [braton.unique_name.clone()].into_iter().collect();
        let own_starred = braton_of(&foundry_rows(&fixture));
        assert!(own_starred.favourite);
        assert!(own_starred.any_favourite);

        fixture.favourites = [braton.unique_name.clone(), barrel.clone()]
            .into_iter()
            .collect();
        let rows = foundry_rows(&fixture);
        let braton = braton_of(&rows);
        assert!(braton.favourite);
        assert!(braton.any_favourite);
        assert_eq!(
            braton
                .components
                .iter()
                .filter(|component| component.favourite)
                .map(|component| component.unique_name.as_str())
                .collect::<Vec<_>>(),
            vec![barrel.as_str()]
        );
        assert!(
            rows.iter()
                .filter(|row| row.name != "Braton Prime")
                .all(|row| !row.favourite)
        );
    }

    #[test]
    fn whole_catalog_listed() {
        let fixture = Fixture::new(
            fixtures::catalog(),
            fixtures::inventory_without_equipment(EXCALIBUR),
        );
        let rows = foundry_rows(&fixture);
        let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, vec!["Braton Prime", "Excalibur", "Trinity Prime"]);

        let excalibur = rows.iter().find(|row| row.name == "Excalibur").unwrap();
        assert_eq!(excalibur.kind, "warframe");
        assert!(!excalibur.progress.owned);
        assert!(excalibur.prime.is_none());
        assert!(!excalibur.progress.ready_to_build);
        let blueprint = excalibur
            .components
            .iter()
            .find(|component| component.name == "Excalibur Blueprint")
            .unwrap();
        assert_eq!(blueprint.owned, 0);
        assert!(!blueprint.enough);
        let neuroptics = excalibur
            .components
            .iter()
            .find(|component| component.name == "Excalibur Neuroptics")
            .unwrap();
        assert_eq!(neuroptics.owned, 7);
        assert!(neuroptics.enough);

        let trinity = rows.iter().find(|row| row.name == "Trinity Prime").unwrap();
        assert!(trinity.progress.owned);
        assert_eq!(trinity.mastered, Some(true));
        assert!(trinity.prime.is_some());

        let tree = details(&fixture.view(), &excalibur.unique_name)
            .unwrap()
            .tree;
        let cell = tree
            .iter()
            .find(|node| node.name == "Orokin Cell")
            .expect("Orokin Cell");
        assert_eq!(cell.owned, 3319);
        assert!(cell.children.is_empty());
        assert!(details(&fixture.view(), "/Lotus/Nope").is_none());

        let with_skins = Fixture::new(
            fixtures::with_skins(fixtures::ITEMS),
            fixture.account.inventory.clone(),
        );
        assert_eq!(foundry_rows(&with_skins), rows);
    }

    #[test]
    fn helminth_and_archon_shards() {
        let rows = foundry_rows(&Fixture::new(
            fixtures::mastery_catalog(),
            fixtures::inventory(),
        ));
        let row = |name: &str| rows.iter().find(|row| row.name == name).unwrap();

        let excalibur = row("Excalibur");
        assert_eq!(
            excalibur.helminth,
            Some(Helminth {
                ability: "Radial Blind".to_owned(),
                subsumed: true,
            })
        );
        assert_eq!(row("Excalibur Umbra").helminth, None);

        assert_eq!(row("Styanax Prime").helminth, None);

        assert_eq!(row("Equinox Prime").archon_shards, Some(5));
        assert_eq!(row("Gara Prime").archon_shards, Some(2));
        assert_eq!(row("Equinox").archon_shards, Some(0));

        let braton = row("Braton Prime");
        assert_eq!(braton.helminth, None);
        assert_eq!(braton.archon_shards, None);
    }

    #[test]
    fn landing_craft_rows() {
        let catalog = fixtures::landing_craft_catalog();
        let rows = foundry_rows(&Fixture::new(catalog, fixtures::inventory()));
        let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, vec!["Mantis", "Nightwave", "Xiphos"]);

        let mantis = &rows[0];
        assert_eq!(mantis.kind, "landing_craft");
        assert!(mantis.progress.owned);
        assert_eq!(mantis.mastered, None);
        assert_eq!(mantis.prime, None);
        assert_eq!(mantis.mastery.required, None);
        assert_eq!(mantis.components[0].name, "Mantis Avionics");
        assert_eq!(mantis.components[0].owned, 5);
        assert_eq!(mantis.components[2].name, "Mantis Blueprint");

        let xiphos = &rows[2];
        assert!(!xiphos.progress.owned);
        assert!(!xiphos.progress.ready_to_build);
    }

    #[test]
    fn necramech_rows() {
        let rows = foundry_rows(&Fixture::new(
            fixtures::mastery_catalog(),
            fixtures::inventory(),
        ));
        let row = |name: &str| rows.iter().find(|row| row.name == name).unwrap();

        for name in ["Voidrig", "Bonewidow"] {
            let mech = row(name);
            assert_eq!(mech.kind, "necramech");
            assert_eq!(mech.helminth, None);
            assert_eq!(mech.archon_shards, None);
        }

        assert_eq!(row("Excalibur").kind, "warframe");
        assert_eq!(row("Odonata").kind, "arch");
        assert!(
            !rows
                .iter()
                .filter(|row| row.kind == "warframe")
                .any(|row| row.name == "Voidrig")
        );
    }

    #[test]
    fn ready_to_build() {
        let inventory = inventory_with(|value| {
            drop_entries(value, &["Suits"], EXCALIBUR);
            for part in [
                "ExcaliburBlueprint",
                "ExcaliburChassisComponent",
                "ExcaliburHelmetComponent",
                "ExcaliburSystemsComponent",
            ] {
                push_counted(
                    value,
                    "Recipes",
                    &format!("/Lotus/Types/Recipes/WarframeRecipes/{part}"),
                    1,
                );
            }
        });
        let fixture = Fixture::new(fixtures::catalog(), inventory);
        let rows = foundry_rows(&fixture);
        let excalibur = rows.iter().find(|row| row.name == "Excalibur").unwrap();
        assert!(excalibur.progress.ready_to_build);
        assert!(
            details(&fixture.view(), &excalibur.unique_name)
                .unwrap()
                .tree
                .iter()
                .all(|node| node.stocked)
        );
    }

    #[test]
    fn world_state_timers() {
        let world = world_state();
        let now = at(VARZIA_TRADING_MS);
        let timers = timers(&world, now);
        assert!(!timers.is_empty());
        let cetus = timers.iter().find(|timer| timer.name == "Cetus").unwrap();
        assert!(cetus.state == "day" || cetus.state == "night");
        assert_eq!(
            cetus.remaining_secs,
            cetus.ends_at.signed_duration_since(now).num_seconds()
        );
    }

    fn inventory_at_rank(rank: u32) -> Inventory {
        inventory_with(|value| value["PlayerLevel"] = json!(rank))
    }

    #[test]
    fn mastery_requirement() {
        let veteran = foundry_rows(&Fixture::new(
            fixtures::foundry_catalog(),
            inventory_at_rank(36),
        ));
        assert!(veteran.iter().all(|row| row.mastery.met));
        assert_eq!(row(&veteran, BOAR).mastery.required, Some(2));

        let fresh = foundry_rows(&Fixture::new(
            fixtures::foundry_catalog(),
            inventory_at_rank(1),
        ));
        assert!(row(&fresh, SKANA).mastery.met);
        assert!(row(&fresh, DUAL_KAMAS).mastery.met);
        assert!(!row(&fresh, BOAR).mastery.met);
    }

    fn resurgent(rows: &[FoundryItem], unique_name: &str) -> bool {
        row(rows, unique_name)
            .prime
            .is_some_and(|prime| prime.resurgence)
    }

    #[test]
    fn varzia_resurgence() {
        let fixture = foundry_fixture();
        let world = world_state();

        let trading = items(&fixture.view(), Some(&world), None, at(VARZIA_TRADING_MS));
        assert!(resurgent(&trading, BANSHEE_PRIME));
        assert!(!resurgent(&trading, DUAL_KAMAS));

        let gone = items(&fixture.view(), Some(&world), None, at(1_790_877_600_000));
        assert!(!resurgent(&gone, BANSHEE_PRIME));

        let offline = items(&fixture.view(), None, None, at(VARZIA_TRADING_MS));
        assert!(!resurgent(&offline, BANSHEE_PRIME));
    }

    #[test]
    fn incarnon_adapted_copy() {
        let untouched = foundry_rows(&foundry_fixture());
        assert_eq!(row(&untouched, SKANA).incarnon, Some(false));
        assert_eq!(row(&untouched, BOAR).incarnon, Some(false));

        let adapted = inventory_with(|value| {
            entries(value, "Melee").push(json!({
                "ItemType": SKANA,
                "ItemId": { "$oid": "00000000000000000000beef" },
                "XP": 0,
                "SkillTree": "0121",
            }));
        });
        let rows = foundry_rows(&Fixture::new(fixtures::foundry_catalog(), adapted));
        assert_eq!(row(&rows, SKANA).incarnon, Some(true));
        assert_eq!(row(&rows, BOAR).incarnon, Some(false));
        assert_eq!(row(&rows, DUAL_KAMAS).incarnon, Some(false));
        assert_eq!(row(&rows, BANSHEE_PRIME).incarnon, None);
    }

    #[test]
    fn founders_items() {
        let founders = [
            "/Lotus/Powersuits/Excalibur/ExcaliburPrime",
            "/Lotus/Weapons/Tenno/Pistol/LatoPrime",
            "/Lotus/Weapons/Tenno/Melee/LongSword/SkanaPrime",
        ];
        let listed = |inventory: &Inventory, chosen: Option<bool>| -> usize {
            let rows = items(
                &Fixture::new(fixtures::mastery_catalog(), inventory.clone()).view(),
                None,
                chosen,
                at(VARZIA_TRADING_MS),
            );
            rows.iter()
                .filter(|row| founders.contains(&row.unique_name.as_str()))
                .count()
        };

        let ordinary = fixtures::inventory();
        assert!(!ordinary.is_founder());
        assert_eq!(listed(&ordinary, None), 0);
        assert_eq!(listed(&ordinary, Some(true)), 3);

        let founder = fixtures::founder_inventory();
        assert!(founder.is_founder());
        assert_eq!(listed(&founder, None), 3);
        assert_eq!(listed(&founder, Some(false)), 0);
    }

    #[test]
    fn single_kama_crafts_into_dual() {
        let rows = foundry_rows(&foundry_fixture());
        let kama = row(&rows, SINGLE_KAMA);
        assert_eq!(kama.crafts_into, Some(vec!["Dual Kamas".to_owned()]));
        assert_eq!(row(&rows, DUAL_KAMAS).crafts_into, Some(vec![]));
        assert_eq!(row(&rows, BANSHEE_PRIME).crafts_into, None);
    }

    #[test]
    fn cards_carry_a_wiki_link_and_the_owned_relics() {
        let inventory = fixtures::inventory_stocked(
            &[],
            &[(
                "MiscItems",
                "/Lotus/Types/Game/Projections/T4VoidProjectionEGold",
                3,
            )],
        );
        let rows = foundry_rows(&Fixture::new(fixtures::catalog(), inventory));
        let braton = rows.iter().find(|row| row.name == "Braton Prime").unwrap();
        assert_eq!(
            braton.wiki_url.as_deref(),
            Some("https://wiki.warframe.com/w/Braton_Prime")
        );
        let stock = braton
            .components
            .iter()
            .find(|component| component.name == "Braton Prime Stock")
            .unwrap();
        assert_eq!(
            stock
                .owned_relics
                .iter()
                .map(|relic| (relic.name.as_str(), relic.owned))
                .collect::<Vec<(&str, i64)>>(),
            vec![("Axi A1 Flawless", 3)]
        );
        let barrel = braton
            .components
            .iter()
            .find(|component| component.name == "Braton Prime Barrel")
            .unwrap();
        assert!(
            barrel.owned_relics.is_empty(),
            "Axi A17 is not in the relic catalogue and no copy is owned"
        );
    }

    #[test]
    fn prime_vault_status() {
        let rows = foundry_rows(&foundry_fixture());
        assert!(row(&rows, BANSHEE_PRIME).prime.is_some());
        assert_eq!(row(&rows, SKANA).prime, None);
    }
}
