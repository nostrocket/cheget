//! `CoordinatorStore` — the coordinator persona's crash-safe public SQLite state
//! (STOR-03).
//!
//! Holds the roster, ceremony transcripts, session logs, the single-row policy
//! config, and the churn ledger. **This database is PUBLIC by design (D-11):** it
//! carries no share, nonce, or partial signature, and it is therefore **never**
//! age-encrypted — unlike the participant [`crate::store::participant`] secret
//! path. A leaked coordinator DB reveals only what the roster and transcripts are
//! meant to be publicly committed to in Phase 7.
//!
//! On [`CoordinatorStore::open`] the connection is configured with
//! `journal_mode=WAL` and `synchronous=NORMAL` (crash-safe, single-writer
//! coordinator discipline, T-02-17), `foreign_keys=ON`, and a 5s `busy_timeout`,
//! then migrated through a `user_version` gate (`0 -> 1` builds
//! [`schema::SCHEMA_V1`] in full; the `v < 2` slot is reserved for Phase 4/5/7).
//!
//! rusqlite errors are folded into the shared [`StoreError::Sqlite`] face
//! following the repo's manual error-enum idiom (mirroring `ChainError`), via the
//! blanket `From<rusqlite::Error>` on `StoreError`.

pub mod schema;

use std::path::Path;

use rusqlite::Connection;

use crate::store::StoreError;
use schema::{SCHEMA_V1, SCHEMA_VERSION};

/// SPEC section 10 policy knobs mirrored into the single-row `policy_config`
/// table (the enforcement engine is Phase 5; this is the durable home now).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyConfig {
    /// Per-spend value cap in sats; operator-set, `None` until configured.
    pub value_cap: Option<i64>,
    /// Distinct-former-holder budget since the last DKG (SPEC default 50).
    pub churn_budget: i64,
    /// Maximum refresh epochs before a forced re-DKG (SPEC default 24).
    pub max_epochs: i64,
    /// Maximum standby-key age in seconds (SPEC default 7776000 = 90 days).
    pub standby_max_age: i64,
}

/// One roster seat: `identifier` (stable hex, the authority) mapped to its real
/// `npub` (D-15), lifecycle `status`, and join/leave epochs. PUBLIC data only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterEntry {
    /// Group-key label this seat belongs to (`"active"` | `"standby"`).
    pub key_id: String,
    /// Lowercase hex of the frost `Identifier` — stable across refresh (Pitfall 16).
    pub identifier: String,
    /// 1..=100 human convenience; nullable because `identifier` is the authority.
    pub seat_index: Option<i64>,
    /// The seat holder's real bech32 npub (from an [`crate::store::IdentityKeypair`]).
    pub npub: String,
    /// Lifecycle status: `ACTIVE` | `STANDBY` | `RETIRED` | `REMOVED`.
    pub status: String,
    /// Epoch at which this seat joined.
    pub join_epoch: i64,
    /// Epoch at which this seat left; `None` while still seated.
    pub leave_epoch: Option<i64>,
}

/// A PUBLIC record of one DKG/refresh/enroll/repair ceremony.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyTranscript {
    /// Stable ceremony identifier.
    pub ceremony_id: String,
    /// Group-key label the ceremony produced/refreshed.
    pub key_id: String,
    /// Epoch the ceremony established.
    pub epoch: i64,
    /// Ceremony kind: `dkg` | `refresh` | `enroll` | `repair`.
    pub kind: String,
    /// The pinned group verifying key `P` (public); `None` until confirmed.
    pub group_verifying_key: Option<Vec<u8>>,
    /// Status: `open` | `complete` | `aborted`.
    pub status: String,
    /// Start time, unix seconds.
    pub started_at: i64,
    /// Completion time, unix seconds; `None` while open.
    pub completed_at: Option<i64>,
}

/// A PUBLIC record of one signing/sweep session (never any nonce or partial).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionLog {
    /// Stable session identifier.
    pub session_id: String,
    /// Group-key label used.
    pub key_id: String,
    /// Epoch in force during the session.
    pub epoch: i64,
    /// Session kind: `sign` | `sweep`.
    pub kind: String,
    /// The signed tx's txid / sighash digest; `None` if not yet known.
    pub psbt_txid: Option<String>,
    /// JSON array of the identifiers that signed; `None` if not recorded.
    pub subset: Option<String>,
    /// Outcome: `success` | `aborted` | `timeout`.
    pub outcome: String,
    /// Creation time, unix seconds.
    pub created_at: i64,
}

/// One churn-ledger row: a distinct former holder since the last DKG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChurnEntry {
    /// Group-key label the seat belonged to.
    pub key_id: String,
    /// Stable hex identifier of the departed seat.
    pub identifier: String,
    /// The departed seat's npub, if known.
    pub npub: Option<String>,
    /// Epoch at which the seat left.
    pub left_epoch: i64,
    /// Time the departure was recorded, unix seconds.
    pub recorded_at: i64,
}

/// The coordinator's public SQLite store (STOR-03) — wraps a single connection.
pub struct CoordinatorStore {
    conn: Connection,
}

impl CoordinatorStore {
    /// The default coordinator DB path under a resolved store root:
    /// `<root>/coordinator/state.db`.
    pub fn default_db_path(root: &Path) -> std::path::PathBuf {
        root.join("coordinator").join("state.db")
    }

    /// Open (creating if absent) the coordinator DB at `path`, set the crash-safe
    /// pragmas, and migrate `user_version` `0 -> 1`.
    ///
    /// The DB's parent directory is created `0700` if missing. The DB is public
    /// (D-11) — it is deliberately NOT age-encrypted.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            crate::store::atomic::create_dir_secure(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Insert (or replace) a roster seat.
    pub fn insert_roster(&self, entry: &RosterEntry) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO roster \
             (key_id, identifier, seat_index, npub, status, join_epoch, leave_epoch) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                entry.key_id,
                entry.identifier,
                entry.seat_index,
                entry.npub,
                entry.status,
                entry.join_epoch,
                entry.leave_epoch,
            ],
        )?;
        Ok(())
    }

    /// List every roster seat for `key_id`, ordered by `identifier`.
    pub fn list_roster(&self, key_id: &str) -> Result<Vec<RosterEntry>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT key_id, identifier, seat_index, npub, status, join_epoch, leave_epoch \
             FROM roster WHERE key_id = ?1 ORDER BY identifier",
        )?;
        let rows = stmt.query_map([key_id], |r| {
            Ok(RosterEntry {
                key_id: r.get(0)?,
                identifier: r.get(1)?,
                seat_index: r.get(2)?,
                npub: r.get(3)?,
                status: r.get(4)?,
                join_epoch: r.get(5)?,
                leave_epoch: r.get(6)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(StoreError::Sqlite)
    }

    /// Insert (or replace) a ceremony transcript.
    pub fn insert_ceremony(&self, t: &CeremonyTranscript) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO ceremony_transcripts \
             (ceremony_id, key_id, epoch, kind, group_verifying_key, status, started_at, completed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                t.ceremony_id,
                t.key_id,
                t.epoch,
                t.kind,
                t.group_verifying_key,
                t.status,
                t.started_at,
                t.completed_at,
            ],
        )?;
        Ok(())
    }

    /// Fetch a ceremony transcript by its id.
    pub fn get_ceremony(&self, ceremony_id: &str) -> Result<Option<CeremonyTranscript>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT ceremony_id, key_id, epoch, kind, group_verifying_key, status, started_at, completed_at \
             FROM ceremony_transcripts WHERE ceremony_id = ?1",
        )?;
        let mut rows = stmt.query_map([ceremony_id], |r| {
            Ok(CeremonyTranscript {
                ceremony_id: r.get(0)?,
                key_id: r.get(1)?,
                epoch: r.get(2)?,
                kind: r.get(3)?,
                group_verifying_key: r.get(4)?,
                status: r.get(5)?,
                started_at: r.get(6)?,
                completed_at: r.get(7)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row.map_err(StoreError::Sqlite)?)),
            None => Ok(None),
        }
    }

    /// Insert (or replace) a signing/sweep session log.
    pub fn insert_session(&self, s: &SessionLog) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO session_logs \
             (session_id, key_id, epoch, kind, psbt_txid, subset, outcome, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                s.session_id,
                s.key_id,
                s.epoch,
                s.kind,
                s.psbt_txid,
                s.subset,
                s.outcome,
                s.created_at,
            ],
        )?;
        Ok(())
    }

    /// Fetch a session log by its id.
    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionLog>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, key_id, epoch, kind, psbt_txid, subset, outcome, created_at \
             FROM session_logs WHERE session_id = ?1",
        )?;
        let mut rows = stmt.query_map([session_id], |r| {
            Ok(SessionLog {
                session_id: r.get(0)?,
                key_id: r.get(1)?,
                epoch: r.get(2)?,
                kind: r.get(3)?,
                psbt_txid: r.get(4)?,
                subset: r.get(5)?,
                outcome: r.get(6)?,
                created_at: r.get(7)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row.map_err(StoreError::Sqlite)?)),
            None => Ok(None),
        }
    }

    /// Record (or replace) a churn-ledger entry.
    pub fn insert_churn(&self, c: &ChurnEntry) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO churn_ledger \
             (key_id, identifier, npub, left_epoch, recorded_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![c.key_id, c.identifier, c.npub, c.left_epoch, c.recorded_at],
        )?;
        Ok(())
    }

    /// List every churn-ledger entry for `key_id`, ordered by `left_epoch`.
    pub fn list_churn(&self, key_id: &str) -> Result<Vec<ChurnEntry>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT key_id, identifier, npub, left_epoch, recorded_at \
             FROM churn_ledger WHERE key_id = ?1 ORDER BY left_epoch, identifier",
        )?;
        let rows = stmt.query_map([key_id], |r| {
            Ok(ChurnEntry {
                key_id: r.get(0)?,
                identifier: r.get(1)?,
                npub: r.get(2)?,
                left_epoch: r.get(3)?,
                recorded_at: r.get(4)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(StoreError::Sqlite)
    }

    /// Read the single-row policy config (SPEC section 10 defaults seeded at migrate).
    pub fn policy_config(&self) -> Result<PolicyConfig, StoreError> {
        let cfg = self.conn.query_row(
            "SELECT value_cap, churn_budget, max_epochs, standby_max_age \
             FROM policy_config WHERE id = 1",
            [],
            |r| {
                Ok(PolicyConfig {
                    value_cap: r.get(0)?,
                    churn_budget: r.get(1)?,
                    max_epochs: r.get(2)?,
                    standby_max_age: r.get(3)?,
                })
            },
        )?;
        Ok(cfg)
    }
}

/// Migrate the connection forward through the `user_version` gate.
///
/// `0 -> 1` applies [`SCHEMA_V1`] in full (build-full-now). The `v < 2` slot is
/// intentionally left open for Phase 4/5/7 incremental steps.
fn migrate(conn: &Connection) -> Result<(), StoreError> {
    let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if v < 1 {
        // Apply the schema AND bump user_version in ONE transaction so the step
        // is all-or-nothing (T-02-17). SQLite DDL is transactional and
        // `PRAGMA user_version` participates in the enclosing transaction, so a
        // crash/rollback mid-migration leaves user_version at 0 with NO partial
        // schema — the next open() cleanly re-runs the whole batch instead of
        // failing forever on "table already exists". `unchecked_transaction`
        // gives a `Transaction` from a `&Connection` (migrate does not own a
        // `&mut`); rusqlite 0.37 API.
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(SCHEMA_V1)?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        tx.commit()?;
    }
    // future: if v < 2 { let tx = conn.unchecked_transaction()?; ... tx.commit()?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::IdentityKeypair;

    /// A unique scratch DB path under the system temp dir.
    fn temp_db() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("cheget-coord-{}-{}-{}", std::process::id(), nanos, n));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("state.db")
    }

    #[test]
    fn open_migrate() {
        let path = temp_db();
        let store = CoordinatorStore::open(&path).unwrap();

        // WAL journal mode and foreign_keys are set.
        let journal: String = store
            .conn
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .unwrap();
        assert_eq!(journal.to_lowercase(), "wal", "journal_mode must be WAL");
        let fk: i64 = store
            .conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1, "foreign_keys must be ON");

        // user_version migrated 0 -> 1.
        let uv: i64 = store
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(uv, SCHEMA_VERSION, "user_version must be migrated to 1");

        // Re-opening an existing DB is a no-op migration and still works.
        drop(store);
        let store2 = CoordinatorStore::open(&path).unwrap();
        let uv2: i64 = store2
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(uv2, SCHEMA_VERSION);

        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    /// A migration is atomic: if applying `SCHEMA_V1` fails partway, the whole
    /// step rolls back — `user_version` is NOT advanced and no partial tables
    /// are left behind (T-02-17). Simulates the exact broken pre-state the old
    /// non-atomic code could leave (a table already exists while
    /// `user_version == 0`), then proves the transaction wrapper does not
    /// half-apply.
    #[test]
    fn migrate_is_atomic_on_failure() {
        let path = temp_db();
        let conn = Connection::open(&path).unwrap();

        // Pre-create `roster` with user_version still 0 — as a mid-batch crash
        // under the old code could have left it. Applying SCHEMA_V1 now conflicts
        // on `CREATE TABLE roster` partway through the batch.
        conn.execute_batch("CREATE TABLE roster (x INTEGER);").unwrap();
        let v0: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v0, 0, "precondition: user_version starts at 0");

        assert!(
            migrate(&conn).is_err(),
            "a conflicting migration batch must fail"
        );

        // The whole transaction rolled back: user_version untouched...
        let v_after: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(
            v_after, 0,
            "a failed migration must NOT advance user_version"
        );

        // ...and none of the later v1 tables were committed.
        let policy_tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master \
                 WHERE type = 'table' AND name = 'policy_config'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            policy_tables, 0,
            "a rolled-back migration must NOT leave partial tables"
        );

        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn roster_roundtrip() {
        let path = temp_db();
        let store = CoordinatorStore::open(&path).unwrap();

        // A REAL npub from a Phase 2 identity key (D-15) — not a placeholder.
        let id = IdentityKeypair::generate();
        let npub = id.npub();
        assert!(npub.starts_with("npub1"));

        let entry = RosterEntry {
            key_id: "active".to_string(),
            identifier: "0100000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            seat_index: Some(1),
            npub: npub.clone(),
            status: "ACTIVE".to_string(),
            join_epoch: 0,
            leave_epoch: None,
        };
        store.insert_roster(&entry).unwrap();

        let back = store.list_roster("active").unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0], entry, "roster row must roundtrip identically");
        assert_eq!(back[0].npub, npub, "roster npub must be the real identity npub");

        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn tables() {
        let path = temp_db();
        let store = CoordinatorStore::open(&path).unwrap();

        // policy_config seed row carries the SPEC section 10 defaults.
        let cfg = store.policy_config().unwrap();
        assert_eq!(cfg.value_cap, None);
        assert_eq!(cfg.churn_budget, 50);
        assert_eq!(cfg.max_epochs, 24);
        assert_eq!(cfg.standby_max_age, 7_776_000);

        // ceremony_transcripts roundtrip.
        let cer = CeremonyTranscript {
            ceremony_id: "cer-1".to_string(),
            key_id: "active".to_string(),
            epoch: 0,
            kind: "dkg".to_string(),
            group_verifying_key: Some(vec![1, 2, 3, 4]),
            status: "complete".to_string(),
            started_at: 1_752_000_000,
            completed_at: Some(1_752_000_100),
        };
        store.insert_ceremony(&cer).unwrap();
        assert_eq!(store.get_ceremony("cer-1").unwrap().as_ref(), Some(&cer));
        assert_eq!(store.get_ceremony("missing").unwrap(), None);

        // session_logs roundtrip.
        let sess = SessionLog {
            session_id: "sess-1".to_string(),
            key_id: "active".to_string(),
            epoch: 0,
            kind: "sign".to_string(),
            psbt_txid: Some("deadbeef".to_string()),
            subset: Some("[\"0100\"]".to_string()),
            outcome: "success".to_string(),
            created_at: 1_752_000_200,
        };
        store.insert_session(&sess).unwrap();
        assert_eq!(store.get_session("sess-1").unwrap().as_ref(), Some(&sess));

        // churn_ledger roundtrip.
        let churn = ChurnEntry {
            key_id: "active".to_string(),
            identifier: "0200000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            npub: Some("npub1example".to_string()),
            left_epoch: 1,
            recorded_at: 1_752_000_300,
        };
        store.insert_churn(&churn).unwrap();
        let back = store.list_churn("active").unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0], churn);

        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }
}
