use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;
use wf_inventory::Inventory;
use wf_log::Event as LogEvent;
use wf_market::{RivenData, WeaponAuctions};
use wf_worldstate::{ArbitrationForecast, ArbitrationTiers, WorldState};

use crate::account::Account;
use crate::catalog::{Catalog, REQUIEM_MARKER};
use crate::comparables::{self, ComparedStat, RivenComparables};
use crate::delta;
use crate::ducatering::{self, Ducatering, DucateringFilter};
use crate::error::Result;
use crate::events::{
    self, AlertSettings, ArbitrationOutlook, ArbitrationSchedule, CoreEvent, Engine,
    ScannedRewards, ScannedTrade, WorldRewardAlert,
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
    account: Option<Account>,
    world: Option<WorldState>,
    arbitrations: Option<LoadedArbitrationForecast>,
    riven_data: Option<RivenData>,
    latest_snapshot: Option<Snapshot>,
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
            account: None,
            world: None,
            arbitrations: None,
            riven_data: None,
            latest_snapshot: None,
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

    pub fn ducatering(&self, filter: &DucateringFilter) -> Option<Ducatering> {
        Some(Ducatering::new(ducatering::entries(&self.view()?, filter)))
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
        self.account = Some(Account::new(inventory));
        Ok(Some(snapshot))
    }

    pub fn ingest_inventory(&mut self, json: &str, now: DateTime<Utc>) -> Result<Vec<CoreEvent>> {
        self.store.keep_raw_inventory(json)?;
        let inventory = Inventory::parse(json)?;
        let previous = self.store.latest_snapshot()?;
        let id = self.store.record_snapshot(&inventory, now)?;
        let mut changes = 0;
        if previous.as_ref().is_none_or(|snapshot| snapshot.id != id) {
            if let Some(previous) = previous
                && let Some(cached) = self.store.cached_inventory()?
            {
                let deltas = delta::compute(&Inventory::parse(&cached)?, &inventory);
                self.store.record_deltas(previous.id, id, &deltas)?;
                changes = deltas.len();
            }
            self.store.cache_inventory(json)?;
        }
        tracing::info!(snapshot = id.0, changes, "Inventory ingested");
        self.latest_snapshot = self.store.snapshot(id)?;
        let events = events::inventory_events(&inventory, changes);
        self.account = Some(Account::new(inventory));
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
    ) -> Result<Vec<CoreEvent>> {
        if let LogEvent::OwnRelicReward { store_item, .. } = event {
            self.own_relic_reward = Some(store_item.clone());
        }
        if matches!(event, LogEvent::RelicRewardScreenOpened) {
            self.reward_screen_recorded = false;
        }
        let events = self.engine.handle_log_event(event, now);
        self.record(&events, now)?;
        Ok(events)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::fixtures::{self, TRINITY_PRIME_SYSTEMS, at, scratch_dir, synced};
    use crate::events::ScannedTradeItem;
    use crate::events::{CyclePhase, FissureFilter, InventorySummary, TimerAlerts};
    use crate::prices::FixedPrices;
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
        let first = core.handle_log_event(&tab, now).expect("chat tab");
        assert_eq!(
            first,
            vec![CoreEvent::NewConversation {
                channel: "Fexampletenno".to_owned(),
                player: "exampletenno".to_owned(),
            }]
        );
        let second = core.handle_log_event(&tab, now).expect("chat tab");
        assert!(second.is_empty());
        core.set_alert_settings(AlertSettings::default());
        let third = core.handle_log_event(&tab, now).expect("chat tab");
        assert!(third.is_empty());
        let region = core
            .handle_log_event(
                &LogEvent::ChatTabAdded {
                    channel: "Q_EN_EU".to_owned(),
                },
                now,
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
}
