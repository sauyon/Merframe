mod alerts;
mod arbitrations;
mod engine;
mod rewards;

use chrono::{DateTime, Utc};
use serde::Serialize;

pub use alerts::{
    AlertSettings, CyclePhase, FissureFilter, FissureSubtype, TimerAlerts, tier_name,
};
pub(crate) use arbitrations::ArbitrationOutlook;
pub use arbitrations::{
    ARBITRATION_FORECAST_TRUSTED, ArbitrationAlert, ArbitrationAlertFilter,
    ArbitrationAlertSettings, ArbitrationSchedule, ArbitrationSlot,
};
pub(crate) use engine::{Engine, inventory_events};
pub(crate) use rewards::world_reward_alerts;
pub use rewards::{WorldRewardAlert, WorldRewardAlertChoices, WorldRewardAlertGroup};

use crate::arbitration_runs::ArbitrationRun;
use crate::relic_planner::RewardScreen;
use crate::trade::Trade;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FissureInfo {
    pub node_id: String,
    pub node_name: Option<&'static str>,
    pub mission_type: String,
    pub mission_name: String,
    pub planet: Option<&'static str>,
    pub tier: String,
    pub steel_path: bool,
    pub is_storm: bool,
    pub faction: Option<&'static str>,
    pub levels: Option<(u32, u32)>,
    pub expiry: DateTime<Utc>,
    pub remaining_secs: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct InventorySummary {
    pub last_sync_oid: String,
    pub mr: u32,
    pub plat: i64,
    pub credits: i64,
    pub endo: i64,
    pub ducats: i64,
    pub changes: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScannedRewards {
    pub own_relic_type: Option<String>,
    pub rewards: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedTradeItem {
    pub name: String,
    pub item_type: Option<String>,
    pub count: i64,
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScannedTrade {
    pub partner: Option<String>,
    pub offered: Vec<ScannedTradeItem>,
    pub received: Vec<ScannedTradeItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub enum CoreEvent {
    InventoryUpdated(InventorySummary),
    RelicRewardScreen {
        relic: Option<String>,
        screen: RewardScreen,
    },
    TradeCompleted {
        at: DateTime<Utc>,
        partner: Option<String>,
        trade: Trade,
    },
    NewConversation {
        channel: String,
        player: String,
    },
    FissureAlert {
        fissure: FissureInfo,
    },
    WorldRewardAlert {
        reward: WorldRewardAlert,
    },
    ArbitrationAlert {
        alert: ArbitrationAlert,
    },
    ArbitrationRunUpdated {
        run: Box<ArbitrationRun>,
    },
    ArbitrationRunRecorded {
        run: Box<ArbitrationRun>,
    },
    TimerAlert {
        name: String,
        next_state: String,
        ends_at: DateTime<Utc>,
        remaining_secs: i64,
    },
}

impl CoreEvent {
    pub fn label(&self) -> &'static str {
        match self {
            Self::InventoryUpdated(_) => "InventoryUpdated",
            Self::RelicRewardScreen { .. } => "RelicRewardScreen",
            Self::TradeCompleted { .. } => "TradeCompleted",
            Self::NewConversation { .. } => "NewConversation",
            Self::FissureAlert { .. } => "FissureAlert",
            Self::WorldRewardAlert { .. } => "WorldRewardAlert",
            Self::ArbitrationAlert { .. } => "ArbitrationAlert",
            Self::ArbitrationRunUpdated { .. } => "ArbitrationRunUpdated",
            Self::ArbitrationRunRecorded { .. } => "ArbitrationRunRecorded",
            Self::TimerAlert { .. } => "TimerAlert",
        }
    }
}

#[cfg(test)]
pub(super) mod support {
    use wf_worldstate::{Fissure, FissureTier, RelicTier};

    use crate::catalog::fixtures::at;

    pub(super) fn fissure(
        tier: RelicTier,
        mission_type: &str,
        node_name: Option<&'static str>,
        steel_path: bool,
    ) -> Fissure {
        Fissure {
            node_id: "SolNode58".to_owned(),
            node_name,
            mission_type: mission_type.to_owned(),
            mission_name: "Extermination".to_owned(),
            tier: FissureTier::Relic(tier),
            steel_path,
            is_storm: false,
            activation: at(0),
            expiry: at(1_000_000),
        }
    }

    pub(super) fn sample() -> Fissure {
        fissure(
            RelicTier::Lith,
            "MT_EXTERMINATION",
            Some("Galatea (Neptune)"),
            false,
        )
    }
}
