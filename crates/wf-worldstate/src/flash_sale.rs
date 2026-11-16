use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::Serialize;
use wf_mongo::MongoDate;

use crate::embedded::{STORE_PACKAGES, lookup};
use crate::season::{last_path_segment, split_words};
use crate::window::Window;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RawFlashSale {
    pub type_name: String,
    pub start_date: MongoDate,
    pub end_date: MongoDate,
    #[serde(default)]
    pub discount: u32,
    #[serde(default)]
    pub premium_override: u32,
    #[serde(default)]
    pub regular_override: u32,
    #[serde(default)]
    pub product_expiry_override: Option<MongoDate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MarketSale {
    pub type_name: String,
    pub package_name: Option<String>,
    pub discount_percent: u32,
    pub platinum: u32,
    pub credits: u32,
    pub ends: DateTime<Utc>,
}

impl RawFlashSale {
    fn window(&self) -> Window {
        Window {
            activation: self.start_date,
            expiry: self.end_date,
        }
    }

    fn is_offer(&self) -> bool {
        (self.discount > 0 || self.product_expiry_override.is_some())
            && (self.premium_override > 0 || self.regular_override > 0)
    }
}

fn package_name(type_name: &str) -> Option<String> {
    let (_, path) = type_name.split_once("/StoreItems/Packages/")?;
    Some(lookup(STORE_PACKAGES, &path.to_lowercase()).map_or_else(
        || split_words(last_path_segment(path)),
        |name| (*name).to_owned(),
    ))
}

pub fn market_sales(raw: &[RawFlashSale], now: DateTime<Utc>) -> Vec<MarketSale> {
    raw.iter()
        .filter(|sale| sale.window().contains(now) && sale.is_offer())
        .map(|sale| MarketSale {
            type_name: sale.type_name.clone(),
            package_name: package_name(&sale.type_name),
            discount_percent: sale.discount,
            platinum: sale.premium_override,
            credits: sale.regular_override,
            ends: sale.end_date.datetime(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::fixtures::at;

    fn sale(type_name: &str, discount: u32, platinum: u32, limited: bool) -> RawFlashSale {
        RawFlashSale {
            type_name: type_name.to_owned(),
            start_date: at(0).into(),
            end_date: at(1000).into(),
            discount,
            premium_override: platinum,
            regular_override: 0,
            product_expiry_override: limited.then(|| at(1000).into()),
        }
    }

    #[test]
    fn discounts_and_limited_offers_only() {
        let raw = vec![
            sale("/Lotus/Upgrades/Skins/Necro/NecroDangles", 20, 40, false),
            sale(
                "/Lotus/Types/Items/ShipDecos/Events/WFTankBobbleHead",
                0,
                35,
                true,
            ),
            sale(
                "/Lotus/Types/StoreItems/Packages/TNWMarketBundle",
                0,
                145,
                false,
            ),
            sale(
                "/Lotus/Types/StoreItems/Packages/VeilbreakerSupporterPack",
                0,
                0,
                false,
            ),
        ];
        let sales = market_sales(&raw, at(500));
        let types: Vec<&str> = sales.iter().map(|sale| sale.type_name.as_str()).collect();
        assert_eq!(
            types,
            vec![
                "/Lotus/Upgrades/Skins/Necro/NecroDangles",
                "/Lotus/Types/Items/ShipDecos/Events/WFTankBobbleHead"
            ]
        );
        assert_eq!(sales[0].discount_percent, 20);
        assert_eq!(sales[0].platinum, 40);
        assert_eq!(sales[0].package_name, None);
    }

    #[test]
    fn sales_outside_window() {
        let raw = vec![sale(
            "/Lotus/Upgrades/Skins/Necro/NecroDangles",
            20,
            40,
            false,
        )];
        assert!(market_sales(&raw, at(1000)).is_empty());
    }

    #[test]
    fn package_names() {
        assert_eq!(
            package_name("/Lotus/Types/StoreItems/Packages/CrpIndexTwoArmorPack").as_deref(),
            Some("Quaro Armor Collection")
        );
        assert_eq!(
            package_name(
                "/Lotus/Types/StoreItems/Packages/2030Bundles/Halloween2030SentinelBundle"
            )
            .as_deref(),
            Some("Halloween 2030 Sentinel Bundle")
        );
        assert_eq!(
            package_name("/Lotus/Upgrades/Skins/Necro/NecroDangles"),
            None
        );
    }

    #[test]
    fn embedded_table_sorted() {
        assert!(STORE_PACKAGES.is_sorted_by(|a, b| a.0 < b.0));
    }
}
