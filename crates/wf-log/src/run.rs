use serde::Deserialize;

use crate::event::{Event, node_and_elite};

const RESOURCE_BLESSING_STORE_ITEM: &str =
    "/Lotus/Types/StoreItems/Boosters/ResourceDropChanceBlessingStoreItem";
const RESOURCE_BLESSING: &str = "/Lotus/Types/Boosters/ResourceDropChanceBlessing";
const DRONE_AGENT: &str = "/Npc/CorpusEliteShieldDroneAgent";

pub fn is_arbitration_drone(agent: &str) -> bool {
    agent.starts_with(DRONE_AGENT)
}

fn leading_number<T: std::str::FromStr>(text: &str) -> Option<T> {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .next()?
        .parse()
        .ok()
}

fn after<'a>(message: &'a str, marker: &str) -> Option<&'a str> {
    message.split_once(marker).map(|(_, rest)| rest)
}

fn first_braced(message: &str) -> Option<&str> {
    let start = message.find('{')?;
    let end = start + message[start..].find('}')?;
    message.get(start..=end)
}

#[derive(Deserialize)]
struct LoadingMissionWire {
    name: String,
}

fn mission_loading(message: &str, host: bool) -> Option<Event> {
    let wire: LoadingMissionWire = serde_json::from_str(first_braced(message)?).ok()?;
    let (node_id, elite) = node_and_elite(&wire.name);
    Some(Event::MissionLoading {
        host,
        node_id,
        elite,
    })
}

fn mission_flow(message: &str) -> Option<Event> {
    if let Some(name) = after(message, "ThemedSquadOverlay.lua: Pending mission: ") {
        let (node_id, elite) = node_and_elite(name.trim());
        return Some(Event::MissionPending { node_id, elite });
    }
    let host_loading = message.contains("ThemedSquadOverlay.lua: Host loading ");
    if host_loading
        || message.contains("Client loaded ")
        || message.contains("Client joining mission in-progress: ")
    {
        return mission_loading(message, host_loading);
    }
    if let Some(side) = message.strip_prefix("**** Starting session on ") {
        return Some(Event::SessionStarted {
            host: side.starts_with("HOST"),
        });
    }
    if let Some(mission_type) = after(message, "OnStateStarted, mission type=") {
        return Some(Event::MissionStateStarted {
            mission_type: mission_type.split_whitespace().next()?.to_owned(),
        });
    }
    None
}

fn mission_end(message: &str) -> Option<Event> {
    if message.ends_with("EndOfMatch.lua: Mission Failed") {
        return Some(Event::MissionFailed);
    }
    if let Some(reason) = message.strip_prefix("TopMenu.lua: Abort: ") {
        return (!reason.starts_with("in hub")).then_some(Event::MissionAborted);
    }
    if message.ends_with("ExtractionTimer.lua: EOM: All players extracting") {
        return Some(Event::PlayersExtracting);
    }
    if let Some(rest) = message.strip_prefix("MISSION_SCORE ") {
        return Some(Event::MissionScore {
            node_id: rest.split_whitespace().next()?.to_owned(),
        });
    }
    if message.starts_with("EOM missionLocationUnlocked=") {
        return Some(Event::MissionLocationUnlocked);
    }
    if let Some(path) = message.strip_prefix("Level=") {
        return Some(Event::LevelLoaded {
            path: path.to_owned(),
        });
    }
    if message.ends_with("EIDOLONMP: Going back to hub") {
        return Some(Event::HubReturned);
    }
    None
}

fn agent_created(message: &str) -> Option<Event> {
    let mut words = message.strip_prefix("OnAgentCreated ")?.split_whitespace();
    let agent = words.next()?;
    let counters: Vec<&str> = words.collect();
    let counter = |name: &str| {
        counters
            .windows(2)
            .find(|pair| pair[0] == name)
            .and_then(|pair| pair[1].parse().ok())
    };
    Some(Event::AgentCreated {
        agent: agent.to_owned(),
        live: counter("Live")?,
        spawned: counter("Spawned")?,
    })
}

fn gameplay(message: &str) -> Option<Event> {
    if message.starts_with("OnAgentCreated ") {
        return agent_created(message);
    }
    if message.ends_with("SurvivalMission.lua: Survival: Starting survival") {
        return Some(Event::SurvivalStarted);
    }
    if let Some(reward) = after(message, "SurvivalMission.lua: Survival: Gave reward tier ") {
        return Some(Event::SurvivalReward {
            elapsed_secs: leading_number(after(reward, " at ")?)?,
        });
    }
    let wave_markers = [
        "WaveDefend.lua: Starting wave ",
        "WaveDefend.lua: Defense wave: ",
        "LoopDefend.lua: Loop Defense wave: ",
    ];
    if let Some(wave) = wave_markers
        .iter()
        .find_map(|marker| after(message, marker))
    {
        return Some(Event::DefenseWave {
            index: leading_number(wave.trim_start())?,
        });
    }
    if message.contains("WaveDefend.lua: _SleepBetweenWaves(") {
        return Some(Event::DefenseWaveCleared);
    }
    let rotation_markers = [
        "Disruption: State change: ARTIFACT_ROUND_DONE",
        "Disruption: Endless mission reward given",
        "ZarimanSurvivalMission.lua: Gave reward tier",
        "Created /Lotus/Interface/DefenseReward.swf",
        "ColonistRescueMission.lua: Defection: Reward ",
        "ExcavationMission.lua: Survival: Gave reward tier ",
    ];
    if rotation_markers
        .iter()
        .any(|marker| message.contains(marker))
    {
        return Some(Event::RotationRewarded);
    }
    None
}

#[derive(Deserialize)]
struct BlessingResult {
    #[serde(rename = "SendTime")]
    send_time: f64,
    #[serde(rename = "Expiry")]
    expiry: f64,
    #[serde(rename = "InventoryChanges")]
    inventory_changes: Option<BlessingBoosters>,
}

#[derive(Deserialize)]
struct BlessingBoosters {
    #[serde(rename = "Boosters", default)]
    boosters: Vec<BlessingBooster>,
}

#[derive(Deserialize)]
struct BlessingBooster {
    #[serde(rename = "ItemType")]
    item_type: String,
}

fn blessing_confirmed(call: &str) -> Option<Event> {
    let (status, body) = call.split_once(" body=")?;
    if !status.ends_with("result=1") {
        return None;
    }
    let result: BlessingResult = serde_json::from_str(body.trim()).ok()?;
    let resource = result.inventory_changes.is_some_and(|changes| {
        changes
            .boosters
            .iter()
            .any(|booster| booster.item_type == RESOURCE_BLESSING)
    });
    Some(Event::HubBlessingConfirmed {
        resource,
        duration_secs: result.expiry - result.send_time,
    })
}

fn blessing(message: &str) -> Option<Event> {
    let call = message.strip_prefix("LotusProfileData::")?;
    let resource = call.contains(RESOURCE_BLESSING_STORE_ITEM);
    if call.starts_with("AddPendingHubBlessing ") {
        return Some(Event::HubBlessingPending { resource });
    }
    if call.starts_with("OnRequestHubBlessings success") {
        return Some(Event::HubBlessingsRequested);
    }
    if call.starts_with("SendHubBlessing ") {
        return Some(Event::HubBlessingSent { resource });
    }
    if call.starts_with("OnSendHubBlessing ") {
        return blessing_confirmed(call);
    }
    None
}

fn clock_anchor(message: &str) -> Option<Event> {
    let utc = after(message, "[UTC:")?.split_once(']')?.0.trim();
    Some(Event::ClockAnchor {
        utc: utc.to_owned(),
    })
}

pub(crate) fn classify(message: &str) -> Option<Event> {
    if message.starts_with("Current time:") {
        return clock_anchor(message);
    }
    mission_flow(message)
        .or_else(|| gameplay(message))
        .or_else(|| mission_end(message))
        .or_else(|| blessing(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mission_flow_lines() {
        assert_eq!(
            classify("ThemedSquadOverlay.lua: Pending mission: ClanNode11_EliteAlert"),
            Some(Event::MissionPending {
                node_id: "ClanNode11".to_owned(),
                elite: true,
            })
        );
        assert_eq!(
            classify("ThemedSquadOverlay.lua: Pending mission: SolNode14_ActiveMission"),
            Some(Event::MissionPending {
                node_id: "SolNode14".to_owned(),
                elite: false,
            })
        );
        assert_eq!(
            classify(
                r#"ThemedSquadOverlay.lua: Host loading {"difficulty":0.60000002384186,"name":"ClanNode11_EliteAlert"} with MissionInfo: "#
            ),
            Some(Event::MissionLoading {
                host: true,
                node_id: "ClanNode11".to_owned(),
                elite: true,
            })
        );
        assert_eq!(
            classify(
                "ThemedSquadOverlay.lua: Host loading {\"name\":\"SolNode177_EliteAlert\"} with MissionInfo: \ninfo={\n    missionType=MT_ARTIFACT\n}"
            ),
            Some(Event::MissionLoading {
                host: true,
                node_id: "SolNode177".to_owned(),
                elite: true,
            })
        );
        assert_eq!(
            classify("**** Starting session on HOST"),
            Some(Event::SessionStarted { host: true })
        );
        assert_eq!(
            classify("OnStateStarted, mission type=MT_SURVIVAL"),
            Some(Event::MissionStateStarted {
                mission_type: "MT_SURVIVAL".to_owned(),
            })
        );
    }

    #[test]
    fn client_lines() {
        assert_eq!(
            classify(
                r#"ThemedSquadOverlay.lua: Client loaded {"difficulty":0.15000000596046,"name":"SolNode450_EliteAlert"} with MissionInfo:"#
            ),
            Some(Event::MissionLoading {
                host: false,
                node_id: "SolNode450".to_owned(),
                elite: true,
            })
        );
        assert_eq!(
            classify(
                r#"ThemedSquadOverlay.lua: LoadLevelMsg received. Client joining mission in-progress: {"difficulty":0.15000000596046,"name":"SolNode450_EliteAlert"}"#
            ),
            Some(Event::MissionLoading {
                host: false,
                node_id: "SolNode450".to_owned(),
                elite: true,
            })
        );
        assert_eq!(
            classify("**** Starting session on CLIENT"),
            Some(Event::SessionStarted { host: false })
        );
    }

    #[test]
    fn clock_line() {
        assert_eq!(
            classify("Current time: Sat Oct 24 18:20:05 2026 [UTC: Sat Oct 24 16:20:05 2026]"),
            Some(Event::ClockAnchor {
                utc: "Sat Oct 24 16:20:05 2026".to_owned(),
            })
        );
    }

    #[test]
    fn agent_lines() {
        assert_eq!(
            classify(
                "OnAgentCreated /Npc/CorpusEliteShieldDroneAgent3 Live 3 Spawned 2 Ticking 3 Paused 0 IgnoredTicking 1 MonitoredTicking 2 AllyLive 1 AllyActive 1 NeutralActive 0"
            ),
            Some(Event::AgentCreated {
                agent: "/Npc/CorpusEliteShieldDroneAgent3".to_owned(),
                live: 3,
                spawned: 2,
            })
        );
        assert_eq!(classify("OnAgentCreated /Npc/MeleeRunner1 Live"), None);
        assert!(is_arbitration_drone("/Npc/CorpusEliteShieldDroneAgent3"));
        assert!(!is_arbitration_drone("/Npc/MeleeRunner1"));
    }

    #[test]
    fn survival_lines() {
        assert_eq!(
            classify("SurvivalMission.lua: Survival: Gave reward tier 1 at 300.494873046875"),
            Some(Event::SurvivalReward {
                elapsed_secs: 300.494_873_046_875,
            })
        );
        assert_eq!(
            classify("SurvivalMission.lua: Survival: Starting survival"),
            Some(Event::SurvivalStarted)
        );
        assert_eq!(
            classify("EndOfMatch.lua: RewardUtilities: Gave reward tier 0"),
            None
        );
    }

    #[test]
    fn defense_lines() {
        assert_eq!(
            classify(
                "WaveDefend.lua: Starting wave 3, spawning a total of 40 tier-2 enemies (12 simultaneous, 3 waves)"
            ),
            Some(Event::DefenseWave { index: 3 })
        );
        assert_eq!(
            classify("WaveDefend.lua: Defense wave: 4"),
            Some(Event::DefenseWave { index: 4 })
        );
        assert_eq!(
            classify("LoopDefend.lua: Loop Defense wave: 1"),
            Some(Event::DefenseWave { index: 1 })
        );
        assert_eq!(
            classify("WaveDefend.lua: _SleepBetweenWaves(12)"),
            Some(Event::DefenseWaveCleared)
        );
    }

    #[test]
    fn rotation_lines() {
        for message in [
            "Disruption: State change: ARTIFACT_ROUND_DONE",
            "Disruption: Endless mission reward given",
            "ZarimanSurvivalMission.lua: Gave reward tier 2",
            "Created /Lotus/Interface/DefenseReward.swf",
            "ColonistRescueMission.lua: Defection: Reward 1 given",
            "ExcavationMission.lua: Survival: Gave reward tier 2",
        ] {
            assert_eq!(
                classify(message),
                Some(Event::RotationRewarded),
                "{message}"
            );
        }
    }

    #[test]
    fn mission_end_lines() {
        assert_eq!(
            classify("EndOfMatch.lua: Mission Failed"),
            Some(Event::MissionFailed)
        );
        assert_eq!(
            classify("TopMenu.lua: Abort: client/session/PVP"),
            Some(Event::MissionAborted)
        );
        assert_eq!(classify("TopMenu.lua: Abort: in hub (1)"), None);
        assert_eq!(
            classify("ExtractionTimer.lua: EOM: All players extracting"),
            Some(Event::PlayersExtracting)
        );
        assert_eq!(
            classify("MISSION_SCORE ClanNode11 1 2"),
            Some(Event::MissionScore {
                node_id: "ClanNode11".to_owned(),
            })
        );
        assert_eq!(
            classify("EIDOLONMP: Going back to hub"),
            Some(Event::HubReturned)
        );
    }

    #[test]
    fn blessing_lines() {
        assert_eq!(
            classify(
                "LotusProfileData::AddPendingHubBlessing /Lotus/Types/StoreItems/Boosters/AffinityBlessingStoreItem"
            ),
            Some(Event::HubBlessingPending { resource: false })
        );
        assert_eq!(
            classify(
                "LotusProfileData::AddPendingHubBlessing /Lotus/Types/StoreItems/Boosters/ResourceDropChanceBlessingStoreItem"
            ),
            Some(Event::HubBlessingPending { resource: true })
        );
        assert_eq!(
            classify("LotusProfileData::OnRequestHubBlessings success"),
            Some(Event::HubBlessingsRequested)
        );
        assert_eq!(
            classify(
                "LotusProfileData::SendHubBlessing /Lotus/Types/StoreItems/Boosters/AffinityBlessingStoreItem"
            ),
            Some(Event::HubBlessingSent { resource: false })
        );
        assert_eq!(
            classify(
                "LotusProfileData::SendHubBlessing /Lotus/Types/StoreItems/Boosters/ResourceDropChanceBlessingStoreItem"
            ),
            Some(Event::HubBlessingSent { resource: true })
        );
        assert_eq!(
            classify(
                r#"LotusProfileData::OnSendHubBlessing result=1 body={"SendTime":1761000000,"Expiry":1761010800,"InventoryChanges":{"Boosters":[{"ItemType":"/Lotus/Types/Boosters/ResourceDropChanceBlessing"}]}}"#
            ),
            Some(Event::HubBlessingConfirmed {
                resource: true,
                duration_secs: 10_800.0,
            })
        );
        assert_eq!(
            classify("LotusProfileData::OnSendHubBlessing result=0 body={}"),
            None
        );
    }
}
