use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use wf_market::OrderType;
use wf_worldstate::WorldState;

use crate::ItemSummary;
use crate::catalog::VaultStatus;
use crate::inventory_view::PartRow;
use crate::inventory_view::{ItemIndex, parts, sets};
use crate::resurgence::prime_resurgence;
use crate::view::View;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum DucateringHidden {
    Listed,
    Unmastered,
    Vaulted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum DucateringCompleteSets {
    Sell,
    Keep,
    KeepAbove { plat: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DucateringFilter {
    pub most_ducats_first: bool,
    pub max_plat: u32,
    pub min_ducats_per_plat: u32,
    pub hidden: Vec<DucateringHidden>,
    pub complete_sets: DucateringCompleteSets,
}

impl DucateringFilter {
    fn hides(&self, kind: DucateringHidden) -> bool {
        self.hidden.contains(&kind)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct DucateringEntry {
    pub item: ItemSummary,
    pub count: u32,
    pub ducats: u32,
    pub plat: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ScannedKioskItem {
    pub item_type: String,
    pub name: String,
    pub count: u32,
    pub ducats: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedKiosk {
    pub owned: Vec<ScannedKioskItem>,
    pub marked: Vec<ScannedKioskItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DucateringStep {
    Waiting,
    Search { position: usize, name: String },
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(tag = "state", content = "position", rename_all = "camelCase")]
pub enum DucateringState {
    Waiting(usize),
    Searching(usize),
    Finished,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct DucateringProgress {
    pub state: DucateringState,
    pub items: Vec<DucateringEntry>,
    pub sold: Vec<ScannedKioskItem>,
    pub ducats: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ducatering {
    items: Vec<DucateringEntry>,
    position: usize,
    searching: Option<usize>,
    marked: Vec<ScannedKioskItem>,
    sold: Vec<ScannedKioskItem>,
}

pub(crate) fn entries(
    view: &View,
    world: Option<&WorldState>,
    now: DateTime<Utc>,
    filter: &DucateringFilter,
) -> Vec<DucateringEntry> {
    let mut index = ItemIndex::default();
    let rows = parts(view, &mut index);
    let listed = view.listings.listed_slugs(OrderType::Sell, view.items);
    let hide_listed = filter.hides(DucateringHidden::Listed);
    let hide_unmastered = filter.hides(DucateringHidden::Unmastered);
    let resurgence = filter
        .hides(DucateringHidden::Vaulted)
        .then(|| prime_resurgence(world, now));
    let expensive_sets: Vec<usize> = match filter.complete_sets {
        DucateringCompleteSets::KeepAbove { plat } => sets(view, &mut index)
            .iter()
            .filter(|set| set.prices.sell.unwrap_or(0.0) > f64::from(plat))
            .map(|set| set.item)
            .collect(),
        DucateringCompleteSets::Sell | DucateringCompleteSets::Keep => Vec::new(),
    };

    let hidden_set = |row: &PartRow| {
        row.set.complete
            && match filter.complete_sets {
                DucateringCompleteSets::Sell => false,
                DucateringCompleteSets::Keep => true,
                DucateringCompleteSets::KeepAbove { .. } => expensive_sets.contains(&row.set.item),
            }
    };
    let hidden_vaulted = |row: &PartRow| {
        resurgence.as_ref().is_some_and(|resurgence| {
            let set = &index[row.set.item];
            set.vault == Some(VaultStatus::Vaulted) && !resurgence.contains(&set.unique_name)
        })
    };
    let below_ducats_per_plat = |row: &PartRow, ducats: u32| {
        row.prices.sell.is_some_and(|plat| {
            plat > 0.0 && f64::from(ducats) / plat < f64::from(filter.min_ducats_per_plat)
        })
    };
    let mut entries: Vec<DucateringEntry> = rows
        .iter()
        .filter(|row| row.count > 0)
        .filter(|row| row.prices.sell.unwrap_or(0.0) <= f64::from(filter.max_plat))
        .filter(|row| !hidden_set(row))
        .filter(|row| !hide_unmastered || row.status.mastered)
        .filter(|row| !hidden_vaulted(row))
        .filter(|row| {
            !(hide_listed
                && index[row.item]
                    .market_slug
                    .as_deref()
                    .is_some_and(|slug| listed.contains(slug)))
        })
        .filter_map(|row| {
            let item = &index[row.item];
            let ducats = item.ducats.filter(|ducats| *ducats > 0)?;
            if below_ducats_per_plat(row, ducats) {
                return None;
            }

            Some(DucateringEntry {
                item: item.clone(),
                count: u32::try_from(row.count).ok()?,
                ducats,
                plat: row.prices.sell,
            })
        })
        .collect();
    if filter.most_ducats_first {
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.ducats));
    }
    entries
}

fn find<'a>(items: &'a [ScannedKioskItem], unique_name: &str) -> Option<&'a ScannedKioskItem> {
    items.iter().find(|item| item.item_type == unique_name)
}

impl Ducatering {
    pub fn new(items: Vec<DucateringEntry>) -> Self {
        Self {
            items,
            position: 0,
            searching: None,
            marked: Vec::new(),
            sold: Vec::new(),
        }
    }

    pub fn sold(&mut self) -> bool {
        let marked = std::mem::take(&mut self.marked);
        let any = !marked.is_empty();
        for item in marked {
            if let Some(entry) = self
                .items
                .iter_mut()
                .find(|entry| entry.item.unique_name == item.item_type)
            {
                entry.count = entry.count.saturating_sub(item.count);
            }
            match self
                .sold
                .iter_mut()
                .find(|sale| sale.item_type == item.item_type)
            {
                Some(sale) => sale.count += item.count,
                None => self.sold.push(item),
            }
        }
        if any {
            self.searching = None;
        }
        any
    }

    pub fn state(&self) -> DucateringState {
        if self.position >= self.items.len() {
            DucateringState::Finished
        } else {
            self.searching.map_or(
                DucateringState::Waiting(self.position),
                DucateringState::Searching,
            )
        }
    }

    pub fn progress(&self) -> DucateringProgress {
        DucateringProgress {
            state: self.state(),
            items: self.items.clone(),
            sold: self.sold.clone(),
            ducats: self.sold.iter().map(|sale| sale.count * sale.ducats).sum(),
        }
    }

    pub fn advance(&mut self, kiosk: &ScannedKiosk) -> DucateringStep {
        if !kiosk.marked.is_empty() {
            self.marked.clone_from(&kiosk.marked);
        }
        while let Some(item) = self.items.get(self.position) {
            let Some(owned) = find(&kiosk.owned, &item.item.unique_name) else {
                self.position += 1;
                continue;
            };
            let marked =
                find(&kiosk.marked, &item.item.unique_name).map_or(0, |marked| marked.count);
            if marked >= item.count.min(owned.count) {
                self.position += 1;
                continue;
            }
            if self.searching == Some(self.position) {
                return DucateringStep::Waiting;
            }
            self.searching = Some(self.position);
            return DucateringStep::Search {
                position: self.position,
                name: owned.name.clone(),
            };
        }
        DucateringStep::Finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::fixtures::{
        self, BRATON_PRIME_BARREL, BRATON_PRIME_BLUEPRINT, BRATON_PRIME_STOCK,
        TRINITY_PRIME_SYSTEMS_BLUEPRINT,
    };
    use crate::prices::FixedPrices;
    use crate::view::Fixture;

    const LINK: &str = "/Lotus/Types/Recipes/Weapons/WeaponParts/AkboltoPrimeLink";
    const EPITAPH_BARREL: &str = "/Lotus/Types/Recipes/Weapons/WeaponParts/EpitaphPrimeBarrel";
    const BRATON_PRIME: &str = "/Lotus/Weapons/Tenno/Rifle/BratonPrime";
    const BANSHEE_PRIME_SYSTEMS_BLUEPRINT: &str =
        "/Lotus/Types/Recipes/WarframeRecipes/BansheePrimeSystemsBlueprint";
    const NOW: DateTime<Utc> = DateTime::from_timestamp_millis(1_788_807_069_000).unwrap();

    fn nothing_hidden() -> DucateringFilter {
        DucateringFilter {
            most_ducats_first: true,
            max_plat: 100,
            min_ducats_per_plat: 0,
            hidden: Vec::new(),
            complete_sets: DucateringCompleteSets::Sell,
        }
    }

    fn hiding(kind: DucateringHidden) -> DucateringFilter {
        DucateringFilter {
            most_ducats_first: false,
            hidden: vec![kind],
            ..nothing_hidden()
        }
    }

    fn names(
        fixture: &Fixture,
        world: Option<&WorldState>,
        filter: &DucateringFilter,
    ) -> Vec<String> {
        let mut names: Vec<String> = entries(&fixture.view(), world, NOW, filter)
            .into_iter()
            .map(|entry| entry.item.name)
            .collect();
        names.sort();
        names
    }

    fn scanned(items: &[(&str, &str, u32)]) -> Vec<ScannedKioskItem> {
        items
            .iter()
            .map(|(item_type, name, count)| ScannedKioskItem {
                item_type: (*item_type).to_owned(),
                name: (*name).to_owned(),
                count: *count,
                ducats: if *item_type == BRATON_PRIME_STOCK {
                    15
                } else {
                    45
                },
            })
            .collect()
    }

    fn kiosk(marked: &[(&str, &str, u32)]) -> ScannedKiosk {
        ScannedKiosk {
            owned: scanned(&[
                (BRATON_PRIME_STOCK, "Braton Prime Stock", 7),
                (LINK, "Akbolto Prime Link", 2),
            ]),
            marked: scanned(marked),
        }
    }

    fn run(items: &[(&str, u32)]) -> Ducatering {
        Ducatering::new(
            items
                .iter()
                .map(|(unique_name, count)| DucateringEntry {
                    item: ItemSummary {
                        unique_name: (*unique_name).to_owned(),
                        name: String::new(),
                        image_name: None,
                        market_slug: None,
                        prime: true,
                        vault: None,
                        ducats: None,
                    },
                    count: *count,
                    ducats: 0,
                    plat: None,
                })
                .collect(),
        )
    }

    fn search(position: usize, name: &str) -> DucateringStep {
        DucateringStep::Search {
            position,
            name: name.to_owned(),
        }
    }

    #[test]
    fn items_in_order() {
        let mut run = run(&[(BRATON_PRIME_STOCK, 3), (LINK, 1)]);
        assert_eq!(run.advance(&kiosk(&[])), search(0, "Braton Prime Stock"));
        assert_eq!(run.advance(&kiosk(&[])), DucateringStep::Waiting);
        let partly = kiosk(&[(BRATON_PRIME_STOCK, "Braton Prime Stock", 2)]);
        assert_eq!(run.advance(&partly), DucateringStep::Waiting);
        let stock = kiosk(&[(BRATON_PRIME_STOCK, "Braton Prime Stock", 3)]);
        assert_eq!(run.advance(&stock), search(1, "Akbolto Prime Link"));
        let both = kiosk(&[
            (BRATON_PRIME_STOCK, "Braton Prime Stock", 3),
            (LINK, "Akbolto Prime Link", 1),
        ]);
        assert_eq!(run.advance(&both), DucateringStep::Finished);
    }

    #[test]
    fn sold_items_stay_done() {
        let mut run = run(&[(BRATON_PRIME_STOCK, 3), (LINK, 1)]);
        let stock = kiosk(&[(BRATON_PRIME_STOCK, "Braton Prime Stock", 3)]);
        assert_eq!(run.advance(&stock), search(1, "Akbolto Prime Link"));
        assert_eq!(run.advance(&kiosk(&[])), DucateringStep::Waiting);
    }

    #[test]
    fn asks_for_no_more_than_is_owned() {
        let mut run = run(&[(LINK, 5), (BRATON_PRIME_STOCK, 1)]);
        let links = kiosk(&[(LINK, "Akbolto Prime Link", 2)]);
        assert_eq!(run.advance(&links), search(1, "Braton Prime Stock"));
    }

    #[test]
    fn skips_items_the_kiosk_does_not_offer() {
        let mut run = run(&[(EPITAPH_BARREL, 1), (LINK, 1)]);
        assert_eq!(run.advance(&kiosk(&[])), search(1, "Akbolto Prime Link"));
    }

    #[test]
    fn state_follows_the_search() {
        let mut run = run(&[(BRATON_PRIME_STOCK, 1)]);
        assert_eq!(run.state(), DucateringState::Waiting(0));
        run.advance(&kiosk(&[]));
        assert_eq!(run.state(), DucateringState::Searching(0));
        run.advance(&kiosk(&[(BRATON_PRIME_STOCK, "Braton Prime Stock", 1)]));
        assert_eq!(run.state(), DucateringState::Finished);
    }

    #[test]
    fn sales_add_up() {
        let mut run = run(&[(BRATON_PRIME_STOCK, 3), (LINK, 1)]);
        assert!(!run.sold());
        run.advance(&kiosk(&[(BRATON_PRIME_STOCK, "Braton Prime Stock", 2)]));
        assert!(run.sold());
        assert_eq!(run.advance(&kiosk(&[])), search(0, "Braton Prime Stock"));
        assert_eq!(run.progress().items[0].count, 1);
        run.advance(&kiosk(&[
            (BRATON_PRIME_STOCK, "Braton Prime Stock", 1),
            (LINK, "Akbolto Prime Link", 1),
        ]));
        assert!(run.sold());
        assert_eq!(run.advance(&kiosk(&[])), DucateringStep::Finished);
        let progress = run.progress();
        assert_eq!(progress.ducats, 3 * 15 + 45);
        assert_eq!(
            progress
                .sold
                .iter()
                .map(|sale| (sale.name.as_str(), sale.count))
                .collect::<Vec<_>>(),
            vec![("Braton Prime Stock", 3), ("Akbolto Prime Link", 1)]
        );
        assert!(!run.sold());
    }

    #[test]
    fn sale_survives_an_empty_read() {
        let mut run = run(&[(BRATON_PRIME_STOCK, 3)]);
        run.advance(&kiosk(&[(BRATON_PRIME_STOCK, "Braton Prime Stock", 3)]));
        run.advance(&kiosk(&[]));
        assert!(run.sold());
        assert_eq!(run.progress().ducats, 3 * 15);
    }

    #[test]
    fn empty_run_is_finished() {
        assert_eq!(run(&[]).advance(&kiosk(&[])), DucateringStep::Finished);
    }

    #[test]
    fn entries_follow_the_filter() {
        let fixture = Fixture::new(
            fixtures::catalog(),
            fixtures::inventory_owning(&[
                (BRATON_PRIME_BARREL, 3),
                (TRINITY_PRIME_SYSTEMS_BLUEPRINT, 1),
            ]),
        )
        .with_prices(FixedPrices::new([
            ("trinity_prime_systems_blueprint", 14.0),
            ("braton_prime_barrel", 8.0),
        ]));
        let filter = DucateringFilter {
            max_plat: 10,
            ..nothing_hidden()
        };
        let cheap = entries(&fixture.view(), None, NOW, &filter);
        assert_eq!(cheap.len(), 1);
        assert_eq!(cheap[0].item.name, "Braton Prime Barrel");
        assert_eq!(cheap[0].count, 3);
        assert!(cheap[0].ducats > 0);
        assert_eq!(cheap[0].plat, Some(8.0));

        let all = entries(&fixture.view(), None, NOW, &nothing_hidden());
        assert_eq!(all.len(), 2);
        assert!(all[0].ducats >= all[1].ducats);
    }

    #[test]
    fn complete_sets_hide_by_set_price() {
        let fixture = Fixture::new(
            fixtures::catalog(),
            fixtures::inventory_owning(&[
                (BRATON_PRIME_BARREL, 1),
                (BRATON_PRIME_STOCK, 1),
                (
                    "/Lotus/Types/Recipes/Weapons/WeaponParts/BratonPrimeReceiver",
                    1,
                ),
                (BRATON_PRIME_BLUEPRINT, 1),
            ]),
        )
        .with_prices(FixedPrices::new([("braton_prime_set", 45.0)]));
        let sets = |complete_sets| DucateringFilter {
            complete_sets,
            ..nothing_hidden()
        };
        let shown = |complete_sets| entries(&fixture.view(), None, NOW, &sets(complete_sets)).len();
        assert_eq!(shown(DucateringCompleteSets::Sell), 4);
        assert_eq!(shown(DucateringCompleteSets::Keep), 0);
        assert_eq!(shown(DucateringCompleteSets::KeepAbove { plat: 10 }), 0);
        assert_eq!(shown(DucateringCompleteSets::KeepAbove { plat: 50 }), 4);
    }

    #[test]
    fn unmastered_parts_hide() {
        let owned = &[
            (BRATON_PRIME_BARREL, 1),
            (TRINITY_PRIME_SYSTEMS_BLUEPRINT, 1),
        ];
        let mastered = Fixture::new(fixtures::catalog(), fixtures::inventory_owning(owned));
        assert_eq!(
            names(&mastered, None, &hiding(DucateringHidden::Unmastered)).len(),
            2
        );

        let unmastered = Fixture::new(
            fixtures::catalog(),
            fixtures::inventory_with(|value| {
                fixtures::drop_entries(value, &["LongGuns", "XPInfo"], BRATON_PRIME);
                for (item_type, count) in owned {
                    fixtures::push_counted(value, "MiscItems", item_type, *count);
                }
            }),
        );
        let shown = names(&unmastered, None, &hiding(DucateringHidden::Unmastered));
        assert_eq!(shown, vec!["Trinity Prime Systems"]);
    }

    #[test]
    fn vaulted_parts_hide_unless_resurgent() {
        let fixture = Fixture::new(
            fixtures::catalog_with(fixtures::ITEMS, fixtures::FOUNDRY_ITEMS),
            fixtures::inventory_owning(&[
                (BANSHEE_PRIME_SYSTEMS_BLUEPRINT, 1),
                (BRATON_PRIME_BARREL, 1),
            ]),
        );
        let world = WorldState::parse(fixtures::WORLD_STATE).unwrap();
        let hiding = hiding(DucateringHidden::Vaulted);
        assert_eq!(names(&fixture, None, &hiding), vec!["Braton Prime Barrel"]);
        assert_eq!(
            names(&fixture, Some(&world), &hiding),
            vec!["Banshee Prime Systems", "Braton Prime Barrel"]
        );
        assert_eq!(names(&fixture, None, &nothing_hidden()).len(), 2);
    }

    #[test]
    fn low_ducats_per_plat_hide() {
        let fixture = Fixture::new(
            fixtures::catalog(),
            fixtures::inventory_owning(&[(BRATON_PRIME_BARREL, 1), (BRATON_PRIME_STOCK, 1)]),
        )
        .with_prices(FixedPrices::new([
            ("braton_prime_barrel", 5.0),
            ("braton_prime_stock", 20.0),
        ]));
        let floor = |min_ducats_per_plat| DucateringFilter {
            min_ducats_per_plat,
            ..nothing_hidden()
        };
        let all = entries(&fixture.view(), None, NOW, &floor(0));
        let ducats: Vec<(&str, u32)> = all
            .iter()
            .map(|entry| (entry.item.name.as_str(), entry.ducats))
            .collect();
        assert_eq!(
            ducats,
            vec![("Braton Prime Stock", 25), ("Braton Prime Barrel", 15)]
        );

        assert_eq!(
            names(&fixture, None, &floor(1)),
            names(&fixture, None, &floor(0))
        );
        assert_eq!(
            names(&fixture, None, &floor(2)),
            vec!["Braton Prime Barrel"]
        );
        assert!(names(&fixture, None, &floor(4)).is_empty());
    }

    #[test]
    fn unpriced_parts_pass_the_floor() {
        let fixture = Fixture::new(
            fixtures::catalog(),
            fixtures::inventory_owning(&[(BRATON_PRIME_BARREL, 1)]),
        );
        let filter = DucateringFilter {
            min_ducats_per_plat: 1_000,
            ..nothing_hidden()
        };
        assert_eq!(names(&fixture, None, &filter), vec!["Braton Prime Barrel"]);
    }
}
