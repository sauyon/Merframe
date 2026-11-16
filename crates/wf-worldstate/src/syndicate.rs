use chrono::{DateTime, Utc};
use serde::Deserialize;
use wf_mongo::ObjectId;

use crate::window::Window;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SyndicateMission {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    #[serde(flatten)]
    pub window: Window,
    pub tag: String,
    #[serde(default)]
    pub nodes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyndicateMissionSet {
    pub tag: String,
    pub id: String,
    pub nodes: Vec<String>,
}

pub fn expiry_for_tag(missions: &[SyndicateMission], tag: &str) -> Option<i64> {
    missions
        .iter()
        .find(|mission| mission.tag == tag)
        .map(|mission| mission.window.expiry.timestamp_millis())
}

pub fn syndicate_mission_sets(
    missions: &[SyndicateMission],
    now: DateTime<Utc>,
) -> Vec<SyndicateMissionSet> {
    missions
        .iter()
        .filter(|mission| !mission.nodes.is_empty() && mission.window.contains(now))
        .map(|mission| SyndicateMissionSet {
            tag: mission.tag.clone(),
            id: mission.id.as_str().to_owned(),
            nodes: mission.nodes.clone(),
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod fixtures {
    use wf_mongo::ObjectId;

    use super::SyndicateMission;
    use crate::window::fixtures::window;

    pub(crate) fn syndicate_mission(
        tag: &str,
        activation_ms: i64,
        expiry_ms: i64,
        nodes: &[&str],
    ) -> SyndicateMission {
        SyndicateMission {
            id: ObjectId("6a9edf461c3038a4ecd9dc5f".to_owned()),
            window: window(activation_ms, expiry_ms),
            tag: tag.to_owned(),
            nodes: nodes.iter().map(|&node| node.to_owned()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::syndicate_mission;
    use super::*;
    use crate::window::fixtures::at;

    #[test]
    fn mission_sets_with_nodes() {
        let missions = vec![
            syndicate_mission("SteelMeridianSyndicate", 1000, 2000, &["SolNode59"]),
            syndicate_mission("CetusSyndicate", 1000, 2000, &[]),
            syndicate_mission("PerrinSyndicate", 3000, 4000, &["SolNode12"]),
        ];
        let sets = syndicate_mission_sets(&missions, at(1500));

        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].tag, "SteelMeridianSyndicate");
        assert_eq!(sets[0].id, "6a9edf461c3038a4ecd9dc5f");
        assert_eq!(sets[0].nodes, ["SolNode59"]);
    }
}
