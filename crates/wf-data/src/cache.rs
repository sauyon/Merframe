use std::collections::HashMap;
use std::path::{Path, PathBuf};

use futures_util::future::{try_join, try_join_all, try_join3};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::error::{DataError, Result};
use crate::game_data::GameData;
use crate::nightwave::NightwaveActs;

const ITEM_CATEGORIES: &[&str] = &[
    "Warframes",
    "Primary",
    "Secondary",
    "Melee",
    "Archwing",
    "Arch-Gun",
    "Arch-Melee",
    "Sentinels",
    "SentinelWeapons",
    "Pets",
    "Arcanes",
    "Mods",
    "Resources",
    "Misc",
    "Gear",
    "Skins",
    "Sigils",
    "Fish",
    "Glyphs",
];

const RELICS_FILE: &str = "Relics.json";
const COMPONENTS_FILE: &str = "Components.json";
const NIGHTWAVE_ACTS_FILE: &str = "ExportNightwave.json";
const DICTIONARY_FILE: &str = "dict.en.json";

const WFCD_ITEMS: &str = "https://raw.githubusercontent.com/WFCD/warframe-items/master/data/json";
const PUBLIC_EXPORT_PLUS: &str =
    "https://raw.githubusercontent.com/calamity-inc/warframe-public-export-plus/senpai";

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheMeta {
    entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    etag: Option<String>,
    last_modified: Option<String>,
}

impl CacheMeta {
    fn record(&mut self, file_name: &str, fresh: Option<CacheEntry>) {
        if let Some(entry) = fresh {
            self.entries.insert(file_name.to_owned(), entry);
        }
    }
}

pub async fn load_or_fetch(dir: &Path, client: &reqwest::Client) -> Result<GameData> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(cache_io(dir))?;
    let meta_path = dir.join("etag.json");
    let mut meta = read_meta(&meta_path).await;

    let item_files: Vec<String> = ITEM_CATEGORIES
        .iter()
        .map(|category| format!("{category}.json"))
        .collect();
    let (items, (relics_body, relics_entry), (components_body, components_entry)) = try_join3(
        try_join_all(
            item_files
                .iter()
                .map(|file_name| fetch_or_use_cache(dir, WFCD_ITEMS, file_name, client, &meta)),
        ),
        fetch_or_use_cache(dir, WFCD_ITEMS, RELICS_FILE, client, &meta),
        fetch_or_use_cache(dir, WFCD_ITEMS, COMPONENTS_FILE, client, &meta),
    )
    .await?;

    let mut item_bodies = Vec::with_capacity(items.len());
    for (file_name, (body, entry)) in item_files.iter().zip(items) {
        item_bodies.push(body);
        meta.record(file_name, entry);
    }
    meta.record(RELICS_FILE, relics_entry);
    meta.record(COMPONENTS_FILE, components_entry);

    let nightwave_acts = load_nightwave_acts(dir, client, &mut meta)
        .await
        .unwrap_or_else(|error| {
            warn!(%error, "Loading the Nightwave acts failed, their names are derived from their type names");
            NightwaveActs::default()
        });
    write_meta(&meta_path, &meta).await?;

    Ok(
        GameData::from_json_parts(&item_bodies, &relics_body, &components_body)?
            .with_nightwave_acts(nightwave_acts),
    )
}

async fn load_nightwave_acts(
    dir: &Path,
    client: &reqwest::Client,
    meta: &mut CacheMeta,
) -> Result<NightwaveActs> {
    let ((acts_body, acts_entry), (dictionary_body, dictionary_entry)) = try_join(
        fetch_or_use_cache(dir, PUBLIC_EXPORT_PLUS, NIGHTWAVE_ACTS_FILE, client, meta),
        fetch_or_use_cache(dir, PUBLIC_EXPORT_PLUS, DICTIONARY_FILE, client, meta),
    )
    .await?;
    let acts = NightwaveActs::from_json(&acts_body, &dictionary_body)?;

    meta.record(NIGHTWAVE_ACTS_FILE, acts_entry);
    meta.record(DICTIONARY_FILE, dictionary_entry);
    Ok(acts)
}

async fn read_meta(path: &Path) -> CacheMeta {
    let Ok(bytes) = tokio::fs::read(path).await else {
        return CacheMeta::default();
    };
    match serde_json::from_slice(&bytes) {
        Ok(meta) => meta,
        Err(error) => {
            warn!(
                path = %path.display(),
                %error,
                "Reading the game data cache index failed"
            );
            CacheMeta::default()
        }
    }
}

async fn write_meta(path: &Path, meta: &CacheMeta) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(meta).map_err(DataError::ParseCacheMeta)?;
    tokio::fs::write(path, bytes).await.map_err(cache_io(path))
}

async fn fetch_or_use_cache(
    dir: &Path,
    source: &str,
    file_name: &str,
    client: &reqwest::Client,
    meta: &CacheMeta,
) -> Result<(String, Option<CacheEntry>)> {
    let file_path: PathBuf = dir.join(file_name);
    match download_fresh(&file_path, source, file_name, client, meta).await {
        Ok(Some((body, entry))) => Ok((body, Some(entry))),
        Ok(None) => Ok((read_cached_file(&file_path).await?, None)),
        Err(error) => {
            warn!(
                file = file_name,
                %error,
                "Downloading the game data file failed, using the cached copy"
            );
            read_cached_file(&file_path)
                .await
                .map(|body| (body, None))
                .map_err(|_| DataError::NoCacheAvailable(file_name.to_owned()))
        }
    }
}

async fn download_fresh(
    file_path: &Path,
    source: &str,
    file_name: &str,
    client: &reqwest::Client,
    meta: &CacheMeta,
) -> Result<Option<(String, CacheEntry)>> {
    let mut request = client.get(format!("{source}/{file_name}"));
    if let Some(entry) = meta.entries.get(file_name) {
        if let Some(etag) = &entry.etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        if let Some(last_modified) = &entry.last_modified {
            request = request.header(reqwest::header::IF_MODIFIED_SINCE, last_modified);
        }
    }

    let response = request
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(DataError::Network)?;
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(None);
    }
    let etag = header(&response, reqwest::header::ETAG);
    let last_modified = header(&response, reqwest::header::LAST_MODIFIED);
    let body = response.text().await.map_err(DataError::Network)?;
    tokio::fs::write(file_path, &body)
        .await
        .map_err(cache_io(file_path))?;
    Ok(Some((
        body,
        CacheEntry {
            etag,
            last_modified,
        },
    )))
}

fn header(response: &reqwest::Response, name: reqwest::header::HeaderName) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

async fn read_cached_file(path: &Path) -> Result<String> {
    tokio::fs::read_to_string(path)
        .await
        .map_err(cache_io(path))
}

fn cache_io(path: &Path) -> impl FnOnce(std::io::Error) -> DataError + '_ {
    |source| DataError::CacheIo {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn corrupt_cache_index() {
        let dir = std::env::temp_dir().join("merframe-game-data-cache-index");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("etag.json");

        assert!(read_meta(&path).await.entries.is_empty());

        tokio::fs::write(&path, b"{\"entries\": ").await.unwrap();
        assert!(read_meta(&path).await.entries.is_empty());

        let written = serde_json::to_vec(&CacheMeta {
            entries: HashMap::from([(
                "Relics.json".to_owned(),
                CacheEntry {
                    etag: Some("\"relics-1\"".to_owned()),
                    last_modified: None,
                },
            )]),
        })
        .unwrap();
        tokio::fs::write(&path, written).await.unwrap();
        let meta = read_meta(&path).await;
        assert_eq!(
            meta.entries["Relics.json"].etag.as_deref(),
            Some("\"relics-1\"")
        );

        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }
}
