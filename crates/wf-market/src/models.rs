use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Pc,
    Xbox,
    Ps4,
    Switch,
    Mobile,
}

impl Platform {
    pub fn as_wire_str(self) -> &'static str {
        match self {
            Platform::Pc => "pc",
            Platform::Xbox => "xbox",
            Platform::Ps4 => "ps4",
            Platform::Switch => "switch",
            Platform::Mobile => "mobile",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Legendary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum Polarity {
    Madurai,
    Vazarin,
    Naramon,
    Unairu,
    Zenurik,
    Penjaga,
    Umbra,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    Offline,
    Online,
    Ingame,
    Invisible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct User {
    pub id: String,
    #[serde(rename = "ingameName")]
    pub ingame_name: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub status: Option<UserStatus>,
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPrivate {
    pub id: String,
    pub tier: String,
    #[serde(rename = "ingameName")]
    pub ingame_name: String,
    pub slug: String,
    #[serde(rename = "masteryRank")]
    pub mastery_rank: u32,
    pub verification: bool,
}

pub struct Session {
    pub user: UserPrivate,
    pub rotated_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemLocalization {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub icon: String,
    pub thumb: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub slug: String,
    #[serde(rename = "gameRef")]
    pub game_ref: String,
    pub tags: Vec<String>,
    #[serde(rename = "maxRank", skip_serializing_if = "Option::is_none")]
    pub max_rank: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtypes: Option<Vec<String>>,
    #[serde(rename = "maxAmberStars", skip_serializing_if = "Option::is_none")]
    pub max_amber_stars: Option<u32>,
    #[serde(rename = "maxCyanStars", skip_serializing_if = "Option::is_none")]
    pub max_cyan_stars: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tradable: Option<bool>,
    #[serde(rename = "bulkTradable", skip_serializing_if = "Option::is_none")]
    pub bulk_tradable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vaulted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ducats: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rarity: Option<Rarity>,
    pub i18n: HashMap<String, ItemLocalization>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct Order {
    pub id: String,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub platinum: u32,
    pub quantity: u32,
    #[serde(rename = "perTrade", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub per_trade: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub subtype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub rank: Option<u32>,
    #[serde(rename = "amberStars", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub amber_stars: Option<u32>,
    #[serde(rename = "cyanStars", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub cyan_stars: Option<u32>,
    pub visible: bool,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    #[serde(rename = "itemId")]
    pub item_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrdersGroupUpdate {
    pub updated: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiEnvelope<T> {
    pub data: T,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateOrderRequest {
    #[serde(rename = "itemId")]
    pub item_id: String,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub platinum: u32,
    pub quantity: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(rename = "perTrade", skip_serializing_if = "Option::is_none")]
    pub per_trade: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    #[serde(rename = "amberStars", skip_serializing_if = "Option::is_none")]
    pub amber_stars: Option<u32>,
    #[serde(rename = "cyanStars", skip_serializing_if = "Option::is_none")]
    pub cyan_stars: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateOrderRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platinum: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloseOrderRequest {
    pub quantity: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetGroupVisibilityRequest {
    pub visible: bool,
}

#[derive(Serialize)]
pub struct SignInRequest {
    pub email: String,
    pub password: String,
    pub auth_type: String,
}

impl SignInRequest {
    pub fn new(email: String, password: String) -> Self {
        Self {
            email,
            password,
            auth_type: "header".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct V1Payload<T> {
    pub payload: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chat {
    pub unread_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct RivenAttributeInstance {
    pub value: f64,
    pub positive: bool,
    pub url_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct AuctionItem {
    pub attributes: Vec<RivenAttributeInstance>,
    pub polarity: Polarity,
    pub mod_rank: u32,
    pub name: String,
    pub re_rolls: u32,
    pub mastery_level: u32,
    pub weapon_url_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors the warframe.market auction object"
)]
pub struct Auction {
    pub id: String,
    pub buyout_price: Option<u32>,
    pub starting_price: u32,
    pub minimal_reputation: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub note: Option<String>,
    pub item: AuctionItem,
    pub private: bool,
    pub visible: bool,
    pub closed: bool,
    pub is_direct_sell: bool,
    pub created: String,
    pub updated: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateAuctionRequest {
    pub item: CreateAuctionItem,
    pub buyout_price: Option<u32>,
    pub starting_price: u32,
    pub minimal_reputation: Option<u32>,
    pub note: String,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename = "riven")]
pub struct CreateAuctionItem {
    pub attributes: Vec<RivenAttributeInstance>,
    pub polarity: Polarity,
    pub mod_rank: u32,
    pub name: String,
    pub re_rolls: u32,
    pub mastery_level: u32,
    pub weapon_url_name: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateAuctionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyout_price: Option<Option<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starting_price: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimal_reputation: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetAuctionsVisibilityRequest {
    pub visibility: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PriceTable {
    pub updated_at: i64,
    pub count: usize,
    pub items: HashMap<String, PriceEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct PriceEntry {
    pub sell_r0: Option<u32>,
    pub sell_max: Option<u32>,
    pub buy_r0: Option<u32>,
    pub volume_48h: Option<u32>,
    pub median_48h: Option<f64>,
    pub volume_7d: Option<u32>,
    pub median_7d: Option<f64>,
    pub volume_30d: Option<u32>,
    pub median_30d: Option<f64>,
    pub volume_90d: Option<u32>,
    pub median_90d: Option<f64>,
    pub volume_prev_48h: Option<u32>,
    pub median_prev_48h: Option<f64>,
    pub volume_prev_7d: Option<u32>,
    pub median_prev_7d: Option<f64>,
    pub volume_prev_30d: Option<u32>,
    pub median_prev_30d: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatRef {
    pub abbr: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoodRollAlternative {
    pub mandatory: Vec<StatRef>,
    pub optional: Vec<StatRef>,
    pub optional_needed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoodRoll {
    pub weapon: String,
    pub weapon_class: String,
    pub alternatives: Vec<GoodRollAlternative>,
    pub accepted_bad: Vec<StatRef>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RivenWeapon {
    pub game_ref: String,
    pub slug: String,
    pub name: String,
    pub group: String,
    pub riven_type: String,
    pub mod_type: String,
    pub mastery_rank: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RivenData {
    pub updated_at: i64,
    pub weapons_updated_at: i64,
    pub good_rolls_updated_at: i64,
    pub attribution: String,
    pub weapons: Vec<RivenWeapon>,
    #[serde(default)]
    pub attributes: Vec<RivenAttribute>,
    pub good_rolls: HashMap<String, GoodRoll>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RivenAttribute {
    pub slug: String,
    pub game_ref: String,
    pub name: String,
    pub prefix: String,
    pub suffix: String,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RivenAuctionAttribute {
    pub url_name: String,
    pub value: f64,
    pub positive: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RivenAuction {
    pub id: String,
    pub buyout_price: Option<u32>,
    pub starting_price: u32,
    pub is_direct_sell: bool,
    pub name: String,
    pub polarity: Polarity,
    pub mod_rank: u32,
    pub re_rolls: u32,
    pub mastery_level: u32,
    pub attributes: Vec<RivenAuctionAttribute>,
    pub updated: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeaponAuctions {
    pub slug: String,
    pub updated_at: i64,
    pub truncated: bool,
    pub auctions: Vec<RivenAuction>,
}
