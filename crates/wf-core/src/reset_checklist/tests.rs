use std::collections::HashSet;

use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use wf_inventory::Inventory;

use super::*;
use crate::catalog::fixtures::{self, at};
use crate::reset_checklist::context::fixtures::{CATALOG, ITEMS, context, synced_at, world_at};

const SORTIE: &str = "6a9edbfe7915bed56981db25";
const ARCHON_HUNT: &str = "6a9dfafed3033a0af5a5d762";

fn tab_of(inventory: &Inventory, now: DateTime<Utc>) -> ResetChecklistTab {
    tab(&context(inventory, now))
}

fn row(tab: &ResetChecklistTab, id: ResetChecklistRowId) -> &ResetChecklistRow {
    tab.rows.iter().find(|row| row.id == id).unwrap()
}

fn progress(inventory: &Inventory, id: ResetChecklistRowId) -> (ResetChecklistProgress, bool) {
    let tab = tab_of(inventory, synced_at());
    let row = row(&tab, id);
    (row.progress, row.done)
}

fn counted(done: u32, total: u32) -> ResetChecklistProgress {
    ResetChecklistProgress::Counted { done, total }
}

#[test]
fn every_row_listed() {
    let tab = tab_of(&fixtures::inventory(), synced_at());

    assert_eq!(tab.rows.len(), 30);
    assert_eq!(tab.synced_at, Some(synced_at()));
    assert_eq!(tab.daily.activation, at(1_788_739_200_000));
    assert_eq!(tab.weekly.expiry, at(1_789_344_000_000));
    assert_eq!(tab.standing.len(), 14);
    assert_eq!(tab.vendors.len(), 11);
}

#[test]
fn sortie_and_archon_hunt() {
    let unclaimed = fixtures::inventory();
    let claimed = fixtures::inventory_replacing(&[
        ("6a9cd300000000000000e101", SORTIE),
        ("6a9b8180000000000000e102", ARCHON_HUNT),
    ]);

    let tab = tab_of(&claimed, synced_at());
    let sortie = row(&tab, ResetChecklistRowId::Sortie);

    assert_eq!(
        progress(&unclaimed, ResetChecklistRowId::Sortie),
        (counted(0, 3), false)
    );
    assert_eq!((sortie.progress, sortie.done), (counted(3, 3), true));
    assert_eq!(sortie.group, ResetChecklistGroup::SortieReset);
    assert_eq!(sortie.own_window, None);
    assert_eq!(tab.sortie.unwrap().expiry, at(1_788_883_200_000));
    assert_eq!(
        progress(&claimed, ResetChecklistRowId::ArchonHunt),
        (counted(2, 3), true)
    );
}

#[test]
fn inventory_counts() {
    let inventory = fixtures::inventory();

    assert_eq!(
        progress(&inventory, ResetChecklistRowId::SearchPulses),
        (counted(3, 5), false)
    );
    assert_eq!(
        progress(&inventory, ResetChecklistRowId::Descendia),
        (counted(7, 21), false)
    );
    assert_eq!(
        progress(&inventory, ResetChecklistRowId::DescendiaSteelPath),
        (counted(0, 21), false)
    );
    assert_eq!(
        progress(&inventory, ResetChecklistRowId::Icebind),
        (counted(2, 3), false)
    );
    assert_eq!(
        progress(&inventory, ResetChecklistRowId::HelpClem),
        (ResetChecklistProgress::YesNo, true)
    );
    assert_eq!(
        progress(&inventory, ResetChecklistRowId::MaroosTreasureHunt),
        (ResetChecklistProgress::YesNo, true)
    );
    assert_eq!(
        progress(&inventory, ResetChecklistRowId::SteelPathIncursions),
        (counted(2, 6), false)
    );
}

#[test]
fn circuit_choices() {
    let tab = tab_of(&fixtures::inventory(), synced_at());

    let normal = row(&tab, ResetChecklistRowId::Circuit);
    let hard = row(&tab, ResetChecklistRowId::CircuitSteelPath);

    assert_eq!(
        (normal.progress, normal.done),
        (
            ResetChecklistProgress::Counted { done: 0, total: 10 },
            false
        )
    );
    assert_eq!(normal.detail, None);
    assert_eq!(
        (hard.progress, hard.done),
        (
            ResetChecklistProgress::Counted { done: 5, total: 10 },
            false
        )
    );
    assert_eq!(hard.detail.as_deref(), Some("Akstiletto and Kronen"));
}

#[test]
fn archimedea_score() {
    let tab = tab_of(&fixtures::inventory(), synced_at());
    let stale = Inventory::parse(&fixtures::synced("6a9b5b80000000000000c001")).unwrap();

    let deep = row(&tab, ResetChecklistRowId::DeepArchimedea);
    let temporal = row(&tab, ResetChecklistRowId::TemporalArchimedea);

    assert!(deep.done);
    assert_eq!(deep.own_window, None);
    assert!(!temporal.done);
    assert_eq!(
        progress(&stale, ResetChecklistRowId::DeepArchimedea),
        (ResetChecklistProgress::YesNo, false)
    );
}

#[test]
fn kahls_garrison_next_mission() {
    let with_latest_mission = |week: i64, completed: bool| {
        fixtures::inventory_with(|value| {
            for affiliation in fixtures::entries(value, "Affiliations") {
                if affiliation["Tag"] == "KahlSyndicate" {
                    affiliation["WeeklyMissions"] = json!([
                        {"WeekCount": 615, "CompletedMission": true},
                        {"WeekCount": week, "CompletedMission": completed},
                    ]);
                }
            }
        })
    };

    assert_eq!(
        progress(
            &with_latest_mission(657, false),
            ResetChecklistRowId::KahlsGarrison
        ),
        (ResetChecklistProgress::YesNo, true)
    );
    assert!(
        !progress(
            &with_latest_mission(656, false),
            ResetChecklistRowId::KahlsGarrison
        )
        .1
    );
    assert!(
        !progress(
            &with_latest_mission(640, false),
            ResetChecklistRowId::KahlsGarrison
        )
        .1
    );
}

#[test]
fn clan_vault_tiers() {
    let contributed = fixtures::inventory_with(|value| {
        fixtures::entries(value, "WeeklyGuildVaultBonusProgress")[1]["WeekCount"] = 656.into();
    });

    assert_eq!(
        progress(
            &fixtures::inventory(),
            ResetChecklistRowId::ClanWeeklyInitiatives
        ),
        (counted(0, 4), false)
    );
    assert_eq!(
        progress(&contributed, ResetChecklistRowId::ClanWeeklyInitiatives),
        (counted(3, 4), false)
    );
}

#[test]
fn helminth_this_week() {
    let applied_today = fixtures::inventory_replacing(&[("1789294320000", "1789380000000")]);

    assert_eq!(
        progress(
            &fixtures::inventory(),
            ResetChecklistRowId::HelminthInvigorations
        ),
        (counted(0, 3), false)
    );
    assert_eq!(
        progress(&applied_today, ResetChecklistRowId::HelminthInvigorations),
        (counted(1, 3), false)
    );
}

#[test]
fn daily_tribute_and_first_win() {
    let inventory = fixtures::inventory();
    let won = ResetChecklistContext::new(
        &inventory,
        &CATALOG,
        &ITEMS,
        Some(world_at(synced_at())),
        synced_at(),
        ResetChecklistStored {
            last_mission_won: Some(at(1_788_775_200_000)),
            ..ResetChecklistStored::default()
        },
    );

    let tomorrow = tab_of(&inventory, synced_at() + TimeDelta::days(1));
    let won = tab(&won);

    assert!(row(&won, ResetChecklistRowId::DailyTribute).done);
    assert!(row(&won, ResetChecklistRowId::FirstWin).done);
    assert!(!row(&tomorrow, ResetChecklistRowId::DailyTribute).done);
    assert!(!row(&tomorrow, ResetChecklistRowId::FirstWin).done);
}

#[test]
fn calendar_days() {
    let other_season = fixtures::inventory_replacing(&[("CST_SUMMER", "CST_WINTER")]);

    assert_eq!(
        progress(&fixtures::inventory(), ResetChecklistRowId::Calendar),
        (counted(6, 16), false)
    );
    assert_eq!(
        progress(&other_season, ResetChecklistRowId::Calendar),
        (counted(0, 16), false)
    );
}

fn with_nightwave(history: Value, progress: Value) -> Inventory {
    fixtures::inventory_with(|value| {
        value["SeasonChallengeHistory"] = history;
        value["ChallengeProgress"] = progress;
    })
}

fn part(name: &str, done: u32, total: u32) -> ResetChecklistRowPart {
    ResetChecklistRowPart::new(name.to_owned(), done, total)
}

#[test]
fn nightwave_counters() {
    let inventory = with_nightwave(
        json!([]),
        json!([
            {"Name": "SeasonDailyKillEnemiesWithPoison", "Progress": 149},
            {"Name": "SeasonDailyInteractWithPet", "Progress": 1},
            {"Name": "SeasonDailyMercyKill", "Progress": 3},
            {"Name": "SeasonWeeklyPermanentCompleteMissions5", "Progress": 15},
        ]),
    );

    let tab = tab_of(&inventory, synced_at());
    let daily = row(&tab, ResetChecklistRowId::NightwaveDailyActs);
    let weekly = row(&tab, ResetChecklistRowId::NightwaveWeeklyActs);

    assert_eq!(daily.progress, counted(2, 3));
    assert_eq!(
        daily.parts,
        [
            part("Poisoner", 149, 150),
            part("Loyalty", 1, 1),
            part("No Mercy", 3, 1)
        ]
    );
    assert!(!daily.parts[0].completed);
    assert!(daily.parts[1].completed);
    assert_eq!(weekly.progress, counted(1, 7));
    assert_eq!(weekly.parts[2], part("Mission Complete V", 15, 15));
    assert_eq!(weekly.parts[3], part("Night and Day", 0, 10));
    assert_eq!(weekly.parts[5], part("Battle Hardened", 0, 6));
}

#[test]
fn nightwave_stale_counter() {
    let full_counter = |sync: &str| {
        fixtures::inventory_with(|value| {
            value["LastInventorySync"]["$oid"] = sync.into();
            value["ChallengeProgress"] =
                json!([{"Name": "SeasonWeeklyPermanentCompleteMissions5", "Progress": 15}]);
        })
    };

    let before_act = tab_of(&full_counter("6a9b5b80000000000000c001"), synced_at());
    let after_act = tab_of(&full_counter("6a9eeb1f000000000000c001"), synced_at());

    assert_eq!(
        row(&before_act, ResetChecklistRowId::NightwaveWeeklyActs).parts[2],
        part("Mission Complete V", 0, 15)
    );
    assert_eq!(
        row(&after_act, ResetChecklistRowId::NightwaveWeeklyActs).parts[2],
        part("Mission Complete V", 15, 15)
    );
}

fn one_weekly_act_open() -> Vec<Value> {
    [
        ("SeasonWeeklyPermanentCompleteMissions5", 15),
        ("SeasonWeeklyPermanentKillEximus5", 30),
        ("SeasonWeeklyPermanentKillEnemies5", 500),
        ("SeasonWeeklyNightAndDay", 10),
        ("SeasonWeeklyCompleteInvasionMissions", 6),
        ("SeasonWeeklyHardFallenAngel", 3),
    ]
    .into_iter()
    .map(|(name, progress)| json!({"Name": name, "Progress": progress}))
    .collect()
}

#[test]
fn nightwave_recovered() {
    let mut counters = one_weekly_act_open();
    counters.push(json!({"Name": "SeasonWeeklyMineRarePlainsResources", "Progress": 1}));
    counters.push(json!({"Name": "SeasonWeeklyCompleteSpy", "Progress": 3}));
    let inventory = with_nightwave(
        json!([
            {"challenge": "SeasonWeeklyBowOffense", "id": "001900040000000000000046"},
            {"challenge": "SeasonWeeklyMineRarePlainsResources", "id": "001900040000000000000040"},
            {"challenge": "SeasonWeeklyHardFriendsMirrorDefense", "id": "001900030000000000000030"},
            {"challenge": "SeasonWeeklyCompleteSpy", "id": "001900040000000000000041"},
            {"challenge": "SeasonWeeklyToAshes", "id": "001800040000000000000042"},
            {"challenge": "SeasonDailyCodexScan", "id": "001900040000000000000043"},
            {"challenge": "SeasonWeeklyNightAndDay", "id": "001900040000000000000044"},
            {"challenge": "SeasonWeeklyCollector", "id": "001900050000000000000045"},
        ]),
        Value::Array(counters),
    );
    let caught_up = with_nightwave(
        json!([{"challenge": "SeasonWeeklyCompleteSpy", "id": "001900040000000000000041"}]),
        json!([{"Name": "SeasonWeeklyCompleteSpy", "Progress": 3}]),
    );
    let all_reserved = with_nightwave(
        json!([{"challenge": "SeasonWeeklyCompleteSpy", "id": "001900040000000000000041"}]),
        json!([]),
    );

    let tab = tab_of(&inventory, synced_at());
    let recovered = row(&tab, ResetChecklistRowId::NightwaveRecoveredActs);
    let reserved_tab = tab_of(&all_reserved, synced_at());
    let reserved = row(&reserved_tab, ResetChecklistRowId::NightwaveRecoveredActs);

    assert_eq!(
        (recovered.progress, recovered.done),
        (ResetChecklistProgress::YesNo, false)
    );
    assert_eq!(recovered.group, ResetChecklistGroup::Weekly);
    assert_eq!(
        recovered.parts,
        [part("Mirror, Mirror", 0, 1), part("Earth Miner", 1, 3)]
    );
    assert_eq!(recovered.detail.as_deref(), Some("1 act on reserve"));
    assert!(reserved.parts.is_empty());
    assert_eq!(reserved.detail.as_deref(), Some("1 act on reserve"));
    assert!(!reserved.done);
    assert_eq!(
        progress(&caught_up, ResetChecklistRowId::NightwaveRecoveredActs),
        (ResetChecklistProgress::YesNo, true)
    );
}

#[test]
fn recovered_act_given_twice() {
    let inventory = with_nightwave(
        json!([
            {"challenge": "SeasonWeeklyCompleteSpy", "id": "001900020000000000000020"},
            {"challenge": "SeasonWeeklyMineRarePlainsResources", "id": "001900030000000000000030"},
            {"challenge": "SeasonWeeklyCompleteSpy", "id": "001900040000000000000041"},
        ]),
        Value::Array(one_weekly_act_open()),
    );

    let tab = tab_of(&inventory, synced_at());
    let recovered = row(&tab, ResetChecklistRowId::NightwaveRecoveredActs);

    assert_eq!(
        recovered.parts,
        [part("Earth Miner", 0, 3), part("Operative", 0, 3)]
    );
    assert_eq!(recovered.detail, None);
}

#[test]
fn syndicate_missions_by_rank() {
    let inventory = fixtures::inventory_replacing(&[
        ("6a9e4c00000000000000e104", "6a9edf461c3038a4ecd9dc5f"),
        ("6a9cfa00000000000000e105", "6a9edf461c3038a4ecd9dc67"),
    ]);
    let arbiters_rank_two = fixtures::inventory_replacing(&[
        ("6a9e4c00000000000000e104", "6a9edf461c3038a4ecd9dc5f"),
        ("6a9cfa00000000000000e105", "6a9edf461c3038a4ecd9dc67"),
        (
            r#""Title":-2,"Tag":"ArbitersSyndicate""#,
            r#""Title":2,"Tag":"ArbitersSyndicate""#,
        ),
    ]);

    let tab = tab_of(&inventory, synced_at());
    let missions = row(&tab, ResetChecklistRowId::SyndicateMissions);
    let ranked_tab = tab_of(&arbiters_rank_two, synced_at());
    let ranked = row(&ranked_tab, ResetChecklistRowId::SyndicateMissions);

    assert_eq!(missions.progress, counted(1, 12));
    assert_eq!(missions.group, ResetChecklistGroup::SortieReset);
    assert_eq!(missions.own_window, None);
    assert_eq!(
        missions.parts,
        [
            part("New Loka", 0, 3),
            part("The Perrin Sequence", 1, 3),
            part("Red Veil", 0, 3),
            part("Steel Meridian", 0, 3)
        ]
    );
    assert_eq!(ranked.progress, counted(2, 15));
    assert_eq!(ranked.parts[0], part("Arbiters of Hexis", 1, 3));
}

#[test]
fn syndicate_missions_between_sets() {
    let tab = tab_of(&fixtures::inventory(), at(1_788_883_170_000));

    assert!(tab.sortie.is_some());
    assert_eq!(
        row(&tab, ResetChecklistRowId::SyndicateMissions).progress,
        ResetChecklistProgress::Unknown
    );
}

#[test]
fn trades_and_focus() {
    let inventory = fixtures::inventory();
    let founder = fixtures::founder_inventory();

    let tomorrow = tab_of(&inventory, synced_at() + TimeDelta::days(1));

    assert_eq!(
        progress(&inventory, ResetChecklistRowId::Trades),
        (
            ResetChecklistProgress::Remaining { left: 6, cap: 14 },
            false
        )
    );
    assert_eq!(
        progress(&founder, ResetChecklistRowId::Trades).0,
        ResetChecklistProgress::Remaining { left: 6, cap: 16 }
    );
    assert_eq!(
        row(&tomorrow, ResetChecklistRowId::Trades).progress,
        ResetChecklistProgress::Remaining { left: 14, cap: 14 }
    );
    assert_eq!(
        progress(&inventory, ResetChecklistRowId::Focus),
        (
            ResetChecklistProgress::Remaining {
                left: 320_000,
                cap: 320_000
            },
            false
        )
    );
}

#[test]
fn gifts_like_trades() {
    let with_gifts = |gifts: Option<u32>| {
        fixtures::inventory_with(|value| match gifts {
            Some(gifts) => value["GiftsRemaining"] = gifts.into(),
            None => {
                value.as_object_mut().unwrap().remove("GiftsRemaining");
            }
        })
    };

    let tomorrow = tab_of(&with_gifts(Some(3)), synced_at() + TimeDelta::days(1));

    assert_eq!(
        progress(&with_gifts(Some(3)), ResetChecklistRowId::Gifts),
        (
            ResetChecklistProgress::Remaining { left: 3, cap: 14 },
            false
        )
    );
    assert_eq!(
        row(&tomorrow, ResetChecklistRowId::Gifts).progress,
        ResetChecklistProgress::Remaining { left: 14, cap: 14 }
    );
    assert_eq!(
        progress(&with_gifts(None), ResetChecklistRowId::Gifts),
        (ResetChecklistProgress::Unknown, false)
    );
}

#[test]
fn icebind_cryobells() {
    let one_held = fixtures::inventory_with(|value| {
        fixtures::entries(value, "KuvaKeys").truncate(1);
    });
    let none_held = fixtures::inventory_with(|value| value["KuvaKeys"] = Value::Array(Vec::new()));

    let detail = |inventory: &Inventory| {
        row(
            &tab_of(inventory, synced_at()),
            ResetChecklistRowId::Icebind,
        )
        .detail
        .clone()
    };

    assert_eq!(
        detail(&fixtures::inventory()).as_deref(),
        Some("2/6 in inventory")
    );
    assert_eq!(detail(&one_held).as_deref(), Some("1/6 in inventory"));
    assert_eq!(detail(&none_held).as_deref(), Some("0/6 in inventory"));
}

#[test]
fn manual_tick() {
    let inventory = fixtures::inventory();
    let context = ResetChecklistContext::new(
        &inventory,
        &CATALOG,
        &ITEMS,
        None,
        synced_at(),
        ResetChecklistStored {
            checked: HashSet::from([ResetChecklistManualRowId::VentkidsRaces]),
            ..ResetChecklistStored::default()
        },
    );

    let tab = tab(&context);
    let races = row(&tab, ResetChecklistRowId::VentkidsRaces);
    let conclave = row(&tab, ResetChecklistRowId::ConclaveChallenges);

    assert_eq!(
        races.progress,
        ResetChecklistProgress::Manual {
            id: ResetChecklistManualRowId::VentkidsRaces,
            checked: true
        }
    );
    assert!(races.done);
    assert_eq!(races.own_window, None);
    assert_eq!(
        conclave.progress,
        ResetChecklistProgress::Manual {
            id: ResetChecklistManualRowId::ConclaveChallenges,
            checked: false
        }
    );
}

#[test]
fn without_world_state() {
    let inventory = fixtures::inventory();
    let context = ResetChecklistContext::new(
        &inventory,
        &CATALOG,
        &ITEMS,
        None,
        synced_at(),
        ResetChecklistStored::default(),
    );

    let tab = tab(&context);
    let sortie = row(&tab, ResetChecklistRowId::Sortie);
    let descendia = row(&tab, ResetChecklistRowId::Descendia);

    assert_eq!(tab.sortie, None);
    assert_eq!(sortie.progress, ResetChecklistProgress::Unknown);
    assert_eq!(sortie.own_window, None);
    assert!(!sortie.done);
    assert_eq!(
        row(&tab, ResetChecklistRowId::NightwaveRecoveredActs).progress,
        ResetChecklistProgress::Unknown
    );
    assert_eq!(descendia.progress, counted(7, 21));
    assert_eq!(descendia.own_window, None);
    assert_eq!(
        row(&tab, ResetChecklistRowId::SearchPulses).progress,
        counted(3, 5)
    );
}

#[test]
fn locked_rows() {
    let tab = tab_of(&fixtures::inventory(), synced_at());

    assert!(row(&tab, ResetChecklistRowId::Sortie).locked);
    assert!(row(&tab, ResetChecklistRowId::SimarisSynthesis).locked);
    assert!(!row(&tab, ResetChecklistRowId::ArchonHunt).locked);
    assert!(!row(&tab, ResetChecklistRowId::SearchPulses).locked);
    assert!(!row(&tab, ResetChecklistRowId::Trades).locked);
    assert!(!row(&tab, ResetChecklistRowId::SyndicateMissions).locked);
    assert!(row(&tab, ResetChecklistRowId::OopsAllEximus).locked);
}

#[test]
fn icebind_unlock() {
    let elite_deep = fixtures::inventory_with(|value| {
        value["EntratiLabConquestHardModeStatus"] = json!(1);
    });

    let locked = |inventory: &Inventory| {
        row(
            &tab_of(inventory, synced_at()),
            ResetChecklistRowId::Icebind,
        )
        .locked
    };

    assert!(locked(&fixtures::inventory()));
    assert!(!locked(&elite_deep));
}

#[test]
fn calendar_own_window() {
    let inventory = fixtures::inventory();
    let mut world = world_at(synced_at());
    let season = world.calendar.as_mut().unwrap();
    season.window.activation += TimeDelta::days(2);
    let window = season.window;
    let shifted = ResetChecklistContext::new(
        &inventory,
        &CATALOG,
        &ITEMS,
        Some(world),
        synced_at(),
        ResetChecklistStored::default(),
    );

    let weekly_season = tab_of(&inventory, synced_at());
    let shifted = tab(&shifted);

    assert_eq!(
        row(&weekly_season, ResetChecklistRowId::Calendar).own_window,
        None
    );
    assert_eq!(
        row(&shifted, ResetChecklistRowId::Calendar).own_window,
        Some(window)
    );
}

#[test]
fn oops_all_eximus_unlock() {
    let campaign_done = fixtures::inventory_with(|value| {
        value["DialogueHistory"]["Dialogues"][0]["Booleans"] = json!(["FF1AdventureComplete"]);
        fixtures::entries(value, "PeriodicMissionCompletions").push(json!({
            "date": {"$date": {"$numberLong": "1788739200000"}},
            "tag": "AmirWeekly",
        }));
    });

    let tab = tab_of(&campaign_done, synced_at());
    let eximus = row(&tab, ResetChecklistRowId::OopsAllEximus);

    assert!(!eximus.locked);
    assert_eq!(
        (eximus.progress, eximus.done),
        (ResetChecklistProgress::YesNo, true)
    );
}

#[test]
fn tick_expires_with_its_period() {
    let store = Store::in_memory().unwrap();
    let sunday_evening = at(1_789_329_600_000);

    set_checked(
        &store,
        ResetChecklistManualRowId::VentkidsRaces,
        true,
        sunday_evening,
    )
    .unwrap();
    set_checked(
        &store,
        ResetChecklistManualRowId::ConclaveChallenges,
        true,
        sunday_evening,
    )
    .unwrap();

    let evening = checked_manual_rows(&store, sunday_evening).unwrap();
    let after_reset = checked_manual_rows(&store, sunday_evening + TimeDelta::hours(5)).unwrap();

    assert_eq!(evening.len(), 2);
    assert!(after_reset.is_empty());
}
