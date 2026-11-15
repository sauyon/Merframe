mod arbitration;
mod archimedea;
mod archon;
mod calendar;
mod circuit;
mod clan_vault;
mod cycles;
mod daily_deal;
mod embedded;
mod error;
#[cfg(feature = "fetch")]
mod fetch;
mod fissure;
mod flash_sale;
mod manifest;
mod mission_types;
mod nodes;
mod prime;
mod resets;
mod rewards;
mod season;
mod sortie;
mod syndicate;
mod void_trader;
mod window;

pub use arbitration::{
    ARBITRATION_FORECAST_URL, ARBITRATION_TIERS_URL, ArbitrationForecast, ArbitrationForecastHour,
    ArbitrationNode, ArbitrationTier, ArbitrationTiers, arbitration_node,
};
pub use archimedea::{ArchimedeaKind, ArchimedeaWeek};
pub use archon::{ArchonHunt, ArchonMission};
pub use calendar::CalendarSeason;
pub use circuit::Circuit;
pub use clan_vault::ClanVaultBonusWeek;
pub use cycles::Timer;
pub use daily_deal::DailyDeal;
pub use error::{Result, WorldStateError};
#[cfg(feature = "fetch")]
pub use fetch::{fetch, fetch_text};

pub const WORLD_STATE_URL: &str = "https://api.warframe.com/cdn/worldState.php";
pub use fissure::{Fissure, FissureTier, RelicTier};
pub use flash_sale::MarketSale;
pub use manifest::ManifestItem;
pub use mission_types::mission_type_name;
pub use nodes::{
    SolNode, is_masterable_node, junction_count, junction_mastery_xp, junction_nodes,
    masterable_node_count, masterable_node_xp_total, masterable_nodes, node_and_planet, node_info,
    node_mastery_xp, node_name,
};
pub use prime::{PrimeAccessAvailability, Varzia, VarziaItem};
pub use resets::{ResetWindow, daily_reset_window, game_week_count, weekly_reset_window};
pub use rewards::{WorldRewardOffer, WorldRewardSource};
pub use season::{
    ChallengeKind, NightwaveActDefinition, NightwaveActText, NightwaveChallenge, NightwaveSeason,
    nightwave_act_definition,
};
pub use sortie::{Sortie, SortieMission};
pub use syndicate::SyndicateMissionSet;
pub use void_trader::BaroStatus;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use archimedea::RawConquest;
use archon::LiteSortie;
use calendar::RawCalendarSeason;
use circuit::EndlessXpWeek;
use clan_vault::RawClanVaultBonusWeek;
use daily_deal::RawDailyDeal;
use fissure::{ActiveMission, VoidStorm};
use flash_sale::RawFlashSale;
use prime::{PrimeVaultTrader, RawPrimeAccessAvailability};
use rewards::{RawAlert, RawGoal, RawInvasion};
use season::SeasonInfo;
use sortie::RawSortie;
use syndicate::SyndicateMission;
use void_trader::VoidTrader;
use window::Window;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawWorldState {
    #[serde(default)]
    active_missions: Vec<ActiveMission>,
    #[serde(default)]
    void_storms: Vec<VoidStorm>,
    #[serde(default)]
    void_traders: Vec<VoidTrader>,
    #[serde(default)]
    sorties: Vec<RawSortie>,
    #[serde(default)]
    lite_sorties: Vec<LiteSortie>,
    #[serde(default)]
    daily_deals: Vec<RawDailyDeal>,
    #[serde(default)]
    flash_sales: Vec<RawFlashSale>,
    #[serde(default)]
    season_info: Option<SeasonInfo>,
    #[serde(default)]
    prime_vault_traders: Vec<PrimeVaultTrader>,
    #[serde(default)]
    prime_access_availability: Option<RawPrimeAccessAvailability>,
    #[serde(default)]
    syndicate_missions: Vec<SyndicateMission>,
    #[serde(default)]
    endless_xp_schedule: Vec<EndlessXpWeek>,
    #[serde(default)]
    invasions: Vec<RawInvasion>,
    #[serde(default)]
    alerts: Vec<RawAlert>,
    #[serde(default)]
    goals: Vec<RawGoal>,
    #[serde(default)]
    known_calendar_seasons: Vec<RawCalendarSeason>,
    #[serde(default)]
    weekly_vault_bonus_rewards: Vec<RawClanVaultBonusWeek>,
    #[serde(default)]
    conquests: Vec<RawConquest>,
    #[serde(default)]
    descents: Vec<Window>,
}

#[derive(Debug, Clone)]
pub struct WorldState {
    raw: RawWorldState,
}

impl WorldState {
    pub fn parse(json: &str) -> Result<Self> {
        let raw: RawWorldState = serde_json::from_str(json)?;
        Ok(Self { raw })
    }

    pub fn fissures(&self, now: DateTime<Utc>) -> Vec<Fissure> {
        fissure::active_fissures(&self.raw.active_missions, &self.raw.void_storms, now)
    }

    pub fn baro(&self, now: DateTime<Utc>) -> Option<BaroStatus> {
        void_trader::baro_status(&self.raw.void_traders, now)
    }

    pub fn timers(&self, now: DateTime<Utc>) -> Vec<Timer> {
        cycles::timers(&self.raw.syndicate_missions, now)
    }

    pub fn sortie(&self, now: DateTime<Utc>) -> Option<Sortie> {
        sortie::current_sortie(&self.raw.sorties, now)
    }

    pub fn archon_hunt(&self, now: DateTime<Utc>) -> Option<ArchonHunt> {
        archon::current_archon_hunt(&self.raw.lite_sorties, now)
    }

    pub fn daily_deals(&self, now: DateTime<Utc>) -> Vec<DailyDeal> {
        daily_deal::daily_deals(&self.raw.daily_deals, now)
    }

    pub fn market_sales(&self, now: DateTime<Utc>) -> Vec<MarketSale> {
        flash_sale::market_sales(&self.raw.flash_sales, now)
    }

    pub fn season(
        &self,
        now: DateTime<Utc>,
        act_text: impl Fn(&str) -> Option<NightwaveActText>,
    ) -> Option<NightwaveSeason> {
        self.raw
            .season_info
            .as_ref()
            .map(|raw| season::current_season(raw, now, act_text))
    }

    pub fn varzia(&self, now: DateTime<Utc>) -> Option<Varzia> {
        prime::current_varzia(&self.raw.prime_vault_traders, now)
    }

    pub fn circuit(&self, now: DateTime<Utc>) -> Option<Circuit> {
        circuit::current_circuit(&self.raw.endless_xp_schedule, now)
    }

    pub fn syndicate_mission_sets(&self, now: DateTime<Utc>) -> Vec<SyndicateMissionSet> {
        syndicate::syndicate_mission_sets(&self.raw.syndicate_missions, now)
    }

    pub fn calendar_season(&self, now: DateTime<Utc>) -> Option<CalendarSeason> {
        calendar::current_calendar_season(&self.raw.known_calendar_seasons, now)
    }

    pub fn clan_vault_bonus_week(&self, now: DateTime<Utc>) -> Option<ClanVaultBonusWeek> {
        clan_vault::current_clan_vault_bonus_week(&self.raw.weekly_vault_bonus_rewards, now)
    }

    pub fn archimedea_weeks(&self, now: DateTime<Utc>) -> Vec<ArchimedeaWeek> {
        archimedea::archimedea_weeks(&self.raw.conquests, now)
    }

    pub fn descendia_window(&self, now: DateTime<Utc>) -> Option<ResetWindow> {
        resets::current_descendia_window(&self.raw.descents, now)
    }

    pub fn world_reward_offers(&self, now: DateTime<Utc>) -> Vec<WorldRewardOffer> {
        rewards::world_reward_offers(&self.raw.invasions, &self.raw.alerts, &self.raw.goals, now)
    }

    pub fn prime_access_availability(&self) -> Option<PrimeAccessAvailability> {
        self.raw
            .prime_access_availability
            .as_ref()
            .map(|raw| PrimeAccessAvailability {
                state: raw.state.clone(),
            })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::*;
    use crate::window::fixtures::at;

    const FIXTURE: &str = include_str!("../../../fixtures/worldState.json");
    const FIXTURE_NOW_MS: i64 = 1_788_807_069_000;

    fn now() -> DateTime<Utc> {
        at(FIXTURE_NOW_MS)
    }

    #[test]
    fn fissure_node_names() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let fissures = state.fissures(now());
        assert!(!fissures.is_empty());
        assert!(fissures.iter().all(|fissure| fissure.node_name.is_some()));
    }

    #[test]
    fn fissure_mission_names() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let fissures = state.fissures(now());
        assert!(!fissures.is_empty());
        for fissure in &fissures {
            assert_ne!(fissure.mission_name, "Unknown");
            assert!(!fissure.mission_name.starts_with("MT_"));
        }
    }

    #[test]
    fn void_storms_among_fissures() {
        let state = WorldState::parse(FIXTURE).unwrap();
        assert_eq!(state.raw.void_storms.len(), 12);
        let fissures = state.fissures(now());
        let storms: Vec<_> = fissures.iter().filter(|fissure| fissure.is_storm).collect();
        assert_eq!(fissures.len(), 28);
        assert_eq!(storms.len(), 6);
        assert!(
            storms
                .iter()
                .all(|storm| storm.node_id.starts_with("CrewBattleNode"))
        );
        assert!(storms.iter().all(|storm| storm.node_name.is_some()));
        assert!(storms.iter().all(|storm| !storm.steel_path));
        assert!(storms.iter().any(|storm| storm.mission_name == "Skirmish"));
        assert!(storms.iter().any(|storm| storm.mission_name == "Volatile"));
        assert!(
            storms
                .iter()
                .any(|storm| storm.tier == FissureTier::Relic(RelicTier::Axi))
        );
    }

    #[test]
    fn fissures_sorted_by_tier() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let fissures = state.fissures(now());
        assert!(fissures.is_sorted_by_key(|fissure| &fissure.tier));
    }

    #[test]
    fn baro_status() {
        let state = WorldState::parse(FIXTURE).unwrap();
        assert!(state.baro(now()).is_some());
    }

    #[test]
    fn sortie_missions() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let sortie = state.sortie(now()).unwrap();
        assert_eq!(sortie.id, "6a9edbfe7915bed56981db25");
        assert_eq!(sortie.missions.len(), 3);
        assert_eq!(sortie.boss_name, "Kela De Thaym");
        assert_eq!(sortie.faction, Some("Grineer"));
        assert!(
            sortie
                .missions
                .iter()
                .all(|mission| !mission.modifier_name.starts_with("SORTIE_"))
        );
    }

    #[test]
    fn archon_hunt_missions() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let hunt = state.archon_hunt(now()).unwrap();
        assert_eq!(hunt.id, "6a9dfafed3033a0af5a5d762");
        assert_eq!(hunt.missions.len(), 3);
        assert_eq!(hunt.boss_name, "Archon Amar");
        assert_eq!(hunt.faction, Some("Narmer"));
    }

    #[test]
    fn cetus_timer() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let timers = state.timers(now());
        let cetus = timers.iter().find(|timer| timer.name == "Cetus").unwrap();
        assert!(cetus.state == "day" || cetus.state == "night");
    }

    #[test]
    fn running_daily_deal() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let deals = state.daily_deals(now());
        assert_eq!(deals.len(), 1);
        assert_eq!(
            deals[0].store_item,
            "/Lotus/StoreItems/Types/Items/Research/BioComponent"
        );
        assert_eq!(deals[0].discount_percent, 20);
        assert_eq!(deals[0].original_price, 10);
        assert_eq!(deals[0].sale_price, 8);
        assert_eq!(deals[0].amount_total, 165);
        assert_eq!(deals[0].expiry.timestamp_millis(), 1_788_814_800_000);
    }

    #[test]
    fn expired_daily_deal() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let long_after = at(1_800_000_000_000);
        assert!(state.daily_deals(long_after).is_empty());
    }

    #[test]
    fn nightwave_season() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let poisoner = |challenge: &str| {
            (challenge == "SeasonDailyKillEnemiesWithPoison").then(|| NightwaveActText {
                title: "Poisoner".to_owned(),
                description: "Kill 150 Enemies with Toxin Damage".to_owned(),
                required: 150,
            })
        };
        let season = state.season(now(), poisoner).unwrap();
        assert_eq!(season.season, 18);
        assert_eq!(season.phase, 0);
        assert_eq!(season.name, "Intermission 16");
        assert_eq!(season.expiry.timestamp_millis(), 1_804_464_000_000);
        assert_eq!(season.challenges.len(), 10);
        let dailies = season
            .challenges
            .iter()
            .filter(|challenge| challenge.act.kind == ChallengeKind::Daily)
            .count();
        assert_eq!(dailies, 3);
        let elite = season
            .challenges
            .iter()
            .filter(|challenge| challenge.act.kind == ChallengeKind::EliteWeekly)
            .count();
        assert_eq!(elite, 2);
        let poison = season
            .challenges
            .iter()
            .find(|challenge| challenge.tag.ends_with("SeasonDailyKillEnemiesWithPoison"))
            .unwrap();
        assert_eq!(poison.id, "001900040000000000000055");
        assert_eq!(poison.act.name, "Poisoner");
        assert_eq!(
            poison.act.description.as_deref(),
            Some("Kill 150 Enemies with Toxin Damage")
        );
        assert_eq!(poison.act.required, 150);
        let titled = season
            .challenges
            .iter()
            .filter(|challenge| challenge.act.description.is_some())
            .count();
        assert_eq!(titled, 1);
        for challenge in &season.challenges {
            assert!(!challenge.act.name.contains('/'));
            assert!(!challenge.act.name.starts_with("Season"));
        }
    }

    #[test]
    fn prime_access_parses() {
        let state = WorldState::parse(FIXTURE).unwrap();
        assert_eq!(state.prime_access_availability().unwrap().state, "PRIME1");
    }

    #[test]
    fn varzia_rotation() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let varzia = state.varzia(now()).unwrap();
        assert_eq!(varzia.node_name, Some("Maroo's Bazaar (Mars)"));
        assert_eq!(varzia.expiry.timestamp_millis(), 1_790_877_600_000);
        assert_eq!(varzia.items.len(), 15);
        assert!(varzia.items.iter().any(|item| {
            item.item_type == "/Lotus/StoreItems/Powersuits/Banshee/BansheePrime"
                && item.regal_aya == 3
        }));
    }

    #[test]
    fn circuit_week() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let circuit = state.circuit(now()).unwrap();
        assert_eq!(circuit.rotates.timestamp_millis(), 1_789_344_000_000);
        assert_eq!(circuit.normal.len(), 3);
        assert_eq!(circuit.hard.len(), 5);
        assert!(circuit.normal.contains(&"Garuda".to_owned()));
        assert!(circuit.hard.contains(&"Gammacor".to_owned()));
    }

    #[test]
    fn syndicate_mission_sets() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let sets = state.syndicate_mission_sets(now());

        let tags: Vec<_> = sets.iter().map(|set| set.tag.as_str()).collect();
        assert_eq!(
            tags,
            vec![
                "ArbitersSyndicate",
                "CephalonSudaSyndicate",
                "NewLokaSyndicate",
                "PerrinSyndicate",
                "RedVeilSyndicate",
                "SteelMeridianSyndicate"
            ]
        );
        assert_eq!(sets[0].id, "6a9edf461c3038a4ecd9dc5f");
        assert!(sets.iter().all(|set| set.nodes.len() == 7));
    }

    #[test]
    fn calendar_season() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let season = state.calendar_season(now()).unwrap();

        assert_eq!(season.season, "CST_SUMMER");
        assert_eq!(season.window.expiry.timestamp_millis(), 1_789_344_000_000);
        assert_eq!(season.days.len(), 16);
    }

    #[test]
    fn clan_vault_bonus_week() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let week = state.clan_vault_bonus_week(now()).unwrap();

        assert_eq!(week.week_count, 656);
        assert_eq!(week.rewards.len(), 4);
    }

    #[test]
    fn archimedea_weeks() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let kinds: Vec<_> = state
            .archimedea_weeks(now())
            .iter()
            .map(|week| week.kind)
            .collect();
        assert_eq!(kinds, vec![ArchimedeaKind::Deep, ArchimedeaKind::Temporal]);
    }

    #[test]
    fn descendia_window() {
        let state = WorldState::parse(FIXTURE).unwrap();
        let window = state.descendia_window(now()).unwrap();

        assert_eq!(window.activation.timestamp_millis(), 1_788_739_200_000);
        assert_eq!(window.expiry.timestamp_millis(), 1_789_344_000_000);
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(WorldState::parse("not json").is_err());
    }
}
