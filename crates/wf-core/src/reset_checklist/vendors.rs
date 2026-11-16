use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::Serialize;
use wf_inventory::{Inventory, VendorOfferPurchase};
use wf_worldstate::game_week_count;

use crate::reset_checklist::cadence::ResetCadence;
use crate::reset_checklist::context::ResetChecklistContext;
use crate::reset_checklist::trackers::tally;
use crate::reset_checklist::unlocks::{self, Quest, ResetChecklistRequirement, SyndicateRank};
use crate::resources::archon_shard_type;

const AMBER_SHARD_WEEK: i64 = 665;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ArchonShardColour {
    Azure,
    Amber,
    Crimson,
}

impl ArchonShardColour {
    fn of_week(now: DateTime<Utc>) -> Self {
        match (game_week_count(now) - AMBER_SHARD_WEEK).rem_euclid(3) {
            0 => Self::Amber,
            1 => Self::Crimson,
            _ => Self::Azure,
        }
    }

    fn item_type(self) -> String {
        archon_shard_type(match self {
            Self::Azure => "Boreal",
            Self::Amber => "Nira",
            Self::Crimson => "Amar",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum VendorKuvaCertainty {
    Confirmed,
    Estimated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VendorWeeklyOffer {
    Kuva {
        amount: u32,
        bought: u32,
        limit: u32,
        certainty: VendorKuvaCertainty,
        locked: bool,
    },
    ArchonShard {
        colour: ArchonShardColour,
        image_name: Option<String>,
        bought: u32,
        limit: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ResetChecklistVendor {
    pub name: &'static str,
    pub place: Option<&'static str>,
    pub locked: bool,
    pub offers_bought: u32,
    pub weekly_offer: Option<VendorWeeklyOffer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct KuvaWeeklySummary {
    pub confirmed: u32,
    pub estimated: u32,
    pub available: u32,
}

#[derive(Clone, Copy)]
pub(super) enum VendorNamedOffer {
    Kuva {
        amount: u32,
        limit: u32,
        certainty: VendorKuvaCertainty,
        requirements: &'static [ResetChecklistRequirement],
    },
    ArchonShard {
        limit: u32,
    },
}

pub(super) struct ResetChecklistVendorDefinition {
    pub(super) manifest: &'static str,
    requirements: &'static [ResetChecklistRequirement],
    name: &'static str,
    place: Option<&'static str>,
    pub(super) offer: Option<VendorNamedOffer>,
}

pub(super) const VENDORS: [ResetChecklistVendorDefinition; 11] = [
    ResetChecklistVendorDefinition {
        manifest: "Hubs/TeshinHardModeVendorManifest",
        requirements: &[ResetChecklistRequirement::SteelPathMissionCompleted],
        name: "Teshin",
        place: Some("The Steel Path Honors"),
        offer: Some(VendorNamedOffer::Kuva {
            amount: 10_000,
            limit: 25,
            certainty: VendorKuvaCertainty::Estimated,
            requirements: &[],
        }),
    },
    ResetChecklistVendorDefinition {
        manifest: "Hubs/IronwakeDondaVendorManifest",
        requirements: &[ResetChecklistRequirement::Quest(Quest::ChainsOfHarrow)],
        name: "Palladino",
        place: Some("Iron Wake"),
        offer: Some(VendorNamedOffer::Kuva {
            amount: 35_000,
            limit: 1,
            certainty: VendorKuvaCertainty::Estimated,
            requirements: &[],
        }),
    },
    ResetChecklistVendorDefinition {
        manifest: "Zariman/ArchimedeanVendorManifest",
        requirements: &[ResetChecklistRequirement::Quest(Quest::AngelsOfTheZariman)],
        name: "Archimedean Yonta",
        place: Some("Chrysalith"),
        offer: Some(VendorNamedOffer::Kuva {
            amount: 35_000,
            limit: 1,
            certainty: VendorKuvaCertainty::Confirmed,
            requirements: &[],
        }),
    },
    ResetChecklistVendorDefinition {
        manifest: "EntratiLabs/EntratiLabVendorManifest",
        requirements: &[ResetChecklistRequirement::Quest(Quest::WhispersInTheWalls)],
        name: "Bird 3",
        place: Some("Sanctum Anatomica"),
        offer: Some(VendorNamedOffer::ArchonShard { limit: 1 }),
    },
    ResetChecklistVendorDefinition {
        manifest: "Duviri/AcrithisVendorManifest",
        requirements: &[ResetChecklistRequirement::Quest(Quest::DuviriParadox)],
        name: "Acrithis",
        place: Some("Dormizone"),
        offer: None,
    },
    ResetChecklistVendorDefinition {
        manifest: "Events/DuviriMurmurPostEventVendorManifest",
        requirements: &[
            ResetChecklistRequirement::Quest(Quest::DuviriParadox),
            ResetChecklistRequirement::Quest(Quest::TheHex),
        ],
        name: "Acrithis",
        place: Some("Dormizone (Scuttler Husks)"),
        offer: Some(VendorNamedOffer::Kuva {
            amount: 14_000,
            limit: 3,
            certainty: VendorKuvaCertainty::Confirmed,
            requirements: &[],
        }),
    },
    ResetChecklistVendorDefinition {
        manifest: "Solaris/NightcapVendorManifest",
        requirements: &[],
        name: "Nightcap",
        place: Some("Fortuna"),
        offer: Some(VendorNamedOffer::Kuva {
            amount: 14_000,
            limit: 3,
            certainty: VendorKuvaCertainty::Estimated,
            requirements: &[ResetChecklistRequirement::SyndicateRank(
                SyndicateRank::NIGHTCAP_3,
            )],
        }),
    },
    ResetChecklistVendorDefinition {
        manifest: "TheHex/Temple1999VendorManifest",
        requirements: &[ResetChecklistRequirement::Quest(Quest::TheHex)],
        name: "Flare",
        place: Some("Hollvania"),
        offer: Some(VendorNamedOffer::Kuva {
            amount: 14_000,
            limit: 3,
            certainty: VendorKuvaCertainty::Confirmed,
            requirements: &[ResetChecklistRequirement::SyndicateRank(
                SyndicateRank::HEX_4,
            )],
        }),
    },
    ResetChecklistVendorDefinition {
        manifest: "Hubs/AspirantZorbaVendorManifest",
        requirements: &[ResetChecklistRequirement::Quest(Quest::ChainsOfHarrow)],
        name: "Aspirant Zorba",
        place: Some("Relays"),
        offer: Some(VendorNamedOffer::Kuva {
            amount: 6_000,
            limit: 7,
            certainty: VendorKuvaCertainty::Confirmed,
            requirements: &[],
        }),
    },
    ResetChecklistVendorDefinition {
        manifest: "Hubs/MelicaVendorManifest",
        requirements: &[
            ResetChecklistRequirement::Quest(Quest::AngelsOfTheZariman),
            ResetChecklistRequirement::Quest(Quest::WhispersInTheWalls),
        ],
        name: "Cephalon Melica",
        place: Some("Yuvan Peak"),
        offer: Some(VendorNamedOffer::Kuva {
            amount: 10_500,
            limit: 4,
            certainty: VendorKuvaCertainty::Confirmed,
            requirements: &[],
        }),
    },
    ResetChecklistVendorDefinition {
        manifest: "Hubs/HunhowVendorManifest",
        requirements: &[ResetChecklistRequirement::Quest(
            Quest::JadeShadowsConstellations,
        )],
        name: "Hunhow",
        place: Some("Pontis Tower"),
        offer: Some(VendorNamedOffer::Kuva {
            amount: 17_000,
            limit: 3,
            certainty: VendorKuvaCertainty::Confirmed,
            requirements: &[],
        }),
    },
];

pub(super) fn weekly_purchases<'a>(
    inventory: &'a Inventory,
    manifest: &'static str,
    now: DateTime<Utc>,
) -> impl Iterator<Item = &'a VendorOfferPurchase> {
    let reset = ResetCadence::Weekly.window(now).expiry;

    inventory
        .recent_vendor_purchases
        .iter()
        .filter(move |history| history.vendor_type.ends_with(manifest))
        .flat_map(move |history| history.active_purchases(now))
        .filter(move |purchase| purchase.expiry.datetime() == reset)
}

pub(crate) fn vendors(context: &ResetChecklistContext) -> Vec<ResetChecklistVendor> {
    VENDORS
        .iter()
        .map(|vendor| vendor_this_week(vendor, context))
        .collect()
}

fn vendor_this_week(
    vendor: &ResetChecklistVendorDefinition,
    context: &ResetChecklistContext,
) -> ResetChecklistVendor {
    let weekly: Vec<_> =
        weekly_purchases(context.inventory, vendor.manifest, context.now).collect();
    let offers: HashSet<&str> = weekly
        .iter()
        .map(|purchase| purchase.item_id.as_str())
        .collect();
    let bought = weekly.iter().map(|purchase| purchase.num_purchased).sum();

    ResetChecklistVendor {
        name: vendor.name,
        place: vendor.place,
        locked: weekly.is_empty() && unlocks::locked(vendor.requirements, context),
        offers_bought: tally(offers),
        weekly_offer: vendor.offer.map(|offer| match offer {
            VendorNamedOffer::Kuva {
                amount,
                limit,
                certainty,
                requirements,
            } => {
                let bought = match certainty {
                    VendorKuvaCertainty::Confirmed => bought,
                    VendorKuvaCertainty::Estimated => context
                        .stored
                        .kuva_bought
                        .get(vendor.manifest)
                        .map_or(0, |&bought| bought.min(limit)),
                };

                VendorWeeklyOffer::Kuva {
                    amount,
                    bought,
                    limit,
                    certainty,
                    locked: bought == 0 && unlocks::locked(requirements, context),
                }
            }
            VendorNamedOffer::ArchonShard { limit } => {
                let colour = ArchonShardColour::of_week(context.now);
                VendorWeeklyOffer::ArchonShard {
                    colour,
                    image_name: context
                        .items
                        .lookup(&colour.item_type())
                        .and_then(|shard| shard.image_name.clone()),
                    bought,
                    limit,
                }
            }
        }),
    }
}

pub(crate) fn kuva_summary(vendors: &[ResetChecklistVendor]) -> KuvaWeeklySummary {
    let mut summary = KuvaWeeklySummary {
        confirmed: 0,
        estimated: 0,
        available: 0,
    };

    for vendor in vendors.iter().filter(|vendor| !vendor.locked) {
        let Some(VendorWeeklyOffer::Kuva {
            amount,
            bought,
            limit,
            certainty,
            locked: false,
        }) = vendor.weekly_offer
        else {
            continue;
        };

        let kuva = amount * bought.min(limit);
        match certainty {
            VendorKuvaCertainty::Confirmed => summary.confirmed += kuva,
            VendorKuvaCertainty::Estimated => summary.estimated += kuva,
        }

        summary.available += amount * limit;
    }

    summary
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;
    use serde_json::{Value, json};

    use super::*;
    use crate::catalog::fixtures::{self, at};
    use crate::reset_checklist::context::fixtures::{context, synced_at};

    const VENDOR_QUESTS: [Quest; 5] = [
        Quest::ChainsOfHarrow,
        Quest::AngelsOfTheZariman,
        Quest::DuviriParadox,
        Quest::TheHex,
        Quest::JadeShadowsConstellations,
    ];

    fn inventory_with_vendors_unlocked(edit: impl FnOnce(&mut Value)) -> Inventory {
        fixtures::inventory_with(|value| {
            for quest in VENDOR_QUESTS {
                fixtures::entries(value, "QuestKeys")
                    .push(json!({"ItemType": quest.key_chain(), "Completed": true}));
            }
            edit(value);
        })
    }

    #[test]
    fn weekly_purchases_only() {
        let inventory = inventory_with_vendors_unlocked(|_| {});
        let vendors = vendors(&context(&inventory, synced_at()));

        let named = |name| vendors.iter().find(|vendor| vendor.name == name).unwrap();

        assert_eq!(vendors.len(), VENDORS.len());
        assert_eq!(named("Teshin").offers_bought, 1);
        assert_eq!(named("Palladino").offers_bought, 0);
        assert_eq!(
            named("Bird 3").weekly_offer,
            Some(VendorWeeklyOffer::ArchonShard {
                colour: ArchonShardColour::Amber,
                image_name: Some("ArchonShardNira.png".to_owned()),
                bought: 0,
                limit: 1,
            })
        );
        assert_eq!(
            named("Archimedean Yonta").weekly_offer,
            Some(VendorWeeklyOffer::Kuva {
                amount: 35_000,
                bought: 1,
                limit: 1,
                certainty: VendorKuvaCertainty::Confirmed,
                locked: false,
            })
        );
        assert_eq!(
            kuva_summary(&vendors),
            KuvaWeeklySummary {
                confirmed: 35_000,
                estimated: 0,
                available: 581_000,
            }
        );
    }

    #[test]
    fn recorded_purchases_count_as_estimated() {
        let inventory = inventory_with_vendors_unlocked(|_| {});
        let mut context = context(&inventory, synced_at());
        context
            .stored
            .kuva_bought
            .insert("Hubs/TeshinHardModeVendorManifest".to_owned(), 2);

        let vendors = vendors(&context);

        assert_eq!(
            vendors[0].weekly_offer,
            Some(VendorWeeklyOffer::Kuva {
                amount: 10_000,
                bought: 2,
                limit: 25,
                certainty: VendorKuvaCertainty::Estimated,
                locked: false,
            })
        );
        assert_eq!(
            kuva_summary(&vendors),
            KuvaWeeklySummary {
                confirmed: 35_000,
                estimated: 20_000,
                available: 581_000,
            }
        );
    }

    #[test]
    fn kuva_bought_capped_at_limit() {
        let inventory = inventory_with_vendors_unlocked(|value| {
            fixtures::entries(value, "RecentVendorPurchases").push(json!({
                "VendorType": "/Lotus/Types/Game/VendorManifests/Hubs/MelicaVendorManifest",
                "PurchaseHistory": [{
                    "Expiry": {"$date": {"$numberLong": "1789344000000"}},
                    "NumPurchased": 5,
                    "ItemId": "6a9a2f60000000000000f205"
                }]
            }));
        });

        let vendors = vendors(&context(&inventory, synced_at()));

        assert_eq!(kuva_summary(&vendors).confirmed, 35_000 + 4 * 10_500);
    }

    #[test]
    fn locked_vendors_leave_the_summary() {
        let inventory = fixtures::inventory();
        let vendors = vendors(&context(&inventory, synced_at()));

        let locked: Vec<_> = vendors
            .iter()
            .filter(|vendor| vendor.locked)
            .map(|vendor| vendor.name)
            .collect();

        assert_eq!(
            locked,
            [
                "Palladino",
                "Acrithis",
                "Acrithis",
                "Flare",
                "Aspirant Zorba",
                "Cephalon Melica",
                "Hunhow"
            ]
        );
        assert_eq!(
            kuva_summary(&vendors),
            KuvaWeeklySummary {
                confirmed: 35_000,
                estimated: 0,
                available: 250_000 + 35_000 + 42_000,
            }
        );
    }

    #[test]
    fn steel_path_locks_teshin_until_a_purchase() {
        let without_steel_path = |value: &mut Value| {
            for mission in fixtures::entries(value, "Missions") {
                mission.as_object_mut().unwrap().remove("Tier");
            }
        };
        let never_bought = fixtures::inventory_with(|value| {
            without_steel_path(value);
            fixtures::entries(value, "RecentVendorPurchases").remove(0);
        });
        let bought = fixtures::inventory_with(without_steel_path);

        assert!(vendors(&context(&never_bought, synced_at()))[0].locked);
        assert!(!vendors(&context(&bought, synced_at()))[0].locked);
    }

    #[test]
    fn kuva_offer_locked_by_rank() {
        let inventory = inventory_with_vendors_unlocked(|value| {
            for affiliation in fixtures::entries(value, "Affiliations") {
                if affiliation["Tag"] == "NightcapJournalSyndicate" {
                    affiliation["Title"] = 2.into();
                } else if affiliation["Tag"] == "HexSyndicate" {
                    affiliation["Title"] = 3.into();
                }
            }
        });
        let vendors = vendors(&context(&inventory, synced_at()));

        let kuva_locked: Vec<_> = vendors
            .iter()
            .filter(|vendor| {
                matches!(
                    vendor.weekly_offer,
                    Some(VendorWeeklyOffer::Kuva { locked: true, .. })
                )
            })
            .map(|vendor| (vendor.name, vendor.locked))
            .collect();

        assert_eq!(kuva_locked, [("Nightcap", false), ("Flare", false)]);
        assert_eq!(kuva_summary(&vendors).available, 581_000 - 84_000);
    }

    #[test]
    fn archon_shard_rotation() {
        let monday = at(1_794_182_400_000);

        let colours: Vec<_> = (0..3)
            .map(|week| ArchonShardColour::of_week(monday + TimeDelta::weeks(week)))
            .collect();

        assert_eq!(
            colours,
            [
                ArchonShardColour::Amber,
                ArchonShardColour::Crimson,
                ArchonShardColour::Azure
            ]
        );
    }
}
