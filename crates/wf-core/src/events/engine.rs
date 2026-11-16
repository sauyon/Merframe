use std::collections::{HashMap, HashSet};

use chrono::{DateTime, TimeDelta, Utc};
use wf_inventory::Inventory;
use wf_log::Event as LogEvent;
use wf_worldstate::{ArbitrationForecast, ArbitrationTiers, WorldState, node_info};

use super::alerts::{AlertSettings, fissure_planet};
use super::arbitrations::{ARBITRATION_FORECAST_TRUSTED, ArbitrationOutlook};
use super::rewards::world_reward_alerts;
use super::{CoreEvent, FissureInfo, InventorySummary};
use crate::catalog::DUCATS_ITEM;
use crate::identity::ItemTable;
use crate::trade::player_name;

fn conversation_player(channel: &str) -> Option<String> {
    let name = player_name(channel.strip_prefix('F')?);
    (!name.is_empty()).then_some(name)
}

pub(crate) struct Engine {
    settings: AlertSettings,
    reward_screen: RewardScreenState,
    world_state_polled: bool,
    announced_fissures: HashMap<String, DateTime<Utc>>,
    announced_timers: HashMap<String, DateTime<Utc>>,
    announced_rewards: HashMap<String, DateTime<Utc>>,
    announced_arbitrations: HashMap<String, DateTime<Utc>>,
    seen_channels: HashMap<String, DateTime<Utc>>,
}

#[derive(Debug, Default)]
struct RewardScreenState {
    generation: u64,
    players: HashSet<String>,
    scan_started: bool,
    announced: usize,
}

const ANNOUNCED_CAP: usize = 250;
const STEEL_PATH_LEVEL_OFFSET: u32 = 100;
const ANNOUNCED_MAX_AGE_SECS: i64 = 3 * 60 * 60;
const INVASION_MEMORY_DAYS: i64 = 30;

fn remember(seen: &mut HashMap<String, DateTime<Utc>>, key: String, now: DateTime<Utc>) -> bool {
    let unseen = seen.insert(key, now).is_none();
    if seen.len() > ANNOUNCED_CAP {
        seen.retain(|_, at| now.signed_duration_since(*at).num_seconds() < ANNOUNCED_MAX_AGE_SECS);
    }
    unseen
}

fn fissure_levels(planet: Option<&str>, node_levels: (u32, u32), steel_path: bool) -> (u32, u32) {
    let (min, max) = match planet {
        Some("Kuva Fortress") => (60, 70),
        Some("Void") => node_levels,
        _ => (node_levels.1 + 3, node_levels.1 + 5),
    };
    let offset = if steel_path {
        STEEL_PATH_LEVEL_OFFSET
    } else {
        0
    };
    (min + offset, max + offset)
}

impl Engine {
    pub fn new(settings: AlertSettings) -> Self {
        Self {
            settings,
            reward_screen: RewardScreenState::default(),
            world_state_polled: false,
            announced_fissures: HashMap::new(),
            announced_timers: HashMap::new(),
            announced_rewards: HashMap::new(),
            announced_arbitrations: HashMap::new(),
            seen_channels: HashMap::new(),
        }
    }

    pub fn set_settings(&mut self, settings: AlertSettings) {
        self.settings = settings;
    }

    pub fn settings(&self) -> &AlertSettings {
        &self.settings
    }

    pub fn handle_log_event(&mut self, event: &LogEvent, now: DateTime<Utc>) -> Vec<CoreEvent> {
        match event {
            LogEvent::RelicRewardScreenOpened => {
                self.reward_screen = RewardScreenState {
                    generation: self.reward_screen.generation + 1,
                    ..RewardScreenState::default()
                };
                Vec::new()
            }
            LogEvent::OwnRelicReward { account_id, .. }
            | LogEvent::SquadRewardInfoReceived { account_id } => {
                self.reward_screen.players.insert(account_id.clone());
                Vec::new()
            }
            LogEvent::ChatTabAdded { channel } => self.new_conversation(channel, now),
            LogEvent::MissionSucceeded => vec![CoreEvent::MissionWon],
            _ => Vec::new(),
        }
    }

    pub fn handle_world_state(
        &mut self,
        world: &WorldState,
        items: &ItemTable,
        unmastered: &HashSet<&str>,
        now: DateTime<Utc>,
    ) -> Vec<CoreEvent> {
        let mut events = Vec::new();
        let first_poll = !self.world_state_polled;
        self.world_state_polled = true;
        for fissure in world.fissures(now) {
            let key = format!("{}@{}", fissure.node_id, fissure.expiry.timestamp());
            let unseen = remember(&mut self.announced_fissures, key, now);
            if first_poll || !unseen || !self.settings.wants(&fissure) {
                continue;
            }
            let node = node_info(&fissure.node_id);
            let planet = fissure_planet(&fissure);
            events.push(CoreEvent::FissureAlert {
                fissure: FissureInfo {
                    node_id: fissure.node_id.clone(),
                    node_name: fissure.node_name,
                    mission_type: fissure.mission_type.clone(),
                    mission_name: fissure.mission_name.clone(),
                    planet,
                    tier: fissure.tier.name().to_owned(),
                    steel_path: fissure.steel_path,
                    is_storm: fissure.is_storm,
                    faction: node.and_then(|node| node.faction),
                    levels: node
                        .and_then(|node| node.levels)
                        .map(|levels| fissure_levels(planet, levels, fissure.steel_path)),
                    expiry: fissure.expiry,
                    remaining_secs: fissure.expiry.signed_duration_since(now).num_seconds(),
                },
            });
        }
        for timer in world.timers(now) {
            if !self.settings.timers.enabled(timer.name, timer.next_state) {
                continue;
            }
            let remaining_secs = timer.ends.signed_duration_since(now).num_seconds();
            if remaining_secs < 0 || remaining_secs > self.settings.timer_lead_secs {
                continue;
            }
            let key = format!("{}@{}", timer.name, timer.ends.timestamp());
            if !remember(&mut self.announced_timers, key, now) {
                continue;
            }
            events.push(CoreEvent::TimerAlert {
                name: timer.name.to_owned(),
                next_state: timer.next_state.to_owned(),
                ends_at: timer.ends,
                remaining_secs,
            });
        }

        self.announced_rewards.retain(|_, until| *until > now);
        if !self.settings.reward_notifications_enabled {
            return events;
        }

        for reward in world_reward_alerts(world, items, unmastered, &self.settings.rewards, now) {
            let until = reward
                .expiry
                .unwrap_or_else(|| now + TimeDelta::days(INVASION_MEMORY_DAYS));
            let unseen = self
                .announced_rewards
                .insert(reward.key.clone(), until)
                .is_none();
            if first_poll || !unseen {
                continue;
            }
            events.push(CoreEvent::WorldRewardAlert { reward });
        }
        events
    }

    pub fn handle_arbitrations(
        &mut self,
        forecast: &ArbitrationForecast,
        tiers: &ArbitrationTiers,
        fetched_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Vec<CoreEvent> {
        if now.signed_duration_since(fetched_at) > ARBITRATION_FORECAST_TRUSTED {
            return Vec::new();
        }

        let outlook = ArbitrationOutlook {
            forecast,
            tiers,
            settings: &self.settings.arbitrations,
        };
        outlook
            .due_alerts(now)
            .into_iter()
            .filter(|alert| remember(&mut self.announced_arbitrations, alert.key(), now))
            .map(|alert| CoreEvent::ArbitrationAlert { alert })
            .collect()
    }

    pub fn reward_screen_generation(&self) -> u64 {
        self.reward_screen.generation
    }

    pub fn reward_player_count(&self) -> usize {
        self.reward_screen.players.len().max(1)
    }

    pub fn start_reward_scan(&mut self) -> bool {
        !std::mem::replace(&mut self.reward_screen.scan_started, true)
    }

    pub fn announce_reward_screen(&mut self, generation: u64, rewards: usize) -> bool {
        if generation != self.reward_screen.generation || rewards <= self.reward_screen.announced {
            return false;
        }
        self.reward_screen.announced = rewards;
        true
    }

    fn new_conversation(&mut self, channel: &str, now: DateTime<Utc>) -> Vec<CoreEvent> {
        let Some(player) = conversation_player(channel) else {
            return Vec::new();
        };
        if !remember(&mut self.seen_channels, channel.to_owned(), now) {
            return Vec::new();
        }
        vec![CoreEvent::NewConversation {
            channel: channel.to_owned(),
            player,
        }]
    }
}

pub(crate) fn inventory_events(inventory: &Inventory, changes: usize) -> Vec<CoreEvent> {
    vec![CoreEvent::InventoryUpdated(InventorySummary {
        last_sync_oid: inventory.last_inventory_sync.as_str().to_owned(),
        mr: inventory.player_level,
        plat: inventory.premium_credits,
        credits: inventory.regular_credits,
        endo: inventory.fusion_points,
        ducats: inventory.counted(DUCATS_ITEM),
        changes,
    })]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::fixtures::{self, at};
    use crate::events::support::sample;
    use crate::events::{CyclePhase, FissureFilter, FissureSubtype, TimerAlerts};

    const EARLY_POLL_MILLIS: i64 = 1_788_801_669_000;
    const POLL_MILLIS: i64 = 1_788_807_069_000;

    #[test]
    fn fissure_level_ranges() {
        assert_eq!(fissure_levels(Some("Mars"), (8, 12), false), (15, 17));
        assert_eq!(fissure_levels(Some("Mars"), (8, 12), true), (115, 117));
        assert_eq!(fissure_levels(Some("Void"), (40, 45), false), (40, 45));
        assert_eq!(
            fissure_levels(Some("Kuva Fortress"), (28, 30), true),
            (160, 170)
        );
        assert_eq!(fissure_levels(None, (1, 3), false), (6, 8));
    }

    #[test]
    fn storm_alerts_from_world_state() {
        let mut engine = Engine::new(AlertSettings {
            fissure_notifications_enabled: true,
            fissure_filters: vec![FissureFilter {
                subtype: FissureSubtype::VoidStorm,
                ..FissureFilter::default()
            }],
            ..AlertSettings::default()
        });
        engine.handle_world_state(
            &world(),
            &ItemTable::default(),
            &HashSet::new(),
            at(EARLY_POLL_MILLIS),
        );
        let alerts: Vec<FissureInfo> = engine
            .handle_world_state(
                &world(),
                &ItemTable::default(),
                &HashSet::new(),
                at(POLL_MILLIS),
            )
            .into_iter()
            .filter_map(|event| match event {
                CoreEvent::FissureAlert { fissure } => Some(fissure),
                _ => None,
            })
            .collect();
        assert_eq!(alerts.len(), 6);
        assert!(alerts.iter().all(|fissure| fissure.is_storm));
        assert!(alerts.iter().all(|fissure| fissure.levels.is_none()));
        assert!(alerts.iter().any(|fissure| {
            fissure.planet == Some("Veil") && fissure.faction == Some("Corpus")
        }));
    }

    fn matching_settings() -> AlertSettings {
        AlertSettings {
            fissure_notifications_enabled: true,
            fissure_filters: vec![FissureFilter::default()],
            ..AlertSettings::default()
        }
    }

    fn world() -> WorldState {
        WorldState::parse(fixtures::WORLD_STATE).expect("world state")
    }

    #[test]
    fn reward_screen_once_per_generation() {
        let mut engine = Engine::new(AlertSettings::default());
        let now = at(1_000_000);
        assert!(
            engine
                .handle_log_event(&LogEvent::RelicRewardScreenOpened, now)
                .is_empty()
        );

        let generation = engine.reward_screen_generation();
        assert!(engine.start_reward_scan());
        assert!(!engine.start_reward_scan());
        assert!(engine.announce_reward_screen(generation, 2));
        assert!(!engine.announce_reward_screen(generation, 2));

        engine.handle_log_event(&LogEvent::RelicRewardScreenOpened, now);
        assert!(engine.start_reward_scan());
        assert!(!engine.announce_reward_screen(generation, 2));
        let next = engine.reward_screen_generation();
        assert!(engine.announce_reward_screen(next, 2));
    }

    #[test]
    fn fuller_rescan() {
        let mut engine = Engine::new(AlertSettings::default());
        let generation = engine.reward_screen_generation();
        assert!(engine.announce_reward_screen(generation, 1));
        assert!(engine.announce_reward_screen(generation, 2));
        assert!(!engine.announce_reward_screen(generation, 2));
    }

    #[test]
    fn empty_reward_screen() {
        let mut engine = Engine::new(AlertSettings::default());
        let generation = engine.reward_screen_generation();
        assert!(!engine.announce_reward_screen(generation, 0));
    }

    #[test]
    fn squad_player_count() {
        let mut engine = Engine::new(AlertSettings::default());
        let now = at(0);
        assert_eq!(engine.reward_player_count(), 1);
        for account in ["a", "b", "c"] {
            engine.handle_log_event(
                &LogEvent::SquadRewardInfoReceived {
                    account_id: account.to_owned(),
                },
                now,
            );
        }
        engine.handle_log_event(
            &LogEvent::OwnRelicReward {
                account_id: "a".to_owned(),
                store_item: "/Lotus/StoreItems/Types/Recipes/Components/FormaBlueprint".to_owned(),
            },
            now,
        );
        assert_eq!(engine.reward_player_count(), 3);
        engine.handle_log_event(&LogEvent::RelicRewardScreenOpened, now);
        assert_eq!(engine.reward_player_count(), 1);
    }

    #[test]
    fn conversation_player_channels() {
        assert_eq!(
            conversation_player("Fexampletenno"),
            Some("exampletenno".to_owned())
        );
        assert_eq!(
            conversation_player("FChanie121"),
            Some("Chanie121".to_owned())
        );
        assert_eq!(
            conversation_player("Fsomeone\u{e000}"),
            Some("someone".to_owned())
        );
        assert_eq!(conversation_player("Q_EN_EU"), None);
        assert_eq!(conversation_player("R_EN_EU"), None);
        assert_eq!(conversation_player("T_EN_EU"), None);
        assert_eq!(conversation_player("S6a9f135d000000000000ab01"), None);
        assert_eq!(conversation_player("H_TRADE_EU_EN_0"), None);
        assert_eq!(conversation_player("FChanie121"), Some("Chanie121".into()));
        assert_eq!(conversation_player("F"), None);
        assert_eq!(conversation_player("F\u{e000}"), None);
    }

    #[test]
    fn repeated_chat_tab() {
        let mut engine = Engine::new(AlertSettings::default());
        let now = at(0);
        let tab = LogEvent::ChatTabAdded {
            channel: "Fexampletenno".to_owned(),
        };
        let mut emitted = Vec::new();
        for _ in 0..5 {
            emitted.extend(engine.handle_log_event(&tab, now));
        }
        assert_eq!(
            emitted,
            vec![CoreEvent::NewConversation {
                channel: "Fexampletenno".to_owned(),
                player: "exampletenno".to_owned(),
            }]
        );
    }

    #[test]
    fn public_chat_tabs() {
        let mut engine = Engine::new(AlertSettings::default());
        let now = at(0);
        for channel in [
            "Q_EN_EU",
            "R_EN_EU",
            "T_EN_EU",
            "H_TRADE_EU_EN_0",
            "S6a9f135d000000000000ab01",
        ] {
            assert!(
                engine
                    .handle_log_event(
                        &LogEvent::ChatTabAdded {
                            channel: channel.to_owned()
                        },
                        now
                    )
                    .is_empty()
            );
        }
    }

    #[test]
    fn fissure_master_switch_off() {
        let settings = AlertSettings {
            fissure_filters: vec![FissureFilter::default()],
            ..AlertSettings::default()
        };
        assert!(!settings.wants(&sample()));

        let mut engine = Engine::new(settings);
        let world = world();
        assert!(
            engine
                .handle_world_state(
                    &world,
                    &ItemTable::default(),
                    &HashSet::new(),
                    at(EARLY_POLL_MILLIS)
                )
                .is_empty()
        );
        assert!(
            engine
                .handle_world_state(
                    &world,
                    &ItemTable::default(),
                    &HashSet::new(),
                    at(POLL_MILLIS)
                )
                .is_empty()
        );
    }

    #[test]
    fn first_poll_is_silent() {
        let world = world();
        let mut engine = Engine::new(matching_settings());
        assert!(
            engine
                .handle_world_state(
                    &world,
                    &ItemTable::default(),
                    &HashSet::new(),
                    at(POLL_MILLIS)
                )
                .is_empty()
        );
        assert!(
            engine
                .handle_world_state(
                    &world,
                    &ItemTable::default(),
                    &HashSet::new(),
                    at(POLL_MILLIS)
                )
                .is_empty()
        );
    }

    #[test]
    fn new_fissures_announced_once() {
        let world = world();
        let mut engine = Engine::new(matching_settings());
        assert!(
            engine
                .handle_world_state(
                    &world,
                    &ItemTable::default(),
                    &HashSet::new(),
                    at(EARLY_POLL_MILLIS)
                )
                .is_empty()
        );
        let events = engine.handle_world_state(
            &world,
            &ItemTable::default(),
            &HashSet::new(),
            at(POLL_MILLIS),
        );
        assert!(!events.is_empty());
        for event in &events {
            match event {
                CoreEvent::FissureAlert { fissure } => {
                    assert!(fissure.remaining_secs > 0);
                    assert_ne!(fissure.mission_name, "Unknown");
                }
                other => panic!("unexpected event {other:?}"),
            }
        }
        assert!(
            engine
                .handle_world_state(
                    &world,
                    &ItemTable::default(),
                    &HashSet::new(),
                    at(POLL_MILLIS)
                )
                .is_empty()
        );
    }

    #[test]
    fn steel_path_lith_filter() {
        let world = world();
        let settings = AlertSettings {
            fissure_notifications_enabled: true,
            fissure_filters: vec![FissureFilter {
                tier: "Lith".to_owned(),
                subtype: FissureSubtype::SteelPath,
                ..FissureFilter::default()
            }],
            ..AlertSettings::default()
        };
        let mut engine = Engine::new(settings);
        engine.handle_world_state(
            &world,
            &ItemTable::default(),
            &HashSet::new(),
            at(EARLY_POLL_MILLIS),
        );
        let events = engine.handle_world_state(
            &world,
            &ItemTable::default(),
            &HashSet::new(),
            at(POLL_MILLIS),
        );
        assert!(!events.is_empty());
        for event in &events {
            match event {
                CoreEvent::FissureAlert { fissure } => {
                    assert_eq!(fissure.tier, "Lith");
                    assert!(fissure.steel_path);
                    assert!(fissure.faction.is_some());
                    assert!(
                        fissure
                            .levels
                            .is_some_and(|(min, max)| min > 100 && max >= min)
                    );
                }
                other => panic!("unexpected event {other:?}"),
            }
        }
    }

    #[test]
    fn timer_inside_lead_window() {
        let world = world();
        let settings = AlertSettings {
            timers: TimerAlerts::from_iter([CyclePhase::EarthDay, CyclePhase::EarthNight]),
            timer_lead_secs: 86_400,
            ..AlertSettings::default()
        };
        let mut engine = Engine::new(settings);
        let now = at(POLL_MILLIS);
        let events = engine.handle_world_state(&world, &ItemTable::default(), &HashSet::new(), now);
        assert_eq!(events.len(), 1);
        match &events[0] {
            CoreEvent::TimerAlert {
                name,
                next_state,
                remaining_secs,
                ..
            } => {
                assert_eq!(name, "Earth");
                assert!(next_state == "day" || next_state == "night");
                assert!(*remaining_secs >= 0);
            }
            other => panic!("unexpected event {other:?}"),
        }
        assert!(
            engine
                .handle_world_state(&world, &ItemTable::default(), &HashSet::new(), now)
                .is_empty()
        );
    }

    #[test]
    fn timers_disabled() {
        let world = world();
        let settings = AlertSettings {
            timer_lead_secs: 86_400,
            ..AlertSettings::default()
        };
        let mut engine = Engine::new(settings);
        assert!(
            engine
                .handle_world_state(
                    &world,
                    &ItemTable::default(),
                    &HashSet::new(),
                    at(POLL_MILLIS)
                )
                .is_empty()
        );
        assert!(!TimerAlerts::default().enabled("Duviri", "joy"));
        assert!(
            !TimerAlerts::from_iter([
                CyclePhase::EarthDay,
                CyclePhase::CetusNight,
                CyclePhase::VallisCold,
                CyclePhase::CambionFass,
            ])
            .enabled("Zariman", "corpus")
        );
    }

    #[test]
    fn arriving_phase_only() {
        let world = world();
        let now = at(POLL_MILLIS);
        let coming = world
            .timers(now)
            .into_iter()
            .find(|timer| timer.name == "Orb Vallis")
            .unwrap();
        let leaving = AlertSettings {
            timers: CyclePhase::of("Orb Vallis", coming.state)
                .into_iter()
                .collect(),
            timer_lead_secs: 86_400,
            ..AlertSettings::default()
        };
        assert!(
            Engine::new(leaving)
                .handle_world_state(&world, &ItemTable::default(), &HashSet::new(), now)
                .is_empty()
        );

        let arriving = AlertSettings {
            timers: CyclePhase::of("Orb Vallis", coming.next_state)
                .into_iter()
                .collect(),
            timer_lead_secs: 86_400,
            ..AlertSettings::default()
        };
        let events = Engine::new(arriving).handle_world_state(
            &world,
            &ItemTable::default(),
            &HashSet::new(),
            now,
        );
        assert_eq!(events.len(), 1);
        match &events[0] {
            CoreEvent::TimerAlert {
                name, next_state, ..
            } => {
                assert_eq!(name, "Orb Vallis");
                assert_eq!(next_state, coming.next_state);
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    const REWARD_WORLD: &str = r#"{"Invasions":[{
        "_id":{"$oid":"6a9e1976d7b7cca80208e4d9"},"Faction":"FC_CORPUS","DefenderFaction":"FC_GRINEER",
        "Node":"SolNode131","Completed":false,
        "AttackerReward":{"countedItems":[{"ItemType":"/Lotus/Types/Items/MiscItems/OrokinCatalyst","ItemCount":1}]},
        "DefenderReward":{"countedItems":[{"ItemType":"/Lotus/Types/Items/Research/EnergyComponent","ItemCount":3}]},
        "Activation":{"$date":{"$numberLong":"1000"}}}]}"#;

    fn reward_alerts(engine: &mut Engine, world: &str, now_ms: i64) -> Vec<String> {
        let world = WorldState::parse(world).unwrap();
        engine
            .handle_world_state(&world, &ItemTable::default(), &HashSet::new(), at(now_ms))
            .into_iter()
            .filter_map(|event| match event {
                CoreEvent::WorldRewardAlert { reward } => Some(reward.label),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn chosen_reward_announced_once() {
        let mut engine = Engine::new(AlertSettings {
            reward_notifications_enabled: true,
            ..AlertSettings::default()
        });
        assert!(reward_alerts(&mut engine, "{}", 2000).is_empty());
        assert_eq!(
            reward_alerts(&mut engine, REWARD_WORLD, 3000),
            vec!["Orokin Catalyst"]
        );
        assert!(reward_alerts(&mut engine, REWARD_WORLD, 4000).is_empty());
    }

    #[test]
    fn rewards_announced_once_switched_on() {
        let mut engine = Engine::new(AlertSettings::default());
        assert!(reward_alerts(&mut engine, "{}", 2000).is_empty());
        assert!(reward_alerts(&mut engine, REWARD_WORLD, 3000).is_empty());

        engine.set_settings(AlertSettings {
            reward_notifications_enabled: true,
            ..AlertSettings::default()
        });
        assert_eq!(
            reward_alerts(&mut engine, REWARD_WORLD, 4000),
            vec!["Orokin Catalyst"]
        );
    }

    #[test]
    fn announced_set_cap() {
        let mut seen = HashMap::new();
        let start = at(POLL_MILLIS);
        for index in 0..ANNOUNCED_CAP {
            assert!(remember(&mut seen, format!("stale-{index}"), start));
        }
        let later = start + chrono::TimeDelta::seconds(ANNOUNCED_MAX_AGE_SECS + 1);
        assert!(remember(&mut seen, "fresh".to_owned(), later));
        assert_eq!(seen.len(), 1);
        assert!(seen.contains_key("fresh"));
    }

    #[test]
    fn inventory_summary() {
        let inventory = fixtures::inventory();
        let events = inventory_events(&inventory, 7);
        assert_eq!(
            events,
            vec![CoreEvent::InventoryUpdated(InventorySummary {
                last_sync_oid: "6a9eeb1f000000000000c001".to_owned(),
                mr: 14,
                plat: 5249,
                credits: 90_814_797,
                endo: 189_082,
                ducats: 937,
                changes: 7,
            })]
        );
    }
}
