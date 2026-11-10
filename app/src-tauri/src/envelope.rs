use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use wf_core::CoreEvent;

use crate::notice::Notice;

static NEXT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct CoreEventEnvelope {
    pub id: String,
    pub event: CoreEvent,
    pub notice: Option<Notice>,
}

impl CoreEventEnvelope {
    pub fn wrap(event: CoreEvent) -> Self {
        Self {
            id: format!(
                "{}-{}",
                kind(&event),
                NEXT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ),
            notice: Notice::for_event(&event),
            event,
        }
    }
}

fn kind(event: &CoreEvent) -> &'static str {
    match event {
        CoreEvent::InventoryUpdated(_) => "inventory-updated",
        CoreEvent::RelicRewardScreen { .. } => "relic-reward-screen",
        CoreEvent::TradeCompleted { .. } => "trade-completed",
        CoreEvent::NewConversation { .. } => "new-conversation",
        CoreEvent::FissureAlert { .. } => "fissure-alert",
        CoreEvent::WorldRewardAlert { .. } => "world-reward-alert",
        CoreEvent::ArbitrationAlert { .. } => "arbitration-alert",
        CoreEvent::ArbitrationRunUpdated { .. } => "arbitration-run-updated",
        CoreEvent::ArbitrationRunRecorded { .. } => "arbitration-run-recorded",
        CoreEvent::TimerAlert { .. } => "timer-alert",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wf_core::{CoreEvent, InventorySummary, RewardScreen};

    fn summary() -> CoreEvent {
        CoreEvent::InventoryUpdated(InventorySummary {
            last_sync_oid: String::from("6a9eeb1f000000000000c001"),
            mr: 36,
            plat: 0,
            credits: 0,
            endo: 0,
            ducats: 0,
            changes: 1,
        })
    }

    #[test]
    fn distinct_ids() {
        let first = CoreEventEnvelope::wrap(summary());
        let second = CoreEventEnvelope::wrap(summary());
        assert_ne!(first.id, second.id);
        assert!(first.id.starts_with("inventory-updated-"));
        assert!(second.id.starts_with("inventory-updated-"));
    }

    #[test]
    fn kind_of_each_variant() {
        assert_eq!(kind(&summary()), "inventory-updated");
        assert_eq!(
            kind(&CoreEvent::NewConversation {
                channel: String::from("F Tenno"),
                player: String::from("Tenno"),
            }),
            "new-conversation"
        );
        assert_eq!(
            kind(&CoreEvent::RelicRewardScreen {
                relic: None,
                screen: RewardScreen {
                    ranked: Vec::new(),
                    account: None,
                },
            }),
            "relic-reward-screen"
        );
    }

    #[test]
    fn nested_event_json() {
        let envelope = CoreEventEnvelope::wrap(summary());
        let json = serde_json::to_value(&envelope).unwrap();
        assert!(json["id"].is_string());
        assert!(json["event"]["InventoryUpdated"].is_object());
        assert_eq!(json["notice"]["title"], "1 inventory changes");
    }
}
