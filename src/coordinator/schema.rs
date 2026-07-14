//! `SCHEMA_V1` — the coordinator SQLite schema (STOR-03, D-11, D-15).
//!
//! **D-11:** every table here holds PUBLIC data only. No column carries a share,
//! a signing nonce, or a partial signature — the coordinator DB is never
//! age-encrypted precisely because it is safe to leak. A reviewer can read this
//! file top-to-bottom and confirm the absence of any secret column (T-02-14).
//!
//! **D-15 / Pitfall 16:** `identifier` is the lowercase hex of the frost
//! `Identifier.serialize()` bytes — the stable authority that survives refresh —
//! and it matches the participant manifest's `seat` spelling so the two indices
//! agree. `seat_index` is a nullable 1..=100 human convenience only.
//!
//! The schema is applied verbatim in one `execute_batch` under the `user_version`
//! migration gate in [`super::migrate`]; build-full-now is appropriate for the
//! greenfield table set, and the gate leaves room for Phase 4/5/7 to add
//! columns/tables incrementally.

/// The complete v1 coordinator schema. Applied once when `user_version` is `0`.
///
/// Five table classes — roster, ceremony_transcripts, session_logs,
/// policy_config (single-row, id=1, SPEC §10 defaults), churn_ledger — plus the
/// seed row for `policy_config`. Public data only (D-11).
pub const SCHEMA_V1: &str = r#"
-- Roster: identifier <-> npub <-> status <-> join/leave epochs (D-15). PUBLIC data.
CREATE TABLE roster (
  key_id        TEXT    NOT NULL,               -- "active" | "standby"
  identifier    TEXT    NOT NULL,               -- hex of frost Identifier.serialize() (stable across refresh)
  seat_index    INTEGER,                        -- 1..=100 convenience (nullable; identifier is authority)
  npub          TEXT    NOT NULL,               -- bech32 npub (real, from D-12 identity keys)
  status        TEXT    NOT NULL,               -- ACTIVE | STANDBY | RETIRED | REMOVED
  join_epoch    INTEGER NOT NULL,
  leave_epoch   INTEGER,                        -- NULL while active
  PRIMARY KEY (key_id, identifier)
);

-- Ceremony transcript: PUBLIC record of each DKG/refresh/enroll (event ids arrive in Phase 7).
CREATE TABLE ceremony_transcripts (
  ceremony_id          TEXT PRIMARY KEY,
  key_id               TEXT    NOT NULL,
  epoch                INTEGER NOT NULL,
  kind                 TEXT    NOT NULL,        -- dkg | refresh | enroll | repair
  group_verifying_key  BLOB,                    -- the pinned P (public); NULL until confirmed
  status               TEXT    NOT NULL,        -- open | complete | aborted
  started_at           INTEGER NOT NULL,        -- unix seconds
  completed_at         INTEGER
);

-- Session logs: PUBLIC record of signing/sweep sessions (no nonces, no partials).
CREATE TABLE session_logs (
  session_id   TEXT PRIMARY KEY,
  key_id       TEXT    NOT NULL,
  epoch        INTEGER NOT NULL,
  kind         TEXT    NOT NULL,               -- sign | sweep
  psbt_txid    TEXT,                           -- or sighash digest; identifies the tx signed
  subset       TEXT,                           -- json array of identifiers that signed
  outcome      TEXT    NOT NULL,               -- success | aborted | timeout
  created_at   INTEGER NOT NULL
);

-- Policy config: single-row (id=1) mirror of SPEC section 10 knobs (tables now; engine is Phase 5).
CREATE TABLE policy_config (
  id               INTEGER PRIMARY KEY CHECK (id = 1),
  value_cap        INTEGER,                    -- sats; operator-set (nullable until set)
  churn_budget     INTEGER NOT NULL DEFAULT 50,
  max_epochs       INTEGER NOT NULL DEFAULT 24,
  standby_max_age  INTEGER NOT NULL DEFAULT 7776000  -- 90 days in seconds
);
INSERT INTO policy_config (id) VALUES (1);

-- Churn ledger: distinct former holders since last DKG (feeds Phase 5 `watch`).
CREATE TABLE churn_ledger (
  key_id       TEXT    NOT NULL,
  identifier   TEXT    NOT NULL,
  npub         TEXT,
  left_epoch   INTEGER NOT NULL,
  recorded_at  INTEGER NOT NULL,
  PRIMARY KEY (key_id, identifier, left_epoch)
);
"#;

/// The schema version this build writes (the `user_version` migration gate).
pub const SCHEMA_VERSION: i64 = 1;
