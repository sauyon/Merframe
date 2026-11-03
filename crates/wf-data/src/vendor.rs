use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use serde::Deserialize;

const OFFERS: &str = include_str!("../data/vendor_offers.json");
const MANIFESTS: &str = include_str!("../data/vendor_manifests.json");

#[derive(Debug, Deserialize)]
pub struct VendorOffer {
    pub item: String,
    pub name: String,
    pub quantity: i64,
    pub prices: Vec<(String, i64)>,
    #[serde(default)]
    pub credits: Option<i64>,
}

impl VendorOffer {
    pub fn price(&self, currency: &str) -> Option<i64> {
        self.prices
            .iter()
            .find(|(item_type, _)| item_type == currency)
            .map(|(_, count)| *count)
    }
}

#[derive(Debug)]
pub struct Vendor {
    pub name: String,
    pub location: Option<String>,
    pub wiki: Option<String>,
    pub offers: Vec<VendorOffer>,
}

#[derive(Default, Deserialize)]
struct OfferFile {
    vendors: BTreeMap<String, Vec<VendorOffer>>,
}

#[derive(Default, Deserialize)]
struct ManifestFile {
    manifests: HashMap<String, VendorLabel>,
    rotating: BTreeMap<String, VendorLabel>,
}

#[derive(Deserialize)]
struct VendorLabel {
    name: String,
    location: String,
    wiki: String,
}

struct Vendors {
    vendors: Vec<Vendor>,
    by_currency: BTreeMap<String, Vec<usize>>,
}

// The tests prove both files parse, so a failure here only empties the table.
static VENDORS: LazyLock<Vendors> = LazyLock::new(|| {
    let offers: OfferFile = serde_json::from_str(OFFERS).unwrap_or_default();
    let mut labels: ManifestFile = serde_json::from_str(MANIFESTS).unwrap_or_default();
    let mut vendors = Vec::new();
    let mut by_currency: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (manifest, offers) in offers.vendors {
        let index = vendors.len();
        for offer in &offers {
            for (currency, _) in &offer.prices {
                let spent_at = by_currency.entry(currency.clone()).or_default();
                if spent_at.last() != Some(&index) {
                    spent_at.push(index);
                }
            }
        }
        vendors.push(match labels.manifests.remove(&manifest) {
            Some(label) => Vendor {
                name: label.name,
                location: Some(label.location),
                wiki: Some(label.wiki),
                offers,
            },
            None => Vendor {
                name: fallback_name(&manifest),
                location: None,
                wiki: None,
                offers,
            },
        });
    }
    for (currency, label) in labels.rotating {
        by_currency.entry(currency).or_default().push(vendors.len());
        vendors.push(Vendor {
            name: label.name,
            location: Some(label.location),
            wiki: Some(label.wiki),
            offers: Vec::new(),
        });
    }
    Vendors {
        vendors,
        by_currency,
    }
});

fn fallback_name(manifest: &str) -> String {
    let tail = manifest.rsplit('/').next().unwrap_or(manifest);
    let tail = ["VendorManifest", "ShopManifest", "Manifest"]
        .iter()
        .find_map(|suffix| tail.strip_suffix(suffix))
        .unwrap_or(tail);
    let mut name = String::with_capacity(tail.len() + 8);
    let mut previous: Option<char> = None;
    for ch in tail.chars() {
        if ch.is_ascii_uppercase() && previous.is_some_and(|p| p.is_ascii_lowercase()) {
            name.push(' ');
        }
        name.push(ch);
        previous = Some(ch);
    }
    name
}

/// Vendors that take `currency` as payment. A vendor whose stock rotates
/// through the worldstate has no offers listed.
pub fn currency_vendors(currency: &str) -> impl Iterator<Item = &'static Vendor> {
    let vendors = &*VENDORS;
    vendors
        .by_currency
        .get(currency)
        .into_iter()
        .flatten()
        .map(|&index| &vendors.vendors[index])
}

pub fn vendor_currencies() -> impl Iterator<Item = &'static str> {
    VENDORS.by_currency.keys().map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VESTIGIAL_MOTES: &str =
        "/Lotus/Types/Gameplay/JadeShadows/Resources/AscensionResourceItem";

    #[test]
    fn every_label_names_a_manifest() {
        let offers: OfferFile = serde_json::from_str(OFFERS).unwrap();
        let labels: ManifestFile = serde_json::from_str(MANIFESTS).unwrap();
        for manifest in labels.manifests.keys() {
            assert!(offers.vendors.contains_key(manifest), "{manifest}");
        }
        for label in labels.manifests.values().chain(labels.rotating.values()) {
            assert!(!label.name.is_empty() && !label.location.is_empty());
            assert!(label.wiki.starts_with("https://wiki.warframe.com/"));
        }
    }

    #[test]
    fn motes_are_spent_with_ordis() {
        let vendors: Vec<&Vendor> = currency_vendors(VESTIGIAL_MOTES).collect();
        let ordis = vendors
            .iter()
            .find(|vendor| vendor.name == "Ordis")
            .expect("Ordis takes motes");
        assert_eq!(ordis.location.as_deref(), Some("Drifter's Camp, Earth"));
        assert!(
            ordis
                .offers
                .iter()
                .all(|offer| offer.price(VESTIGIAL_MOTES).is_some())
        );
        assert!(vendor_currencies().any(|currency| currency == VESTIGIAL_MOTES));
    }

    #[test]
    fn rotating_vendor_has_no_offers() {
        let baro: Vec<&Vendor> =
            currency_vendors("/Lotus/Types/Items/MiscItems/PrimeBucks").collect();
        assert!(
            baro.iter()
                .any(|vendor| vendor.name == "Baro Ki'Teer" && vendor.offers.is_empty())
        );
    }

    #[test]
    fn unlabelled_manifest_names_itself() {
        assert_eq!(
            fallback_name("/Lotus/Types/Game/VendorManifests/Events/MechSurvivalVendorManifest"),
            "Mech Survival"
        );
        assert!(
            currency_vendors("/Lotus/Types/Items/MiscItems/NotSold")
                .next()
                .is_none()
        );
    }
}
