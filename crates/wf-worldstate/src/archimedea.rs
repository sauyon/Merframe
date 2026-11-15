use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::resets::ResetWindow;
use crate::window::Window;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RawConquest {
    #[serde(flatten)]
    pub window: Window,
    #[serde(rename = "Type")]
    pub conquest_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchimedeaKind {
    Deep,
    Temporal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchimedeaWeek {
    pub kind: ArchimedeaKind,
    pub window: ResetWindow,
}

fn archimedea_kind(conquest_type: &str) -> Option<ArchimedeaKind> {
    match conquest_type {
        "CT_LAB" => Some(ArchimedeaKind::Deep),
        "CT_HEX" => Some(ArchimedeaKind::Temporal),
        _ => None,
    }
}

pub fn archimedea_weeks(conquests: &[RawConquest], now: DateTime<Utc>) -> Vec<ArchimedeaWeek> {
    conquests
        .iter()
        .filter(|conquest| conquest.window.contains(now))
        .filter_map(|conquest| {
            Some(ArchimedeaWeek {
                kind: archimedea_kind(&conquest.conquest_type)?,
                window: conquest.window.into(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::fixtures::{at, window};

    fn conquest(conquest_type: &str, activation_ms: i64, expiry_ms: i64) -> RawConquest {
        RawConquest {
            window: window(activation_ms, expiry_ms),
            conquest_type: conquest_type.to_owned(),
        }
    }

    #[test]
    fn running_weeks() {
        let conquests = [
            conquest("CT_LAB", 1000, 2000),
            conquest("CT_HEX", 1000, 2000),
            conquest("CT_LAB", 2000, 3000),
            conquest("CT_NEW", 1000, 2000),
        ];
        let weeks = archimedea_weeks(&conquests, at(1500));

        let kinds: Vec<_> = weeks.iter().map(|week| week.kind).collect();
        assert_eq!(kinds, vec![ArchimedeaKind::Deep, ArchimedeaKind::Temporal]);
        assert_eq!(weeks[0].window.expiry.timestamp_millis(), 2000);
    }
}
