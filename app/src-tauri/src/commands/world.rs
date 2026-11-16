use std::collections::HashMap;
use std::sync::atomic::Ordering;

use chrono::{DateTime, Utc};
use serde::Serialize;
use wf_core::{
    ARBITRATION_FORECAST_TRUSTED, ArbitrationSchedule, Catalog, Core, ItemTable, WorldRewardAlert,
};
use wf_data::store_item_to_type;
use wf_worldstate::{
    ArchonHunt, BaroStatus, Circuit, DailyDeal, Fissure, MarketSale, NightwaveSeason, ResetWindow,
    Sortie, Timer, Varzia, daily_reset_window, weekly_reset_window,
};

use super::{Shared, ready};
use crate::error::CommandResult;
use crate::settings::{DiscordMirroredAlert, Settings};
use crate::state::{lock, read};

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct WorldStateView {
    pub fissures: Vec<Fissure>,
    pub baro: Option<BaroStatus>,
    pub baro_manifest: Vec<BaroGroup>,
    pub sortie: Option<Sortie>,
    pub archon_hunt: Option<ArchonHunt>,
    pub timers: Vec<Timer>,
    pub daily_deals: Vec<DarvoDeal>,
    pub market_sales: Vec<MarketOffer>,
    pub circuit: Option<Circuit>,
    pub prime_resurgence: Option<PrimeResurgence>,
    pub nightwave: Option<NightwaveSeason>,
    pub world_reward_alerts: Vec<WorldRewardAlert>,
    pub daily_reset: ResetWindow,
    pub weekly_reset: ResetWindow,
    pub fetched_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ArbitrationForecastStatus {
    Fresh,
    Cached,
    Stale,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ArbitrationDashboard {
    pub status: ArbitrationForecastStatus,
    pub fetched_at: Option<DateTime<Utc>>,
    pub schedule: Option<ArbitrationSchedule>,
    pub notifications_available: bool,
}

fn notifications_available(settings: &Settings) -> bool {
    let webhook_set = settings.discord.webhook().is_some();
    settings.toasts.toasts_enabled
        || settings.notifications.windows_notifications_enabled
        || (settings
            .discord
            .mirrored
            .mirrors(DiscordMirroredAlert::Arbitration)
            && webhook_set)
}

impl ArbitrationDashboard {
    pub fn build(
        core: &Core,
        settings: &Settings,
        refresh_failed: bool,
        now: DateTime<Utc>,
    ) -> Self {
        let schedule = core.arbitration_schedule(now);
        let fetched_at = core.arbitration_forecast_fetched_at();
        let status = match fetched_at.filter(|_| schedule.is_some()) {
            None => ArbitrationForecastStatus::Unavailable,
            Some(at) if now.signed_duration_since(at) > ARBITRATION_FORECAST_TRUSTED => {
                ArbitrationForecastStatus::Stale
            }
            Some(_) if refresh_failed => ArbitrationForecastStatus::Cached,
            Some(_) => ArbitrationForecastStatus::Fresh,
        };
        Self {
            status,
            fetched_at,
            schedule,
            notifications_available: notifications_available(settings),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct InvasionWeaponPart {
    pub unique_name: String,
    pub name: String,
    pub image_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct BaroOffer {
    pub name: String,
    pub ducats: Option<u32>,
    pub credits: Option<u32>,
    pub owned: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct BaroGroup {
    pub name: &'static str,
    pub items: Vec<BaroOffer>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct DarvoDeal {
    pub name: String,
    pub discount_percent: u32,
    pub original_price: u32,
    pub sale_price: u32,
    pub amount_total: u32,
    pub amount_sold: u32,
    pub expiry: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct MarketOffer {
    pub name: String,
    pub discount_percent: u32,
    pub platinum: u32,
    pub credits: u32,
    pub ends: DateTime<Utc>,
    pub owned: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ResurgenceOffering {
    pub name: String,
    pub regal_aya: u32,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct PrimeResurgence {
    pub node_id: String,
    pub node_name: Option<&'static str>,
    pub ends: DateTime<Utc>,
    pub offerings: Vec<ResurgenceOffering>,
}

fn named_circuit(catalog: &Catalog, circuit: &Circuit) -> Circuit {
    let by_squashed_name: HashMap<String, &str> = catalog
        .items()
        .map(|item| {
            let squashed = item.name.replace(' ', "").replace('&', "And");
            (squashed, item.name.as_str())
        })
        .collect();
    let named = |choices: &[String]| -> Vec<String> {
        choices
            .iter()
            .map(|choice| match by_squashed_name.get(choice) {
                Some(name) => (*name).to_owned(),
                None => choice.clone(),
            })
            .collect()
    };
    Circuit {
        rotates: circuit.rotates,
        normal: named(&circuit.normal),
        hard: named(&circuit.hard),
    }
}

fn baro_group_of(item: Option<&wf_data::Item>) -> &'static str {
    match item.map(|item| item.category.as_str()) {
        Some("Mods") => "Mods",
        Some(
            "Warframes" | "Archwing" | "Primary" | "Secondary" | "Melee" | "Arch-Gun"
            | "Arch-Melee",
        ) => "Weapons",
        Some("Skins" | "Sigils") => "Cosmetics",
        _ => "Others",
    }
}

fn baro_offer_name(items: &ItemTable, item_type: &str) -> String {
    if let Some(record) = items.lookup(item_type) {
        return record.name.clone();
    }

    if item_type.ends_with("/MummyQuestKeyBlueprint") {
        return "Sands of Inaros Blueprint".to_owned();
    }

    if item_type.contains("/AvatarImages/") {
        return "Icon".to_owned();
    }

    if item_type.contains("/Boosters/") {
        return "Booster".to_owned();
    }

    items.name(item_type)
}

fn baro_manifest(core: &Core, baro: Option<&BaroStatus>) -> Vec<BaroGroup> {
    let Some(BaroStatus::Present { items, .. }) = baro else {
        return Vec::new();
    };

    let mut groups: Vec<BaroGroup> = ["Mods", "Weapons", "Cosmetics", "Others"]
        .into_iter()
        .map(|name| BaroGroup {
            name,
            items: Vec::new(),
        })
        .collect();
    for offer in items {
        let item_type = store_item_to_type(&offer.item_type);
        if item_type.ends_with("/BaroTreasureBox") {
            continue;
        }

        let group = baro_group_of(core.catalog().item(&item_type));
        if let Some(target) = groups.iter_mut().find(|target| target.name == group) {
            target.items.push(BaroOffer {
                name: baro_offer_name(core.items(), &item_type),
                ducats: offer.ducats,
                credits: offer.credits,
                owned: core.inventory().map(|inventory| inventory.owns(&item_type)),
            });
        }
    }

    for group in &mut groups {
        group.items.sort_by(|a, b| a.name.cmp(&b.name));
    }
    groups.retain(|group| !group.items.is_empty());
    groups
}

fn export_item_name(core: &Core, item_type: &str) -> Option<String> {
    core.catalog().item(&store_item_to_type(item_type))?;

    Some(core.items().name(item_type))
}

fn darvo_deals(items: &ItemTable, deals: Vec<DailyDeal>) -> Vec<DarvoDeal> {
    deals
        .into_iter()
        .map(|deal| DarvoDeal {
            name: items.name(&deal.store_item),
            discount_percent: deal.discount_percent,
            original_price: deal.original_price,
            sale_price: deal.sale_price,
            amount_total: deal.amount_total,
            amount_sold: deal.amount_sold,
            expiry: deal.expiry,
        })
        .collect()
}

fn market_offers(core: &Core, sales: Vec<MarketSale>) -> Vec<MarketOffer> {
    let mut offers: Vec<MarketOffer> = sales
        .into_iter()
        .filter_map(|sale| {
            let owned = match sale.package_name {
                Some(_) => None,
                None => core
                    .inventory()
                    .map(|inventory| inventory.owns(&sale.type_name)),
            };
            let name = sale
                .package_name
                .or_else(|| export_item_name(core, &sale.type_name))?;
            Some(MarketOffer {
                name,
                discount_percent: sale.discount_percent,
                platinum: sale.platinum,
                credits: sale.credits,
                ends: sale.ends,
                owned,
            })
        })
        .collect();
    offers.sort_by(|a, b| a.ends.cmp(&b.ends).then_with(|| a.name.cmp(&b.name)));
    offers
}

fn prime_resurgence(core: &Core, varzia: Varzia) -> PrimeResurgence {
    let mut offerings: Vec<ResurgenceOffering> = varzia
        .items
        .iter()
        .filter_map(|item| {
            Some(ResurgenceOffering {
                name: export_item_name(core, &item.item_type)?,
                regal_aya: item.regal_aya,
            })
        })
        .collect();
    offerings.sort_by(|a, b| a.name.cmp(&b.name));
    PrimeResurgence {
        node_id: varzia.node_id,
        node_name: varzia.node_name,
        ends: varzia.expiry,
        offerings,
    }
}

impl WorldStateView {
    pub fn build(core: &Core, now: DateTime<Utc>, fetched_at: Option<DateTime<Utc>>) -> Self {
        let Some(world) = core.world() else {
            return Self {
                fissures: Vec::new(),
                baro: None,
                baro_manifest: Vec::new(),
                sortie: None,
                archon_hunt: None,
                timers: Vec::new(),
                daily_deals: Vec::new(),
                market_sales: Vec::new(),
                circuit: None,
                prime_resurgence: None,
                nightwave: None,
                world_reward_alerts: Vec::new(),
                daily_reset: daily_reset_window(now),
                weekly_reset: weekly_reset_window(now),
                fetched_at,
            };
        };

        let baro = world.baro(now);
        Self {
            fissures: world.fissures(now),
            baro_manifest: baro_manifest(core, baro.as_ref()),
            baro,
            sortie: world.sortie(now),
            archon_hunt: world.archon_hunt(now),
            timers: world.timers(now),
            daily_deals: darvo_deals(core.items(), world.daily_deals(now)),
            market_sales: market_offers(core, world.market_sales(now)),
            circuit: world
                .circuit(now)
                .map(|circuit| named_circuit(core.catalog(), &circuit)),
            prime_resurgence: world
                .varzia(now)
                .map(|varzia| prime_resurgence(core, varzia)),
            nightwave: world.season(now, |challenge| {
                core.catalog().nightwave_act_text(challenge)
            }),
            world_reward_alerts: core.world_reward_alerts(now),
            daily_reset: daily_reset_window(now),
            weekly_reset: weekly_reset_window(now),
            fetched_at,
        }
    }
}

#[tauri::command]
pub async fn invasion_weapon_parts(state: Shared<'_>) -> CommandResult<Vec<InvasionWeaponPart>> {
    let state = ready(&state).await?;
    let parts = lock(&state.core)
        .invasion_weapon_parts()
        .into_iter()
        .map(|record| InvasionWeaponPart {
            unique_name: record.unique_name.clone(),
            name: record.name.clone(),
            image_name: record.image_name.clone(),
        })
        .collect();
    Ok(parts)
}

#[tauri::command]
pub async fn arbitrations(state: Shared<'_>) -> CommandResult<ArbitrationDashboard> {
    let state = ready(&state).await?;
    let settings = read(&state.settings).clone();
    let refresh_failed = state.arbitration_refresh_failed.load(Ordering::Relaxed);
    Ok(ArbitrationDashboard::build(
        &lock(&state.core),
        &settings,
        refresh_failed,
        Utc::now(),
    ))
}

#[tauri::command]
pub async fn worldstate(state: Shared<'_>) -> CommandResult<WorldStateView> {
    let state = ready(&state).await?;
    let fetched_at = read(&state.status).world_state_at;
    Ok(WorldStateView::build(
        &lock(&state.core),
        Utc::now(),
        fetched_at,
    ))
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeDelta};
    use wf_core::Catalog;
    use wf_worldstate::{ArbitrationForecast, ArbitrationTiers, WorldState};

    use super::*;
    use crate::settings::{NotificationSettings, ToastSettings};
    use crate::test_support::{ARBITRATIONS, core_with, empty_catalog};

    const FIXTURE: &str = include_str!("../../../../fixtures/worldState.json");
    const INVENTORY: &str = include_str!("../../../../fixtures/inventory.json");
    const FIXTURE_NOW_MS: i64 = 1_788_807_069_000;
    const ITEMS: &str = r#"[{
        "uniqueName": "/Lotus/Types/Items/Research/BioComponent",
        "name": "Mutagen Mass",
        "category": "Misc",
        "type": "Resource",
        "tradable": false
    }]"#;

    fn core_at(catalog: Catalog, world: &str, now: DateTime<Utc>) -> Core {
        let mut core = core_with(catalog);
        core.ingest_world_state(WorldState::parse(world).unwrap(), now);
        core
    }

    fn view() -> WorldStateView {
        let catalog = Catalog::from_json(ITEMS, "[]", "[]").unwrap();
        let now = DateTime::from_timestamp_millis(FIXTURE_NOW_MS).unwrap();
        WorldStateView::build(&core_at(catalog, FIXTURE, now), now, Some(now))
    }

    #[test]
    fn fixture_view_panels() {
        let view = view();
        assert!(!view.fissures.is_empty());
        assert!(!view.timers.is_empty());
        assert!(view.baro.is_some());
        assert!(view.sortie.is_some());
        assert!(view.archon_hunt.is_some());
        assert!(view.circuit.is_some());
        assert!(view.prime_resurgence.is_some());
        assert!(view.fetched_at.is_some());
    }

    #[test]
    fn storms_and_named_sortie_in_view() {
        let view = view();
        assert_eq!(
            view.fissures
                .iter()
                .filter(|fissure| fissure.is_storm)
                .count(),
            6
        );
        assert_eq!(view.sortie.unwrap().boss_name, "Kela De Thaym");
        assert_eq!(view.archon_hunt.unwrap().faction, Some("Narmer"));
    }

    #[test]
    fn darvo_deal() {
        let view = view();
        assert_eq!(view.daily_deals.len(), 1);
        let deal = &view.daily_deals[0];
        assert_eq!(deal.name, "Mutagen Mass");
        assert_eq!(deal.sale_price, 8);
        assert_eq!(deal.original_price, 10);
        assert_eq!(deal.discount_percent, 20);
        assert_eq!(deal.amount_total, 165);
        assert_eq!(deal.expiry.timestamp_millis(), 1_788_814_800_000);
    }

    #[test]
    fn market_sale_offers() {
        const CATALOG: &str = r#"[
            {"uniqueName":"/Lotus/Upgrades/Skins/Sigils/BossSigilJackal","name":"Jackal Sigil",
             "category":"Sigils","type":"Sigil","tradable":false},
            {"uniqueName":"/Lotus/Upgrades/Skins/Necro/NecroDangles","name":"Mortos Binds",
             "category":"Skins","type":"Skin","tradable":false}
        ]"#;
        const SALES: &str = r#"{"FlashSales":[
            {"TypeName":"/Lotus/Upgrades/Skins/Necro/NecroDangles","Discount":20,"PremiumOverride":40,
             "StartDate":{"$date":{"$numberLong":"1788800000000"}},
             "EndDate":{"$date":{"$numberLong":"1788900000000"}}},
            {"TypeName":"/Lotus/Upgrades/Skins/Sigils/BossSigilJackal","RegularOverride":1,
             "ProductExpiryOverride":{"$date":{"$numberLong":"1788880000000"}},
             "StartDate":{"$date":{"$numberLong":"1788800000000"}},
             "EndDate":{"$date":{"$numberLong":"1788880000000"}}},
            {"TypeName":"/Lotus/Types/StoreItems/Packages/CrpIndexTwoArmorPack","Discount":20,"PremiumOverride":80,
             "StartDate":{"$date":{"$numberLong":"1788800000000"}},
             "EndDate":{"$date":{"$numberLong":"1788890000000"}}},
            {"TypeName":"/Lotus/Upgrades/Skins/Unlisted/UnlistedSkin","Discount":20,"PremiumOverride":60,
             "StartDate":{"$date":{"$numberLong":"1788800000000"}},
             "EndDate":{"$date":{"$numberLong":"1788900000000"}}}
        ]}"#;

        let catalog = Catalog::from_json(CATALOG, "[]", "[]").unwrap();
        let now = DateTime::from_timestamp_millis(1_788_850_000_000).unwrap();
        let mut core = core_at(catalog, SALES, now);
        core.ingest_inventory(INVENTORY, now).unwrap();
        let view = WorldStateView::build(&core, now, None);
        let offers: Vec<(&str, u32, Option<bool>)> = view
            .market_sales
            .iter()
            .map(|offer| (offer.name.as_str(), offer.discount_percent, offer.owned))
            .collect();
        assert_eq!(
            offers,
            vec![
                ("Jackal Sigil", 0, Some(true)),
                ("Quaro Armor Collection", 20, None),
                ("Mortos Binds", 20, Some(false)),
            ]
        );
        assert_eq!(view.market_sales[0].credits, 1);
        assert_eq!(view.market_sales[2].platinum, 40);
    }

    #[test]
    fn fixture_market_sales() {
        let named: Vec<String> = view()
            .market_sales
            .into_iter()
            .map(|offer| offer.name)
            .collect();
        assert_eq!(
            named,
            vec![
                "Halloween 2023 Sentinel Bundle",
                "Dog Days Noggle Pack",
                "Quaro Armor Collection"
            ]
        );
        let world = WorldState::parse(FIXTURE).unwrap();
        let now = DateTime::from_timestamp_millis(FIXTURE_NOW_MS).unwrap();
        assert_eq!(world.market_sales(now).len(), 17);
    }

    #[test]
    fn baro_offers() {
        const CATALOG: &str = r#"[
            {"uniqueName":"/Lotus/Upgrades/Mods/Rifle/PrimeAmmoMutation","name":"Rifle Ammo Mutation",
             "category":"Mods","type":"Mod","tradable":true},
            {"uniqueName":"/Lotus/Weapons/Corpus/LongGuns/Prisma/PrismaGrakata","name":"Prisma Grakata",
             "category":"Primary","type":"Rifle","tradable":true},
            {"uniqueName":"/Lotus/Upgrades/Skins/Sigils/PrismaSigil","name":"Prisma Sigil",
             "category":"Skins","type":"Sigil","tradable":true},
            {"uniqueName":"/Lotus/Upgrades/Skins/Sigils/RhinoDeluxeSigil","name":"Rhino Palatine Sigil",
             "category":"Sigils","type":"Sigil","tradable":false},
            {"uniqueName":"/Lotus/Types/Items/MiscItems/PrimeBucks","name":"Ducats",
             "category":"Misc","type":"Misc","tradable":false}
        ]"#;
        const PRESENT: &str = r#"{"VoidTraders":[{
            "Activation":{"$date":{"$numberLong":"1788800000000"}},
            "Expiry":{"$date":{"$numberLong":"1788900000000"}},
            "Character":"Baro'Ki Teel",
            "Node":"TradeHUB1",
            "Manifest":[
              {"ItemType":"/Lotus/StoreItems/Weapons/Corpus/LongGuns/Prisma/PrismaGrakata","PrimePrice":550,"RegularPrice":250000},
              {"ItemType":"/Lotus/StoreItems/Upgrades/Mods/Rifle/PrimeAmmoMutation","PrimePrice":300,"RegularPrice":150000},
              {"ItemType":"/Lotus/StoreItems/Upgrades/Skins/Sigils/PrismaSigil","PrimePrice":200,"RegularPrice":100000},
              {"ItemType":"/Lotus/StoreItems/Types/Items/MiscItems/PrimeBucks","PrimePrice":1,"RegularPrice":1},
              {"ItemType":"/Lotus/StoreItems/Types/Boosters/AffinityBooster","PrimePrice":400,"RegularPrice":200000},
              {"ItemType":"/Lotus/StoreItems/Types/Game/Projections/T4VoidProjectionEBronze","PrimePrice":125,"RegularPrice":55000},
              {"ItemType":"/Lotus/StoreItems/Types/BoosterPacks/BaroTreasureBox","PrimePrice":0,"RegularPrice":50000},
              {"ItemType":"/Lotus/StoreItems/Types/Keys/MummyQuestKeyBlueprint","PrimePrice":100,"RegularPrice":25000},
              {"ItemType":"/Lotus/StoreItems/Upgrades/Skins/Sigils/RhinoDeluxeSigil","PrimePrice":45,"RegularPrice":55000}
            ]}]}"#;
        const RELICS: &str = r#"[{"uniqueName":"/Lotus/Types/Game/Projections/T4VoidProjectionEBronze",
            "name":"Axi A1 Intact","category":"Relics","type":"Relic","tradable":true,"rewards":[]}]"#;

        let catalog = Catalog::from_json(CATALOG, RELICS, "[]").unwrap();
        let now = DateTime::from_timestamp_millis(1_788_850_000_000).unwrap();
        let view = WorldStateView::build(&core_at(catalog, PRESENT, now), now, Some(now));
        let groups: Vec<(&str, Vec<&str>)> = view
            .baro_manifest
            .iter()
            .map(|group| {
                (
                    group.name,
                    group
                        .items
                        .iter()
                        .map(|offer| offer.name.as_str())
                        .collect(),
                )
            })
            .collect();
        assert_eq!(
            groups,
            vec![
                ("Mods", vec!["Rifle Ammo Mutation"]),
                ("Weapons", vec!["Prisma Grakata"]),
                ("Cosmetics", vec!["Prisma Sigil", "Rhino Palatine Sigil"]),
                (
                    "Others",
                    vec![
                        "Axi A1 Relic (Intact)",
                        "Booster",
                        "Ducats",
                        "Sands of Inaros Blueprint"
                    ]
                ),
            ]
        );
        assert_eq!(view.baro_manifest[1].items[0].ducats, Some(550));
        assert_eq!(view.baro_manifest[1].items[0].credits, Some(250_000));
        assert_eq!(view.baro_manifest[1].items[0].owned, None);
    }

    #[test]
    fn baro_offers_owned() {
        const PRESENT: &str = r#"{"VoidTraders":[{
            "Activation":{"$date":{"$numberLong":"1788800000000"}},
            "Expiry":{"$date":{"$numberLong":"1788900000000"}},
            "Character":"Baro'Ki Teel",
            "Node":"TradeHUB1",
            "Manifest":[
              {"ItemType":"/Lotus/StoreItems/Upgrades/Skins/Sigils/BossSigilJackal","PrimePrice":50,"RegularPrice":50000},
              {"ItemType":"/Lotus/StoreItems/Upgrades/Skins/Sigils/PrismaSigil","PrimePrice":200,"RegularPrice":100000}
            ]}]}"#;

        let now = DateTime::from_timestamp_millis(1_788_850_000_000).unwrap();
        let mut core = core_at(empty_catalog(), PRESENT, now);
        core.ingest_inventory(INVENTORY, now).unwrap();
        let view = WorldStateView::build(&core, now, None);
        let owned: Vec<Option<bool>> = view.baro_manifest[0]
            .items
            .iter()
            .map(|offer| offer.owned)
            .collect();
        assert_eq!(owned, vec![Some(true), Some(false)]);
    }

    #[test]
    fn baro_away() {
        assert!(view().baro_manifest.is_empty());
    }

    #[test]
    fn nightwave_season() {
        let nightwave = view().nightwave.unwrap();
        assert_eq!(nightwave.season, 18);
        assert_eq!(nightwave.name, "Intermission 16");
        assert_eq!(nightwave.challenges.len(), 10);
    }

    #[test]
    fn arbitration_dashboard_status() {
        let forecast = ArbitrationForecast::parse(ARBITRATIONS).unwrap();
        let fetched_at = DateTime::from_timestamp_millis(1_727_884_800_000).unwrap();
        let settings = Settings::default();
        let mut core = core_with(empty_catalog());
        let unavailable = ArbitrationDashboard::build(&core, &settings, false, fetched_at);
        assert_eq!(unavailable.status, ArbitrationForecastStatus::Unavailable);
        assert!(unavailable.schedule.is_none());
        assert!(unavailable.notifications_available);

        core.set_arbitration_forecast(forecast, ArbitrationTiers::default(), fetched_at);
        let fresh = ArbitrationDashboard::build(&core, &settings, false, fetched_at);
        assert_eq!(fresh.status, ArbitrationForecastStatus::Fresh);
        assert_eq!(fresh.fetched_at, Some(fetched_at));
        assert_eq!(fresh.schedule.unwrap().slots.len(), 48);
        let cached = ArbitrationDashboard::build(&core, &settings, true, fetched_at);
        assert_eq!(cached.status, ArbitrationForecastStatus::Cached);
        let day_later = fetched_at + TimeDelta::hours(25);
        let stale = ArbitrationDashboard::build(&core, &settings, false, day_later);
        assert_eq!(stale.status, ArbitrationForecastStatus::Stale);
        let after_the_end = fetched_at + TimeDelta::hours(48);
        let ended = ArbitrationDashboard::build(&core, &settings, false, after_the_end);
        assert_eq!(ended.status, ArbitrationForecastStatus::Unavailable);

        let silent = Settings {
            toasts: ToastSettings {
                toasts_enabled: false,
                ..ToastSettings::default()
            },
            notifications: NotificationSettings {
                windows_notifications_enabled: false,
                ..NotificationSettings::default()
            },
            ..Settings::default()
        };
        let muted = ArbitrationDashboard::build(&core, &silent, false, fetched_at);
        assert!(!muted.notifications_available);
    }

    #[test]
    fn empty_view_before_fetch() {
        let catalog = Catalog::from_json(ITEMS, "[]", "[]").unwrap();
        let now = DateTime::from_timestamp_millis(FIXTURE_NOW_MS).unwrap();
        let view = WorldStateView::build(&core_with(catalog), now, None);
        assert!(view.fissures.is_empty());
        assert!(view.timers.is_empty());
        assert!(view.daily_deals.is_empty());
        assert!(view.market_sales.is_empty());
        assert!(view.nightwave.is_none());
        assert!(view.fetched_at.is_none());
        assert_eq!(
            view.daily_reset.expiry.timestamp_millis(),
            1_788_825_600_000
        );
        assert_eq!(
            view.weekly_reset.expiry.timestamp_millis(),
            1_789_344_000_000
        );
    }
}
