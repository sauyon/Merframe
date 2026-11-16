use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use wf_mongo::{MongoDate, ObjectId};

use crate::model::{EquipmentItem, Inventory};

const STEEL_PATH_TIER: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PeriodicMissionCompletion {
    date: MongoDate,
    tag: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct VendorPurchaseHistory {
    pub vendor_type: String,
    purchase_history: Vec<VendorOfferPurchase>,
}

impl VendorPurchaseHistory {
    pub fn active_purchases(
        &self,
        now: DateTime<Utc>,
    ) -> impl Iterator<Item = &VendorOfferPurchase> {
        self.purchase_history
            .iter()
            .filter(move |purchase| unexpired(purchase.expiry, now))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct VendorOfferPurchase {
    pub item_id: String,
    pub num_purchased: u32,
    pub expiry: MongoDate,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct SortieRewardClaim {
    sortie_id: ObjectId,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CircuitProgress {
    pub category: String,
    #[serde(default)]
    pub choices: Vec<String>,
    earn: u32,
    #[serde(default)]
    pending_rewards: Vec<CircuitRewardTier>,
    expiry: MongoDate,
}

impl CircuitProgress {
    pub fn reward_tiers(&self) -> &[CircuitRewardTier] {
        &self.pending_rewards
    }

    pub fn reward_tiers_earned(&self) -> impl Iterator<Item = &CircuitRewardTier> {
        self.pending_rewards
            .iter()
            .filter(|tier| tier.required_total_xp <= self.earn)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CircuitRewardTier {
    required_total_xp: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DescendiaProgress {
    pub category: String,
    pub floor_claimed: u32,
    expiry: MongoDate,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IcebindRewardChoices {
    expiry: MongoDate,
    choices: Vec<IcebindRewardChoice>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IcebindRewardChoice {
    #[serde(default)]
    claimed: bool,
}

impl IcebindRewardChoices {
    pub fn claimed_choices(
        &self,
        now: DateTime<Utc>,
    ) -> impl Iterator<Item = &IcebindRewardChoice> {
        let current = unexpired(self.expiry, now);
        self.choices
            .iter()
            .filter(move |choice| current && choice.claimed)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ClanVaultBonusProgress {
    pub week_count: i64,
    pub max_reward_claimed: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SyndicateWeeklyMission {
    pub week_count: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CalendarProgress {
    pub season_progress: CalendarSeasonProgress,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CalendarSeasonProgress {
    pub season_type: String,
    last_completed_day_idx: i32,
}

impl CalendarSeasonProgress {
    pub fn last_completed_day(&self) -> Option<u32> {
        u32::try_from(self.last_completed_day_idx).ok()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GivenNightwaveAct {
    pub challenge: String,
    pub id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NightwaveWeek {
    pub season: u16,
    pub week: u16,
}

impl NightwaveWeek {
    pub fn of_act(id: &str) -> Option<Self> {
        Some(Self {
            season: u16::from_str_radix(id.get(..4)?, 16).ok()?,
            week: u16::from_str_radix(id.get(4..8)?, 16).ok()?,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct QuestKey {
    item_type: String,
    #[serde(default)]
    completed: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct DialogueHistory {
    #[serde(default)]
    dialogues: Vec<DialogueRecord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DialogueRecord {
    #[serde(default)]
    booleans: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DailyStandingPool {
    Factions,
    Cetus,
    Quills,
    Solaris,
    Ventkids,
    Vox,
    Entrati,
    Necraloid,
    Zariman,
    Kahl,
    Cavia,
    Hex,
    Library,
    Pvp,
}

impl EquipmentItem {
    pub fn helminth_invigoration_expiry(&self, now: DateTime<Utc>) -> Option<MongoDate> {
        self.upgrades_expiry
            .filter(|expiry| unexpired(*expiry, now))
    }
}

fn unexpired(expiry: MongoDate, now: DateTime<Utc>) -> bool {
    expiry.datetime() > now
}

fn claimed_sortie_id(claims: &[SortieRewardClaim]) -> Option<&str> {
    claims.first().map(|claim| claim.sortie_id.as_str())
}

fn nodes_completed_under<'a>(entries: &'a [String], id: &'a str) -> impl Iterator<Item = &'a str> {
    entries.iter().filter_map(move |entry| {
        entry
            .strip_suffix(id)
            .and_then(|node| node.strip_suffix('_'))
    })
}

impl Inventory {
    pub fn daily_standing_remaining(&self) -> impl Iterator<Item = (DailyStandingPool, u32)> {
        [
            (DailyStandingPool::Factions, self.daily_affiliation),
            (DailyStandingPool::Cetus, self.daily_affiliation_cetus),
            (DailyStandingPool::Quills, self.daily_affiliation_quills),
            (DailyStandingPool::Solaris, self.daily_affiliation_solaris),
            (DailyStandingPool::Ventkids, self.daily_affiliation_ventkids),
            (DailyStandingPool::Vox, self.daily_affiliation_vox),
            (DailyStandingPool::Entrati, self.daily_affiliation_entrati),
            (
                DailyStandingPool::Necraloid,
                self.daily_affiliation_necraloid,
            ),
            (DailyStandingPool::Zariman, self.daily_affiliation_zariman),
            (DailyStandingPool::Kahl, self.daily_affiliation_kahl),
            (DailyStandingPool::Cavia, self.daily_affiliation_cavia),
            (DailyStandingPool::Hex, self.daily_affiliation_hex),
            (DailyStandingPool::Library, self.daily_affiliation_library),
            (DailyStandingPool::Pvp, self.daily_affiliation_pvp),
        ]
        .into_iter()
        .filter_map(|(pool, remaining)| Some((pool, remaining?)))
    }

    pub fn periodic_missions_since(&self, since: DateTime<Utc>) -> impl Iterator<Item = &str> {
        self.periodic_mission_completions
            .iter()
            .filter(move |completion| completion.date.datetime() >= since)
            .map(|completion| completion.tag.as_str())
    }

    pub fn search_pulses_spent(&self, now: DateTime<Utc>) -> u32 {
        self.entrati_vault_count_reset_date
            .filter(|reset| unexpired(*reset, now))
            .and(self.entrati_vault_count_last_period)
            .unwrap_or_default()
    }

    pub fn circuit_progress(&self, now: DateTime<Utc>) -> impl Iterator<Item = &CircuitProgress> {
        self.endless_xp
            .iter()
            .filter(move |circuit| unexpired(circuit.expiry, now))
    }

    pub fn descendia_progress(
        &self,
        now: DateTime<Utc>,
    ) -> impl Iterator<Item = &DescendiaProgress> {
        self.descent_rewards
            .iter()
            .filter(move |descent| unexpired(descent.expiry, now))
    }

    pub fn last_sortie_id(&self) -> Option<&str> {
        claimed_sortie_id(&self.last_sortie_reward)
    }

    pub fn last_archon_hunt_id(&self) -> Option<&str> {
        claimed_sortie_id(&self.last_lite_sortie_reward)
    }

    pub fn sortie_nodes_completed<'a>(
        &'a self,
        sortie_id: &'a str,
    ) -> impl Iterator<Item = &'a str> {
        nodes_completed_under(&self.completed_sorties, sortie_id)
    }

    pub fn syndicate_nodes_completed<'a>(
        &'a self,
        mission_id: &'a str,
    ) -> impl Iterator<Item = &'a str> {
        nodes_completed_under(&self.completed_syndicates, mission_id)
    }

    pub fn challenge_progress_of(&self, name: &str) -> u32 {
        self.challenge_progress
            .iter()
            .find(|challenge| challenge.name == name)
            .and_then(|challenge| u32::try_from(challenge.progress).ok())
            .unwrap_or_default()
    }

    pub fn nightwave_acts_given_before(
        &self,
        current: NightwaveWeek,
    ) -> impl Iterator<Item = &GivenNightwaveAct> {
        self.season_challenge_history.iter().filter(move |act| {
            NightwaveWeek::of_act(&act.id)
                .is_some_and(|given| given.season == current.season && given.week < current.week)
        })
    }

    pub fn cryobells_held(&self) -> usize {
        self.kuva_keys.len()
    }

    pub fn steel_path_mission_completed(&self) -> bool {
        self.missions
            .iter()
            .any(|mission| mission.tier == Some(STEEL_PATH_TIER))
    }

    pub fn elite_archimedea_unlocked(&self) -> bool {
        self.entrati_lab_conquest_hard_mode_status != 0
            || self.echoes_hex_conquest_hard_mode_status != 0
    }

    pub fn dialogue_flag_set(&self, flag: &str) -> bool {
        self.dialogue_history
            .dialogues
            .iter()
            .flat_map(|dialogue| &dialogue.booleans)
            .any(|set| set == flag)
    }

    pub fn completed_quests(&self) -> HashSet<&str> {
        self.quest_keys
            .iter()
            .filter(|quest| quest.completed)
            .map(|quest| quest.item_type.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::{DailyStandingPool, NightwaveWeek};
    use crate::model::Inventory;
    use crate::views::tests::{MINIMAL, fixture};

    const SORTIE: &str = "6a9cd300000000000000e101";
    const ARCHON_HUNT: &str = "6a9b8180000000000000e102";
    const SYNDICATE_MISSIONS: &str = "6a9e4c00000000000000e104";

    fn utc(text: &str) -> DateTime<Utc> {
        text.parse().unwrap()
    }

    fn synced() -> DateTime<Utc> {
        utc("2026-09-07T16:49:35Z")
    }

    #[test]
    fn daily_standing_remaining() {
        let pools: Vec<_> = fixture().daily_standing_remaining().collect();

        assert_eq!(pools.len(), 14);
        assert_eq!(pools[0], (DailyStandingPool::Factions, 21_000));
        assert!(pools.contains(&(DailyStandingPool::Entrati, 18_500)));
        assert!(pools.contains(&(DailyStandingPool::Library, 0)));
    }

    #[test]
    fn periodic_missions_since() {
        let inv = fixture();

        let this_week: Vec<_> = inv
            .periodic_missions_since(utc("2026-09-07T00:00:00Z"))
            .collect();

        assert_eq!(
            this_week,
            [
                "GetClem",
                "TreasureHuntA",
                "HardDaily1",
                "HardDaily4",
                "EliteAlert"
            ]
        );
        assert_eq!(
            inv.periodic_missions_since(utc("2026-08-31T00:00:00Z"))
                .count(),
            7
        );
    }

    #[test]
    fn active_vendor_purchases() {
        let inv = fixture();

        let teshin = &inv.recent_vendor_purchases[0];
        let active: Vec<_> = teshin.active_purchases(synced()).collect();

        assert_eq!(inv.recent_vendor_purchases.len(), 3);
        assert!(
            teshin
                .vendor_type
                .ends_with("/TeshinHardModeVendorManifest")
        );
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].item_id, "6a9a2f60000000000000f201");
        assert_eq!(active[0].num_purchased, 2);
        assert_eq!(
            teshin.active_purchases(utc("2026-09-14T00:00:00Z")).count(),
            0
        );
    }

    #[test]
    fn search_pulses_spent() {
        let inv = fixture();

        assert_eq!(inv.search_pulses_spent(synced()), 3);
        assert_eq!(inv.search_pulses_spent(utc("2026-09-14T00:00:00Z")), 0);
    }

    #[test]
    fn sortie_and_archon_hunt() {
        let inv = fixture();

        assert_eq!(inv.last_sortie_id(), Some(SORTIE));
        assert_eq!(inv.last_archon_hunt_id(), Some(ARCHON_HUNT));
        assert_eq!(inv.sortie_nodes_completed(SORTIE).count(), 3);
        assert_eq!(inv.sortie_nodes_completed(ARCHON_HUNT).count(), 2);
        assert_eq!(
            inv.sortie_nodes_completed("6a9cd300000000000000ffff")
                .count(),
            0
        );
    }

    #[test]
    fn syndicate_nodes_completed() {
        let inv = fixture();

        let nodes: Vec<_> = inv.syndicate_nodes_completed(SYNDICATE_MISSIONS).collect();

        assert_eq!(nodes, ["SolNode42", "SolNode12"]);
    }

    #[test]
    fn circuit_choices() {
        let inv = fixture();

        let current: Vec<_> = inv.circuit_progress(synced()).collect();
        let [hard] = &current[..] else { panic!() };

        assert_eq!(inv.endless_xp.len(), 2);
        assert_eq!(hard.category, "EXC_HARD");
        assert_eq!(hard.choices, ["Akstiletto", "Kronen"]);
        assert_eq!(hard.reward_tiers_earned().count(), 5);
        assert_eq!(hard.reward_tiers().len(), 10);
    }

    #[test]
    fn descendia_floors() {
        let inv = fixture();

        let floors: Vec<_> = inv
            .descendia_progress(synced())
            .map(|descent| (descent.category.as_str(), descent.floor_claimed))
            .collect();

        assert_eq!(inv.descent_rewards.len(), 2);
        assert_eq!(floors, [("DM_COH_NORMAL", 7)]);
    }

    #[test]
    fn icebind_rewards() {
        let inv = fixture();

        let rewards = inv.kuva_keys_rewards.as_ref().unwrap();

        assert_eq!(inv.cryobells_held(), 2);
        assert_eq!(rewards.claimed_choices(synced()).count(), 2);
        assert_eq!(
            rewards.claimed_choices(utc("2026-09-14T00:00:00Z")).count(),
            0
        );
    }

    #[test]
    fn kahl_weekly_missions() {
        let inv = fixture();

        let kahl = inv
            .affiliations
            .iter()
            .find(|affiliation| affiliation.tag == "KahlSyndicate")
            .unwrap();
        let weeks: Vec<_> = kahl
            .weekly_missions
            .iter()
            .map(|mission| mission.week_count)
            .collect();

        assert_eq!(weeks, [605, 606]);
    }

    #[test]
    fn clan_vault_bonus() {
        let inv = fixture();

        let claimed: Vec<_> = inv
            .weekly_guild_vault_bonus_progress
            .iter()
            .map(|week| (week.week_count, week.max_reward_claimed))
            .collect();

        assert_eq!(claimed, [(652, None), (653, Some(2))]);
    }

    #[test]
    fn calendar_days() {
        let inv = fixture();

        let season = &inv.calendar_progress.as_ref().unwrap().season_progress;

        assert_eq!(season.season_type, "CST_SUMMER");
        assert_eq!(season.last_completed_day(), Some(5));
    }

    #[test]
    fn helminth_invigoration() {
        let inv = fixture();

        let invigorated: Vec<_> = inv
            .suits
            .iter()
            .filter_map(|suit| {
                Some((
                    suit.item_type.as_str(),
                    suit.helminth_invigoration_expiry(synced())?,
                ))
            })
            .collect();
        let [(suit, expiry)] = &invigorated[..] else {
            panic!()
        };

        assert_eq!(*suit, "/Lotus/Powersuits/YinYang/EquinoxPrime");
        assert_eq!(expiry.datetime(), utc("2026-09-13T10:12:00Z"));
        assert!(inv.suits.iter().all(|suit| {
            suit.helminth_invigoration_expiry(utc("2026-09-13T10:12:00Z"))
                .is_none()
        }));
    }

    #[test]
    fn challenge_progress_of() {
        let inv = fixture();

        assert_eq!(
            inv.challenge_progress_of("SeasonWeeklyCompleteDisruptionConduits"),
            52
        );
        assert_eq!(inv.challenge_progress_of("SeasonWeeklyCompleteSpy"), 0);
        assert_eq!(inv.challenge_progress_of("SeasonDailyBulletJump"), 0);
    }

    #[test]
    fn nightwave_weeks() {
        assert_eq!(
            NightwaveWeek::of_act("001900140000000000000187"),
            Some(NightwaveWeek {
                season: 0x19,
                week: 0x14
            })
        );
        assert_eq!(NightwaveWeek::of_act("0019"), None);
        assert_eq!(NightwaveWeek::of_act("zz1900140000000000000187"), None);
    }

    #[test]
    fn nightwave_acts_given_before() {
        let inv = Inventory::parse(&MINIMAL.replacen(
            '{',
            r#"{"SeasonChallengeHistory":[
                {"challenge":"SeasonWeeklyEarlier","id":"001900130000000000000180"},
                {"challenge":"SeasonWeeklyCurrent","id":"001900140000000000000187"},
                {"challenge":"SeasonWeeklyOtherSeason","id":"001800130000000000000090"}
            ],"#,
            1,
        ))
        .unwrap();

        let given: Vec<_> = inv
            .nightwave_acts_given_before(NightwaveWeek {
                season: 0x19,
                week: 0x14,
            })
            .map(|act| act.challenge.as_str())
            .collect();

        assert_eq!(given, ["SeasonWeeklyEarlier"]);
    }

    #[test]
    fn elite_archimedea_unlocked() {
        let with_status = |deep: u32, temporal: u32| {
            Inventory::parse(&MINIMAL.replacen(
                '{',
                &format!(
                    r#"{{"EntratiLabConquestHardModeStatus":{deep},"EchoesHexConquestHardModeStatus":{temporal},"#
                ),
                1,
            ))
            .unwrap()
        };

        assert!(
            !Inventory::parse(MINIMAL)
                .unwrap()
                .elite_archimedea_unlocked()
        );
        assert!(!with_status(0, 0).elite_archimedea_unlocked());
        assert!(with_status(1, 0).elite_archimedea_unlocked());
        assert!(with_status(0, 1).elite_archimedea_unlocked());
    }

    #[test]
    fn dialogue_flags() {
        let inv = fixture();

        assert!(inv.dialogue_flag_set("FFFirstInteractionDone"));
        assert!(!inv.dialogue_flag_set("FF1AdventureComplete"));
        assert!(
            !Inventory::parse(MINIMAL)
                .unwrap()
                .dialogue_flag_set("FFFirstInteractionDone")
        );
    }

    #[test]
    fn steel_path_mission() {
        let mut inv = fixture();

        assert!(inv.steel_path_mission_completed());

        for mission in &mut inv.missions {
            mission.tier = None;
        }

        assert!(!inv.steel_path_mission_completed());
    }

    #[test]
    fn completed_quests() {
        let inv = fixture();

        let quests = inv.completed_quests();

        assert_eq!(quests.len(), 2);
        assert!(quests.contains("/Lotus/Types/Keys/KahlQuest/KahlQuestKeyChain"));
    }

    #[test]
    fn daily_scalars() {
        let inv = fixture();

        assert_eq!(inv.daily_focus, Some(320_000));
        assert_eq!(inv.gifts_remaining, Some(24));
        assert_eq!(inv.entrati_lab_conquest_cache_score_mission, 20);
        assert_eq!(inv.echoes_hex_conquest_cache_score_mission, 0);
        assert_eq!(inv.season_challenge_history.len(), 2);
        assert_eq!(inv.periodic_mission_completions.len(), 7);
    }

    #[test]
    fn missing_reset_fields() {
        let inv = Inventory::parse(MINIMAL).unwrap();

        assert_eq!(inv.daily_standing_remaining().count(), 0);
        assert_eq!(inv.search_pulses_spent(synced()), 0);
        assert_eq!(inv.last_sortie_id(), None);
        assert_eq!(inv.sortie_nodes_completed(SORTIE).count(), 0);
        assert!(inv.completed_quests().is_empty());
        assert!(inv.kuva_keys_rewards.is_none());
        assert!(inv.calendar_progress.is_none());
    }
}
