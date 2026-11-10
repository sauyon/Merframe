use std::collections::HashMap;

use memchr::memmem;
use serde::Deserialize;
use serde_json::value::RawValue;

pub const MAX_INVENTORY_BODY: usize = 64 << 20;
pub const REQUIRED_KEYS: [&str; 16] = [
    "Created",
    "FusionPoints",
    "LastInventorySync",
    "LongGuns",
    "MiscItems",
    "Missions",
    "PlayerLevel",
    "PremiumCredits",
    "PremiumCreditsFree",
    "Recipes",
    "RegularCredits",
    "RewardSeed",
    "Suits",
    "TradesRemaining",
    "Upgrades",
    "XPInfo",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryBuffer {
    pub addr: u64,
    pub len: usize,
    pub last_sync: String,
    pub json: String,
    pub mission_rewards: Option<Vec<MissionEndReward>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MissionEndReward {
    #[serde(rename = "StoreItem")]
    pub store_item: String,
    #[serde(rename = "ItemCount")]
    pub item_count: u32,
}

#[derive(Deserialize)]
struct MissionEnd {
    #[serde(rename = "InventoryJson")]
    inventory_json: String,
    #[serde(rename = "MissionRewards", default)]
    mission_rewards: Vec<MissionEndReward>,
}

#[derive(Deserialize)]
struct Oid {
    #[serde(rename = "$oid")]
    oid: String,
}

pub(crate) fn accept(addr: u64, body: &[u8]) -> Option<InventoryBuffer> {
    if body.trim_ascii_end().last() != Some(&b'}') {
        return None;
    }

    let text = str::from_utf8(body).ok()?;
    let (json, mission_rewards) =
        if memmem::find(body, b"\"LastInventorySync\":{\"$oid\":\"").is_some() {
            (text.to_owned(), None)
        } else {
            let end = serde_json::from_str::<MissionEnd>(text).ok()?;
            (end.inventory_json, Some(end.mission_rewards))
        };

    let fields: HashMap<&str, &RawValue> = serde_json::from_str(&json).ok()?;
    if !REQUIRED_KEYS.iter().all(|key| fields.contains_key(key)) {
        return None;
    }
    let last_sync = serde_json::from_str::<Oid>(fields.get("LastInventorySync")?.get()).ok()?;

    Some(InventoryBuffer {
        addr,
        len: body.len(),
        last_sync: last_sync.oid,
        json,
        mission_rewards,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADDR: u64 = 0x7fff_0000;
    const OID: &str = "6a9f2bde000000000000c104";

    fn document(oid: &str, keys: &[&str]) -> String {
        let fields = keys
            .iter()
            .map(|key| match *key {
                "LastInventorySync" => format!("\"{key}\":{{\"$oid\":\"{oid}\"}}"),
                _ => format!("\"{key}\":[]"),
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("{{{fields}}}")
    }

    #[test]
    fn full_document() {
        let body = document(OID, &REQUIRED_KEYS);
        let buffer = accept(ADDR, body.as_bytes()).unwrap();
        assert_eq!(buffer.addr, ADDR);
        assert_eq!(buffer.len, body.len());
        assert_eq!(buffer.last_sync, OID);
        assert_eq!(buffer.json, body);
        assert_eq!(buffer.mission_rewards, None);
    }

    #[test]
    fn keys_in_any_order_and_trailing_space() {
        let mut value: serde_json::Value =
            serde_json::from_str(&document(OID, &REQUIRED_KEYS)).unwrap();
        value["RegularCredits"] = serde_json::json!(75_880_730);
        let mut body = serde_json::to_string(&value).unwrap();
        assert!(body.starts_with("{\"Created\""));
        body.push(' ');
        assert_eq!(accept(ADDR, body.as_bytes()).unwrap().last_sync, OID);
    }

    #[test]
    fn missing_key() {
        for missing in REQUIRED_KEYS {
            let kept: Vec<&str> = REQUIRED_KEYS
                .into_iter()
                .filter(|key| *key != missing)
                .collect();
            let body = document(OID, &kept);
            assert_eq!(accept(ADDR, body.as_bytes()), None, "{missing}");
        }
    }

    #[test]
    fn mission_end_response_carries_the_inventory() {
        let inventory = document(OID, &REQUIRED_KEYS);
        let body = serde_json::json!({
            "InventoryJson": inventory,
            "MissionRewards": [{"StoreItem": "/Lotus/StoreItems/Types/Recipes/Components/FormaBlueprint", "ItemCount": 2}],
            "InventoryChanges": {"Recipes": [{"ItemType": "/Lotus/Types/Recipes/Components/FormaBlueprint", "ItemCount": 2}]}
        })
        .to_string();
        let buffer = accept(ADDR, body.as_bytes()).unwrap();
        assert_eq!(buffer.last_sync, OID);
        assert_eq!(buffer.json, inventory);
        assert_eq!(buffer.len, body.len());
        assert_eq!(
            buffer.mission_rewards,
            Some(vec![MissionEndReward {
                store_item: String::from(
                    "/Lotus/StoreItems/Types/Recipes/Components/FormaBlueprint"
                ),
                item_count: 2,
            }])
        );
    }

    #[test]
    fn mission_end_response_without_inventory() {
        let body = serde_json::json!({"InventoryJson": "", "MissionRewards": []}).to_string();
        assert_eq!(accept(ADDR, body.as_bytes()), None);
    }

    #[test]
    fn non_inventory_response() {
        let body = b"{\"WorldSeed\":\"x\",\"Events\":[]}";
        assert_eq!(accept(ADDR, body), None);
    }

    #[test]
    fn truncated_document() {
        let body = document(OID, &REQUIRED_KEYS);
        let cut = &body.as_bytes()[..body.len() - 1];
        assert_eq!(accept(ADDR, cut), None);
    }

    #[test]
    fn missing_head() {
        let body = document(OID, &REQUIRED_KEYS);
        let tail = &body.as_bytes()[20..];
        assert_eq!(accept(ADDR, tail), None);
    }

    #[test]
    fn invalid_utf8() {
        let mut body = document(OID, &REQUIRED_KEYS).into_bytes();
        body[3] = 0xff;
        assert_eq!(accept(ADDR, &body), None);
    }
}
