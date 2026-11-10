use std::marker::PhantomData;

use reqwest::StatusCode;
use reqwest::header::{ETAG, IF_NONE_MATCH};
use serde::de::DeserializeOwned;

use crate::client::{MERFRAME_API, REQUEST_TIMEOUT, accepted, segment, send};
use crate::error::Result;
use crate::models::{PriceTable, RivenData, WeaponAuctions};
use crate::ratelimit::{self, RateLimiter};

pub struct Etagged<T> {
    http: reqwest::Client,
    url: String,
    etag: Option<String>,
    rate_limiter: RateLimiter,
    document: PhantomData<fn() -> T>,
}

pub fn bulk_prices(http: reqwest::Client) -> Etagged<PriceTable> {
    Etagged::new(http, format!("{MERFRAME_API}/prices"))
}

pub fn bulk_riven_data(http: reqwest::Client) -> Etagged<RivenData> {
    Etagged::new(http, format!("{MERFRAME_API}/riven"))
}

pub fn riven_auctions(http: reqwest::Client, weapon_slug: &str) -> Etagged<WeaponAuctions> {
    Etagged::new(
        http,
        format!("{MERFRAME_API}/riven/auctions/{}", segment(weapon_slug)),
    )
}

impl<T: DeserializeOwned> Etagged<T> {
    fn new(http: reqwest::Client, url: String) -> Self {
        Self {
            http,
            url,
            etag: None,
            rate_limiter: ratelimit::shared(),
            document: PhantomData,
        }
    }

    #[must_use]
    pub fn with_etag(mut self, etag: Option<String>) -> Self {
        self.etag = etag;
        self
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    pub async fn fetch(&mut self) -> Result<Option<T>> {
        let mut request = self.http.get(&self.url).timeout(REQUEST_TIMEOUT);
        if let Some(etag) = &self.etag {
            request = request.header(IF_NONE_MATCH, etag);
        }

        let response = send(&self.http, &self.rate_limiter, request.build()?).await?;
        let status = response.status();
        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        self.accept(status, etag, response.text().await?)
    }

    fn accept(
        &mut self,
        status: StatusCode,
        etag: Option<String>,
        body: String,
    ) -> Result<Option<T>> {
        if status == StatusCode::NOT_MODIFIED {
            return Ok(None);
        }

        let body = accepted(status, body)?;
        let document = serde_json::from_str(&body)?;
        self.etag = etag;
        Ok(Some(document))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PriceTable;

    const TABLE: &str = include_str!("../tests/fixtures/prices_bulk.json");

    fn prices() -> Etagged<PriceTable> {
        bulk_prices(reqwest::Client::new())
    }

    #[test]
    fn merframe_urls() {
        assert_eq!(prices().url(), "https://api.merframe.net/v1/prices");
        assert_eq!(
            bulk_riven_data(reqwest::Client::new()).url(),
            "https://api.merframe.net/v1/riven"
        );
        assert_eq!(
            riven_auctions(reqwest::Client::new(), "torid").url(),
            "https://api.merframe.net/v1/riven/auctions/torid"
        );
    }

    #[test]
    fn auctions_document_etag() {
        let mut client = riven_auctions(reqwest::Client::new(), "torid");
        let document = client
            .accept(
                StatusCode::OK,
                Some("\"1789240000-40\"".to_owned()),
                include_str!("../../../fixtures/riven_auctions.json").to_owned(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(document.slug, "torid");
        assert_eq!(document.auctions.len(), 40);
        assert_eq!(document.auctions[0].buyout_price, Some(330));
        assert_eq!(document.auctions[0].attributes.len(), 3);
        assert_eq!(client.etag.as_deref(), Some("\"1789240000-40\""));
    }

    #[test]
    fn table_etag_stored() {
        let mut client = prices();
        assert_eq!(client.etag, None);
        let table = client
            .accept(
                StatusCode::OK,
                Some("\"prices-1\"".to_owned()),
                TABLE.to_owned(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(table.count, 10);
        assert_eq!(client.etag.as_deref(), Some("\"prices-1\""));
    }

    #[test]
    fn not_modified() {
        let mut client = prices();
        client
            .accept(
                StatusCode::OK,
                Some("\"prices-1\"".to_owned()),
                TABLE.to_owned(),
            )
            .unwrap();
        let unchanged = client
            .accept(StatusCode::NOT_MODIFIED, None, String::new())
            .expect("not modified");
        assert!(unchanged.is_none());
        assert_eq!(client.etag.as_deref(), Some("\"prices-1\""));
    }

    #[test]
    fn error_keeps_etag() {
        let mut client = prices();
        client
            .accept(
                StatusCode::OK,
                Some("\"prices-1\"".to_owned()),
                TABLE.to_owned(),
            )
            .unwrap();
        let error = client
            .accept(
                StatusCode::BAD_GATEWAY,
                Some("\"prices-2\"".to_owned()),
                "upstream is down".to_owned(),
            )
            .expect_err("http error");
        assert_eq!(
            error.to_string(),
            "Warframe.market 502 Bad Gateway: upstream is down"
        );
        assert_eq!(client.etag.as_deref(), Some("\"prices-1\""));
    }

    #[test]
    fn riven_document() {
        let mut client = bulk_riven_data(reqwest::Client::new());
        let data = client
            .accept(
                StatusCode::OK,
                Some("\"riven-1\"".to_owned()),
                include_str!("../../../fixtures/riven_data.json").to_owned(),
            )
            .unwrap()
            .unwrap();

        assert_eq!(client.etag.as_deref(), Some("\"riven-1\""));
        assert!(data.attribution.contains("44bananas"));
        assert!(data.updated_at > 0);
        assert!(data.weapons_updated_at > 0);
        assert!(data.good_rolls_updated_at > 0);

        let onos = data
            .weapons
            .iter()
            .find(|weapon| weapon.name == "Onos")
            .unwrap();
        assert_eq!(
            onos.game_ref,
            "/Lotus/Weapons/Thanotech/EntratiWristGun/EntratiWristGunWeapon"
        );
        assert_eq!(onos.slug, "onos");
        assert_eq!(onos.group, "secondary");
        assert_eq!(onos.riven_type, "pistol");
        assert_eq!(
            onos.mod_type,
            "/Lotus/Upgrades/Mods/Randomized/LotusPistolRandomModRare"
        );
        assert_eq!(onos.mastery_rank, 14);

        let good = data.good_rolls.get("onos").unwrap();
        assert_eq!(good.weapon, "onos");
        assert_eq!(good.weapon_class, "secondary");
        assert_eq!(good.alternatives.len(), 1);
        assert_eq!(good.alternatives[0].mandatory.len(), 1);
        assert_eq!(good.alternatives[0].mandatory[0].abbr, "CD");
        assert_eq!(
            good.alternatives[0].mandatory[0].tags,
            ["WeaponCritDamageMod"]
        );
        assert_eq!(good.alternatives[0].optional.len(), 5);
        assert_eq!(good.alternatives[0].optional_needed, 2);
        assert_eq!(
            good.accepted_bad
                .iter()
                .map(|stat| stat.abbr.as_str())
                .collect::<Vec<&str>>(),
            ["PUNC", "REC", "ZOOM"]
        );
        assert_eq!(good.note, None);
    }

    #[test]
    fn empty_riven_document() {
        let mut client = bulk_riven_data(reqwest::Client::new());
        assert!(
            client
                .accept(
                    StatusCode::OK,
                    Some("\"riven-1\"".to_owned()),
                    "{}".to_owned()
                )
                .is_err()
        );
        assert_eq!(client.etag, None);
    }
}
