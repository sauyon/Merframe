use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::Serialize;
use wf_mongo::ObjectId;

use crate::mission_types::mission_type_name;
use crate::nodes::node_name;
use crate::sortie::{boss, boss_name};
use crate::window::Window;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiteSortieMission {
    pub mission_type: String,
    pub node: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LiteSortie {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    #[serde(flatten)]
    pub window: Window,
    pub boss: String,
    pub missions: Vec<LiteSortieMission>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ArchonMission {
    pub mission_type: String,
    pub mission_name: String,
    pub node_id: String,
    pub node_name: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ArchonHunt {
    pub id: String,
    pub boss: String,
    pub boss_name: String,
    pub faction: Option<&'static str>,
    pub activation: DateTime<Utc>,
    pub expiry: DateTime<Utc>,
    pub missions: Vec<ArchonMission>,
}

pub fn current_archon_hunt(sorties: &[LiteSortie], now: DateTime<Utc>) -> Option<ArchonHunt> {
    sorties
        .iter()
        .find(|sortie| sortie.window.contains(now))
        .map(|sortie| ArchonHunt {
            id: sortie.id.as_str().to_owned(),
            boss: sortie.boss.clone(),
            boss_name: boss_name(&sortie.boss),
            faction: boss(&sortie.boss).map(|boss| boss.faction),
            activation: sortie.window.activation.datetime(),
            expiry: sortie.window.expiry.datetime(),
            missions: sortie
                .missions
                .iter()
                .map(|mission| ArchonMission {
                    mission_type: mission.mission_type.clone(),
                    mission_name: mission_type_name(&mission.mission_type),
                    node_id: mission.node.clone(),
                    node_name: node_name(&mission.node),
                })
                .collect(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::fixtures::{at, window};

    #[test]
    fn active_hunt() {
        let now = at(1500);
        let sorties = vec![LiteSortie {
            id: ObjectId("6a9cba7e2d3c4b5a6f7e8d9c".to_owned()),
            window: window(1000, 2000),
            boss: "SORTIE_BOSS_AMAR".to_owned(),
            missions: vec![
                LiteSortieMission {
                    mission_type: "MT_RESCUE".to_owned(),
                    node: "SolNode1".to_owned(),
                },
                LiteSortieMission {
                    mission_type: "MT_TERRITORY".to_owned(),
                    node: "SolNode1".to_owned(),
                },
                LiteSortieMission {
                    mission_type: "MT_ASSASSINATION".to_owned(),
                    node: "SolNode1".to_owned(),
                },
            ],
        }];
        let hunt = current_archon_hunt(&sorties, now).unwrap();
        assert_eq!(hunt.missions.len(), 3);
        assert_eq!(hunt.id, "6a9cba7e2d3c4b5a6f7e8d9c");
        assert_eq!(hunt.boss, "SORTIE_BOSS_AMAR");
        assert_eq!(hunt.boss_name, "Archon Amar");
        assert_eq!(hunt.faction, Some("Narmer"));
        assert_eq!(hunt.missions[0].mission_name, "Rescue");
        assert_eq!(hunt.missions[1].mission_name, "Interception");
        assert_eq!(hunt.missions[2].mission_name, "Assassination");
    }
}
