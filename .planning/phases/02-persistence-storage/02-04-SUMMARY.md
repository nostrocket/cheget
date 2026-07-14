---
phase: 02-persistence-storage
plan: 04
subsystem: database
tags: [rusqlite, sqlite, wal, coordinator, roster, clap, cli, frost, age, headless-ci]

# Dependency graph
requires:
  - phase: 02-01
    provides: StoreError surface, StoreRoot resolution, rusqlite 0.37 pin, atomic write/dir helpers
  - phase: 02-02
    provides: ParticipantStore, Manifest/ShareEntry/ShareState, IdentityKeypair::npub (D-15), PassphraseSource seam
provides:
  - "CoordinatorStore — crash-safe public SQLite state (WAL + foreign_keys, user_version 0->1 migration)"
  - "SCHEMA_V1 — roster, ceremony_transcripts, session_logs, single-row policy_config (SPEC section 10 defaults), churn_ledger; public data only (D-11)"
  - "cheget participant share-status — lists held shares from the store with no unlock (D-05)"
  - "cheget coordinator roster — lists the roster from the coordinator DB"
  - "ParticipantStore::read_manifest — static unlock-free manifest read path"
  - "tests/store_headless.rs — headless persist/reload proving the in-code PassphraseSource CI seam (D-03)"
affects: [03-transport, 04-rotation, 05-policy-watch-sweep, 07-nostr-roster-commit]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SQLite open+pragma+user_version-gate migration (build-full-now, v<2 slot reserved)"
    - "INSERT OR REPLACE + query_map roundtrip CRUD wrapping rusqlite::Error into StoreError::Sqlite"
    - "Static unlock-free read path (ParticipantStore::read_manifest) — reads never construct a PassphraseSource"

key-files:
  created:
    - src/coordinator/mod.rs
    - src/coordinator/schema.rs
    - tests/store_headless.rs
  modified:
    - src/lib.rs
    - src/cli/mod.rs
    - src/store/participant.rs

key-decisions:
  - "Coordinator DB is public (D-11) and never age-encrypted; a reviewer can confirm no secret column in schema.rs"
  - "identifier = hex of frost Identifier.serialize() is the roster authority (Pitfall 16); seat_index is a nullable convenience"
  - "share-status reads only the plaintext manifest via a static read_manifest — no PassphraseSource is constructed, so it can never prompt (T-02-16)"
  - "coordinator DB default path is <root>/coordinator/state.db; open() creates the parent dir 0700"

patterns-established:
  - "SQLite migration gate: pragma user_version, execute_batch(SCHEMA_V1) when v<1, bump to 1"
  - "CLI stays routing-only — new share-status/roster handlers delegate to store/coordinator library functions"

requirements-completed: [STOR-03]

coverage:
  - id: D1
    description: "CoordinatorStore opens with WAL + foreign_keys and migrates user_version 0->1"
    requirement: STOR-03
    verification:
      - kind: unit
        ref: "src/coordinator/mod.rs#coordinator::tests::open_migrate"
        status: pass
    human_judgment: false
  - id: D2
    description: "Roster/ceremony/session/policy-defaults/churn all insert+query roundtrip with real npubs and no secret columns"
    requirement: STOR-03
    verification:
      - kind: unit
        ref: "src/coordinator/mod.rs#coordinator::tests::roster_roundtrip"
        status: pass
      - kind: unit
        ref: "src/coordinator/mod.rs#coordinator::tests::tables"
        status: pass
    human_judgment: false
  - id: D3
    description: "cheget participant share-status lists held shares from the store with no unlock and no --pubkey file"
    requirement: STOR-03
    verification:
      - kind: manual_procedural
        ref: "CHEGET_HOME=<tmp> cargo run -- participant share-status (prints 'no shares held' with no prompt)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Headless PassphraseSource drives full persist/reload with no TTY; public package + manifest read unlock-free"
    requirement: STOR-03
    verification:
      - kind: integration
        ref: "tests/store_headless.rs#headless_persist_reload_no_prompt"
        status: pass
    human_judgment: false

# Metrics
duration: 24min
completed: 2026-07-14
status: complete
---

# Phase 2 Plan 04: Coordinator SQLite Store + CLI Store Wiring Summary

**CoordinatorStore (WAL, user_version-gated SCHEMA_V1: roster/transcripts/session-logs/policy/churn, public data only) plus unlock-free `participant share-status` / `coordinator roster` CLI and a headless persist/reload integration test.**

## Performance

- **Duration:** ~24 min
- **Started:** 2026-07-14T16:58Z (approx)
- **Completed:** 2026-07-14T17:22Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments
- `CoordinatorStore` opens a crash-safe SQLite DB (`journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`, 5s busy_timeout) and migrates `user_version` 0→1 by applying `SCHEMA_V1` in full, leaving a `v<2` slot for Phase 4/5/7.
- `SCHEMA_V1` defines the five public table classes — roster, ceremony_transcripts, session_logs, single-row `policy_config` (SPEC §10 defaults 50/24/7776000), churn_ledger — with **no** share/nonce/partial column (D-11, T-02-14). Roster roundtrips a **real** `IdentityKeypair::npub()` (D-15).
- `cheget participant share-status` lists held shares (key_id, epoch, seat, state) from the plaintext manifest with **no unlock** and no `--pubkey` file (D-05); `cheget coordinator roster` lists the roster from the coordinator DB.
- `tests/store_headless.rs` drives `put_share`→`load_share` byte-equal via `CHEGET_HOME` + the in-code `PassphraseSource` with no prompt/no TTY, and reads the public envelope + manifest unlock-free (D-03/D-05). Runs in the default suite.
- Coordinator DB builds on MSRV 1.85 with the 02-01-pinned rusqlite 0.37.

## Task Commits

Each task was committed atomically:

1. **Task 1: CoordinatorStore — open/migrate + SCHEMA_V1** - `4e4289f` (feat)
2. **Task 2: CLI wiring — share-status + coordinator roster** - `a4f2808` (feat)
3. **Task 3: Headless PassphraseSource integration test** - `e723a21` (test)

_Task 3 is a single `test` commit: the store substrate it exercises was already delivered in waves 1–3, so the RED/GREEN cycle collapses to a validation test (see Issues Encountered)._

## Files Created/Modified
- `src/coordinator/schema.rs` - `SCHEMA_V1` (five public table classes + policy seed row) and `SCHEMA_VERSION` gate
- `src/coordinator/mod.rs` - `CoordinatorStore` open/migrate + insert/query CRUD for every table; `PolicyConfig`/`RosterEntry`/`CeremonyTranscript`/`SessionLog`/`ChurnEntry` public types; `default_db_path`
- `src/lib.rs` - `pub mod coordinator;` + module-map bullet
- `src/cli/mod.rs` - `ParticipantCmd::ShareStatus` + `CoordinatorCmd::Roster` args/handlers/dispatch; `resolve_root` helper (routing-only)
- `src/store/participant.rs` - `ParticipantStore::read_manifest` static unlock-free read path
- `tests/store_headless.rs` - headless persist/reload integration test

## Decisions Made
- **Static unlock-free manifest read:** rather than construct a `ParticipantStore` (which requires a `PassphraseSource`) for `share-status`, added `ParticipantStore::read_manifest(root)` which reads `manifest.json` directly with no passphrase source at all. This keeps the in-code `InCodePassphrase` out of the shipped CLI path entirely (T-02-16) — a stronger guarantee than "constructed but never invoked".
- **`--home` CLI override:** both new subcommands accept an optional `--home` on top of `CHEGET_HOME`/`~/.cheget` resolution, so the commands are testable without mutating process env.
- Followed plan for everything else (schema verbatim from RESEARCH, WAL pragmas, real-npub roster).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Coordinator DB parent-dir creation on open**
- **Found during:** Task 2 (wiring `coordinator roster`)
- **Issue:** `Connection::open` does not create missing parent directories, so `coordinator roster` against a fresh `~/.cheget` would fail with an I/O error before the roster could be read.
- **Fix:** `CoordinatorStore::open` now creates the DB's parent directory `0700` (via `store::atomic::create_dir_secure`) before opening; added `CoordinatorStore::default_db_path`.
- **Files modified:** src/coordinator/mod.rs
- **Verification:** `CHEGET_HOME=<tmp> cargo run -- coordinator roster` prints `roster empty for key_id=active` from a non-existent store; unit tests still green.
- **Committed in:** `a4f2808` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 missing critical)
**Impact on plan:** Necessary for the coordinator command to work from a fresh store. No scope creep.

## Issues Encountered
- **TDD RED/GREEN collapse (Task 3):** the plan marks Task 3 `tdd="true"`, but the store substrate it validates (`ParticipantStore`, in-code `PassphraseSource`, DKG) was already shipped in waves 1–3. Per the fail-fast rule, a test that passes immediately against pre-existing implementation is expected here — Task 3 is a validation/regression test, not new behavior, so it is a single `test` commit.
- **Out-of-scope clippy warnings (deferred):** `cargo clippy --tests` flags `needless_borrows_for_generic_args` in `src/store/checkpoint.rs:305/:360` (02-03 code) and `tests/dkg_100_correctness.rs:55` (Phase 1 code). Not touched by 02-04. `cargo clippy --lib --bins -- -D warnings` is clean including all 02-04 files. Logged in `deferred-items.md`.

## TDD Gate Compliance
Task 1 (`tdd="true"`): a single `feat` commit carries both the `#[cfg(test)]` unit tests (`open_migrate`, `roster_roundtrip`, `tables`) and the implementation. The tests were authored against the RESEARCH schema before the CRUD methods were fleshed out and drove the API shape; they are committed together as one atomic unit rather than split RED/GREEN commits. Task 3 (`tdd="true"`) validates pre-existing substrate (see Issues Encountered).

## Threat Mitigations Verified
- **T-02-14 (secret leaked into coordinator DB):** `grep -niE 'share|nonce|partial|secret|signing_key|private' src/coordinator/schema.rs` matches only doc comments — no such column exists. DB never age-encrypted (D-11).
- **T-02-15 (roster npub not the real identity key):** `roster_roundtrip` populates the row from `IdentityKeypair::generate().npub()` and asserts the stored npub equals it; identifier stored as stable hex (Pitfall 16).
- **T-02-16 (in-code PassphraseSource reachable in shipped binary):** `share-status` reads via `read_manifest` which constructs no `PassphraseSource`; `InteractivePassphrase` remains the only production source and stays `#[cfg(not(test))]`.
- **T-02-17 (concurrent-writer corruption):** WAL + `busy_timeout(5s)` set on every open.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Durable-state foundation for Phase 2 is complete: participant secret store (02-01/02-02), identity + manifest (02-02), DKG checkpoints (02-03), and now the coordinator public store (02-04).
- Phase 3 (transport) and headless n=100 persist/reload checks can drive the store end-to-end with no TTY via the in-code `PassphraseSource`.
- Roster/transcript tables carry the real npubs Phase 7's roster-hash-commit will hash.

## Self-Check: PASSED

---
*Phase: 02-persistence-storage*
*Completed: 2026-07-14*
