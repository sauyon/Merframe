use std::collections::{BTreeSet, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use wf_worldstate::{WorldRewardOffer, WorldRewardSource, WorldState};

use crate::identity::{ItemKind, ItemTable, is_unusual_reward_weapon};
use crate::text::thousands;

const CATALYSTS: [&str; 2] = [
    "/Lotus/Types/Items/MiscItems/OrokinCatalyst",
    "/Lotus/Types/Recipes/Components/OrokinCatalystBlueprint",
];
const REACTORS: [&str; 2] = [
    "/Lotus/Types/Items/MiscItems/OrokinReactor",
    "/Lotus/Types/Recipes/Components/OrokinReactorBlueprint",
];
const EXILUS_ADAPTERS: [&str; 4] = [
    "/Lotus/Types/Items/MiscItems/UtilityUnlocker",
    "/Lotus/Types/Recipes/Components/UtilityUnlockerBlueprint",
    "/Lotus/Types/Items/MiscItems/WeaponUtilityUnlocker",
    "/Lotus/Types/Recipes/Components/WeaponUtilityUnlockerBlueprint",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum WorldRewardAlertGroup {
    Catalysts,
    Reactors,
    ExilusAdapters,
    UnmasteredWeapons,
    UnusualWeapons,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct WorldRewardAlertChoices {
    pub groups: BTreeSet<WorldRewardAlertGroup>,
    pub weapon_parts: Vec<String>,
}

impl Default for WorldRewardAlertChoices {
    fn default() -> Self {
        Self {
            groups: BTreeSet::from([
                WorldRewardAlertGroup::Catalysts,
                WorldRewardAlertGroup::Reactors,
                WorldRewardAlertGroup::ExilusAdapters,
                WorldRewardAlertGroup::UnmasteredWeapons,
                WorldRewardAlertGroup::UnusualWeapons,
            ]),
            weapon_parts: Vec::new(),
        }
    }
}

fn weapon_of_reward<'a>(items: &'a ItemTable, item_type: &str) -> Option<&'a str> {
    let record = items.lookup(item_type)?;
    Some(match record.kind {
        ItemKind::Part { set, .. } => items[set].unique_name.as_str(),
        _ => record.unique_name.as_str(),
    })
}

impl WorldRewardAlertChoices {
    pub fn wants(&self, item_type: &str, items: &ItemTable, unmastered: &HashSet<&str>) -> bool {
        let unmastered_weapon =
            || weapon_of_reward(items, item_type).is_some_and(|weapon| unmastered.contains(weapon));
        self.weapon_parts.iter().any(|part| part == item_type)
            || self.groups.iter().any(|group| match group {
                WorldRewardAlertGroup::Catalysts => CATALYSTS.contains(&item_type),
                WorldRewardAlertGroup::Reactors => REACTORS.contains(&item_type),
                WorldRewardAlertGroup::ExilusAdapters => EXILUS_ADAPTERS.contains(&item_type),
                WorldRewardAlertGroup::UnmasteredWeapons => unmastered_weapon(),
                WorldRewardAlertGroup::UnusualWeapons => {
                    is_unusual_reward_weapon(item_type) && unmastered_weapon()
                }
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct WorldRewardAlert {
    pub key: String,
    pub source: WorldRewardSource,
    pub label: String,
    pub image_name: Option<String>,
    pub location: Option<String>,
    pub mission: Option<String>,
    pub side: Option<String>,
    pub requirement: Option<String>,
    pub expiry: Option<DateTime<Utc>>,
}

fn requirement(offer: &WorldRewardOffer) -> Option<String> {
    match offer.source {
        WorldRewardSource::Invasion => Some("Run 3 missions for this side".to_owned()),
        WorldRewardSource::Alert => None,
        WorldRewardSource::Event => Some(offer.event_target.map_or_else(
            || "Finish the event".to_owned(),
            |target| format!("Event goal {}", thousands(target)),
        )),
    }
}

fn world_reward_alert(offer: WorldRewardOffer, items: &ItemTable) -> WorldRewardAlert {
    let record = items.resolve(&offer.item_type);
    let label = if offer.count > 1 {
        format!("{} x {}", offer.count, record.name)
    } else {
        record.name.clone()
    };
    let location = offer
        .node_name
        .map(str::to_owned)
        .or_else(|| offer.node_id.clone());
    let requirement = requirement(&offer);
    WorldRewardAlert {
        key: offer.key,
        source: offer.source,
        label,
        image_name: record.image_name.clone(),
        location,
        requirement,
        mission: offer.mission,
        side: offer.side,
        expiry: offer.expiry,
    }
}

pub(crate) fn world_reward_alerts(
    world: &WorldState,
    items: &ItemTable,
    unmastered: &HashSet<&str>,
    choices: &WorldRewardAlertChoices,
    now: DateTime<Utc>,
) -> Vec<WorldRewardAlert> {
    let mut rewards: Vec<WorldRewardAlert> = world
        .world_reward_offers(now)
        .into_iter()
        .filter(|offer| choices.wants(&offer.item_type, items, unmastered))
        .map(|offer| world_reward_alert(offer, items))
        .collect();
    rewards.sort_by(|a, b| {
        let a_expiry = a.expiry.unwrap_or(DateTime::<Utc>::MAX_UTC);
        let b_expiry = b.expiry.unwrap_or(DateTime::<Utc>::MAX_UTC);
        a_expiry.cmp(&b_expiry).then_with(|| a.label.cmp(&b.label))
    });
    rewards
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::fixtures::at;

    const WORLD: &str = r#"{"Invasions":[
        {"_id":{"$oid":"6a9e1976d7b7cca80208e4d9"},"Faction":"FC_CORPUS","DefenderFaction":"FC_GRINEER",
         "Node":"SolNode131","Completed":false,
         "AttackerReward":{"countedItems":[{"ItemType":"/Lotus/Types/Items/MiscItems/OrokinCatalyst","ItemCount":1}]},
         "DefenderReward":{"countedItems":[{"ItemType":"/Lotus/Types/Recipes/Weapons/WeaponParts/DeraVandalBarrel","ItemCount":1}]},
         "Activation":{"$date":{"$numberLong":"1000"}}}],
      "Alerts":[{
        "_id":{"$oid":"6ad0d7380dfe89c5d8229302"},
        "Activation":{"$date":{"$numberLong":"1000"}},"Expiry":{"$date":{"$numberLong":"5000"}},
        "MissionInfo":{"missionType":"MT_HIVE","location":"SolNode153",
          "missionReward":{"countedItems":[{"ItemType":"/Lotus/Types/Recipes/Components/OrokinReactorBlueprint","ItemCount":1}]}}}],
      "Goals":[{
        "_id":{"$oid":"6a71fe700000000000000000"},
        "Activation":{"$date":{"$numberLong":"1000"}},"Expiry":{"$date":{"$numberLong":"4000"}},
        "Goal":40000,"Node":"EventNode25",
        "Reward":{"countedItems":[{"ItemType":"/Lotus/Types/Items/MiscItems/WeaponUtilityUnlocker","ItemCount":2}]}}]
    }"#;

    fn world() -> WorldState {
        WorldState::parse(WORLD).unwrap()
    }

    const DERA_BARREL: &str = "/Lotus/Types/Recipes/Weapons/WeaponParts/DeraVandalBarrel";

    fn wants(choices: &WorldRewardAlertChoices, item_type: &str) -> bool {
        choices.wants(item_type, &ItemTable::default(), &HashSet::new())
    }

    #[test]
    fn default_choices() {
        let choices = WorldRewardAlertChoices::default();
        assert!(wants(
            &choices,
            "/Lotus/Types/Items/MiscItems/OrokinCatalyst"
        ));
        assert!(wants(
            &choices,
            "/Lotus/Types/Recipes/Components/OrokinReactorBlueprint"
        ));
        assert!(wants(
            &choices,
            "/Lotus/Types/Items/MiscItems/WeaponUtilityUnlocker"
        ));
        assert!(!wants(&choices, DERA_BARREL));
        assert!(!wants(
            &choices,
            "/Lotus/Types/Items/Research/EnergyComponent"
        ));
    }

    #[test]
    fn unusual_weapons_skip_mastered() {
        let table = ItemTable::build(&crate::catalog::fixtures::catalog());
        let barrel = "/Lotus/Types/Recipes/Weapons/WeaponParts/BratonPrimeBarrel";
        let weapon = weapon_of_reward(&table, barrel).unwrap();
        let choices = WorldRewardAlertChoices {
            groups: BTreeSet::from([WorldRewardAlertGroup::UnusualWeapons]),
            ..WorldRewardAlertChoices::default()
        };
        assert!(choices.wants(barrel, &table, &HashSet::from([weapon])));
        assert!(!choices.wants(barrel, &table, &HashSet::new()));
    }

    #[test]
    fn unmastered_weapon_parts() {
        let table = ItemTable::build(&crate::catalog::fixtures::catalog());
        let barrel = "/Lotus/Types/Recipes/Weapons/WeaponParts/BratonPrimeBarrel";
        let weapon = weapon_of_reward(&table, barrel).unwrap();
        assert!(weapon.ends_with("/BratonPrime"));
        let choices = WorldRewardAlertChoices {
            groups: BTreeSet::from([WorldRewardAlertGroup::UnmasteredWeapons]),
            ..WorldRewardAlertChoices::default()
        };
        assert!(choices.wants(barrel, &table, &HashSet::from([weapon])));
        assert!(!choices.wants(barrel, &table, &HashSet::new()));
    }

    #[test]
    fn chosen_weapon_part() {
        let choices = WorldRewardAlertChoices {
            groups: BTreeSet::new(),
            weapon_parts: vec![DERA_BARREL.to_owned()],
        };
        let rewards = world_reward_alerts(
            &world(),
            &ItemTable::default(),
            &HashSet::new(),
            &choices,
            at(2000),
        );
        assert_eq!(rewards.len(), 1);
        assert_eq!(rewards[0].side.as_deref(), Some("Support Grineer"));
        assert_eq!(
            rewards[0].requirement.as_deref(),
            Some("Run 3 missions for this side")
        );
        assert_eq!(rewards[0].location.as_deref(), Some("Pallas (Ceres)"));
        assert_eq!(
            rewards[0].key,
            "invasion/6a9e1976d7b7cca80208e4d9@1000/defender#/Lotus/Types/Recipes/Weapons/WeaponParts/DeraVandalBarrel"
        );
    }

    #[test]
    fn expiry_then_label_order() {
        let rewards = world_reward_alerts(
            &world(),
            &ItemTable::default(),
            &HashSet::new(),
            &WorldRewardAlertChoices::default(),
            at(2000),
        );
        let order: Vec<(WorldRewardSource, &str, Option<&str>)> = rewards
            .iter()
            .map(|reward| {
                (
                    reward.source,
                    reward.label.as_str(),
                    reward.requirement.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            order,
            vec![
                (
                    WorldRewardSource::Event,
                    "2 x Weapon Utility Unlocker",
                    Some("Event goal 40,000")
                ),
                (WorldRewardSource::Alert, "Orokin Reactor Blueprint", None),
                (
                    WorldRewardSource::Invasion,
                    "Orokin Catalyst",
                    Some("Run 3 missions for this side")
                ),
            ]
        );
        assert_eq!(rewards[1].mission.as_deref(), Some("Hive"));
        assert_eq!(rewards[2].mission.as_deref(), Some("Extermination"));
    }
}
