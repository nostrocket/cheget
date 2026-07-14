---
phase: 02-persistence-storage
plan: 01
subsystem: infra
tags: [age, scrypt, rusqlite, secp256k1, bech32, home, rpassword, zeroize, atomic-write, msrv]

# Dependency graph
requires:
  - phase: 01-foundation
    provides: "manual error-enum idiom (ChainError), Zeroizing nonce discipline, Transport/ChainBackend trait-seam shape, CHEGET_* env prefix"
provides:
  - "Pinned Phase 2 dependency set (age, rusqlite bundled, secp256k1, bech32, home, rpassword) proven to build on MSRV 1.85"
  - "src/store/ foundation: StoreError (shared manual error enum) + StoreRoot (CHEGET_HOME / ~/.cheget resolution, 0700 create)"
  - "store::atomic::write_atomic — crash-safe temp+fsync+rename+dir-fsync writer + create_dir_secure (0700) (D-07)"
  - "store::crypto::{encrypt_secret, decrypt_secret} — age/scrypt one-shot, Zeroizing plaintext boundary (D-06)"
  - "store::passphrase::PassphraseSource seam — interactive (prod, gated) + in-code (test) impls; no passphrase env var/CLI flag (D-01/D-03)"
affects: [02-02, 02-03, 02-04, "identity-store", "checkpoint-store", "coordinator-sqlite"]

# Tech tracking
tech-stack:
  added:
    - "age 0.11.5 (scrypt Recipient/Identity one-shot at-rest encryption)"
    - "rusqlite 0.37.0 + bundled (libsqlite3-sys 0.35.0) — MSRV-safe downgrade from 0.40.1"
    - "secp256k1 0.29.1 (rand,std) — transport identity keypair (wired 02-02)"
    - "bech32 0.11.1 — npub encoding (wired 02-02)"
    - "home 0.5.9 — store-root resolution (MSRV-safe pin from 0.5.12)"
    - "rpassword 7.5.4 — no-echo interactive passphrase read"
  patterns:
    - "Store layer sits outside the pure crypto core; persistence handles only already-produced FROST bytes"
    - "Shared StoreError via the repo manual Debug + hand-written Display + empty Error idiom (no thiserror)"
    - "Two-impls-behind-one-trait seam (PassphraseSource) mirroring Transport/ChainBackend"

key-files:
  created:
    - "src/store/mod.rs"
    - "src/store/atomic.rs"
    - "src/store/crypto.rs"
    - "src/store/passphrase.rs"
  modified:
    - "Cargo.toml"
    - "Cargo.lock"
    - "src/lib.rs"

key-decisions:
  - "MSRV gate branch (b): rusqlite pinned 0.40.1 -> 0.37.0 (libsqlite3-sys 0.38.1 uses the 1.88-only cfg_select! macro; 0.35.0 does not). rusqlite kept `bundled`; reproducibility preserved, MSRV 1.85 unchanged."
  - "home pinned 0.5.12 -> 0.5.9: 0.5.12 declares rust-version 1.88, breaking the MSRV 1.85 build (second, unplanned MSRV violator alongside rusqlite)."
  - "rpassword legitimacy gate (Task 1) APPROVED by human orchestrator. NOTE: actual license is Apache-2.0, not MIT as the plan's how-to-verify text stated — a benign text discrepancy, not a red flag (48.6M downloads, author conradkleinespel, repo github.com/conradkleinespel/rpassword, 7.5.4 not yanked)."
  - "PassphraseSource::passphrase returns Result<SecretString, StoreError> (plan said 'returning SecretString') so the interactive impl can surface IO / confirmation-mismatch errors; in-code impl is infallible."
  - "scrypt work factor const SCRYPT_LOG_N = 18 (age interactive default); deliberate per D-09."
  - "InteractivePassphrase gated #[cfg(not(test))] so no test build links/blocks on a terminal prompt."

patterns-established:
  - "write_atomic: temp(create_new,0600) -> file fsync -> rename -> DIRECTORY fsync (Pitfall 2 durability); existing final never opened for truncation"
  - "decrypt_secret returns Zeroizing<Vec<u8>> so plaintext is wiped at the caller's drop (D-06)"
  - "CHEGET_HOME is a store-PATH override only, never a passphrase source"

requirements-completed: [STOR-01]

coverage:
  - id: D1
    description: "Full pinned dependency set (age, rusqlite bundled, secp256k1, bech32, home, rpassword) resolves and builds on MSRV 1.85"
    requirement: STOR-01
    verification:
      - kind: other
        ref: "cargo +1.85.0 check (clean build after rusqlite 0.37 / home 0.5.9 pins)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Atomic crash-safe write leaves no partial/truncated file and sets 0700 dir / 0600 file perms on Unix"
    requirement: STOR-01
    verification:
      - kind: unit
        ref: "src/store/atomic.rs#perms"
        status: pass
      - kind: unit
        ref: "src/store/atomic.rs#atomic_no_partial"
        status: pass
      - kind: unit
        ref: "src/store/atomic.rs#interrupted_write_does_not_truncate_existing"
        status: pass
    human_judgment: false
  - id: D3
    description: "age/scrypt encrypt->decrypt round-trips under a passphrase; wrong passphrase returns Err with no partial plaintext; decrypt returns Zeroizing"
    requirement: STOR-01
    verification:
      - kind: unit
        ref: "src/store/crypto.rs#age_roundtrip"
        status: pass
      - kind: unit
        ref: "src/store/crypto.rs#wrong_passphrase_fails"
        status: pass
    human_judgment: false
  - id: D4
    description: "PassphraseSource seam drives encrypt/decrypt headlessly via the in-code impl (test/CI path)"
    requirement: STOR-01
    verification:
      - kind: unit
        ref: "src/store/passphrase.rs#in_code_source_drives_roundtrip_headlessly"
        status: pass
    human_judgment: false
  - id: D5
    description: "Production passphrase path is interactive-only, no-echo, confirm-twice on create with unrecoverability warning; no passphrase env var/CLI flag ships"
    requirement: STOR-01
    verification:
      - kind: manual_procedural
        ref: "run `cheget` store-create at a real terminal; confirm no echo, double-prompt+match, warning printed"
        status: unknown
    human_judgment: true
    rationale: "No-echo terminal behavior and the absence of a passphrase env/flag are UX/security properties that need a human at a real TTY (end-of-phase human-verify); the interactive impl is cfg-gated out of test builds and cannot be exercised headlessly."

# Metrics
duration: 11min
completed: 2026-07-14
status: complete
---

# Phase 2 Plan 01: Store Substrate Summary

**age/scrypt at-rest encryption (Zeroizing boundary), a crash-safe atomic writer with 0700/0600 perms, and an interactive-only PassphraseSource seam — all on a dependency set proven to build on MSRV 1.85 after MSRV-driven rusqlite and home downgrades.**

## Performance

- **Duration:** 11 min
- **Started:** 2026-07-14T08:23:34Z
- **Completed:** 2026-07-14T08:34:58Z
- **Tasks:** 3 (Task 1 gate pre-approved; Tasks 2–3 executed)
- **Files modified:** 7 (4 created, 3 modified)

## Accomplishments
- Resolved RESEARCH Open Question 1 (rusqlite-vs-1.85): the full dependency set now builds clean on `cargo +1.85.0 check` after pinning rusqlite 0.40.1→0.37.0 and home 0.5.12→0.5.9.
- `store::atomic::write_atomic` implements the temp→file-fsync→rename→**directory-fsync** discipline (D-07, Pitfall 2); Unix perms 0700 dir / 0600 file; three unit tests including a failure-injection proof that an interrupted write never truncates a live file.
- `store::crypto` gives one-shot age/scrypt `encrypt_secret`/`decrypt_secret`; decrypt returns `Zeroizing<Vec<u8>>` (D-06); wrong-passphrase yields `StoreError::Age` with no partial plaintext.
- `store::passphrase::PassphraseSource` seam: production `InteractivePassphrase` (rpassword no-echo, confirm-twice + unrecoverability warning, `#[cfg(not(test))]`) and headless `InCodePassphrase`; no passphrase env var or CLI flag exists (D-01/D-03).
- `StoreError` shared error enum + `StoreRoot` (CHEGET_HOME / ~/.cheget, 0700) landed following the repo's manual error idiom (no thiserror).

## Task Commits

Each task was committed atomically:

1. **Task 1: rpassword legitimacy gate** — APPROVED (built nothing; record folded into Task 2's commit `ad83d59`)
2. **Task 2: deps + MSRV gate + store root + atomic writer** — `ad83d59` (feat)
3. **Task 3: age/scrypt crypto + PassphraseSource seam** — `6fca577` (test / RED) → `3d358b2` (feat / GREEN)

**Plan metadata:** committed separately (docs: complete plan).

## Files Created/Modified
- `Cargo.toml` — Phase 2 dependency block with per-line rationale; rusqlite `0.37` (bundled), home `0.5`.
- `Cargo.lock` — pinned rusqlite 0.37.0 / libsqlite3-sys 0.35.0 / home 0.5.9 (committed, reproducible).
- `src/lib.rs` — `pub mod store;` + module-map bullet ("persistence never enters the pure crypto core").
- `src/store/mod.rs` — `StoreError` (manual idiom) + `StoreRoot` + flat `pub use` surface.
- `src/store/atomic.rs` — `write_atomic` + `create_dir_secure`; unix perms + atomic-no-partial tests.
- `src/store/crypto.rs` — `encrypt_secret`/`decrypt_secret` (age scrypt, log_n=18, Zeroizing).
- `src/store/passphrase.rs` — `PassphraseSource` trait + interactive/in-code impls.

## Decisions Made
- **MSRV gate → branch (b):** rusqlite 0.40.1 pulls libsqlite3-sys 0.38.1, whose build script uses the `cfg_select!` macro (std, 1.88+). Pinned rusqlite `0.37.0` (libsqlite3-sys 0.35.0), which keeps the `bundled` feature and builds on 1.85. rust-version stays 1.85 — reproducibility requirement untouched. 02-04's schema will target rusqlite 0.37.
- **rpassword decision:** APPROVED (Task 1 blocking-human gate, pre-resolved by orchestrator). License is **Apache-2.0** (plan text said MIT) — noted as a benign discrepancy; the crate is `MIT OR Apache-2.0` dual-licensed in practice and the identity checks (downloads/author/repo/not-yanked) all passed.
- **scrypt log_n = 18** per D-09 (age interactive default) — makes the crypto unit tests take ~39s combined, which is expected for the real work factor and was kept rather than lowered.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Pinned `home` 0.5.12 → 0.5.9 to satisfy MSRV 1.85**
- **Found during:** Task 2 (MSRV gate)
- **Issue:** `home 0.5.12` declares `rust-version = 1.88`, so `cargo +1.85.0 check` refused to build. The plan's MSRV branch only anticipated rusqlite as the violator; `home` was a second, unplanned one.
- **Fix:** `cargo update home --precise 0.5.9` (0.5.9 predates the MSRV bump); committed the lock.
- **Files modified:** Cargo.lock
- **Verification:** `cargo +1.85.0 check` clean.
- **Committed in:** `ad83d59` (Task 2 commit)

**2. [Rule 2 - Missing Critical] PassphraseSource made fallible (`Result<SecretString, StoreError>`)**
- **Found during:** Task 3 (passphrase seam)
- **Issue:** The plan specified a method "returning `SecretString`", but the interactive prompt can fail (terminal IO error, confirm-twice mismatch); a non-fallible signature would force a panic on those paths.
- **Fix:** Trait method returns `Result<SecretString, StoreError>`; in-code impl returns `Ok`.
- **Files modified:** src/store/passphrase.rs
- **Verification:** headless roundtrip test passes; interactive impl returns `Err(StoreError::Age)` on mismatch.
- **Committed in:** `6fca577` / `3d358b2`

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 missing-critical)
**Impact on plan:** Both necessary for a shippable MSRV build and a non-panicking prompt. No scope creep. `age` also resolved to 0.11.5 (semver-compatible with the `0.11.3` floor).

## Issues Encountered
- **Pre-existing clippy failure (out of scope):** `cargo clippy --tests -- -D warnings` fails on `tests/dkg_100_correctness.rs:55` (`needless_borrows_for_generic_args`, Phase 1 code, commit c537bf0). `cargo clippy --lib -- -D warnings` — including all new store modules — is clean. Logged to `.planning/phases/02-persistence-storage/deferred-items.md`; not fixed per scope boundary.

## User Setup Required
None — no external service configuration required. (`CHEGET_HOME` is an optional store-path override for testing; the store passphrase is interactive-only by design.)

## Next Phase Readiness
- 02-02/02-03/02-04 can now encrypt (`store::crypto`), write atomically (`store::atomic`), acquire a passphrase (`store::passphrase`), and share the `StoreError`/`StoreRoot` substrate.
- 02-04 must target rusqlite **0.37** (bundled), not 0.40, per the MSRV pin.
- End-of-phase human-verify should exercise the interactive no-echo prompt (coverage D5) at a real terminal.

## Self-Check: PASSED

- All four `src/store/` modules present on disk.
- SUMMARY.md present.
- Task commits `ad83d59`, `6fca577`, `3d358b2` all present in git history.

---
*Phase: 02-persistence-storage*
*Completed: 2026-07-14*
