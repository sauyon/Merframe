use std::fmt::Write as _;

use serde::Serialize;
use wf_core::{
    ArbitrationAlert, ArbitrationRun, CoreEvent, FissureInfo, InventorySummary, WorldRewardAlert,
    thousands,
};
use wf_worldstate::WorldRewardSource;

use crate::settings::InGameToast;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Success,
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct Notice {
    pub title: String,
    pub body: String,
    pub tone: Tone,
    pub in_game: Option<InGameToast>,
}

impl Notice {
    pub fn for_event(event: &CoreEvent) -> Option<Self> {
        match event {
            CoreEvent::InventoryUpdated(summary) => Self::inventory(summary),
            CoreEvent::RelicRewardScreen { .. } | CoreEvent::ArbitrationRunUpdated { .. } => None,
            CoreEvent::TradeCompleted { trade, partner, .. } => {
                Some(Self::trade(trade.plat, partner.as_deref()))
            }
            CoreEvent::NewConversation { player, .. } => Some(Self::conversation(player)),
            CoreEvent::FissureAlert { fissure } => Some(Self::fissure(fissure)),
            CoreEvent::WorldRewardAlert { reward } => Some(Self::reward(reward)),
            CoreEvent::ArbitrationAlert { alert } => Some(Self::arbitration(alert)),
            CoreEvent::ArbitrationRunRecorded { run } => Some(Self::arbitration_run(run)),
            CoreEvent::TimerAlert {
                name,
                next_state,
                remaining_secs,
                ..
            } => Some(Self::timer(name, next_state, *remaining_secs)),
        }
    }

    fn inventory(summary: &InventorySummary) -> Option<Self> {
        (summary.changes > 0).then(|| Self {
            title: format!("{} inventory changes", summary.changes),
            body: format!(
                "{} plat, {} endo, MR {}",
                thousands(summary.plat),
                thousands(summary.endo),
                summary.mr
            ),
            tone: Tone::Info,
            in_game: Some(InGameToast::Inventory),
        })
    }

    fn trade(plat: i64, partner: Option<&str>) -> Self {
        Self {
            title: String::from("Trade completed"),
            body: format!("{plat} plat with {}", partner.unwrap_or("an unknown Tenno")),
            tone: Tone::Success,
            in_game: Some(InGameToast::Trade),
        }
    }

    pub fn conversation(player: &str) -> Self {
        Self {
            title: String::from("New in-game conversation"),
            body: player.to_owned(),
            tone: Tone::Info,
            in_game: None,
        }
    }

    pub fn fissure(fissure: &FissureInfo) -> Self {
        let node = fissure.node_name.unwrap_or(fissure.node_id.as_str());
        let kind = if fissure.is_storm {
            "Void Storm"
        } else {
            "Fissure"
        };
        let mut body = fissure.mission_name.clone();
        if let Some((min, max)) = fissure.levels {
            let _ = write!(body, " ({min}-{max})");
        }
        if let Some(faction) = fissure.faction {
            let _ = write!(body, " - {faction}");
        }
        if fissure.steel_path {
            body.push_str(", Steel Path");
        }
        let _ = write!(body, ", {} left", countdown(fissure.remaining_secs));
        Self {
            title: format!("New {} {kind} - {node}", fissure.tier),
            body,
            tone: Tone::Warning,
            in_game: Some(InGameToast::Fissure),
        }
    }

    pub fn reward(reward: &WorldRewardAlert) -> Self {
        let source = match reward.source {
            WorldRewardSource::Invasion => "Invasion",
            WorldRewardSource::Alert => "Alert",
            WorldRewardSource::Event => "Event",
        };
        let title = format!("{source} reward: {}", reward.label);
        let place = reward
            .mission
            .iter()
            .map(String::as_str)
            .chain(reward.location.as_deref())
            .chain(reward.side.as_deref())
            .collect::<Vec<&str>>()
            .join(", ");
        let body = [place.as_str()]
            .into_iter()
            .chain(reward.requirement.as_deref())
            .filter(|part| !part.is_empty())
            .collect::<Vec<&str>>()
            .join(". ");
        Self {
            title,
            body,
            tone: Tone::Warning,
            in_game: Some(InGameToast::Reward),
        }
    }

    pub fn arbitration(alert: &ArbitrationAlert) -> Self {
        let node = &alert.slot.node;
        let mission = node.mission_type.unwrap_or("Unknown mission");
        let title = if alert.at_start {
            format!("Arbitration now: {mission}")
        } else {
            format!("Arbitration in {} min: {mission}", alert.minutes_ahead)
        };
        let mut body = node.node.clone();
        if let Some(planet) = node.planet {
            let _ = write!(body, " ({planet})");
        }
        if let Some(faction) = node.faction {
            let _ = write!(body, ", {faction}");
        }
        let _ = write!(body, ", tier {}, forecast by browse.wf", node.tier.label());
        Self {
            title,
            body,
            tone: Tone::Warning,
            in_game: Some(InGameToast::Arbitration),
        }
    }

    pub fn arbitration_run(run: &ArbitrationRun) -> Self {
        let mut body = run.place();
        if let Some(mission) = &run.mission_type {
            let _ = write!(body, ", {mission}");
        }
        let seconds = whole_seconds(run.duration_secs);
        if seconds > 0 {
            let _ = write!(body, ", {}", countdown(seconds));
        }

        Self {
            title: String::from("Arbitration run recorded"),
            body,
            tone: Tone::Success,
            in_game: None,
        }
    }

    pub fn timer(name: &str, next_state: &str, remaining_secs: i64) -> Self {
        Self {
            title: format!("{name} turns {next_state}"),
            body: format!("in {}", countdown(remaining_secs)),
            tone: Tone::Warning,
            in_game: Some(InGameToast::Timer),
        }
    }

    pub fn webhook_message(&self) -> String {
        format!("{}\n{}", self.title, self.body)
    }
}

fn whole_seconds(secs: f64) -> i64 {
    std::time::Duration::try_from_secs_f64(secs).map_or(0, |duration| {
        i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
    })
}

pub fn countdown(seconds: i64) -> String {
    if seconds <= 0 {
        return String::from("ready");
    }
    let days = seconds / 86_400;
    let hours = seconds % 86_400 / 3_600;
    let minutes = seconds % 3_600 / 60;
    let rest = seconds % 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m {rest:02}s")
    } else {
        format!("{rest}s")
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use wf_core::ArbitrationSlot;
    use wf_worldstate::{ArbitrationNode, ArbitrationTier};

    use super::*;

    fn fissure() -> FissureInfo {
        FissureInfo {
            node_id: String::from("SolNode58"),
            node_name: Some("Hellas (Mars)"),
            mission_type: String::from("MT_EXTERMINATION"),
            mission_name: String::from("Extermination"),
            planet: Some("Mars"),
            tier: String::from("Lith"),
            steel_path: false,
            is_storm: false,
            faction: Some("Grineer"),
            levels: Some((15, 17)),
            expiry: DateTime::UNIX_EPOCH,
            remaining_secs: 5400,
        }
    }

    #[test]
    fn fissure_names_tier_node_mission_levels_and_faction() {
        let notice = Notice::fissure(&fissure());
        assert_eq!(notice.title, "New Lith Fissure - Hellas (Mars)");
        assert_eq!(notice.body, "Extermination (15-17) - Grineer, 1h 30m left");
        assert_eq!(notice.in_game, Some(InGameToast::Fissure));
    }

    #[test]
    fn fissure_modifiers() {
        let steel_path = FissureInfo {
            steel_path: true,
            levels: Some((115, 117)),
            ..fissure()
        };
        assert_eq!(
            Notice::fissure(&steel_path).body,
            "Extermination (115-117) - Grineer, Steel Path, 1h 30m left"
        );
        let storm = Notice::fissure(&FissureInfo {
            is_storm: true,
            levels: None,
            ..fissure()
        });
        assert_eq!(storm.title, "New Lith Void Storm - Hellas (Mars)");
        assert_eq!(storm.body, "Extermination - Grineer, 1h 30m left");
    }

    #[test]
    fn timer_and_webhook_message() {
        let notice = Notice::timer("Cetus", "night", 300);
        assert_eq!(notice.title, "Cetus turns night");
        assert_eq!(notice.body, "in 5m 00s");
        assert_eq!(notice.webhook_message(), "Cetus turns night\nin 5m 00s");
    }

    #[test]
    fn inventory_without_changes_is_silent() {
        let summary = InventorySummary {
            last_sync_oid: String::from("6a9eeb1f000000000000c001"),
            mr: 14,
            plat: 1234,
            credits: 0,
            endo: 56789,
            ducats: 0,
            changes: 0,
        };
        assert_eq!(Notice::inventory(&summary), None);
        let changed = InventorySummary {
            changes: 3,
            ..summary
        };
        let notice = Notice::inventory(&changed).unwrap();
        assert_eq!(notice.title, "3 inventory changes");
        assert_eq!(notice.body, "1,234 plat, 56,789 endo, MR 14");
    }

    #[test]
    fn trade_without_partner() {
        assert_eq!(
            Notice::trade(120, None).body,
            "120 plat with an unknown Tenno"
        );
        assert_eq!(
            Notice::trade(120, Some("Tenno")).body,
            "120 plat with Tenno"
        );
    }

    #[test]
    fn arbitration_notice() {
        let umbriel = ArbitrationSlot {
            id: String::from("1727892000:SolNode64"),
            node: ArbitrationNode {
                node_id: String::from("SolNode64"),
                node: String::from("Umbriel"),
                planet: Some("Uranus"),
                mission_type: Some("Interception"),
                faction: Some("Grineer"),
                tier: ArbitrationTier::S,
            },
            starts: DateTime::UNIX_EPOCH,
            ends: DateTime::UNIX_EPOCH,
            favourite: false,
            matches: true,
            reminder: false,
        };
        let ahead = Notice::arbitration(&ArbitrationAlert {
            slot: umbriel.clone(),
            at_start: false,
            minutes_ahead: 10,
            reminder: false,
        });
        assert_eq!(ahead.title, "Arbitration in 10 min: Interception");
        assert_eq!(
            ahead.body,
            "Umbriel (Uranus), Grineer, tier S, forecast by browse.wf"
        );
        assert_eq!(ahead.in_game, Some(InGameToast::Arbitration));

        let unknown = Notice::arbitration(&ArbitrationAlert {
            slot: ArbitrationSlot {
                node: ArbitrationNode {
                    node_id: String::from("SolNode99999"),
                    node: String::from("SolNode99999"),
                    planet: None,
                    mission_type: None,
                    faction: None,
                    tier: ArbitrationTier::F,
                },
                ..umbriel
            },
            at_start: true,
            minutes_ahead: 0,
            reminder: true,
        });
        assert_eq!(unknown.title, "Arbitration now: Unknown mission");
        assert_eq!(unknown.body, "SolNode99999, tier F, forecast by browse.wf");
    }

    #[test]
    fn countdown_units() {
        assert_eq!(countdown(0), "ready");
        assert_eq!(countdown(45), "45s");
        assert_eq!(countdown(25 * 60 + 5), "25m 05s");
        assert_eq!(countdown(3 * 3600 + 120), "3h 02m");
        assert_eq!(countdown(2 * 86_400 + 5 * 3600), "2d 5h");
    }

    #[test]
    fn reward_notice() {
        let invasion = WorldRewardAlert {
            key: String::from("invasion/6a9e1976d7b7cca80208e4d9@1000/attacker#catalyst"),
            source: WorldRewardSource::Invasion,
            label: String::from("Orokin Catalyst Blueprint"),
            image_name: None,
            location: Some(String::from("Pallas (Ceres)")),
            mission: Some(String::from("Extermination")),
            side: Some(String::from("Support Corpus")),
            requirement: Some(String::from("Run 3 missions for this side")),
            expiry: None,
        };
        let notice = Notice::reward(&invasion);
        assert_eq!(notice.title, "Invasion reward: Orokin Catalyst Blueprint");
        assert_eq!(
            notice.body,
            "Extermination, Pallas (Ceres), Support Corpus. Run 3 missions for this side"
        );
        assert_eq!(notice.in_game, Some(InGameToast::Reward));

        let event = WorldRewardAlert {
            source: WorldRewardSource::Event,
            label: String::from("3 x Orokin Catalyst Blueprint"),
            location: None,
            mission: None,
            side: None,
            requirement: Some(String::from("Event goal 40,000")),
            ..invasion
        };
        let notice = Notice::reward(&event);
        assert_eq!(notice.title, "Event reward: 3 x Orokin Catalyst Blueprint");
        assert_eq!(notice.body, "Event goal 40,000");
    }
}
