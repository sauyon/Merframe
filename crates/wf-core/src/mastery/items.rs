use std::collections::{HashMap, HashSet};

use wf_data::{Item, Refinement};
use wf_inventory::Inventory;

use super::{
    Acquisition, CategoryTotals, Level, MasteryComponent, MasteryGroup, MasteryItem, MasteryOptions,
};
use crate::account::Account;
use crate::catalog::{Catalog, Stock, names_a_prime, part_name};
use crate::identity::{ItemTable, market_slug, part_identity};
use crate::prices::PriceSource;
use crate::view::View;

pub(super) const FOUNDERS_ITEMS: [&str; 3] = [
    "/Lotus/Powersuits/Excalibur/ExcaliburPrime",
    "/Lotus/Weapons/Tenno/Pistol/LatoPrime",
    "/Lotus/Weapons/Tenno/Melee/LongSword/SkanaPrime",
];

const PRIME_COLLECTION_CATEGORIES: [&str; 8] = [
    "Warframes",
    "Archwing",
    "Primary",
    "Secondary",
    "Melee",
    "Arch-Gun",
    "Arch-Melee",
    "Pets",
];

fn is_sentinel_weapon(item: &Item) -> bool {
    item.product_category.as_deref() == Some("SentinelWeapons")
}

pub(super) fn group_of(item: &Item) -> MasteryGroup {
    if is_sentinel_weapon(item) {
        return MasteryGroup::Companions;
    }
    match item.category.as_str() {
        "Warframes" | "Archwing" => MasteryGroup::Warframes,
        "Sentinels" | "Pets" => MasteryGroup::Companions,
        "Primary" | "Secondary" | "Melee" | "Arch-Gun" | "Arch-Melee" => MasteryGroup::Weapons,
        _ => MasteryGroup::Other,
    }
}

pub(crate) fn kind_of(item: &Item) -> &'static str {
    if item.is_landing_craft() {
        return "landing_craft";
    }

    match group_of(item) {
        MasteryGroup::Warframes => {
            if item.is_archwing() {
                "arch"
            } else if item.is_necramech() {
                "necramech"
            } else {
                "warframe"
            }
        }
        MasteryGroup::Companions => "companion",
        MasteryGroup::Weapons => match item.category.as_str() {
            "Primary" => "primary",
            "Secondary" => "secondary",
            "Melee" => "melee",
            _ => "arch",
        },
        MasteryGroup::Other => "modular",
    }
}

pub(crate) fn includes_founders(inventory: &Inventory, chosen: Option<bool>) -> bool {
    chosen.unwrap_or_else(|| inventory.is_founder())
}

pub(crate) fn masterable(catalog: &Catalog, include_founders: bool) -> impl Iterator<Item = &Item> {
    catalog
        .items()
        .filter(|item| item.masterable())
        .filter(|item| !item.name.starts_with("Kavasa Prime"))
        .filter(move |item| {
            include_founders || !FOUNDERS_ITEMS.contains(&item.unique_name.as_str())
        })
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a platinum cost is a small non-negative number"
)]
fn market_cost(missing: &[&MasteryComponent], prices: &dyn PriceSource) -> (u64, bool) {
    let quoted: Vec<(&MasteryComponent, f64)> = missing
        .iter()
        .filter_map(|component| {
            let plat = prices.plat(&market_slug(&component.name))?;
            (plat > 0.0).then_some((*component, plat))
        })
        .collect();
    let plat_cost = quoted
        .iter()
        .map(|(component, plat)| {
            let amount = (component.required - component.owned).max(0).unsigned_abs();
            (plat * amount as f64).round().max(0.0) as u64
        })
        .sum();
    let unpriced_blueprint = missing.iter().any(|component| {
        component.name.contains("Blueprint")
            && !quoted
                .iter()
                .any(|(other, _)| other.unique_name == component.unique_name)
    });
    (plat_cost, !unpriced_blueprint && !quoted.is_empty())
}

fn components_of(stock: &Stock, items: &ItemTable, item: &Item) -> Vec<MasteryComponent> {
    item.components
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|component| {
            let owned = stock.count(&component.unique_name);
            let required = i64::from(component.item_count);
            MasteryComponent {
                unique_name: component.unique_name.clone(),
                name: part_name(item, component),
                image_name: items
                    .part(item, component)
                    .and_then(|part| part.image_name.clone()),
                owned,
                required,
                enough: owned >= required,
            }
        })
        .collect()
}

struct OwnedRelicDrops<'a> {
    by_reward: HashMap<&'a str, Vec<(f64, f64)>>,
}

impl<'a> OwnedRelicDrops<'a> {
    #[allow(
        clippy::cast_precision_loss,
        reason = "owned counts stay far below 2^53"
    )]
    fn new(catalog: &'a Catalog, stock: &Stock) -> Self {
        let mut by_reward: HashMap<&'a str, Vec<(f64, f64)>> = HashMap::new();
        for relic in catalog.relics() {
            for refinement in Refinement::ALL {
                let owned = relic
                    .unique_names
                    .get(&refinement)
                    .map_or(0, |unique_name| stock.count(unique_name));
                if owned <= 0 {
                    continue;
                }
                let copies = owned.unsigned_abs() as f64;
                for reward in relic.rewards_for(refinement) {
                    by_reward
                        .entry(part_identity(&reward.item_unique_name))
                        .or_default()
                        .push((reward.chance, copies));
                }
            }
        }
        Self { by_reward }
    }

    fn probability(&self, item: &MasteryItem) -> f64 {
        if !names_a_prime(&item.name) {
            return 0.0;
        }
        let mut probability = 1.0_f64;
        for component in item.components.iter().filter(|component| !component.enough) {
            let mut none = 1.0_f64;
            for (chance, copies) in self
                .by_reward
                .get(part_identity(&component.unique_name))
                .map_or_default(Vec::as_slice)
            {
                none *= (1.0 - chance / 100.0).powf(*copies);
            }
            probability *= 1.0 - none;
        }
        probability
    }
}

pub(super) fn items(view: &View, options: MasteryOptions) -> Vec<MasteryItem> {
    let View {
        account,
        catalog,
        items,
        prices,
        favourites,
        ..
    } = *view;
    let inventory = &account.inventory;
    let pending: HashSet<&str> = inventory
        .pending_recipes
        .iter()
        .map(|recipe| recipe.item_type.as_str())
        .collect();
    let mut rows: Vec<MasteryItem> = masterable(
        catalog,
        includes_founders(inventory, options.include_founders_items),
    )
    .map(|item| {
        let affinity = account.affinity_of(&item.unique_name);
        let current_level = item.mastery_rank_at(affinity);
        let max_level = item.max_mastery_rank();
        let components = components_of(&account.stock, items, item);
        let pending_in_foundry = components
            .iter()
            .any(|component| pending.contains(component.unique_name.as_str()));
        let missing: Vec<&MasteryComponent> = components
            .iter()
            .filter(|component| !component.enough)
            .collect();
        let (plat_cost, purchasable) = market_cost(&missing, prices);
        MasteryItem {
            unique_name: item.unique_name.clone(),
            name: item.name.clone(),
            kind: kind_of(item),
            group: group_of(item),
            image_name: item.image_name.clone(),
            owned: account.built(&item.unique_name) || pending_in_foundry,
            mastered: affinity >= item.affinity_cap(),
            level: Level {
                current: current_level,
                max: max_level,
                xp_remaining: u64::from(
                    item.mastery_per_rank() * max_level.saturating_sub(current_level),
                ),
            },
            acquisition: Acquisition {
                missing_parts: missing.len(),
                plat_cost,
                purchasable,
                relic_probability: 0.0,
            },
            favourite: favourites.contains(&item.unique_name),
            components,
        }
    })
    .collect();
    let drops = OwnedRelicDrops::new(catalog, &account.stock);
    for row in &mut rows {
        row.acquisition.relic_probability = drops.probability(row);
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

pub(crate) fn unmastered_types<'a>(account: &Account, catalog: &'a Catalog) -> HashSet<&'a str> {
    masterable(catalog, account.inventory.is_founder())
        .filter(|item| !account.mastered(item))
        .map(|item| item.unique_name.as_str())
        .collect()
}

pub(super) fn totals(items: &[MasteryItem]) -> (CategoryTotals, CategoryTotals, CategoryTotals) {
    let count = |group: MasteryGroup| {
        let max = items.iter().filter(|item| item.group == group).count();
        let current = items
            .iter()
            .filter(|item| item.group == group && item.mastered)
            .count();
        CategoryTotals::counted(current, max)
    };
    (
        count(MasteryGroup::Warframes),
        count(MasteryGroup::Weapons),
        count(MasteryGroup::Companions),
    )
}

fn is_prime_collection_item(item: &Item) -> bool {
    names_a_prime(&item.name)
        && PRIME_COLLECTION_CATEGORIES.contains(&item.category.as_str())
        && !is_sentinel_weapon(item)
}

pub(crate) fn prime_ownership(
    account: &Account,
    catalog: &Catalog,
    options: MasteryOptions,
) -> (u32, u32) {
    let mut owned = 0;
    let mut total = 0;
    for item in masterable(
        catalog,
        includes_founders(&account.inventory, options.include_founders_items),
    )
    .filter(|item| is_prime_collection_item(item))
    {
        total += 1;
        if account.holds(item) {
            owned += 1;
        }
    }
    (owned, total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::Account;
    use crate::catalog::fixtures;
    use crate::mastery::support::{excluding_founders, fixture, prices};
    use crate::view::Fixture;

    const MASTERED_PRIME_SUIT: &str = "/Lotus/Powersuits/Loki/LokiPrime";

    #[test]
    fn prime_collection_totals() {
        let account = Account::new(fixtures::inventory());
        let catalog = fixtures::mastery_catalog();
        let (owned, total) = prime_ownership(&account, &catalog, MasteryOptions::default());
        assert!(owned <= total);
        let primes: Vec<&Item> = masterable(&catalog, account.inventory.is_founder())
            .filter(|item| is_prime_collection_item(item))
            .collect();
        assert!(
            primes
                .iter()
                .all(|item| group_of(item) != MasteryGroup::Other)
        );
        assert!(
            !primes
                .iter()
                .any(|item| item.category == "Sentinels" || item.name.contains("Kavasa"))
        );
        assert_eq!(u32::try_from(primes.len()).unwrap(), total);
        let sold = Account::new(fixtures::inventory_with(|value| {
            fixtures::drop_entries(value, &["Suits"], MASTERED_PRIME_SUIT);
        }));
        assert!(!sold.built(MASTERED_PRIME_SUIT));
        let (still_owned, sold_total) = prime_ownership(&sold, &catalog, MasteryOptions::default());
        assert_eq!(sold_total, total);
        assert_eq!(still_owned, owned);
    }

    #[test]
    fn prime_collection_with_founders() {
        let catalog = fixtures::mastery_catalog();
        let plain = prime_ownership(
            &Account::new(fixtures::inventory()),
            &catalog,
            MasteryOptions::default(),
        );
        let founder = Account::new(fixtures::founder_inventory());
        let with_founders = prime_ownership(&founder, &catalog, MasteryOptions::default());
        let without_founders = prime_ownership(&founder, &catalog, excluding_founders());
        assert_eq!(with_founders.1 - plain.1, 3);
        assert_eq!(without_founders, plain);
        assert!(with_founders.0 < with_founders.1);
    }

    #[test]
    fn relic_probability_of_missing_part() {
        let fixture = Fixture::new(
            fixtures::catalog(),
            fixtures::inventory_owning(&[
                ("/Lotus/Types/Game/Projections/T4VoidProjectionEBronze", 3),
                (
                    "/Lotus/Types/Recipes/WarframeRecipes/TrinityPrimeBlueprint",
                    1,
                ),
                (
                    "/Lotus/Types/Recipes/WarframeRecipes/TrinityPrimeChassisComponent",
                    1,
                ),
                (
                    "/Lotus/Types/Recipes/WarframeRecipes/TrinityPrimeHelmetComponent",
                    1,
                ),
            ]),
        )
        .with_prices(prices());
        let rows = items(&fixture.view(), MasteryOptions::default());
        let trinity = rows
            .iter()
            .find(|item| item.name == "Trinity Prime")
            .expect("Trinity Prime");
        let missing: Vec<&str> = trinity
            .components
            .iter()
            .filter(|component| !component.enough)
            .map(|component| component.name.as_str())
            .collect();
        assert_eq!(missing, vec!["Trinity Prime Systems"]);
        let expected = 1.0 - (1.0 - 25.33 / 100.0_f64).powf(3.0);
        assert!((trinity.acquisition.relic_probability - expected).abs() < 1e-9);
    }

    #[test]
    fn no_kavasa_prime() {
        let fixture = fixture(fixtures::inventory());
        let rows = items(&fixture.view(), MasteryOptions::default());
        assert!(
            !rows
                .iter()
                .any(|item| item.name.starts_with("Kavasa Prime"))
        );
    }
}
