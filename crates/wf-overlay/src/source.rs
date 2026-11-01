//! Where buff readings come from.

use crate::tracker::Reading;

pub trait BuffSource {
    /// Buffs active right now. Called every frame.
    fn poll(&mut self, now: f64) -> Vec<Reading>;
    /// Shown in the ring's centre while the source has nothing to report.
    fn status(&self) -> Option<String> {
        None
    }
}

/// Placeholder until the game's buff list is located in memory.
pub struct Pending;

impl BuffSource for Pending {
    fn poll(&mut self, _now: f64) -> Vec<Reading> {
        Vec::new()
    }

    fn status(&self) -> Option<String> {
        Some("buff source not wired up".into())
    }
}

/// Synthetic buffs for checking the ring's look. Labelled so it can't be mistaken for game data.
pub struct Demo;

impl BuffSource for Demo {
    fn poll(&mut self, now: f64) -> Vec<Reading> {
        [("demo A", 12.0), ("demo B", 30.0), ("demo C", 45.0)]
            .into_iter()
            .filter_map(|(label, period)| {
                // Each demo buff runs for `period`, then is down for 3s.
                let t = now % (period + 3.0);
                (t < period).then(|| Reading {
                    id: label.into(),
                    label: label.into(),
                    remaining: period - t,
                })
            })
            .collect()
    }

    fn status(&self) -> Option<String> {
        Some("DEMO".into())
    }
}
