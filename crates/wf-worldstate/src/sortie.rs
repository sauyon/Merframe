use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::Serialize;
use wf_mongo::ObjectId;

use crate::embedded::{SORTIE_BOSSES, SORTIE_MODIFIERS, SortieBoss, lookup};
use crate::mission_types::{known_or_title_case, mission_type_name};
use crate::nodes::node_name;
use crate::window::Window;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawSortieVariant {
    pub mission_type: String,
    pub modifier_type: String,
    pub node: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RawSortie {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    #[serde(flatten)]
    pub window: Window,
    pub boss: String,
    pub variants: Vec<RawSortieVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct SortieMission {
    pub mission_type: String,
    pub mission_name: String,
    pub modifier: String,
    pub modifier_name: String,
    pub node_id: String,
    pub node_name: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct Sortie {
    pub id: String,
    pub boss: String,
    pub boss_name: String,
    pub faction: Option<&'static str>,
    pub activation: DateTime<Utc>,
    pub expiry: DateTime<Utc>,
    pub missions: Vec<SortieMission>,
}

pub fn boss(id: &str) -> Option<&'static SortieBoss> {
    lookup(SORTIE_BOSSES, id)
}

pub fn boss_name(id: &str) -> String {
    known_or_title_case(SORTIE_BOSSES, id, "SORTIE_BOSS_", |boss| boss.name)
}

fn modifier_name(id: &str) -> String {
    known_or_title_case(SORTIE_MODIFIERS, id, "SORTIE_MODIFIER_", |modifier| {
        modifier.name
    })
}

pub fn current_sortie(sorties: &[RawSortie], now: DateTime<Utc>) -> Option<Sortie> {
    sorties
        .iter()
        .find(|sortie| sortie.window.contains(now))
        .map(|sortie| Sortie {
            id: sortie.id.as_str().to_owned(),
            boss: sortie.boss.clone(),
            boss_name: boss_name(&sortie.boss),
            faction: boss(&sortie.boss).map(|boss| boss.faction),
            activation: sortie.window.activation.datetime(),
            expiry: sortie.window.expiry.datetime(),
            missions: sortie
                .variants
                .iter()
                .map(|variant| SortieMission {
                    mission_type: variant.mission_type.clone(),
                    mission_name: mission_type_name(&variant.mission_type),
                    modifier: variant.modifier_type.clone(),
                    modifier_name: modifier_name(&variant.modifier_type),
                    node_id: variant.node.clone(),
                    node_name: node_name(&variant.node),
                })
                .collect(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::fixtures::{at, window};

    #[test]
    fn active_sortie() {
        let now = at(1500);
        let sorties = vec![RawSortie {
            id: ObjectId("6a9edbfe7915bed56981db25".to_owned()),
            window: window(1000, 2000),
            boss: "SORTIE_BOSS_KELA".to_owned(),
            variants: vec![
                RawSortieVariant {
                    mission_type: "MT_RETRIEVAL".to_owned(),
                    modifier_type: "SORTIE_MODIFIER_SHOTGUN_ONLY".to_owned(),
                    node: "SolNode1".to_owned(),
                },
                RawSortieVariant {
                    mission_type: "MT_EXCAVATE".to_owned(),
                    modifier_type: "SORTIE_MODIFIER_ARMOR".to_owned(),
                    node: "SolNode1".to_owned(),
                },
                RawSortieVariant {
                    mission_type: "MT_ASSASSINATION".to_owned(),
                    modifier_type: "SORTIE_MODIFIER_EXIMUS".to_owned(),
                    node: "SolNode1".to_owned(),
                },
            ],
        }];
        let sortie = current_sortie(&sorties, now).unwrap();
        assert_eq!(sortie.missions.len(), 3);
        assert_eq!(sortie.id, "6a9edbfe7915bed56981db25");
        assert_eq!(sortie.boss, "SORTIE_BOSS_KELA");
        assert_eq!(sortie.boss_name, "Kela De Thaym");
        assert_eq!(sortie.faction, Some("Grineer"));
        assert_eq!(
            sortie.missions[0].modifier_name,
            "Weapon Restriction: Shotgun Only"
        );
        assert_eq!(sortie.missions[1].modifier_name, "Augmented Enemy Armor");
        assert_eq!(sortie.missions[2].modifier_name, "Eximus Stronghold");
        assert_eq!(sortie.missions[0].mission_name, "Hijack");
        assert_eq!(sortie.missions[1].mission_name, "Excavation");
        assert_eq!(sortie.missions[2].mission_name, "Assassination");
    }

    #[test]
    fn unknown_boss_and_modifier() {
        assert_eq!(boss_name("SORTIE_BOSS_NEW_GUY"), "New Guy");
        assert!(boss("SORTIE_BOSS_NEW_GUY").is_none());
        assert_eq!(modifier_name("SORTIE_MODIFIER_NEW_THING"), "New Thing");
    }

    #[test]
    fn embedded_tables_sorted() {
        assert!(SORTIE_BOSSES.is_sorted_by(|a, b| a.0 < b.0));
        assert!(SORTIE_MODIFIERS.is_sorted_by(|a, b| a.0 < b.0));
        assert!(
            SORTIE_BOSSES
                .iter()
                .all(|(id, _)| id.starts_with("SORTIE_BOSS_"))
        );
        assert!(
            SORTIE_MODIFIERS
                .iter()
                .all(|(id, _)| id.starts_with("SORTIE_MODIFIER_"))
        );
    }

    #[test]
    fn no_active_sortie() {
        let now = at(0);
        let sorties = vec![RawSortie {
            id: ObjectId("6a9edbfe7915bed56981db25".to_owned()),
            window: window(1000, 2000),
            boss: "SORTIE_BOSS_KELA".to_owned(),
            variants: vec![],
        }];
        assert!(current_sortie(&sorties, now).is_none());
    }
}
