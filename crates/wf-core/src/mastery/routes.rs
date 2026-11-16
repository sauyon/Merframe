use wf_data::DEFAULT_MAX_RANK;

use super::xp::{
    VENARI, WARFRAME_MASTERY_PER_RANK, counted_affinity_types, unlisted_ranks, warframe_rank,
};
use super::{LevelUpRoute, MasteryItem, RouteMember, intrinsics, star_chart};
use crate::account::Account;
use crate::catalog::Catalog;
use crate::identity::ItemTable;
use crate::text::counted;

pub(super) fn route(
    kind: &'static str,
    label: &'static str,
    unit: &'static str,
    count: u32,
    mut members: Vec<RouteMember>,
) -> LevelUpRoute {
    members.sort_by(|left, right| {
        right
            .xp
            .cmp(&left.xp)
            .then_with(|| left.name.cmp(&right.name))
    });
    LevelUpRoute {
        kind,
        label,
        unit,
        count,
        xp_available: members.iter().map(|member| member.xp).sum(),
        members,
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "a route has far fewer than 2^32 members"
)]
pub(super) fn member_route(
    kind: &'static str,
    label: &'static str,
    unit: &'static str,
    members: Vec<RouteMember>,
) -> LevelUpRoute {
    route(kind, label, unit, members.len() as u32, members)
}

fn ranks_phrase(ranks: u32) -> String {
    format!("{} left", counted(ranks, "rank"))
}

fn item_routes(items: &[MasteryItem]) -> [LevelUpRoute; 2] {
    let mut owned: Vec<RouteMember> = Vec::new();
    let mut unowned: Vec<RouteMember> = Vec::new();
    for item in items.iter().filter(|item| !item.mastered) {
        let members = if item.owned { &mut owned } else { &mut unowned };
        members.push(RouteMember {
            name: item.name.clone(),
            detail: ranks_phrase(item.level.max.saturating_sub(item.level.current)),
            xp: item.level.xp_remaining,
        });
    }
    [
        member_route("owned_items", "Rank up gear you already own", "item", owned),
        member_route(
            "unowned_items",
            "Add gear you have never owned",
            "item",
            unowned,
        ),
    ]
}

fn ranks_left(affinity: u64) -> u32 {
    let rank = warframe_rank(affinity);
    DEFAULT_MAX_RANK.saturating_sub(u32::try_from(rank).unwrap_or(DEFAULT_MAX_RANK))
}

pub(super) fn rank_route(
    kind: &'static str,
    label: &'static str,
    entries: impl IntoIterator<Item = (String, u32, u64)>,
) -> LevelUpRoute {
    let mut count = 0;
    let mut members = Vec::new();
    for (name, ranks, xp_per_rank) in entries {
        if ranks == 0 {
            continue;
        }
        count += ranks;
        members.push(RouteMember {
            name,
            detail: ranks_phrase(ranks),
            xp: xp_per_rank * u64::from(ranks),
        });
    }
    route(kind, label, "rank", count, members)
}

fn unlisted_route(account: &Account, catalog: &Catalog, table: &ItemTable) -> LevelUpRoute {
    let counted = counted_affinity_types(account, catalog);
    let entries = unlisted_ranks(&account.inventory, &counted)
        .into_iter()
        .map(|(item_type, per_rank, rank)| {
            let left = u32::try_from(u64::from(DEFAULT_MAX_RANK).saturating_sub(rank))
                .unwrap_or(DEFAULT_MAX_RANK);
            (table.name(item_type), left, per_rank)
        });
    rank_route("unlisted_gear", "Rank up modular gear you own", entries)
}

pub(super) fn level_up_routes(
    account: &Account,
    catalog: &Catalog,
    table: &ItemTable,
    items: &[MasteryItem],
) -> Vec<LevelUpRoute> {
    let inventory = &account.inventory;
    let plexus = rank_route(
        "plexus",
        "Rank up the Railjack Plexus",
        inventory.crew_ship_harnesses.iter().map(|harness| {
            (
                "Plexus".to_owned(),
                ranks_left(harness.xp),
                WARFRAME_MASTERY_PER_RANK,
            )
        }),
    );
    let venari = rank_route(
        "venari",
        "Rank up Khora's Venari",
        VENARI.map(|(unique_name, name)| {
            (
                name.to_owned(),
                ranks_left(account.affinity_of(unique_name)),
                WARFRAME_MASTERY_PER_RANK,
            )
        }),
    );
    let mut routes: Vec<LevelUpRoute> = item_routes(items)
        .into_iter()
        .chain(star_chart::routes(inventory))
        .chain(intrinsics::routes(inventory))
        .chain([plexus, venari, unlisted_route(account, catalog, table)])
        .filter(|route| route.xp_available > 0)
        .collect();
    routes.sort_by(|left, right| {
        right
            .xp_available
            .cmp(&left.xp_available)
            .then_with(|| left.label.cmp(right.label))
    });
    routes
}

#[cfg(test)]
mod tests {
    use wf_inventory::Inventory;

    use super::*;
    use crate::catalog::fixtures::{self, EXCALIBUR, drop_entries, entries, inventory_with};
    use crate::mastery::MasteryOptions;
    use crate::mastery::items::items;
    use crate::mastery::support::{fixture, mission, set_affinity};

    const VENARI_PRIME: &str = "/Lotus/Powersuits/Khora/Kavat/KhoraPrimeKavatPowerSuit";
    const AMP_BARREL: &str =
        "/Lotus/Weapons/Corpus/OperatorAmplifiers/Set1/Barrel/CorpAmpSet1BarrelPartB";

    fn for_account(inventory: Inventory) -> Vec<LevelUpRoute> {
        let fixture = fixture(inventory);
        let rows = items(&fixture.view(), MasteryOptions::default());
        level_up_routes(&fixture.account, &fixture.catalog, &fixture.items, &rows)
    }

    fn route(routes: &[LevelUpRoute], kind: &str) -> Option<LevelUpRoute> {
        routes.iter().find(|route| route.kind == kind).cloned()
    }

    #[test]
    fn fixture_routes() {
        let routes = for_account(fixtures::inventory());
        let kinds: Vec<&str> = routes.iter().map(|route| route.kind).collect();
        assert_eq!(kinds, vec!["unowned_items", "owned_items"]);
        assert!(routes.iter().all(|route| route.xp_available > 0));
    }

    #[test]
    fn sorted_by_xp() {
        let routes = for_account(inventory_with(|value| {
            value["PlayerSkills"]["LPS_TACTICAL"] = serde_json::json!(4);
            drop_entries(value, &["XPInfo"], VENARI_PRIME);
            entries(value, "Missions").retain(|entry| {
                entry.get("Tag").and_then(serde_json::Value::as_str) != Some("SolNode27")
            });
        }));
        assert!(routes.len() > 3);
        for pair in routes.windows(2) {
            assert!(pair[0].xp_available >= pair[1].xp_available);
        }
    }

    #[test]
    fn owned_gear_route() {
        let before = for_account(fixtures::inventory());
        let after = for_account(inventory_with(|value| {
            drop_entries(value, &["XPInfo"], EXCALIBUR);
        }));
        let before = route(&before, "owned_items").unwrap();
        let after = route(&after, "owned_items").unwrap();
        assert_eq!(after.count, before.count + 1);
        assert_eq!(after.xp_available, before.xp_available + 6000);
        assert_eq!(after.unit, "item");
    }

    #[test]
    fn unowned_gear_route() {
        let before = for_account(fixtures::inventory());
        let after = for_account(inventory_with(|value| {
            drop_entries(value, &["XPInfo", "Suits"], EXCALIBUR);
        }));
        let before = route(&before, "unowned_items").unwrap();
        let after = route(&after, "unowned_items").unwrap();
        assert_eq!(after.count, before.count + 1);
        assert_eq!(after.xp_available, before.xp_available + 6000);
    }

    #[test]
    fn uncleared_nodes() {
        let routes = for_account(inventory_with(|value| {
            entries(value, "Missions").retain(|entry| {
                !matches!(
                    entry.get("Tag").and_then(serde_json::Value::as_str),
                    Some("SolNode27" | "SolNode108")
                )
            });
        }));
        let normal = route(&routes, "star_chart").expect("star chart");
        assert_eq!(normal.count, 2);
        assert_eq!(normal.xp_available, 49);
        assert_eq!(normal.unit, "node");
        let steel = route(&routes, "steel_path").expect("steel path");
        assert_eq!(steel.count, 2);
        assert_eq!(steel.xp_available, 49);
    }

    #[test]
    fn node_cleared_off_steel_path() {
        let routes = for_account(inventory_with(|value| {
            mission(value, "SolNode27")["Tier"] = serde_json::json!(0);
        }));
        assert!(route(&routes, "star_chart").is_none());
        let steel = route(&routes, "steel_path").expect("steel path");
        assert_eq!(steel.count, 1);
        assert_eq!(steel.xp_available, 24);
    }

    #[test]
    fn missing_junction() {
        let routes = for_account(inventory_with(|value| {
            entries(value, "Missions").retain(|entry| {
                entry.get("Tag").and_then(serde_json::Value::as_str) != Some("EarthToVenusJunction")
            });
        }));
        let normal = route(&routes, "junctions").unwrap();
        assert_eq!(normal.count, 1);
        assert_eq!(normal.xp_available, 1000);
        assert_eq!(normal.unit, "junction");
        let steel = route(&routes, "steel_path_junctions").expect("steel path junctions");
        assert_eq!(steel.count, 1);
        assert_eq!(steel.xp_available, 1000);
    }

    #[test]
    fn junction_beaten_off_steel_path() {
        let routes = for_account(inventory_with(|value| {
            let junction = mission(value, "EarthToVenusJunction");
            junction["Completes"] = serde_json::json!(1);
            junction["Tier"] = serde_json::json!(0);
        }));
        assert!(route(&routes, "junctions").is_none());
        let steel = route(&routes, "steel_path_junctions").expect("steel path junctions");
        assert_eq!(steel.count, 1);
        assert_eq!(steel.xp_available, 1000);
    }

    #[test]
    fn railjack_intrinsic_ranks() {
        let routes = for_account(inventory_with(|value| {
            value["PlayerSkills"]["LPS_TACTICAL"] = serde_json::json!(4);
        }));
        let railjack = route(&routes, "railjack_intrinsics").unwrap();
        assert_eq!(railjack.count, 6);
        assert_eq!(railjack.xp_available, 9000);
        assert_eq!(railjack.unit, "rank");
        assert!(route(&routes, "duviri_intrinsics").is_none());
    }

    #[test]
    fn drifter_intrinsic_ranks() {
        let routes = for_account(inventory_with(|value| {
            value["PlayerSkills"]["LPS_DRIFT_RIDING"] = serde_json::json!(0);
        }));
        let duviri = route(&routes, "duviri_intrinsics").unwrap();
        assert_eq!(duviri.count, 10);
        assert_eq!(duviri.xp_available, 15_000);
        assert!(route(&routes, "railjack_intrinsics").is_none());
    }

    #[test]
    fn plexus_ranks() {
        let routes = for_account(inventory_with(|value| {
            entries(value, "CrewShipHarnesses")[0]["XP"] = serde_json::json!(0);
        }));
        let plexus = route(&routes, "plexus").unwrap();
        assert_eq!(plexus.count, 30);
        assert_eq!(plexus.xp_available, 6000);
    }

    #[test]
    fn venari_and_venari_prime() {
        let routes = for_account(inventory_with(|value| {
            drop_entries(value, &["XPInfo"], VENARI_PRIME);
        }));
        let venari = route(&routes, "venari").unwrap();
        assert_eq!(venari.count, 30);
        assert_eq!(venari.xp_available, 6000);

        let both = for_account(inventory_with(|value| {
            drop_entries(value, &["XPInfo"], VENARI_PRIME);
            drop_entries(value, &["XPInfo"], VENARI[0].0);
        }));
        let venari = route(&both, "venari").unwrap();
        assert_eq!(venari.count, 60);
        assert_eq!(venari.xp_available, 12_000);
    }

    #[test]
    fn modular_gear_ranks() {
        let routes = for_account(inventory_with(|value| {
            set_affinity(value, AMP_BARREL, 312_500);
        }));
        let modular = route(&routes, "unlisted_gear").expect("unlisted gear");
        assert_eq!(modular.count, 5);
        assert_eq!(modular.xp_available, 500);
    }

    fn account_with_every_route_left() -> Inventory {
        inventory_with(|value| {
            drop_entries(value, &["XPInfo"], EXCALIBUR);
            mission(value, "SolNode108")["Tier"] = serde_json::json!(0);
            entries(value, "Missions").retain(|entry| {
                !matches!(
                    entry.get("Tag").and_then(serde_json::Value::as_str),
                    Some("SolNode27" | "EarthToVenusJunction")
                )
            });
            value["PlayerSkills"]["LPS_TACTICAL"] = serde_json::json!(4);
            value["PlayerSkills"]["LPS_DRIFT_RIDING"] = serde_json::json!(0);
            entries(value, "CrewShipHarnesses")[0]["XP"] = serde_json::json!(0);
            drop_entries(value, &["XPInfo"], VENARI_PRIME);
            set_affinity(value, AMP_BARREL, 312_500);
        })
    }

    #[test]
    fn members_add_up() {
        let routes = for_account(account_with_every_route_left());
        let mut kinds: Vec<&str> = routes.iter().map(|route| route.kind).collect();
        kinds.sort_unstable();
        assert_eq!(
            kinds,
            vec![
                "duviri_intrinsics",
                "junctions",
                "owned_items",
                "plexus",
                "railjack_intrinsics",
                "star_chart",
                "steel_path",
                "steel_path_junctions",
                "unlisted_gear",
                "unowned_items",
                "venari",
            ]
        );
        for route in &routes {
            assert!(!route.members.is_empty(), "{} lists no members", route.kind);
            assert_eq!(
                route.members.iter().map(|member| member.xp).sum::<u64>(),
                route.xp_available,
                "{} does not add up",
                route.kind
            );
            if route.unit == "rank" {
                assert!(
                    route
                        .members
                        .iter()
                        .all(|member| member.detail.ends_with("rank left")
                            || member.detail.ends_with("ranks left"))
                );
            } else {
                assert_eq!(route.members.len(), route.count as usize, "{}", route.kind);
            }
            for pair in route.members.windows(2) {
                assert!(pair[0].xp >= pair[1].xp, "{} is out of order", route.kind);
            }
        }
    }

    #[test]
    fn member_names_and_details() {
        let routes = for_account(account_with_every_route_left());
        let member = |kind: &str, name: &str| {
            route(&routes, kind)
                .unwrap_or_else(|| panic!("the {kind} route"))
                .members
                .into_iter()
                .find(|member| member.name == name)
                .unwrap_or_else(|| panic!("{kind} lists {name}"))
        };

        let excalibur = member("owned_items", "Excalibur");
        assert_eq!(excalibur.detail, "30 ranks left");
        assert_eq!(excalibur.xp, 6000);
        assert_eq!(
            member("star_chart", "Earth, E Prime (Extermination)").xp,
            24
        );
        assert_eq!(
            member("steel_path", "Mercury, Tolstoj (Assassination)").xp,
            25
        );
        assert_eq!(member("junctions", "Earth, Venus Junction").xp, 1000);
        assert_eq!(
            member("steel_path_junctions", "Earth, Venus Junction").xp,
            1000
        );

        let tactical = member("railjack_intrinsics", "Tactical");
        assert_eq!(tactical.detail, "6 ranks left");
        assert_eq!(tactical.xp, 9000);
        assert_eq!(member("duviri_intrinsics", "Riding").xp, 15_000);

        let plexus = member("plexus", "Plexus");
        assert_eq!(plexus.detail, "30 ranks left");
        assert_eq!(plexus.xp, 6000);
        assert_eq!(member("venari", "Venari Prime").xp, 6000);

        let modular = route(&routes, "unlisted_gear").expect("unlisted gear");
        assert_eq!(modular.members.len(), 1);
        assert_eq!(modular.members[0].detail, "5 ranks left");
        assert_eq!(modular.members[0].xp, 500);
        assert!(!modular.members[0].name.contains('/'));
    }
}
