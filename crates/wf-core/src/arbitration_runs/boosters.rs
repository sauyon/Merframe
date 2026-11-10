use chrono::{DateTime, Utc};
use wf_inventory::Booster;
use wf_log::Event as LogEvent;

use super::{ArbitrationRunResourceBooster, ArbitrationRunResourceBoosterKind};

const HUB_BLESSING_SECS: f64 = 10_800.0;
const INVENTORY_BOOSTERS: [(ArbitrationRunResourceBoosterKind, &str); 3] = [
    (
        ArbitrationRunResourceBoosterKind::Amount,
        "/Lotus/Types/Boosters/ResourceAmountBooster",
    ),
    (
        ArbitrationRunResourceBoosterKind::DropChance,
        "/Lotus/Types/Boosters/ResourceDropChanceBooster",
    ),
    (
        ArbitrationRunResourceBoosterKind::DropChanceBlessing,
        "/Lotus/Types/Boosters/ResourceDropChanceBlessing",
    ),
];

struct LoggedResourceBlessing {
    acquired_secs: f64,
    expires_secs: f64,
}

#[derive(Default)]
pub(super) struct ArbitrationRunBoosterTracker {
    pending_blessing_secs: Option<f64>,
    logged_blessings: Vec<LoggedResourceBlessing>,
    own_blessing_sent_secs: Option<f64>,
    inventory_expires_at: Vec<(ArbitrationRunResourceBoosterKind, DateTime<Utc>)>,
}

impl ArbitrationRunBoosterTracker {
    pub(super) fn observe(&mut self, event: &LogEvent, secs: f64) {
        match event {
            LogEvent::HubBlessingPending { resource: true } => {
                self.pending_blessing_secs = Some(secs);
            }
            LogEvent::HubBlessingsRequested => {
                if let Some(acquired_secs) = self.pending_blessing_secs.take() {
                    self.logged_blessings.push(LoggedResourceBlessing {
                        acquired_secs,
                        expires_secs: acquired_secs + HUB_BLESSING_SECS,
                    });
                }
            }
            LogEvent::HubBlessingSent { resource } => {
                self.own_blessing_sent_secs = resource.then_some(secs);
            }
            LogEvent::HubBlessingConfirmed {
                resource: true,
                duration_secs,
            } => {
                if let Some(acquired_secs) = self.own_blessing_sent_secs.take() {
                    self.logged_blessings.push(LoggedResourceBlessing {
                        acquired_secs,
                        expires_secs: acquired_secs + duration_secs,
                    });
                }
            }
            LogEvent::HubBlessingConfirmed { .. } => self.own_blessing_sent_secs = None,
            _ => {}
        }
    }

    pub(super) fn keep_latest_booster_expiries(&mut self, boosters: &[Booster]) {
        self.inventory_expires_at = INVENTORY_BOOSTERS
            .into_iter()
            .filter_map(|(kind, item_type)| {
                boosters
                    .iter()
                    .filter(|booster| booster.item_type == item_type)
                    .filter_map(|booster| DateTime::from_timestamp(booster.expiry_date, 0))
                    .chain(self.synced_expiry(kind))
                    .max()
                    .map(|expires_at| (kind, expires_at))
            })
            .collect();
    }

    fn synced_expiry(&self, kind: ArbitrationRunResourceBoosterKind) -> Option<DateTime<Utc>> {
        self.inventory_expires_at
            .iter()
            .find(|(known, _)| *known == kind)
            .map(|(_, expires_at)| *expires_at)
    }

    pub(super) fn forget_logged(&mut self) {
        self.pending_blessing_secs = None;
        self.logged_blessings.clear();
        self.own_blessing_sent_secs = None;
    }

    pub(super) fn running_at_run_start(
        &self,
        first_secs: f64,
        start_secs: f64,
        started_at: Option<DateTime<Utc>>,
    ) -> Vec<ArbitrationRunResourceBooster> {
        let logged_blessing = self
            .logged_blessings
            .iter()
            .filter(|blessing| blessing.acquired_secs <= first_secs)
            .map(|blessing| blessing.expires_secs - start_secs)
            .reduce(f64::max);
        INVENTORY_BOOSTERS
            .into_iter()
            .filter_map(|(kind, _)| {
                let from_inventory = self
                    .synced_expiry(kind)
                    .zip(started_at)
                    .map(|(expires, started)| (expires - started).as_seconds_f64());
                let from_log = logged_blessing
                    .filter(|_| kind == ArbitrationRunResourceBoosterKind::DropChanceBlessing);
                [from_inventory, from_log]
                    .into_iter()
                    .flatten()
                    .reduce(f64::max)
                    .filter(|secs| *secs > 0.0)
                    .map(|expires_secs| ArbitrationRunResourceBooster { kind, expires_secs })
            })
            .collect()
    }
}

#[cfg(test)]
pub(super) fn inventory_booster(
    kind: ArbitrationRunResourceBoosterKind,
    expiry_date: i64,
) -> Booster {
    let (_, item_type) = INVENTORY_BOOSTERS
        .into_iter()
        .find(|(known, _)| *known == kind)
        .unwrap();
    Booster {
        item_type: item_type.to_owned(),
        expiry_date,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLESSING: ArbitrationRunResourceBoosterKind =
        ArbitrationRunResourceBoosterKind::DropChanceBlessing;

    fn expiry(
        boosters: &[ArbitrationRunResourceBooster],
        kind: ArbitrationRunResourceBoosterKind,
    ) -> Option<f64> {
        boosters
            .iter()
            .find(|booster| booster.kind == kind)
            .map(|booster| booster.expires_secs)
    }

    #[test]
    fn logged_handshakes() {
        let mut boosters = ArbitrationRunBoosterTracker::default();

        boosters.observe(&LogEvent::HubBlessingPending { resource: false }, 1.0);
        boosters.observe(&LogEvent::HubBlessingPending { resource: true }, 5.0);
        boosters.observe(&LogEvent::HubBlessingsRequested, 6.0);
        assert_eq!(
            expiry(&boosters.running_at_run_start(5.0, 10.0, None), BLESSING),
            Some(10_795.0)
        );
        assert!(boosters.running_at_run_start(4.0, 10.0, None).is_empty());

        boosters.observe(&LogEvent::HubBlessingSent { resource: true }, 20.0);
        boosters.observe(
            &LogEvent::HubBlessingConfirmed {
                resource: false,
                duration_secs: 99.0,
            },
            21.0,
        );
        boosters.observe(&LogEvent::HubBlessingSent { resource: true }, 30.0);
        boosters.observe(
            &LogEvent::HubBlessingConfirmed {
                resource: true,
                duration_secs: 20_000.0,
            },
            31.0,
        );
        assert_eq!(
            expiry(&boosters.running_at_run_start(30.0, 30.0, None), BLESSING),
            Some(20_000.0)
        );

        boosters.forget_logged();
        assert!(boosters.running_at_run_start(30.0, 30.0, None).is_empty());
    }

    #[test]
    fn inventory_and_log_merged() {
        let started_at = DateTime::from_timestamp(1_791_130_000, 0);
        let mut boosters = ArbitrationRunBoosterTracker::default();

        boosters.keep_latest_booster_expiries(&[
            Booster {
                item_type: "/Lotus/Types/Boosters/AffinityBooster".to_owned(),
                expiry_date: 1_791_200_000,
            },
            inventory_booster(ArbitrationRunResourceBoosterKind::Amount, 1_791_130_200),
            inventory_booster(ArbitrationRunResourceBoosterKind::DropChance, 1_791_129_000),
            inventory_booster(BLESSING, 1_791_130_500),
            inventory_booster(BLESSING, 1_791_131_000),
        ]);
        let running = boosters.running_at_run_start(10.0, 10.0, started_at);

        assert_eq!(
            expiry(&running, ArbitrationRunResourceBoosterKind::Amount),
            Some(200.0)
        );
        assert_eq!(
            expiry(&running, ArbitrationRunResourceBoosterKind::DropChance),
            None
        );
        assert_eq!(expiry(&running, BLESSING), Some(1_000.0));
        assert!(boosters.running_at_run_start(10.0, 10.0, None).is_empty());

        boosters.observe(&LogEvent::HubBlessingSent { resource: true }, 1.0);
        boosters.observe(
            &LogEvent::HubBlessingConfirmed {
                resource: true,
                duration_secs: 5_000.0,
            },
            2.0,
        );
        assert_eq!(
            expiry(
                &boosters.running_at_run_start(10.0, 10.0, started_at),
                BLESSING
            ),
            Some(4_991.0)
        );

        boosters.keep_latest_booster_expiries(&[]);
        boosters.forget_logged();
        assert_eq!(
            expiry(
                &boosters.running_at_run_start(10.0, 10.0, started_at),
                BLESSING
            ),
            Some(1_000.0)
        );
    }
}
