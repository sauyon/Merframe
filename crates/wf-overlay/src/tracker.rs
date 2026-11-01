//! Turns per-poll buff readings into what the ring shows. The game only tells us time
//! remaining, so a buff's full length is the largest remaining value seen since it was
//! last applied.

#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub id: String,
    pub label: String,
    pub remaining: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub label: String,
    /// `None` once the buff has dropped off.
    pub remaining: Option<f64>,
    pub total: f64,
}

struct Entry {
    id: String,
    label: String,
    total: f64,
    remaining: Option<f64>,
    gone_at: Option<f64>,
}

/// Remaining time jumping up by more than this means the buff was reapplied.
const REFRESH_JUMP: f64 = 0.5;

pub struct Tracker {
    entries: Vec<Entry>,
    /// Seconds an expired buff stays on the ring, so a drop is noticed.
    linger: f64,
}

impl Tracker {
    pub fn new(linger: f64) -> Self {
        Self {
            entries: Vec::new(),
            linger,
        }
    }

    pub fn update(&mut self, readings: &[Reading], now: f64) {
        for r in readings {
            match self.entries.iter_mut().find(|e| e.id == r.id) {
                Some(e) => {
                    let refreshed = e.remaining.is_none_or(|old| r.remaining > old + REFRESH_JUMP);
                    e.total = if refreshed { r.remaining } else { e.total.max(r.remaining) };
                    e.remaining = Some(r.remaining);
                    e.gone_at = None;
                    e.label.clone_from(&r.label);
                }
                None => self.entries.push(Entry {
                    id: r.id.clone(),
                    label: r.label.clone(),
                    total: r.remaining,
                    remaining: Some(r.remaining),
                    gone_at: None,
                }),
            }
        }
        for e in &mut self.entries {
            if e.remaining.is_some() && !readings.iter().any(|r| r.id == e.id) {
                e.remaining = None;
                e.gone_at = Some(now);
            }
        }
        let linger = self.linger;
        self.entries
            .retain(|e| e.gone_at.is_none_or(|t| now - t < linger));
    }

    pub fn shown(&self) -> Vec<Shown> {
        self.entries
            .iter()
            .map(|e| Shown {
                label: e.label.clone(),
                remaining: e.remaining,
                total: e.total,
            })
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, reason = "values are copied, never computed")]
mod tests {
    use super::*;

    fn r(id: &str, remaining: f64) -> Reading {
        Reading {
            id: id.into(),
            label: id.into(),
            remaining,
        }
    }

    #[test]
    fn total_is_first_seen_and_survives_countdown() {
        let mut t = Tracker::new(3.0);
        t.update(&[r("lash", 60.0)], 0.0);
        t.update(&[r("lash", 50.0)], 10.0);
        let s = t.shown();
        assert_eq!(s[0].total, 60.0);
        assert_eq!(s[0].remaining, Some(50.0));
    }

    #[test]
    fn recast_resets_total() {
        let mut t = Tracker::new(3.0);
        t.update(&[r("lash", 60.0)], 0.0);
        t.update(&[r("lash", 10.0)], 50.0);
        t.update(&[r("lash", 75.0)], 51.0);
        assert_eq!(t.shown()[0].total, 75.0);
    }

    #[test]
    fn expired_lingers_then_drops() {
        let mut t = Tracker::new(3.0);
        t.update(&[r("a", 5.0), r("b", 5.0)], 0.0);
        t.update(&[r("b", 4.0)], 1.0);
        let s = t.shown();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].remaining, None);
        t.update(&[r("b", 1.0)], 4.0);
        assert_eq!(t.shown().len(), 1);
    }

    #[test]
    fn reappearing_buff_starts_fresh() {
        let mut t = Tracker::new(3.0);
        t.update(&[r("a", 5.0)], 0.0);
        t.update(&[], 5.0);
        t.update(&[r("a", 3.0)], 6.0);
        let s = t.shown();
        assert_eq!(s[0].total, 3.0);
        assert_eq!(s[0].remaining, Some(3.0));
    }

    #[test]
    fn order_is_first_seen() {
        let mut t = Tracker::new(3.0);
        t.update(&[r("b", 5.0)], 0.0);
        t.update(&[r("a", 5.0), r("b", 4.0)], 1.0);
        let labels: Vec<_> = t.shown().into_iter().map(|s| s.label).collect();
        assert_eq!(labels, ["b", "a"]);
    }
}
