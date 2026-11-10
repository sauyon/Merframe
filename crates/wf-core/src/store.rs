use std::cell::RefCell;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use rusqlite::{Connection, OptionalExtension, Params, Row, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use wf_inventory::Inventory;

use crate::arbitration_runs::ArbitrationRun;
use crate::catalog::DUCATS_ITEM;
use crate::delta::ItemDelta;
use crate::error::{CoreError, Result};
use crate::favourites::Favourites;
use crate::stats::AYA_ITEM;
use crate::trade::Trade;

const MIGRATIONS: [(i64, &str); 2] = [
    (1, include_str!("sql/0001_init.sql")),
    (2, include_str!("sql/0002_arbitration_runs.sql")),
];
const SNAPSHOT_COLUMNS: &str = "id, taken_at, last_sync_oid, plat, credits, endo, ducats, mr";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct SnapshotId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub id: SnapshotId,
    pub taken_at: DateTime<Utc>,
    pub last_sync_oid: String,
    pub plat: i64,
    pub credits: i64,
    pub endo: i64,
    pub ducats: i64,
    pub mr: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct StatPoint {
    pub snapshot: SnapshotId,
    pub at: DateTime<Utc>,
    pub plat: i64,
    pub credits: i64,
    pub endo: i64,
    pub ducats: i64,
    pub aya: Option<i64>,
    pub mr: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeRange {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

impl TimeRange {
    pub fn all() -> Self {
        Self {
            from: DateTime::<Utc>::MIN_UTC,
            to: DateTime::<Utc>::MAX_UTC,
        }
    }

    pub fn since(from: DateTime<Utc>) -> Self {
        Self {
            from,
            to: DateTime::<Utc>::MAX_UTC,
        }
    }

    pub fn new(from: DateTime<Utc>, to: DateTime<Utc>) -> Self {
        Self { from, to }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoredDelta {
    pub item_type: String,
    pub category: String,
    pub delta: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct StoredTrade {
    pub id: i64,
    pub at: DateTime<Utc>,
    pub partner: Option<String>,
    pub trade: Trade,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RelicOpening {
    pub id: i64,
    pub at: DateTime<Utc>,
    pub relic: String,
    pub reward_item: String,
    pub player_count: i64,
}

enum InventoryCache {
    File(PathBuf),
    Held(RefCell<Option<String>>),
}

impl InventoryCache {
    fn read(&self) -> Result<Option<String>> {
        match self {
            Self::File(path) => match std::fs::read(path) {
                Ok(compressed) => gunzip(&compressed, path).map(Some),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(source) => Err(CoreError::Read {
                    path: path.clone(),
                    source,
                }),
            },
            Self::Held(json) => Ok(json.borrow().clone()),
        }
    }

    fn write(&self, json: &str) -> Result<()> {
        match self {
            Self::File(path) => write_atomically(path, &gzip(json.as_bytes())?),
            Self::Held(held) => {
                *held.borrow_mut() = Some(json.to_owned());
                Ok(())
            }
        }
    }
}

pub struct Store {
    connection: Connection,
    inventory: InventoryCache,
    raw_inventory: InventoryCache,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        restrict(path)?;
        Self::prepare(
            connection,
            InventoryCache::File(path.with_file_name("inventory.json.gz")),
            InventoryCache::File(path.with_file_name("inventory_latest_raw.json.gz")),
        )
    }

    pub fn in_memory() -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        Self::prepare(
            connection,
            InventoryCache::Held(RefCell::new(None)),
            InventoryCache::Held(RefCell::new(None)),
        )
    }

    fn prepare(
        connection: Connection,
        inventory: InventoryCache,
        raw_inventory: InventoryCache,
    ) -> Result<Self> {
        connection.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        let store = Self {
            connection,
            inventory,
            raw_inventory,
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS migrations (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL)",
        )?;
        let current: i64 = self.connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM migrations",
            [],
            |row| row.get(0),
        )?;
        let latest = MIGRATIONS[MIGRATIONS.len() - 1].0;
        if current > latest {
            return Err(CoreError::SchemaVersion(current));
        }
        let transaction = self.connection.unchecked_transaction()?;
        for (version, sql) in MIGRATIONS.iter().filter(|(version, _)| *version > current) {
            transaction.execute_batch(sql)?;
            transaction.execute(
                "INSERT INTO migrations (version, applied_at) VALUES (?1, ?2)",
                params![version, Utc::now().timestamp_millis()],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn cached_inventory(&self) -> Result<Option<String>> {
        self.inventory.read()
    }

    pub fn cache_inventory(&self, json: &str) -> Result<()> {
        self.inventory.write(json)
    }

    pub fn keep_raw_inventory(&self, json: &str) -> Result<()> {
        self.raw_inventory.write(json)
    }

    pub fn record_snapshot(&self, inventory: &Inventory, at: DateTime<Utc>) -> Result<SnapshotId> {
        if let Some(latest) = self.latest_snapshot()? {
            if latest.last_sync_oid == inventory.last_inventory_sync.as_str() {
                tracing::debug!(snapshot = latest.id.0, "Inventory sync unchanged");
                return Ok(latest.id);
            }
            if let (Some(stored), Some(incoming)) = (
                wf_inventory::oid_seconds(&latest.last_sync_oid),
                inventory.last_inventory_sync.seconds(),
            ) && incoming < stored
            {
                tracing::debug!(
                    snapshot = latest.id.0,
                    stored = latest.last_sync_oid,
                    incoming = inventory.last_inventory_sync.as_str(),
                    "Inventory sync is older than the newest stored snapshot"
                );
                return Ok(latest.id);
            }
        }
        self.connection.execute(
            "INSERT INTO snapshots (taken_at, last_sync_oid, plat, credits, endo, ducats, aya, mr) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                at.timestamp_millis(),
                inventory.last_inventory_sync.as_str(),
                inventory.premium_credits,
                inventory.regular_credits,
                inventory.fusion_points,
                inventory.counted(DUCATS_ITEM),
                inventory.counted(AYA_ITEM),
                i64::from(inventory.player_level),
            ],
        )?;
        Ok(SnapshotId(self.connection.last_insert_rowid()))
    }

    pub fn latest_snapshot(&self) -> Result<Option<Snapshot>> {
        self.snapshot_where("ORDER BY id DESC LIMIT 1", [])
    }

    pub fn snapshot(&self, id: SnapshotId) -> Result<Option<Snapshot>> {
        self.snapshot_where("WHERE id = ?1", params![id.0])
    }

    pub fn snapshot_before(&self, id: SnapshotId) -> Result<Option<Snapshot>> {
        self.snapshot_where("WHERE id < ?1 ORDER BY id DESC LIMIT 1", params![id.0])
    }

    fn snapshot_where(&self, filter: &str, params: impl Params) -> Result<Option<Snapshot>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {SNAPSHOT_COLUMNS} FROM snapshots {filter}"
        ))?;
        statement
            .query_and_then(params, snapshot_from_row)?
            .next()
            .transpose()
    }

    fn in_range<T>(
        &self,
        sql: &str,
        range: TimeRange,
        read: impl FnMut(&Row<'_>) -> Result<T>,
    ) -> Result<Vec<T>> {
        let mut statement = self.connection.prepare(sql)?;
        statement
            .query_and_then(
                params![range.from.timestamp_millis(), range.to.timestamp_millis()],
                read,
            )?
            .collect()
    }

    pub fn stats_series(&self, range: TimeRange) -> Result<Vec<StatPoint>> {
        self.in_range(
            "SELECT id, taken_at, plat, credits, endo, ducats, aya, mr FROM snapshots \
             WHERE taken_at >= ?1 AND taken_at <= ?2 ORDER BY taken_at",
            range,
            |row| {
                Ok(StatPoint {
                    snapshot: SnapshotId(row.get(0)?),
                    at: datetime(row.get(1)?)?,
                    plat: row.get(2)?,
                    credits: row.get(3)?,
                    endo: row.get(4)?,
                    ducats: row.get(5)?,
                    aya: row.get(6)?,
                    mr: row.get(7)?,
                })
            },
        )
    }

    pub fn record_deltas(
        &self,
        from: SnapshotId,
        to: SnapshotId,
        deltas: &[ItemDelta],
    ) -> Result<()> {
        let transaction = self.connection.unchecked_transaction()?;
        {
            let mut statement = transaction.prepare(
                "INSERT INTO deltas (from_snapshot, to_snapshot, item_type, category, delta) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for delta in deltas {
                statement.execute(params![
                    from.0,
                    to.0,
                    delta.item_type,
                    delta.category,
                    delta.delta()
                ])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn deltas(&self, range: TimeRange) -> Result<Vec<StoredDelta>> {
        self.in_range(
            "SELECT d.item_type, d.category, SUM(d.delta) AS total FROM deltas d \
             JOIN snapshots s ON s.id = d.to_snapshot \
             WHERE s.taken_at >= ?1 AND s.taken_at <= ?2 \
             GROUP BY d.item_type, d.category HAVING total != 0 \
             ORDER BY d.category, d.item_type",
            range,
            |row| {
                Ok(StoredDelta {
                    item_type: row.get(0)?,
                    category: row.get(1)?,
                    delta: row.get(2)?,
                })
            },
        )
    }

    pub fn record_trade(
        &self,
        at: DateTime<Utc>,
        partner: Option<&str>,
        trade: &Trade,
    ) -> Result<i64> {
        let items_json = serde_json::to_string(trade)?;
        self.connection.execute(
            "INSERT INTO trades (at, partner, items_json, plat) VALUES (?1, ?2, ?3, ?4)",
            params![at.timestamp_millis(), partner, items_json, trade.plat],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn update_trade(
        &self,
        id: i64,
        at: DateTime<Utc>,
        partner: Option<&str>,
        trade: &Trade,
    ) -> Result<()> {
        let items_json = serde_json::to_string(trade)?;
        self.connection.execute(
            "UPDATE trades SET at = ?2, partner = ?3, items_json = ?4, plat = ?5 WHERE id = ?1",
            params![id, at.timestamp_millis(), partner, items_json, trade.plat],
        )?;
        Ok(())
    }

    pub fn delete_trade(&self, id: i64) -> Result<()> {
        self.connection
            .execute("DELETE FROM trades WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn trades(&self, range: TimeRange) -> Result<Vec<StoredTrade>> {
        self.in_range(
            "SELECT id, at, partner, items_json FROM trades \
             WHERE at >= ?1 AND at <= ?2 ORDER BY at",
            range,
            |row| {
                let items_json: String = row.get(3)?;
                Ok(StoredTrade {
                    id: row.get(0)?,
                    at: datetime(row.get(1)?)?,
                    partner: row.get(2)?,
                    trade: serde_json::from_str(&items_json)?,
                })
            },
        )
    }

    pub fn record_relic_opening(
        &self,
        at: DateTime<Utc>,
        relic: &str,
        reward_item: &str,
        player_count: usize,
    ) -> Result<i64> {
        self.connection.execute(
            "INSERT INTO relic_openings (at, relic, reward_item, player_count) \
             VALUES (?1, ?2, ?3, ?4)",
            params![at.timestamp_millis(), relic, reward_item, player_count],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn relic_openings(&self, range: TimeRange) -> Result<Vec<RelicOpening>> {
        self.in_range(
            "SELECT id, at, relic, reward_item, player_count FROM relic_openings \
             WHERE at >= ?1 AND at <= ?2 ORDER BY at",
            range,
            |row| {
                Ok(RelicOpening {
                    id: row.get(0)?,
                    at: datetime(row.get(1)?)?,
                    relic: row.get(2)?,
                    reward_item: row.get(3)?,
                    player_count: row.get(4)?,
                })
            },
        )
    }

    pub fn favourites(&self) -> Result<Favourites> {
        let mut statement = self
            .connection
            .prepare("SELECT unique_name FROM favourites")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<Favourites>>()?)
    }

    pub fn toggle_favourite(&self, unique_name: &str, at: DateTime<Utc>) -> Result<bool> {
        let removed = self.connection.execute(
            "DELETE FROM favourites WHERE unique_name = ?1",
            params![unique_name],
        )?;
        if removed > 0 {
            return Ok(false);
        }
        self.connection.execute(
            "INSERT INTO favourites (unique_name, since) VALUES (?1, ?2)",
            params![unique_name, at.timestamp_millis()],
        )?;
        Ok(true)
    }

    pub fn save_arbitration_run(
        &self,
        run: &ArbitrationRun,
        recorded_at: DateTime<Utc>,
    ) -> Result<bool> {
        let run_json = serde_json::to_string(run)?;
        let inserted = self.connection.execute(
            "INSERT INTO arbitration_runs (id, started_at, recorded_at, run_json) \
             VALUES (?1, ?2, ?3, ?4) ON CONFLICT(id) DO NOTHING",
            params![
                run.id,
                run.started_at.map(|at| at.timestamp_millis()),
                recorded_at.timestamp_millis(),
                run_json,
            ],
        )?;
        Ok(inserted == 1)
    }

    pub fn update_arbitration_run(&self, run: &ArbitrationRun) -> Result<()> {
        let run_json = serde_json::to_string(run)?;
        self.connection.execute(
            "UPDATE arbitration_runs SET run_json = ?2 WHERE id = ?1",
            params![run.id, run_json],
        )?;
        Ok(())
    }

    pub fn arbitration_runs(&self) -> Result<Vec<ArbitrationRun>> {
        let mut statement = self.connection.prepare(
            "SELECT run_json FROM arbitration_runs \
             ORDER BY started_at IS NULL, started_at DESC, recorded_at DESC",
        )?;
        let runs = statement
            .query_and_then([], |row| {
                let run_json: String = row.get(0)?;
                Ok(serde_json::from_str(&run_json)?)
            })?
            .collect::<Result<Vec<ArbitrationRun>>>()?;
        Ok(runs)
    }

    pub fn arbitration_run(&self, id: &str) -> Result<Option<ArbitrationRun>> {
        let run_json: Option<String> = self
            .connection
            .query_row(
                "SELECT run_json FROM arbitration_runs WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        run_json
            .map(|run_json| serde_json::from_str(&run_json))
            .transpose()
            .map_err(Into::into)
    }

    pub fn delete_arbitration_run(&self, id: &str) -> Result<()> {
        self.connection
            .execute("DELETE FROM arbitration_runs WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn setting<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let raw: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        match raw {
            Some(value) => Ok(Some(serde_json::from_str(&value)?)),
            None => Ok(None),
        }
    }

    pub fn set_setting<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let encoded = serde_json::to_string(value)?;
        self.connection.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, encoded],
        )?;
        Ok(())
    }
}

fn snapshot_from_row(row: &Row<'_>) -> Result<Snapshot> {
    Ok(Snapshot {
        id: SnapshotId(row.get(0)?),
        taken_at: datetime(row.get(1)?)?,
        last_sync_oid: row.get(2)?,
        plat: row.get(3)?,
        credits: row.get(4)?,
        endo: row.get(5)?,
        ducats: row.get(6)?,
        mr: row.get(7)?,
    })
}

fn datetime(millis: i64) -> Result<DateTime<Utc>> {
    DateTime::from_timestamp_millis(millis).ok_or(CoreError::Timestamp(millis))
}

fn gzip(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).map_err(CoreError::Compress)?;
    encoder.finish().map_err(CoreError::Compress)
}

fn gunzip(bytes: &[u8], path: &Path) -> Result<String> {
    let mut decoder = GzDecoder::new(bytes);
    let mut out = String::new();
    std::io::Read::read_to_string(&mut decoder, &mut out).map_err(|source| {
        CoreError::Decompress {
            path: path.to_path_buf(),
            source,
        }
    })?;
    Ok(out)
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let partial = path.with_extension("part");
    std::fs::write(&partial, bytes).map_err(CoreError::io(&partial))?;
    restrict(&partial)?;
    std::fs::rename(&partial, path).map_err(CoreError::io(path))
}

#[cfg(unix)]
fn restrict(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(CoreError::io(path))
}

#[cfg(not(unix))]
fn restrict(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::fixtures::{self, FERRITE, at, scratch_dir, synced};
    use crate::delta;
    use crate::rivens::support::riven_table;
    use crate::trade::TradeItem;

    #[test]
    fn arbitration_run_round_trip() {
        let store = Store::in_memory().unwrap();
        let runs = crate::arbitration_runs::fixture_runs();
        let run = &runs[0];
        assert!(store.save_arbitration_run(run, at(1_000_000)).unwrap());
        assert!(!store.save_arbitration_run(run, at(1_000_001)).unwrap());
        assert_eq!(store.arbitration_run(&run.id).unwrap().unwrap().id, run.id);

        let mut changed = run.clone();
        changed.manual_vitus = Some(4);
        store.update_arbitration_run(&changed).unwrap();
        let stored = store.arbitration_runs().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].manual_vitus, Some(4));

        store.delete_arbitration_run(&run.id).unwrap();
        assert!(store.arbitration_run(&run.id).unwrap().is_none());
    }

    #[test]
    fn snapshot_round_trip() {
        let store = Store::in_memory().unwrap();
        let inventory = fixtures::inventory();
        let id = store.record_snapshot(&inventory, at(1_000_000)).unwrap();

        let latest = store.latest_snapshot().unwrap().unwrap();
        assert_eq!(latest.id, id);
        assert_eq!(latest.last_sync_oid, "6a9eeb1f000000000000c001");
        assert_eq!(latest.plat, 5249);
        assert_eq!(latest.credits, 90_814_797);
        assert_eq!(latest.endo, 189_082);
        assert_eq!(latest.ducats, 937);
        assert_eq!(latest.mr, 14);
        assert_eq!(latest.taken_at, at(1_000_000));
    }

    #[test]
    fn unchanged_sync_oid() {
        let store = Store::in_memory().unwrap();
        let inventory = fixtures::inventory();
        let first = store.record_snapshot(&inventory, at(1_000_000)).unwrap();
        let second = store.record_snapshot(&inventory, at(2_000_000)).unwrap();
        assert_eq!(first, second);
        assert_eq!(store.stats_series(TimeRange::all()).unwrap().len(), 1);
    }

    #[test]
    fn older_sync_oid_refused() {
        let store = Store::in_memory().unwrap();
        let newer = Inventory::parse(&synced("6a9fb57d000000000000c101")).unwrap();
        let newest = store.record_snapshot(&newer, at(2_000_000)).unwrap();

        let older = fixtures::inventory();
        assert_eq!(
            older.last_inventory_sync.as_str(),
            "6a9eeb1f000000000000c001"
        );
        let reused = store.record_snapshot(&older, at(3_000_000)).unwrap();
        assert_eq!(reused, newest);
        assert_eq!(store.stats_series(TimeRange::all()).unwrap().len(), 1);
        assert_eq!(
            store.latest_snapshot().unwrap().unwrap().last_sync_oid,
            "6a9fb57d000000000000c101"
        );
    }

    #[test]
    fn sync_oid_in_same_second() {
        let store = Store::in_memory().unwrap();
        let first = store
            .record_snapshot(&fixtures::inventory(), at(1_000_000))
            .unwrap();
        let sibling = Inventory::parse(&synced("6a9eeb1f000000000000c000")).unwrap();
        let second = store.record_snapshot(&sibling, at(1_000_500)).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn changed_sync_oid_appends() {
        let store = Store::in_memory().unwrap();
        let inventory = fixtures::inventory();
        let first = store.record_snapshot(&inventory, at(1_000_000)).unwrap();

        let changed = Inventory::parse(&synced("6a9eeb1f000000000000c002")).unwrap();
        let second = store.record_snapshot(&changed, at(2_000_000)).unwrap();
        assert_ne!(first, second);

        let series = store.stats_series(TimeRange::all()).unwrap();
        assert_eq!(series.len(), 2);
        assert_eq!(series[0].at, at(1_000_000));
        assert_eq!(series[1].snapshot, second);

        let narrowed = store.stats_series(TimeRange::since(at(1_500_000))).unwrap();
        assert_eq!(narrowed.len(), 1);
        assert_eq!(store.snapshot_before(second).unwrap().unwrap().id, first);
    }

    #[test]
    fn deltas_summed_over_range() {
        let store = Store::in_memory().unwrap();
        let inventory = fixtures::inventory();
        let first = store.record_snapshot(&inventory, at(1_000_000)).unwrap();
        let changed = Inventory::parse(&synced("6a9eeb1f000000000000c002")).unwrap();
        let second = store.record_snapshot(&changed, at(2_000_000)).unwrap();
        let again = Inventory::parse(&synced("6a9eeb1f000000000000c003")).unwrap();
        let third = store.record_snapshot(&again, at(3_000_000)).unwrap();

        let ferrite = |before, after| delta::ItemDelta {
            item_type: String::from(FERRITE),
            category: String::from("MiscItems"),
            before,
            after,
        };
        store
            .record_deltas(first, second, &[ferrite(10, 25)])
            .unwrap();
        store
            .record_deltas(second, third, &[ferrite(25, 20)])
            .unwrap();

        let all = store.deltas(TimeRange::all()).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].delta, 10);
        assert_eq!(all[0].category, "MiscItems");
        let latest = store.deltas(TimeRange::since(at(3_000_000))).unwrap();
        assert_eq!(latest[0].delta, -5);
        assert!(
            store
                .deltas(TimeRange::since(at(4_000_000)))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn reopen_on_disk_store() {
        let dir = scratch_dir("wf-core-store-test");
        let path = dir.join("merframe.sqlite3");
        let inventory = fixtures::inventory();
        let id = {
            let store = Store::open(&path).unwrap();
            store.record_snapshot(&inventory, at(1_000_000)).unwrap()
        };

        let reopened = Store::open(&path).unwrap();
        let latest = reopened.latest_snapshot().unwrap().unwrap();
        assert_eq!(latest.id, id);
        assert_eq!(
            reopened.record_snapshot(&inventory, at(3_000_000)).unwrap(),
            id
        );
        drop(reopened);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn inventory_cache_file() {
        let dir = scratch_dir("wf-core-store-inventory-cache");
        let path = dir.join("merframe.sqlite");
        let cache = dir.join("inventory.json.gz");

        let store = Store::open(&path).unwrap();
        assert!(store.cached_inventory().unwrap().is_none());
        store.cache_inventory(fixtures::INVENTORY).unwrap();
        assert!(!dir.join("inventory.json.part").exists());
        assert!(std::fs::read(&cache).unwrap().starts_with(&[0x1f, 0x8b]));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            let mode = std::fs::metadata(&cache).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        drop(store);

        let reopened = Store::open(&path).unwrap();
        assert_eq!(
            reopened.cached_inventory().unwrap().as_deref(),
            Some(fixtures::INVENTORY)
        );
        drop(reopened);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn trades_and_relic_openings() {
        let store = Store::in_memory().unwrap();
        let trade = Trade {
            offered: vec![TradeItem {
                name: String::from("Forma Blueprint"),
                count: 2,
                rank: None,
            }],
            received: Vec::new(),
            plat: 45,
        };
        store
            .record_trade(at(5_000_000), Some("SomePlayer"), &trade)
            .unwrap();
        let trades = store.trades(TimeRange::all()).unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].partner.as_deref(), Some("SomePlayer"));
        assert_eq!(trades[0].trade.plat, 45);
        assert_eq!(trades[0].trade.offered[0].count, 2);

        let edited = Trade {
            plat: 60,
            ..trade.clone()
        };
        store
            .update_trade(trades[0].id, at(5_500_000), None, &edited)
            .unwrap();
        let trades = store.trades(TimeRange::all()).unwrap();
        assert_eq!(trades[0].at, at(5_500_000));
        assert_eq!(trades[0].partner, None);
        assert_eq!(trades[0].trade.plat, 60);
        store.delete_trade(trades[0].id).unwrap();
        assert!(store.trades(TimeRange::all()).unwrap().is_empty());

        store
            .record_relic_opening(at(6_000_000), "Lith K12", "Forma Blueprint", 4)
            .unwrap();
        let openings = store.relic_openings(TimeRange::all()).unwrap();
        assert_eq!(openings.len(), 1);
        assert_eq!(openings[0].relic, "Lith K12");
        assert_eq!(openings[0].player_count, 4);
    }

    #[test]
    fn aya_in_series() {
        let store = Store::in_memory().unwrap();
        store
            .record_snapshot(&fixtures::inventory(), at(1_000_000))
            .unwrap();
        let series = store.stats_series(TimeRange::all()).unwrap();
        assert_eq!(series[0].aya, Some(11));
        assert_eq!(series[0].ducats, 937);
    }

    #[test]
    fn typed_settings() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.setting::<Vec<String>>("tiers").unwrap(), None);
        store
            .set_setting("tiers", &vec!["Lith".to_owned(), "Meso".to_owned()])
            .unwrap();
        store.set_setting("tiers", &vec!["Axi".to_owned()]).unwrap();
        assert_eq!(
            store.setting::<Vec<String>>("tiers").unwrap(),
            Some(vec!["Axi".to_owned()])
        );
    }

    #[test]
    fn riven_data_setting_round_trip() {
        let document = riven_table();
        let store = Store::in_memory().unwrap();
        store.set_setting("riven_data", &document).unwrap();
        let read = store
            .setting::<wf_market::RivenData>("riven_data")
            .unwrap()
            .unwrap();
        assert_eq!(read, document);
    }

    #[test]
    fn favourites_toggle_and_reopen() {
        let dir = scratch_dir("wf-core-store-favourites-test");
        let path = dir.join("merframe.sqlite3");
        let mirage = "/Lotus/Powersuits/Mirage/MirageWarframe";
        let styanax = "/Lotus/Types/Recipes/WarframeRecipes/StyanaxPrimeBlueprint";
        {
            let store = Store::open(&path).unwrap();
            assert!(store.favourites().unwrap().is_empty());
            assert!(store.toggle_favourite(mirage, at(1_000_000)).unwrap());
            assert!(store.toggle_favourite(styanax, at(1_000_001)).unwrap());
            assert!(!store.toggle_favourite(mirage, at(1_000_002)).unwrap());
            let favourites = store.favourites().unwrap();
            assert_eq!(favourites.len(), 1);
            assert!(favourites.contains(styanax));
            assert!(!favourites.contains(mirage));
        }

        let reopened = Store::open(&path).unwrap();
        let favourites = reopened.favourites().unwrap();
        assert_eq!(favourites.names().collect::<Vec<_>>(), [styanax]);
        let since: i64 = reopened
            .connection
            .query_row(
                "SELECT since FROM favourites WHERE unique_name = ?1",
                params![styanax],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(since, 1_000_001);
        drop(reopened);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn migrate_twice() {
        let store = Store::in_memory().unwrap();
        store.migrate().unwrap();
        let applied: i64 = store
            .connection
            .query_row("SELECT COUNT(*) FROM migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(applied, i64::try_from(MIGRATIONS.len()).unwrap());
    }
}
