use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Serialize;
use wf_inventory::Inventory;
use wf_log::Event as LogEvent;
use wf_market::{RivenData, WeaponAuctions};
use wf_worldstate::{ArbitrationForecast, ArbitrationTiers, WorldState};

use crate::account::Account;
use crate::arbitration_runs::{
    ArbitrationRun, ArbitrationRunRecorder, ArbitrationRunReward, ArbitrationRunSource,
    ArbitrationRunUpdate, MissionEndReward, VITUS_ESSENCE,
};
use crate::catalog::{Catalog, REQUIEM_MARKER};
use crate::comparables::{self, ComparedStat, RivenComparables};
use crate::delta;
use crate::ducatering::{self, Ducatering, DucateringFilter};
use crate::error::Result;
use crate::events::{
    self, ARBITRATION_FORECAST_TRUSTED, AlertSettings, ArbitrationOutlook, ArbitrationSchedule,
    CoreEvent, Engine, ScannedRewards, ScannedTrade, WorldRewardAlert,
};
use crate::export::{ExportBundle, export};
use crate::favourites::Favourites;
use crate::foundry::{self, FoundryTab};
use crate::identity::{ItemKind, ItemRecord, ItemTable};
use crate::inventory_view::{self, InventoryTab, ItemSummary};
use crate::listings::MarketListings;
use crate::mastery::{self, MasteryOptions, MasteryTab};
use crate::prices::PriceSource;
use crate::relic_planner::{self, MissingPart, RelicPlan, RelicSource, RewardScreen};
use crate::reset_checklist::{
    self, MISSION_WON_SETTING, ResetChecklistContext, ResetChecklistManualRowId,
    ResetChecklistStored, ResetChecklistTab, ResetChecklistWorld,
};
use crate::resources::{self, ResourceQuery, ResourcesTab};
use crate::rivens::{Grader, RivenRow, RivensTab};
use crate::stats::{self, DailyCount, DeltaRow, OpeningRow, StatsSummary};
use crate::store::{Snapshot, StatPoint, Store, TimeRange};
use crate::trade::{Trade, player_name};
use crate::view::View;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct RelicPlannerTab {
    pub squad_size: u32,
    pub only_owned: bool,
    pub void_traces: i64,
    pub missing_parts: Vec<MissingPart>,
    pub plans: Vec<RelicPlan>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct StatsTab {
    pub series: Vec<StatPoint>,
    pub relics_per_day: Vec<DailyCount>,
    pub trades_per_day: Vec<DailyCount>,
    pub days_played: Vec<DailyCount>,
    pub relics_opened: usize,
    pub trade_count: usize,
    pub trade_plat: i64,
    pub summary: Option<StatsSummary>,
}

struct LoadedArbitrationForecast {
    forecast: ArbitrationForecast,
    tiers: ArbitrationTiers,
    fetched_at: DateTime<Utc>,
}

pub struct Core {
    store: Store,
    catalog: Catalog,
    items: ItemTable,
    prices: Arc<dyn PriceSource + Send + Sync>,
    engine: Engine,
    arbitration_recorder: ArbitrationRunRecorder,
    account: Option<Account>,
    world: Option<WorldState>,
    arbitrations: Option<LoadedArbitrationForecast>,
    riven_data: Option<RivenData>,
    latest_snapshot: Option<Snapshot>,
    synced_vitus: Option<u32>,
    own_relic_reward: Option<String>,
    reward_screen_recorded: bool,
    favourites: Favourites,
    pending_rerolls: HashMap<String, String>,
    listings: MarketListings,
}

fn unmastered<'a>(account: Option<&Account>, catalog: &'a Catalog) -> HashSet<&'a str> {
    account.map_or_else(HashSet::new, |account| {
        mastery::unmastered_types(account, catalog)
    })
}

/// The last part of a game path: `/Lotus/Upgrades/.../AbilityDurationOnCast` gives
/// `AbilityDurationOnCast`.
/// The stems an ability's buffs' names can start with: its own name without the `Ability`
/// suffix, and its icon's name, which is what the game names its buffs by where the two
/// differ: Grendel's Nourish is `DevourerConsumeAbility` but its buff `GrendelConsume...`,
/// like its icon `GrendelConsume.png`.
fn ability_stems(ability: &wf_data::Ability) -> Vec<String> {
    let own = stem(&ability.unique_name);
    let mut stems = vec![own.strip_suffix("Ability").unwrap_or(&own).to_owned()];
    if let Some(icon) = ability
        .image_name
        .as_deref()
        .and_then(|image| image.rsplit_once('.'))
        .map(|(icon, _)| icon.to_owned())
        && !stems.contains(&icon)
    {
        stems.push(icon);
    }
    stems
}

fn stem(unique_name: &str) -> String {
    unique_name
        .rsplit('/')
        .next()
        .unwrap_or(unique_name)
        .to_owned()
}

impl Core {
    pub fn new(
        store: Store,
        catalog: Catalog,
        prices: Arc<dyn PriceSource + Send + Sync>,
        alerts: AlertSettings,
    ) -> Result<Self> {
        let favourites = store.favourites()?;
        Ok(Self {
            store,
            items: ItemTable::build(&catalog),
            catalog,
            prices,
            engine: Engine::new(alerts),
            arbitration_recorder: ArbitrationRunRecorder::new(ArbitrationRunSource::Live),
            account: None,
            world: None,
            arbitrations: None,
            riven_data: None,
            latest_snapshot: None,
            synced_vitus: None,
            own_relic_reward: None,
            reward_screen_recorded: false,
            favourites,
            pending_rerolls: HashMap::new(),
            listings: MarketListings::default(),
        })
    }

    pub fn set_alert_settings(&mut self, settings: AlertSettings) {
        self.engine.set_settings(settings);
    }

    pub fn set_riven_data(&mut self, data: RivenData) {
        self.riven_data = Some(data);
    }

    pub fn index_market(&mut self, items: &[wf_market::Item]) -> usize {
        self.items.index_market(items)
    }

    pub fn items(&self) -> &ItemTable {
        &self.items
    }

    pub fn set_market_listings(&mut self, listings: MarketListings) {
        self.listings = listings;
    }

    pub fn market_listings(&self) -> &MarketListings {
        &self.listings
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn toggle_favourite(&mut self, unique_name: &str) -> Result<bool> {
        let favourite = self.store.toggle_favourite(unique_name, Utc::now())?;
        self.favourites = self.store.favourites()?;
        Ok(favourite)
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn ducatering(&self, now: DateTime<Utc>, filter: &DucateringFilter) -> Option<Ducatering> {
        Some(Ducatering::new(ducatering::entries(
            &self.view()?,
            self.world.as_ref(),
            now,
            filter,
        )))
    }

    pub fn inventory(&self) -> Option<&Inventory> {
        self.account.as_ref().map(|account| &account.inventory)
    }

    /// The equipped warframe's abilities in slot order, with a subsumed Helminth ability in
    /// the slot it replaces.
    pub fn equipped_abilities(&self) -> Option<Vec<String>> {
        let (suit, config) = self.inventory()?.equipped_suit()?;
        let mut abilities: Vec<String> = self
            .catalog
            .item(&suit.item_type)?
            .abilities
            .as_ref()?
            .iter()
            .map(|ability| ability.unique_name.clone())
            .collect();
        let helminth = suit
            .configs
            .get(config)
            .and_then(|config| config.ability_override.as_ref());
        if let Some(helminth) = helminth
            && let Some(slot) = abilities.get_mut(helminth.index)
        {
            slot.clone_from(&helminth.ability);
        }
        Some(abilities)
    }

    /// Icons for HUD buffs, keyed by the stem a buff's name starts with: an arcane by the last
    /// part of its path (`AbilityDurationOnCast`), an ability by its name without the
    /// `Ability` suffix (`PagemasterFinalChapter`).
    pub fn buff_icons(&self) -> Vec<(String, String)> {
        let arcanes = self
            .catalog
            .items()
            .filter(|item| item.category == "Arcanes")
            .filter_map(|item| Some((stem(&item.unique_name), item.image_name.clone()?)));
        let abilities = self
            .catalog
            .items()
            .filter_map(|item| item.abilities.as_ref())
            .flatten()
            .filter_map(|ability| Some((ability, ability.image_name.clone()?)))
            .flat_map(|(ability, image)| {
                ability_stems(ability)
                    .into_iter()
                    .map(move |stem| (stem, image.clone()))
            });
        let mut icons: Vec<(String, String)> = arcanes.chain(abilities).collect();
        icons.sort();
        icons.dedup_by(|a, b| a.0 == b.0);
        icons
    }

    /// Abilities' in-game names, keyed like [`Self::buff_icons`] by the stems the ability's
    /// own name and its buffs' names start with (`BrokenGrasp` gives `Grasp of Lohk`), so the
    /// buff strip can say whose each timer is.
    pub fn ability_names(&self) -> Vec<(String, String)> {
        let mut names: Vec<(String, String)> = self
            .catalog
            .items()
            .filter_map(|item| item.abilities.as_ref())
            .flatten()
            .flat_map(|ability| {
                ability_stems(ability)
                    .into_iter()
                    .map(|stem| (stem, ability.name.clone()))
            })
            .collect();
        names.sort();
        names.dedup_by(|a, b| a.0 == b.0);
        names
    }

    /// The operator's focus school, `Madurai`, from the way its equipped focus ability is in.
    pub fn focus_school(&self) -> Option<&'static str> {
        let ability = self.inventory()?.focus_ability.as_deref()?;
        let way = ability
            .strip_prefix("/Lotus/Upgrades/Focus/")?
            .split('/')
            .next()?;
        match way {
            "Power" => Some("Madurai"),
            "Defense" => Some("Vazarin"),
            "Tactic" => Some("Naramon"),
            "Ward" => Some("Unairu"),
            "Attack" => Some("Zenurik"),
            _ => None,
        }
    }

    pub fn latest_snapshot(&self) -> Option<&Snapshot> {
        self.latest_snapshot.as_ref()
    }

    pub fn load_snapshot(&mut self) -> Result<Option<Snapshot>> {
        let Some(snapshot) = self.store.latest_snapshot()? else {
            return Ok(None);
        };
        let Some(json) = self.store.cached_inventory()? else {
            return Ok(None);
        };
        let inventory = Inventory::parse(&json)?;
        tracing::info!(
            snapshot = snapshot.id.0,
            last_sync = snapshot.last_sync_oid,
            "Snapshot restored as the current inventory"
        );
        self.latest_snapshot = Some(snapshot.clone());
        self.adopt_inventory(inventory);
        Ok(Some(snapshot))
    }

    fn adopt_inventory(&mut self, inventory: Inventory) {
        self.arbitration_recorder
            .keep_latest_booster_expiries(&inventory.boosters);
        self.account = Some(Account::new(inventory));
    }

    pub fn ingest_inventory(&mut self, json: &str, now: DateTime<Utc>) -> Result<Vec<CoreEvent>> {
        self.store.keep_raw_inventory(json)?;
        let inventory = Inventory::parse(json)?;
        let previous = self.store.latest_snapshot()?;
        let id = self.store.record_snapshot(&inventory, now)?;
        let mut changes = 0;
        self.synced_vitus = None;
        if previous.as_ref().is_none_or(|snapshot| snapshot.id != id) {
            if let Some(previous) = previous
                && let Some(cached) = self.store.cached_inventory()?
            {
                let cached = Inventory::parse(&cached)?;
                let deltas = delta::compute(&cached, &inventory);
                self.store.record_deltas(previous.id, id, &deltas)?;
                reset_checklist::record_kuva_bought(&self.store, &cached, &inventory, now)?;
                changes = deltas.len();
                self.synced_vitus = synced_vitus(&deltas);
            }
            self.store.cache_inventory(json)?;
        }
        tracing::info!(snapshot = id.0, changes, "Inventory ingested");
        self.latest_snapshot = self.store.snapshot(id)?;
        let events = events::inventory_events(&inventory, changes);
        self.adopt_inventory(inventory);
        Ok(events)
    }

    pub fn ingest_world_state(&mut self, world: WorldState, now: DateTime<Utc>) -> Vec<CoreEvent> {
        self.world = Some(world);
        self.evaluate_world_state(now)
    }

    pub fn set_arbitration_forecast(
        &mut self,
        forecast: ArbitrationForecast,
        tiers: ArbitrationTiers,
        fetched_at: DateTime<Utc>,
    ) {
        self.arbitrations = Some(LoadedArbitrationForecast {
            forecast,
            tiers,
            fetched_at,
        });
    }

    pub fn arbitration_forecast_fetched_at(&self) -> Option<DateTime<Utc>> {
        self.arbitrations.as_ref().map(|loaded| loaded.fetched_at)
    }

    pub fn arbitration_schedule(&self, now: DateTime<Utc>) -> Option<ArbitrationSchedule> {
        let loaded = self.arbitrations.as_ref()?;
        ArbitrationOutlook {
            forecast: &loaded.forecast,
            tiers: &loaded.tiers,
            settings: &self.engine.settings().arbitrations,
        }
        .schedule(now)
    }

    fn arbitration_events(&mut self, now: DateTime<Utc>) -> Vec<CoreEvent> {
        let Some(loaded) = self.arbitrations.as_ref() else {
            return Vec::new();
        };
        self.engine
            .handle_arbitrations(&loaded.forecast, &loaded.tiers, loaded.fetched_at, now)
    }

    pub fn world_reward_alerts(&self, now: DateTime<Utc>) -> Vec<WorldRewardAlert> {
        self.world.as_ref().map_or_else(Vec::new, |world| {
            events::world_reward_alerts(
                world,
                &self.items,
                &unmastered(self.account.as_ref(), &self.catalog),
                &self.engine.settings().rewards,
                now,
            )
        })
    }

    pub fn invasion_weapon_parts(&self) -> Vec<&ItemRecord> {
        self.items.invasion_weapon_parts()
    }

    pub fn world(&self) -> Option<&WorldState> {
        self.world.as_ref()
    }

    pub fn evaluate_world_state(&mut self, now: DateTime<Utc>) -> Vec<CoreEvent> {
        let mut events = match self.world.as_ref() {
            Some(world) => {
                let unmastered = unmastered(self.account.as_ref(), &self.catalog);
                self.engine
                    .handle_world_state(world, &self.items, &unmastered, now)
            }
            None => Vec::new(),
        };
        events.extend(self.arbitration_events(now));
        events
    }

    pub fn handle_log_event(
        &mut self,
        event: &LogEvent,
        now: DateTime<Utc>,
        secs: f64,
    ) -> Result<Vec<CoreEvent>> {
        if let LogEvent::OwnRelicReward { store_item, .. } = event {
            self.own_relic_reward = Some(store_item.clone());
        }
        if matches!(event, LogEvent::RelicRewardScreenOpened) {
            self.reward_screen_recorded = false;
        }
        let mut events = self.engine.handle_log_event(event, now);
        events.extend(self.record_arbitration_run(event, now, secs)?);
        self.record(&events, now)?;
        Ok(events)
    }

    fn record_arbitration_run(
        &mut self,
        event: &LogEvent,
        now: DateTime<Utc>,
        secs: f64,
    ) -> Result<Option<CoreEvent>> {
        let current_node = self.current_arbitration_node(now);
        let update = self
            .arbitration_recorder
            .observe(event, secs, current_node.as_deref());
        self.arbitration_run_event(update, now)
    }

    fn arbitration_run_event(
        &mut self,
        update: ArbitrationRunUpdate,
        now: DateTime<Utc>,
    ) -> Result<Option<CoreEvent>> {
        match update {
            ArbitrationRunUpdate::Finished(run) => {
                self.store.save_arbitration_run(&run, now)?;
                Ok(Some(CoreEvent::ArbitrationRunRecorded { run }))
            }
            ArbitrationRunUpdate::Unchanged => Ok(None),
            ArbitrationRunUpdate::Changed(run) => {
                Ok(Some(CoreEvent::ArbitrationRunUpdated { run }))
            }
        }
    }

    fn current_arbitration_node(&self, now: DateTime<Utc>) -> Option<String> {
        let loaded = self.arbitrations.as_ref()?;
        if now.signed_duration_since(loaded.fetched_at) > ARBITRATION_FORECAST_TRUSTED {
            return None;
        }

        let index = loaded.forecast.index_at(now)?;
        loaded
            .forecast
            .hour(index)
            .map(|hour| hour.node_id.to_owned())
    }

    pub fn live_arbitration_run(&self) -> Option<ArbitrationRun> {
        self.arbitration_recorder.live()
    }

    pub fn finish_arbitration_run(&mut self, now: DateTime<Utc>) -> Result<Vec<CoreEvent>> {
        let update = self.arbitration_recorder.finish();
        Ok(self
            .arbitration_run_event(update, now)?
            .into_iter()
            .collect())
    }

    pub fn arbitration_runs(&self) -> Result<Vec<ArbitrationRun>> {
        self.store.arbitration_runs()
    }

    pub fn arbitration_run(&self, id: &str) -> Result<Option<ArbitrationRun>> {
        self.store.arbitration_run(id)
    }

    pub fn delete_arbitration_run(&self, id: &str) -> Result<()> {
        self.store.delete_arbitration_run(id)
    }

    pub fn set_arbitration_run_vitus(&self, id: &str, vitus: Option<u32>) -> Result<bool> {
        let Some(mut run) = self.store.arbitration_run(id)? else {
            return Ok(false);
        };

        run.set_manual_vitus(vitus);
        self.store.update_arbitration_run(&run)?;
        Ok(true)
    }

    pub fn record_mission_end(&mut self, rewards: &[MissionEndReward]) -> bool {
        let mut merged: IndexMap<String, ArbitrationRunReward> = IndexMap::new();
        for reward in rewards {
            let record = self.items.resolve(&reward.store_item);
            merged
                .entry(record.unique_name.clone())
                .or_insert_with_key(|item_type| ArbitrationRunReward {
                    item_type: item_type.clone(),
                    name: record.name.clone(),
                    count: 0,
                })
                .count += reward.item_count;
        }
        let rewards: Vec<ArbitrationRunReward> = merged.into_values().collect();
        self.arbitration_recorder
            .attach_mission_end(&rewards, self.synced_vitus)
    }

    pub fn import_arbitration_runs(
        &self,
        lines: &[wf_log::LogLine],
        now: DateTime<Utc>,
    ) -> Result<usize> {
        let mut known = self.store.arbitration_runs()?;
        known.extend(self.arbitration_recorder.live());
        let mut imported = 0;
        for run in ArbitrationRunRecorder::replay(lines, ArbitrationRunSource::Import) {
            if known.iter().any(|seen| seen.same_mission(&run)) {
                continue;
            }

            if self.store.save_arbitration_run(&run, now)? {
                imported += 1;
            }
        }
        Ok(imported)
    }

    pub fn handle_trade_screen(
        &mut self,
        screen: &ScannedTrade,
        now: DateTime<Utc>,
    ) -> Result<Vec<CoreEvent>> {
        let events = vec![CoreEvent::TradeCompleted {
            at: now,
            partner: screen.partner.as_deref().map(player_name),
            trade: Trade::from_screen(screen, &self.catalog, &self.items),
        }];
        self.record(&events, now)?;
        Ok(events)
    }

    pub fn reward_screen_generation(&self) -> u64 {
        self.engine.reward_screen_generation()
    }

    pub fn reward_player_count(&self) -> usize {
        self.engine.reward_player_count()
    }

    pub fn start_reward_scan(&mut self) -> bool {
        self.engine.start_reward_scan()
    }

    pub fn handle_reward_screen(
        &mut self,
        now: DateTime<Utc>,
        generation: u64,
        screen: ScannedRewards,
    ) -> Result<Vec<CoreEvent>> {
        if !self
            .engine
            .announce_reward_screen(generation, screen.rewards.len())
        {
            return Ok(Vec::new());
        }

        let relic = screen
            .own_relic_type
            .and_then(|relic_type| self.catalog.relic_by_unique_name(&relic_type))
            .map(|(relic, _)| relic.name.clone());
        let rewards: Vec<ItemSummary> = screen
            .rewards
            .iter()
            .map(|reward| self.items.summary(reward))
            .collect();
        let events = vec![CoreEvent::RelicRewardScreen {
            relic,
            screen: self.recommend(&rewards),
        }];
        self.record(&events, now)?;
        Ok(events)
    }

    fn record(&mut self, events: &[CoreEvent], now: DateTime<Utc>) -> Result<()> {
        for event in events {
            match event {
                CoreEvent::TradeCompleted { at, partner, trade } => {
                    self.store.record_trade(*at, partner.as_deref(), trade)?;
                }
                CoreEvent::MissionWon => self.store.set_setting(MISSION_WON_SETTING, &now)?,
                CoreEvent::RelicRewardScreen { relic, screen } => {
                    if let (Some(relic), Some(reward)) = (relic, &self.own_relic_reward)
                        && !relic.contains(REQUIEM_MARKER)
                        && !std::mem::replace(&mut self.reward_screen_recorded, true)
                    {
                        self.store
                            .record_relic_opening(now, relic, reward, screen.ranked.len())?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn view(&self) -> Option<View<'_>> {
        Some(View {
            account: self.account.as_ref()?,
            catalog: &self.catalog,
            items: &self.items,
            prices: self.prices.as_ref(),
            favourites: &self.favourites,
            listings: &self.listings,
        })
    }

    pub fn market_owned(
        &self,
        record: &ItemRecord,
        listed_name: &str,
        rank: Option<u32>,
        subtype: Option<&str>,
    ) -> i64 {
        self.view().map_or(0, |view| {
            view.market_owned(record, listed_name, rank, subtype)
        })
    }

    pub fn inventory_tab(&self) -> Option<InventoryTab> {
        Some(inventory_view::tab(&self.view()?))
    }

    pub fn foundry_tab(
        &self,
        include_founders: Option<bool>,
        now: DateTime<Utc>,
    ) -> Option<FoundryTab> {
        Some(foundry::tab(
            &self.view()?,
            self.world.as_ref(),
            include_founders,
            now,
        ))
    }

    pub fn craft_tree(&self, unique_name: &str) -> Option<foundry::CraftDetails> {
        foundry::details(&self.view()?, unique_name)
    }

    pub fn mastery_tab(&self, options: MasteryOptions) -> Option<MasteryTab> {
        Some(mastery::tab(&self.view()?, options))
    }

    pub fn resources_tab(
        &self,
        query: &ResourceQuery,
        include_founders: Option<bool>,
        now: DateTime<Utc>,
    ) -> Option<ResourcesTab> {
        Some(resources::tab(&self.view()?, query, include_founders, now))
    }

    pub fn reset_checklist_tab(&self, now: DateTime<Utc>) -> Result<Option<ResetChecklistTab>> {
        let Some(inventory) = self.inventory() else {
            return Ok(None);
        };

        let context = ResetChecklistContext::new(
            inventory,
            &self.catalog,
            &self.items,
            self.world
                .as_ref()
                .map(|world| ResetChecklistWorld::new(world, &self.catalog, now)),
            now,
            ResetChecklistStored::load(&self.store, now)?,
        );

        Ok(Some(reset_checklist::tab(&context)))
    }

    pub fn set_reset_checklist_checked(
        &self,
        id: ResetChecklistManualRowId,
        checked: bool,
        now: DateTime<Utc>,
    ) -> Result<()> {
        reset_checklist::set_checked(&self.store, id, checked, now)
    }

    pub fn relic_planner_tab(
        &self,
        squad_size: u32,
        only_owned: bool,
        wanted: &[String],
    ) -> Option<RelicPlannerTab> {
        let view = self.view()?;
        Some(RelicPlannerTab {
            squad_size,
            only_owned,
            void_traces: relic_planner::void_traces(&view.account.inventory),
            missing_parts: relic_planner::missing_parts(view.account, view.catalog, view.items),
            plans: relic_planner::plan(&view, wanted, squad_size, only_owned, None),
        })
    }

    pub fn relic_plans(
        &self,
        tier: Option<&str>,
        squad_size: u32,
        wanted: &[String],
    ) -> Option<Vec<RelicPlan>> {
        Some(relic_planner::plan(
            &self.view()?,
            wanted,
            squad_size,
            true,
            tier,
        ))
    }

    pub fn owned_relic_slugs(&self) -> Option<Vec<String>> {
        let inventory = self.inventory()?;
        Some(
            inventory
                .relics()
                .filter(|(_, count)| *count > 0)
                .filter_map(|(unique_name, _)| {
                    self.items
                        .get(unique_name)
                        .filter(|record| matches!(record.kind, ItemKind::Relic { .. }))?
                        .market_slug
                        .clone()
                })
                .collect(),
        )
    }

    pub fn relics_for(&self, part_unique_name: &str) -> Option<Vec<RelicSource>> {
        let account = self.account.as_ref()?;
        Some(relic_planner::relics_for(
            part_unique_name,
            &account.stock,
            &self.catalog,
        ))
    }

    pub fn recommend(&self, rewards: &[ItemSummary]) -> RewardScreen {
        relic_planner::recommend(
            self.account.as_ref(),
            &self.catalog,
            &self.items,
            self.prices.as_ref(),
            &self.favourites,
            rewards,
        )
    }

    fn grader(&self) -> Grader<'_> {
        Grader::new(&self.catalog, &self.items, self.riven_data.as_ref())
    }

    pub fn rivens_tab(&self) -> Option<RivensTab> {
        let inventory = self.inventory()?;
        Some(self.grader().tab(inventory, &self.listings))
    }

    pub fn riven_row(&self, item_id: &str) -> Option<RivenRow> {
        let inventory = self.inventory()?;
        self.grader()
            .unveiled_row(item_id, inventory, &self.listings)
    }

    pub fn riven_attribution(&self) -> Option<String> {
        self.grader().attribution()
    }

    pub fn riven_cycled(&self, row: &RivenRow, current: &str, pending: &str) -> Option<RivenRow> {
        self.grader().cycled(row, current, pending)
    }

    pub fn riven_at_station(
        &self,
        item_id: &str,
        shown: &str,
        offered: Option<&str>,
    ) -> Option<RivenRow> {
        self.grader()
            .at_station(item_id, shown, offered, self.inventory())
    }

    pub fn riven_comparables(
        &self,
        stats: &[ComparedStat],
        auctions: &WeaponAuctions,
    ) -> RivenComparables {
        let attributes = self
            .riven_data
            .as_ref()
            .map_or(&[][..], |data| data.attributes.as_slice());
        comparables::comparables(stats, auctions, attributes)
    }

    pub fn riven_in_dialog(&self, mod_type: &str, fingerprint: &str) -> Option<RivenRow> {
        self.grader()
            .in_dialog(mod_type, fingerprint, self.inventory())
    }

    pub fn apply_reroll(&mut self, item_id: &str, current: &str, pending: &str) -> bool {
        if !self.set_riven_fingerprint(item_id, current) {
            return false;
        }
        self.pending_rerolls
            .insert(item_id.to_owned(), pending.to_owned());
        true
    }

    pub fn discard_pending_roll(&mut self, item_id: &str) -> bool {
        self.pending_rerolls.remove(item_id).is_some()
    }

    pub fn keep_pending_roll(&mut self, item_id: &str) -> bool {
        let Some(pending) = self.pending_rerolls.remove(item_id) else {
            return false;
        };
        self.set_riven_fingerprint(item_id, &pending)
    }

    fn set_riven_fingerprint(&mut self, item_id: &str, fingerprint: &str) -> bool {
        let Some(upgrade) = self.account.as_mut().and_then(|account| {
            account
                .inventory
                .upgrades
                .iter_mut()
                .find(|upgrade| upgrade.item_id.as_str() == item_id)
        }) else {
            return false;
        };
        upgrade.upgrade_fingerprint = Some(fingerprint.to_owned());
        true
    }

    pub fn stats_tab(&self, range: TimeRange, now: DateTime<Utc>) -> Result<StatsTab> {
        let series = stats::daily_series(&self.store.stats_series(range)?);
        let trades = self.store.trades(range)?;
        let openings = self.store.relic_openings(range)?;
        let earliest = [
            series.first().map(|point| point.at),
            trades.first().map(|trade| trade.at),
            openings.first().map(|opening| opening.at),
        ]
        .into_iter()
        .flatten()
        .min();
        let span = stats::day_span(range, earliest, now);
        let summary = self
            .account
            .as_ref()
            .map(|account| stats::summary(account, &self.catalog, &series));
        Ok(StatsTab {
            relics_per_day: stats::daily_counts(openings.iter().map(|opening| opening.at), span),
            trades_per_day: stats::daily_counts(trades.iter().map(|trade| trade.at), span),
            days_played: stats::daily_counts(series.iter().map(|point| point.at), span),
            series,
            relics_opened: openings.len(),
            trade_count: trades.len(),
            trade_plat: trades.iter().map(|trade| trade.trade.plat).sum(),
            summary,
        })
    }

    pub fn stats_deltas(&self, range: TimeRange) -> Result<Vec<DeltaRow>> {
        Ok(stats::delta_rows(&self.items, self.store.deltas(range)?))
    }

    pub fn relic_openings(&self, range: TimeRange) -> Result<Vec<OpeningRow>> {
        Ok(stats::opening_rows(
            &self.items,
            self.store.relic_openings(range)?,
        ))
    }

    pub fn export(&self, dir: &Path, now: DateTime<Utc>) -> Result<()> {
        let (Some(document), Some(inventory), Some(rivens), Some(foundry)) = (
            self.store.cached_inventory()?,
            self.inventory_tab(),
            self.rivens_tab(),
            self.foundry_tab(None, now),
        ) else {
            return Ok(());
        };
        export(
            dir,
            &ExportBundle {
                document: &document,
                inventory: &inventory,
                rivens: &rivens,
                foundry: &foundry,
            },
        )
    }
}

fn synced_vitus(deltas: &[delta::ItemDelta]) -> Option<u32> {
    let change = deltas
        .iter()
        .find(|delta| delta.item_type == VITUS_ESSENCE)
        .map_or(0, |delta| delta.after - delta.before);
    u32::try_from(change).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arbitration_runs::ArbitrationRunStatus;
    use crate::catalog::fixtures::{self, TRINITY_PRIME_SYSTEMS, at, scratch_dir, synced};
    use crate::events::ScannedTradeItem;
    use crate::events::{CyclePhase, FissureFilter, InventorySummary, TimerAlerts};
    use crate::prices::FixedPrices;
    use crate::reset_checklist::{ResetChecklistProgress, ResetChecklistRowId};
    use crate::resources::{ResourceScope, ResourceSource};
    use crate::rivens::support::{PENDING_ROLL, paid_roll, riven_table};

    fn core_over(store: Store) -> Core {
        Core::new(
            store,
            fixtures::catalog(),
            Arc::new(FixedPrices::new([("styanax_prime_blueprint", 100.0)])),
            AlertSettings::default(),
        )
        .unwrap()
    }

    fn core() -> Core {
        core_over(Store::in_memory().unwrap())
    }

    fn ingested() -> Core {
        let mut core = core();
        core.ingest_inventory(fixtures::INVENTORY, at(1_000_000))
            .unwrap();
        core
    }

    const REROLLED_ITEM: &str = "f29623082864e554d187a3f7";

    #[test]
    fn riven_row_matches_tab() {
        let mut core = core();
        core.ingest_inventory(fixtures::INVENTORY, Utc::now())
            .unwrap();
        let tab = core.rivens_tab().unwrap();
        let veiled = tab
            .veiled
            .iter()
            .flat_map(|group| &group.rivens)
            .find(|riven| !riven.pre_veiled)
            .unwrap();

        assert_eq!(core.riven_row(&veiled.riven_id), None);
        for row in tab.unveiled {
            assert_eq!(core.riven_row(&row.item_id), Some(row));
        }
    }

    fn rerolled_row(core: &Core) -> RivenRow {
        core.rivens_tab()
            .expect("rivens tab")
            .unveiled
            .into_iter()
            .find(|row| row.item_id == REROLLED_ITEM)
            .unwrap()
    }

    #[test]
    fn reroll_keep_and_discard() {
        let mut core = core();
        core.ingest_inventory(fixtures::INVENTORY, at(1_000))
            .unwrap();
        assert_eq!(rerolled_row(&core).rerolls, 74);
        let paid = paid_roll();
        assert!(!core.apply_reroll("000000000000000000000000", &paid, PENDING_ROLL));

        assert!(core.apply_reroll(REROLLED_ITEM, &paid, PENDING_ROLL));
        let current = rerolled_row(&core);
        assert_eq!(current.rerolls, 75);
        assert_eq!(current.attributes.len(), 3);
        assert_eq!(current.attributes[1].tag, "WeaponToxinDamageMod");

        assert!(core.keep_pending_roll(REROLLED_ITEM));
        let kept = rerolled_row(&core);
        assert_eq!(kept.rerolls, 75);
        assert_eq!(kept.attributes.len(), 4);
        assert_eq!(kept.attributes[1].tag, "WeaponCritDamageMod");
        assert!(!core.keep_pending_roll(REROLLED_ITEM));

        assert!(core.apply_reroll(REROLLED_ITEM, &paid, PENDING_ROLL));
        assert!(core.discard_pending_roll(REROLLED_ITEM));
        assert!(!core.keep_pending_roll(REROLLED_ITEM));
        assert_eq!(rerolled_row(&core).attributes.len(), 3);
    }

    #[test]
    fn same_inventory_twice() {
        let mut core = core();
        let events = core
            .ingest_inventory(fixtures::INVENTORY, at(1_000_000))
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            CoreEvent::InventoryUpdated(InventorySummary { changes, mr, .. }) => {
                assert_eq!(*changes, 0);
                assert_eq!(*mr, 14);
            }
            other => panic!("unexpected event {other:?}"),
        }

        core.ingest_inventory(fixtures::INVENTORY, at(2_000_000))
            .unwrap();
        let stats = core.stats_tab(TimeRange::all(), Utc::now()).unwrap();
        assert_eq!(stats.series.len(), 1);
        assert!(core.stats_deltas(TimeRange::all()).unwrap().is_empty());
    }

    #[test]
    fn latest_snapshot_follows_ingest() {
        let mut core = core();
        assert!(core.latest_snapshot().is_none());
        core.ingest_inventory(fixtures::INVENTORY, at(1_000_000))
            .unwrap();
        let first = core.latest_snapshot().unwrap().clone();
        assert_eq!(first.taken_at, at(1_000_000));
        assert_eq!(first.last_sync_oid, "6a9eeb1f000000000000c001");
        assert_eq!(core.store().latest_snapshot().unwrap(), Some(first.clone()));

        core.ingest_inventory(fixtures::INVENTORY, at(2_000_000))
            .unwrap();
        assert_eq!(core.latest_snapshot(), Some(&first));

        let changed = synced("6a9eeb1f000000000000c002");
        core.ingest_inventory(&changed, at(3_000_000)).unwrap();
        let second = core.latest_snapshot().unwrap();
        assert_eq!(second.taken_at, at(3_000_000));
        assert_eq!(second.last_sync_oid, "6a9eeb1f000000000000c002");
        assert_ne!(second.id, first.id);
    }

    #[test]
    fn load_cached_snapshot() {
        let dir = scratch_dir("wf-core-facade-cached-inventory");
        let path = dir.join("merframe.sqlite");

        let mut writer = core_over(Store::open(&path).unwrap());
        assert!(writer.load_snapshot().unwrap().is_none());
        writer
            .ingest_inventory(fixtures::INVENTORY, at(1_000_000))
            .unwrap();
        drop(writer);
        assert!(dir.join("inventory.json.gz").exists());

        let mut reader = core_over(Store::open(&path).unwrap());
        assert!(reader.inventory().is_none());
        let snapshot = reader.load_snapshot().unwrap().unwrap();
        assert_eq!(snapshot.last_sync_oid, "6a9eeb1f000000000000c001");
        assert_eq!(reader.latest_snapshot(), Some(&snapshot));
        assert!(reader.inventory().is_some());
        assert!(reader.inventory_tab().is_some());
        let stats = reader.stats_tab(TimeRange::all(), Utc::now()).unwrap();
        assert_eq!(stats.series.len(), 1);
        assert!(reader.stats_deltas(TimeRange::all()).unwrap().is_empty());
        drop(reader);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unparsed_inventory_is_kept_apart_from_the_cache() {
        let dir = scratch_dir("wf-core-facade-raw-inventory");
        let mut core = core_over(Store::open(&dir.join("merframe.sqlite")).unwrap());
        assert!(core.ingest_inventory("{}", at(1_000_000)).is_err());
        assert!(dir.join("inventory_latest_raw.json.gz").exists());
        assert!(!dir.join("inventory.json.gz").exists());
    }

    #[test]
    fn changed_inventory_deltas() {
        let mut core = ingested();
        let changed = synced("6a9eeb1f000000000000c002").replacen(
            r#"{"ItemCount":51923112,"ItemType":"/Lotus/Types/Items/MiscItems/Ferrite"}"#,
            r#"{"ItemCount":51923260,"ItemType":"/Lotus/Types/Items/MiscItems/Ferrite"}"#,
            1,
        );
        let events = core.ingest_inventory(&changed, at(90_000_000)).unwrap();
        match &events[0] {
            CoreEvent::InventoryUpdated(InventorySummary { changes, .. }) => {
                assert_eq!(*changes, 1);
            }
            other => panic!("unexpected event {other:?}"),
        }
        let stats = core.stats_tab(TimeRange::all(), Utc::now()).unwrap();
        assert_eq!(stats.series.len(), 2);
        let deltas = core.stats_deltas(TimeRange::all()).unwrap();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].delta, 148);
    }

    #[test]
    fn two_snapshots_same_day() {
        let mut core = ingested();
        let richer = synced("6a9eeb1f000000000000c002").replacen(
            r#""PremiumCredits":5249"#,
            r#""PremiumCredits":5427"#,
            1,
        );
        core.ingest_inventory(&richer, at(80_000_000)).unwrap();
        let stats = core.stats_tab(TimeRange::all(), at(80_000_000)).unwrap();
        assert_eq!(stats.series.len(), 1);
        assert_eq!(stats.series[0].at, at(0));
        assert_eq!(stats.series[0].plat, 5427);
        assert_eq!(stats.days_played.len(), 1);
        assert_eq!(stats.days_played[0].count, 1);
        assert_eq!(stats.summary.unwrap().snapshot_days, 1);
    }

    #[test]
    fn tabs_after_ingest() {
        let mut core = core();
        assert!(core.inventory_tab().is_none());
        core.ingest_inventory(fixtures::INVENTORY, at(1_000_000))
            .unwrap();
        assert!(core.inventory_tab().is_some());
        assert!(core.foundry_tab(None, at(1_000_000)).is_some());
        assert_eq!(
            core.mastery_tab(MasteryOptions::default()).unwrap().rank,
            14
        );
        assert!(
            core.resources_tab(
                &ResourceQuery {
                    source: ResourceSource::Held,
                    scope: ResourceScope::Mastery,
                    kind: None,
                    prime: None,
                    owned: None,
                },
                None,
                Utc::now()
            )
            .is_some()
        );
        let planner = core
            .relic_planner_tab(relic_planner::DEFAULT_SQUAD_SIZE, true, &[])
            .unwrap();
        assert_eq!(planner.missing_parts.len(), 8);
        assert_eq!(planner.plans.len(), 1);
        assert!(core.rivens_tab().is_some());
        let summary = core
            .stats_tab(TimeRange::all(), at(1_000_000))
            .unwrap()
            .summary
            .unwrap();
        assert_eq!(summary.snapshot_days, 1);
        assert!(summary.prime_owned > 0);
        assert!(summary.prime_owned <= summary.prime_total);
        assert!(summary.prime_percent > 0.0);
        assert!(
            core.relics_for(TRINITY_PRIME_SYSTEMS)
                .is_some_and(|sources| !sources.is_empty())
        );
    }

    #[test]
    fn reset_checklist_tab_after_ingest() {
        let mut core = core();
        let now = at(1_788_800_000_000);
        let row = |core: &Core, id: ResetChecklistRowId| {
            core.reset_checklist_tab(now)
                .unwrap()
                .unwrap()
                .rows
                .into_iter()
                .find(|row| row.id == id)
                .unwrap()
        };

        assert!(core.reset_checklist_tab(now).unwrap().is_none());

        core.ingest_inventory(fixtures::INVENTORY, now).unwrap();

        assert!(!row(&core, ResetChecklistRowId::FirstWin).done);
        assert!(!row(&core, ResetChecklistRowId::VentkidsRaces).done);

        let events = core
            .handle_log_event(&LogEvent::MissionSucceeded, now, 1.0)
            .unwrap();
        core.set_reset_checklist_checked(ResetChecklistManualRowId::VentkidsRaces, true, now)
            .unwrap();

        assert_eq!(events, [CoreEvent::MissionWon]);
        assert!(row(&core, ResetChecklistRowId::FirstWin).done);
        assert!(row(&core, ResetChecklistRowId::VentkidsRaces).done);
        assert_eq!(
            row(&core, ResetChecklistRowId::Sortie).progress,
            ResetChecklistProgress::Unknown
        );
    }

    #[test]
    fn kuva_bought_recorded_on_sync() {
        let mut core = core();
        let now = at(1_788_800_000_000);
        core.ingest_inventory(fixtures::INVENTORY, now).unwrap();
        let bought = synced("6a9eeb1f000000000000c002")
            .replacen(
                r#""NumPurchased":2,"ItemId":"6a9a2f60000000000000f201""#,
                r#""NumPurchased":5,"ItemId":"6a9a2f60000000000000f201""#,
                1,
            )
            .replacen(
                r#""MiscItems":["#,
                r#""MiscItems":[{"ItemCount":30000,"ItemType":"/Lotus/Types/Items/MiscItems/Kuva"},"#,
                1,
            );

        core.ingest_inventory(&bought, now).unwrap();

        let kuva = core.reset_checklist_tab(now).unwrap().unwrap().kuva;
        assert_eq!(kuva.estimated, 30_000);
    }

    #[test]
    fn relic_plans_of_one_tier() {
        let core = ingested();
        let squad = relic_planner::DEFAULT_SQUAD_SIZE;
        let tab = core.relic_planner_tab(squad, true, &[]).unwrap();
        assert!(!tab.plans.is_empty());
        assert_eq!(core.relic_plans(None, squad, &[]).unwrap(), tab.plans);

        for tier in ["Lith", "Meso", "Neo", "Axi"] {
            let of_tier: Vec<RelicPlan> = tab
                .plans
                .iter()
                .filter(|plan| plan.tier == tier)
                .cloned()
                .collect();
            assert_eq!(core.relic_plans(Some(tier), squad, &[]).unwrap(), of_tier);
        }
    }

    #[test]
    fn riven_attribution_follows_riven_data() {
        let mut core = core();
        assert!(core.riven_attribution().is_none());

        core.set_riven_data(riven_table());
        assert!(
            core.riven_attribution()
                .is_some_and(|attribution| attribution.contains("44bananas"))
        );
    }

    #[test]
    fn favourite_toggle_across_tabs() {
        let mut core = ingested();
        let relic = "/Lotus/Types/Game/Projections/T4VoidProjectionStyanaxPrimeABronze";
        assert!(core.favourites.is_empty());

        assert!(core.toggle_favourite(relic).unwrap());
        assert!(core.favourites.contains(relic));
        let tab = core.inventory_tab().unwrap();
        let starred = tab
            .relics
            .iter()
            .filter(|row| tab.favourites.contains(&row.item))
            .map(|row| tab.items[row.item].unique_name.clone())
            .collect::<Vec<_>>();
        assert_eq!(starred, vec![relic.to_owned()]);
        assert!(
            core.relic_planner_tab(relic_planner::DEFAULT_SQUAD_SIZE, true, &[])
                .unwrap()
                .plans
                .iter()
                .any(|plan| plan.favourite)
        );

        assert!(!core.toggle_favourite(relic).unwrap());
        assert!(core.favourites.is_empty());
        assert!(core.inventory_tab().unwrap().favourites.is_empty());
    }

    #[test]
    fn chat_tab_notified_once() {
        let mut core = core();
        let now = at(4_000_000);
        let tab = LogEvent::ChatTabAdded {
            channel: "Fexampletenno".to_owned(),
        };
        let first = core.handle_log_event(&tab, now, 1.0).expect("chat tab");
        assert_eq!(
            first,
            vec![CoreEvent::NewConversation {
                channel: "Fexampletenno".to_owned(),
                player: "exampletenno".to_owned(),
            }]
        );
        let second = core.handle_log_event(&tab, now, 1.0).expect("chat tab");
        assert!(second.is_empty());
        core.set_alert_settings(AlertSettings::default());
        let third = core.handle_log_event(&tab, now, 1.0).expect("chat tab");
        assert!(third.is_empty());
        let region = core
            .handle_log_event(
                &LogEvent::ChatTabAdded {
                    channel: "Q_EN_EU".to_owned(),
                },
                now,
                1.0,
            )
            .expect("chat tab");
        assert!(region.is_empty());
    }

    #[test]
    fn requiem_opening_skipped() {
        let mut core = core();
        let now = at(3_000_000);

        core.handle_log_event(
            &LogEvent::OwnRelicReward {
                account_id: "5f0a1b2c3d4e5f6a7b8c9d0e".to_owned(),
                store_item: "/Lotus/StoreItems/Upgrades/Mods/Immortal/RequiemModA".to_owned(),
            },
            now,
            1.0,
        )
        .unwrap();
        let events = core
            .handle_reward_screen(
                now,
                core.reward_screen_generation(),
                ScannedRewards {
                    own_relic_type: Some(
                        "/Lotus/Types/Game/Projections/T5VoidProjectionImmortalCPlatinum"
                            .to_owned(),
                    ),
                    rewards: vec![
                        "/Lotus/StoreItems/Upgrades/Mods/Immortal/RequiemModA".to_owned(),
                    ],
                },
            )
            .unwrap();
        assert_eq!(events.len(), 1);

        assert!(core.relic_openings(TimeRange::all()).unwrap().is_empty());
    }

    #[test]
    fn trade_and_opening_recorded() {
        let mut core = core();
        let now = at(3_000_000);

        core.handle_log_event(
            &LogEvent::OwnRelicReward {
                account_id: "5f0a1b2c3d4e5f6a7b8c9d0e".to_owned(),
                store_item: "/Lotus/StoreItems/Types/Recipes/WarframeRecipes/StyanaxPrimeBlueprint"
                    .to_owned(),
            },
            now,
            1.0,
        )
        .unwrap();
        let events = core
            .handle_reward_screen(
                now,
                core.reward_screen_generation(),
                ScannedRewards {
                    own_relic_type: Some(
                        "/Lotus/Types/Game/Projections/T4VoidProjectionStyanaxPrimeAGold"
                            .to_owned(),
                    ),
                    rewards: vec![
                        "/Lotus/StoreItems/Types/Recipes/WarframeRecipes/StyanaxPrimeBlueprint"
                            .to_owned(),
                    ],
                },
            )
            .unwrap();
        let [CoreEvent::RelicRewardScreen { relic, screen }] = events.as_slice() else {
            panic!("one reward screen event, got {events:?}");
        };
        assert_eq!(relic.as_deref(), Some("Axi A21"));
        assert_eq!(screen.ranked[0].item.name, "Styanax Prime Blueprint");
        assert!(screen.account.is_none());
        assert!(screen.ranked[0].ownership.is_none());

        let events = core
            .handle_trade_screen(
                &ScannedTrade {
                    partner: Some(String::from("TestSquadA\u{e000}")),
                    offered: vec![ScannedTradeItem {
                        name: String::from("Styanax Prime Blueprint"),
                        item_type: Some(String::from(
                            "/Lotus/Types/Recipes/WarframeRecipes/StyanaxPrimeBlueprint",
                        )),
                        count: 1,
                        fingerprint: None,
                    }],
                    received: vec![ScannedTradeItem {
                        name: String::from("Platinum"),
                        item_type: None,
                        count: 90,
                        fingerprint: None,
                    }],
                },
                now,
            )
            .unwrap();
        assert!(matches!(
            &events[..],
            [CoreEvent::TradeCompleted { at, partner: Some(partner), trade }]
                if *at == now && partner == "TestSquadA" && trade.offered[0].name == "Styanax Prime Blueprint"
        ));

        let stats = core
            .stats_tab(TimeRange::all(), now + chrono::TimeDelta::days(2))
            .unwrap();
        assert_eq!(stats.trade_count, 1);
        assert_eq!(stats.trade_plat, 90);
        assert_eq!(stats.relics_opened, 1);
        let openings = core.relic_openings(TimeRange::all()).unwrap();
        assert_eq!(openings.len(), 1);
        assert_eq!(openings[0].relic, "Axi A21");
        assert_eq!(openings[0].player_count, 1);
        assert_eq!(
            stats
                .trades_per_day
                .iter()
                .map(|entry| entry.count)
                .collect::<Vec<_>>(),
            vec![1, 0, 0]
        );
        assert_eq!(
            stats
                .relics_per_day
                .iter()
                .map(|entry| entry.count)
                .collect::<Vec<_>>(),
            vec![1, 0, 0]
        );
        assert!(stats.summary.is_none());
    }

    #[test]
    fn world_state_alerts_and_timers() {
        let mut core = core();
        core.set_alert_settings(AlertSettings {
            fissure_notifications_enabled: true,
            fissure_filters: vec![FissureFilter {
                tier: "Lith".to_owned(),
                ..FissureFilter::default()
            }],
            timers: TimerAlerts::from_iter([CyclePhase::EarthDay, CyclePhase::EarthNight]),
            timer_lead_secs: 86_400,
            ..AlertSettings::default()
        });
        let world_state = WorldState::parse(fixtures::WORLD_STATE).expect("world state");
        let early = at(1_788_801_669_000);
        let now = at(1_788_807_069_000);
        let first = core.ingest_world_state(world_state.clone(), early);
        assert!(
            first
                .iter()
                .all(|event| matches!(event, CoreEvent::TimerAlert { .. }))
        );
        let events = core.ingest_world_state(world_state, now);
        assert!(events.iter().any(|event| match event {
            CoreEvent::FissureAlert { fissure } => fissure.tier == "Lith",
            _ => false,
        }));
        core.ingest_inventory(fixtures::INVENTORY, now).unwrap();
        let foundry = core.foundry_tab(None, now).unwrap();
        assert!(!foundry.timers.is_empty());
    }

    #[test]
    fn export_needs_inventory() {
        let mut core = core();
        let root = scratch_dir("wf-core-facade-export");
        let dir = root.join("export");
        core.export(&dir, at(0)).unwrap();
        assert!(!dir.exists());

        core.ingest_inventory(fixtures::INVENTORY, at(1_000_000))
            .unwrap();
        core.export(&dir, at(1_000_000)).unwrap();
        assert!(dir.join("parts.json").exists());
        assert_eq!(
            std::fs::read_to_string(dir.join("inventory.json")).unwrap(),
            fixtures::INVENTORY
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    fn observe_lines(core: &mut Core, lines: &[wf_log::LogLine], now: DateTime<Utc>) {
        for line in lines {
            if let Some(event) = wf_log::classify(line) {
                core.handle_log_event(&event, now, line.time).unwrap();
            }
        }
    }

    #[test]
    fn arbitration_log_import_is_idempotent() {
        let lines = crate::arbitration_runs::fixture_lines();
        let core = core();
        let now = at(1_792_858_804_954);
        assert_eq!(core.import_arbitration_runs(&lines, now).unwrap(), 1);
        assert_eq!(core.import_arbitration_runs(&lines, now).unwrap(), 0);
        let runs = core.arbitration_runs().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].source, ArbitrationRunSource::Import);
        assert!(
            core.set_arbitration_run_vitus(&runs[0].id, Some(4))
                .unwrap()
        );
        assert_eq!(
            core.arbitration_run(&runs[0].id)
                .unwrap()
                .unwrap()
                .manual_vitus,
            Some(4)
        );
        core.delete_arbitration_run(&runs[0].id).unwrap();
        assert!(core.arbitration_runs().unwrap().is_empty());
    }

    fn synced_with_vitus(oid: &str, count: i64) -> String {
        let mut synced: serde_json::Value = serde_json::from_str(fixtures::INVENTORY).unwrap();
        fixtures::push_counted(&mut synced, "MiscItems", VITUS_ESSENCE, count);
        synced["LastInventorySync"] = serde_json::json!({ "$oid": oid });
        synced.to_string()
    }

    fn mission_end_rewards() -> Vec<MissionEndReward> {
        let vitus = || MissionEndReward {
            store_item: String::from("/Lotus/StoreItems/Types/Items/MiscItems/Elitium"),
            item_count: 1,
        };
        vec![vitus(), vitus()]
    }

    #[test]
    fn mission_end_attaches_to_the_live_run() {
        let lines = crate::arbitration_runs::fixture_lines();
        let home = lines
            .iter()
            .rposition(|line| matches!(wf_log::classify(line), Some(LogEvent::LevelLoaded { .. })))
            .unwrap();
        let (mission, rest) = lines.split_at(home);
        let mut core = ingested();
        let now = at(1_792_858_804_954);
        observe_lines(&mut core, mission, now);
        core.ingest_inventory(&synced_with_vitus("6a9eeb1f000000000000c002", 3), now)
            .unwrap();

        let rewards = mission_end_rewards();
        assert!(core.record_mission_end(&rewards));
        assert!(!core.record_mission_end(&rewards));
        let live = core.live_arbitration_run().unwrap();
        assert_eq!(live.status, ArbitrationRunStatus::Completed);
        assert_eq!(live.vitus, Some(3));

        observe_lines(&mut core, rest, now);
        let runs = core.arbitration_runs().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].collected_vitus, Some(3));
        assert_eq!(
            runs[0].rewards,
            Some(vec![ArbitrationRunReward {
                item_type: String::from(VITUS_ESSENCE),
                name: String::from("Vitus Essence"),
                count: 2,
            }])
        );
    }

    #[test]
    fn mission_end_without_a_previous_sync_reads_no_vitus() {
        let lines = crate::arbitration_runs::fixture_lines();
        let home = lines
            .iter()
            .rposition(|line| matches!(wf_log::classify(line), Some(LogEvent::LevelLoaded { .. })))
            .unwrap();
        let mut core = core();
        let now = at(1_792_858_804_954);
        observe_lines(&mut core, &lines[..home], now);
        core.ingest_inventory(&synced_with_vitus("6a9eeb1f000000000000c002", 3), now)
            .unwrap();

        assert!(core.record_mission_end(&mission_end_rewards()));
        let live = core.live_arbitration_run().unwrap();
        assert_eq!(live.vitus, None);
        assert_eq!(live.rewards.as_ref().map(Vec::len), Some(1));
    }

    #[test]
    fn arbitration_log_import_skips_a_run_keyed_differently() {
        let lines = crate::arbitration_runs::fixture_lines();
        let core = core();
        let now = at(1_792_858_804_954);
        let mut stored = crate::arbitration_runs::fixture_runs().remove(0);
        stored.id = format!("drone:{}", stored.id);
        stored.started_at = stored
            .started_at
            .map(|at| at + chrono::Duration::seconds(30));
        core.store.save_arbitration_run(&stored, now).unwrap();

        assert_eq!(core.import_arbitration_runs(&lines, now).unwrap(), 0);

        let mut later = stored.clone();
        later.id = "later".to_owned();
        later.started_at = stored
            .started_at
            .map(|at| at + chrono::Duration::seconds(60));
        assert!(stored.same_mission(&later));
        assert!(later.same_mission(&stored));

        let mut elsewhere = stored.clone();
        elsewhere.id = "elsewhere".to_owned();
        elsewhere.node_id = Some("SolNode1".to_owned());
        assert!(!stored.same_mission(&elsewhere));
        assert_eq!(core.arbitration_runs().unwrap().len(), 1);
    }

    #[test]
    fn arbitration_run_without_a_reward_is_dropped() {
        let mut core = core();
        let now = at(1_792_858_804_954);
        let mission = LogEvent::MissionLoading {
            host: true,
            node_id: "SolNode64".to_owned(),
            elite: true,
        };
        core.handle_log_event(&mission, now, 10.0).unwrap();
        core.handle_log_event(&LogEvent::MissionAborted, now, 40.0)
            .unwrap();
        let home = LogEvent::LevelLoaded {
            path: "/Lotus/Levels/Proc/PlayerShip/AeAo.lp".to_owned(),
        };
        let events = core.handle_log_event(&home, now, 50.0).unwrap();

        assert!(matches!(
            events.as_slice(),
            [CoreEvent::ArbitrationRunUpdated { run }] if run.status == ArbitrationRunStatus::Aborted
        ));
        assert!(core.live_arbitration_run().is_none());
        assert!(core.arbitration_runs().unwrap().is_empty());
    }

    #[test]
    fn arbitration_log_import_skips_the_live_run() {
        let lines = crate::arbitration_runs::fixture_lines();
        let first_reward = lines
            .iter()
            .position(|line| {
                matches!(
                    wf_log::classify(line),
                    Some(LogEvent::SurvivalReward { .. })
                )
            })
            .unwrap();
        let (started, rest) = lines.split_at(first_reward + 1);
        let mut core = core();
        let now = at(1_792_858_804_954);
        observe_lines(&mut core, started, now);
        let live = core.live_arbitration_run().unwrap();

        assert_eq!(core.import_arbitration_runs(started, now).unwrap(), 0);

        observe_lines(&mut core, rest, now);
        core.finish_arbitration_run(now).unwrap();

        let runs = core.arbitration_runs().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, live.id);
        assert_eq!(runs[0].source, ArbitrationRunSource::Live);
        assert_eq!(runs[0].status, ArbitrationRunStatus::Completed);
    }
}
