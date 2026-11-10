mod active;
mod boosters;
mod recorder;
mod report;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub(crate) use recorder::{ArbitrationRunRecorder, ArbitrationRunUpdate};
#[cfg(test)]
pub(crate) use recorder::{fixture_lines, fixture_runs};
pub use report::{svg_report, text_report};

pub const VITUS_ESSENCE: &str = "/Lotus/Types/Items/MiscItems/Elitium";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ArbitrationRunStatus {
    Recording,
    Completed,
    Failed,
    Aborted,
    Ended,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ArbitrationRunSource {
    Live,
    Import,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ArbitrationRunRotation {
    pub index: usize,
    pub at_secs: f64,
    pub duration_secs: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ArbitrationRunWave {
    pub index: u32,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ArbitrationRunSample {
    pub secs: f64,
    pub drone_spawns: u32,
    pub live_agents: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ArbitrationRunResourceBoosterKind {
    Amount,
    DropChance,
    DropChanceBlessing,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ArbitrationRunResourceBooster {
    pub kind: ArbitrationRunResourceBoosterKind,
    pub expires_secs: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ArbitrationRun {
    pub id: String,
    pub source: ArbitrationRunSource,
    pub status: ArbitrationRunStatus,
    pub node_id: Option<String>,
    pub node: Option<String>,
    pub planet: Option<String>,
    pub mission_type: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub duration_secs: f64,
    pub host: Option<bool>,
    pub complete_capture: bool,
    pub notes: Vec<String>,
    pub rotations: Vec<ArbitrationRunRotation>,
    pub waves: Vec<ArbitrationRunWave>,
    pub reward_cycles: Option<usize>,
    pub enemy_spawns: Option<u32>,
    pub drone_spawns: Option<u32>,
    pub peak_live_agents: Option<u32>,
    pub boosters: Vec<ArbitrationRunResourceBooster>,
    pub samples: Vec<ArbitrationRunSample>,
    #[serde(default)]
    pub rewards: Option<Vec<ArbitrationRunReward>>,
    #[serde(default)]
    pub collected_vitus: Option<u32>,
    #[serde(default)]
    pub manual_vitus: Option<u32>,
    #[serde(default)]
    pub vitus: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ArbitrationRunReward {
    pub item_type: String,
    pub name: String,
    pub count: u32,
}

pub struct MissionEndReward {
    pub store_item: String,
    pub item_count: u32,
}

impl ArbitrationRun {
    pub(crate) fn rewarded(&self) -> bool {
        self.reward_cycles.is_some_and(|cycles| cycles > 0)
    }

    pub(crate) fn set_manual_vitus(&mut self, vitus: Option<u32>) {
        self.manual_vitus = vitus;
        self.derive_vitus();
    }

    pub(crate) fn attach_mission_end(
        &mut self,
        rewards: Vec<ArbitrationRunReward>,
        collected_vitus: Option<u32>,
    ) {
        self.rewards = Some(rewards);
        self.collected_vitus = collected_vitus;
        self.derive_vitus();
    }

    fn derive_vitus(&mut self) {
        self.vitus = self.manual_vitus.or(self.collected_vitus);
    }

    pub fn same_mission(&self, other: &Self) -> bool {
        if self.id == other.id {
            return true;
        }

        let Some((started, other_started)) = self.started_at.zip(other.started_at) else {
            return false;
        };
        let gap = (other_started - started).as_seconds_f64();
        self.node_id == other.node_id && gap <= self.duration_secs && -gap <= other.duration_secs
    }

    pub fn place(&self) -> String {
        match (&self.node, &self.planet) {
            (Some(node), Some(planet)) => format!("{node}, {planet}"),
            _ => String::from("Unknown node"),
        }
    }
}
