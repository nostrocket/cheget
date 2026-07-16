---
phase: 03-dkg-at-scale-local
plan: 01
subsystem: cli
tags: [frost, dkg, keygen, store, age, scrypt, passphrase, persistence]

# Dependency graph
requires:
  - phase: 02-participant-store
    provides: "ParticipantStore::put_share/load_share/load_public_envelope/read_manifest, PassphraseSource trait, InteractivePassphrase/InCodePassphrase, ShareTag, ShareState, D-07 write ordering"
  - phase: 01-foundation
    provides: "run_inprocess_dkg, PubkeyEnvelope::from_package/decode_package, KeyId/Epoch/SeatId newtypes, keygen CLI handler"
provides:
  - "cheget participant keygen --persist: in-process DKG -> 100 per-seat encrypted store roots under one prompt-once passphrase"
  - "ResolvedPassphrase: prompt-once passphrase reuse seam (D-04), production + test builds"
  - "persist_dkg_shares(base,t,n,&dyn PassphraseSource): pub, test-drivable non-interactive write glue"
  - "crate::cli::resolve_root now pub(crate); acquire_store_passphrase cfg-split CLI edge"
  - "InteractivePassphrase re-exported (cfg(not(test)))"
affects: [03-02-sign-from-store, phase-02-uat-test-1]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "prompt-once-reuse: acquire SecretString once at CLI edge, wrap in ResolvedPassphrase clones for N stores"
    - "cfg-split interactive edge: #[cfg(not(test))] prompts, #[cfg(test)] returns fixed secret so lib compiles under cargo test"
    - "test-drivable write helper: pub fn takes &dyn PassphraseSource so tests inject InCodePassphrase without a terminal"

key-files:
  created:
    - tests/keygen_persist.rs
  modified:
    - src/store/passphrase.rs
    - src/store/mod.rs
    - src/cli/mod.rs
    - src/cli/keygen.rs

key-decisions:
  - "Per-seat root topology (D-03): each seat gets its own <base>/seat-NNNN/ ParticipantStore, not one store holding 100 tagged shares."
  - "persist_dkg_shares is pub (not pub(crate)) so tests/ (external crate linkage) can drive the real write glue instead of re-implementing it."
  - "keygen requires at least one of --out/--persist; --out-only keeps the Phase-1 standalone public-envelope path unchanged."
  - "D-07 disposition: tests/store_checkpoint_n100.rs::persist_reload_100 kept #[ignore]d as the at-scale durability check; the small-n keygen_persist test is the criterion-3 vehicle via the real command."

patterns-established:
  - "Prompt-once passphrase reuse: ResolvedPassphrase wraps one acquired SecretString and hands out clones (D-04)."
  - "Interactive prompt stays at the thin CLI edge (acquire_store_passphrase); persist/load loops are passphrase-source-generic (hazard 3)."

requirements-completed: [KEY-06]

coverage:
  - id: D1
    description: "ResolvedPassphrase prompt-once reuse seam: repeated passphrase() calls agree and two per-seat sources from one secret decrypt interchangeably (D-04)."
    requirement: "KEY-06"
    verification:
      - kind: unit
        ref: "src/store/passphrase.rs#resolved_passphrase_reuses_one_secret_across_calls_and_stores"
        status: pass
    human_judgment: false
  - id: D2
    description: "persist_dkg_shares writes per-seat encrypted roots that reload byte-equal, with a decodable public envelope and a one-entry Active manifest (KEY-06 write half)."
    requirement: "KEY-06"
    verification:
      - kind: integration
        ref: "tests/keygen_persist.rs#persist_dkg_shares_writes_reloadable_per_seat_roots"
        status: pass
    human_judgment: false
  - id: D3
    description: "keygen --persist / --base surface exists and delegates to persist_dkg_shares; --out still writes the public envelope; crypto core stays I/O-free."
    requirement: "KEY-06"
    verification:
      - kind: other
        ref: "cargo run -q -- participant keygen --help (lists --persist, --base); grep -rn 'use std::fs|crate::store' src/crypto/ (empty)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Interactive for_new_store confirm-twice UX exercised end-to-end by a real keygen --persist run (unblocks Phase 2 UAT Test 1)."
    verification: []
    human_judgment: true
    rationale: "The interactive prompt is #[cfg(not(test))] and cannot run under automated tests; the confirm-twice/no-echo/warning UX needs a human at a terminal (deferred to /gsd-verify-work 02 post-phase)."

# Metrics
duration: 25min
completed: 2026-07-16
status: complete
---

# Phase 3 Plan 01: keygen persist wiring Summary

**`cheget participant keygen --persist` now runs the in-process DKG and writes one age/scrypt-encrypted `seat-NNNN/` store root per seat under a single prompt-once passphrase, delivering the KEY-06 write half via a test-drivable `persist_dkg_shares` helper.**

## Performance

- **Duration:** ~25 min (dominated by scrypt log_n=18 test runs: ~75s lib test, ~109s integration test)
- **Started:** 2026-07-16T13:47:00Z (approx)
- **Completed:** 2026-07-16T13:54:19Z
- **Tasks:** 3
- **Files modified:** 4 modified + 1 created

## Accomplishments
- Added `ResolvedPassphrase` — the prompt-once passphrase reuse seam (D-04): one acquired `SecretString` served to all 100 per-seat stores via clones, never re-prompting, never env/flag-sourced.
- Extracted `persist_dkg_shares(base, t, n, &dyn PassphraseSource)` — the entire non-interactive write glue (DKG -> per-seat `seat-NNNN` root -> `put_share` loop) — `pub` so integration tests drive the real handler code instead of re-implementing it.
- Rewired `keygen::run`: `--persist` resolves the store base, prompts once via the confirm-twice `for_new_store` path, then delegates; `--out` still writes the public envelope; both are composable.
- Re-exported `InteractivePassphrase` (cfg(not(test))) and made `resolve_root` `pub(crate)`; added the cfg-split `acquire_store_passphrase` edge so the lib compiles in both profiles (hazards 1 and 3).
- New small-n integration test proves per-seat byte-equal share reload, public-envelope verifying-key match, and a one-entry Active manifest per root.

## Task Commits

Each task was committed atomically:

1. **Task 1: Prompt-once passphrase seam + re-exports + cfg-split CLI edge** (TDD) - `6e9e615` (test RED) → `a1c0dee` (feat GREEN)
2. **Task 2: Extract persist_dkg_shares + rewire keygen::run** - `ec9cb15` (feat)
3. **Task 3: Small-n writer-correctness test + D-07 disposition** - `90f3cff` (test)

_Note: Task 1 was TDD (failing test → implementation)._

## Files Created/Modified
- `src/store/passphrase.rs` - Added `ResolvedPassphrase` struct + `PassphraseSource` impl (D-04 reuse seam) and its unit test.
- `src/store/mod.rs` - Re-export `ResolvedPassphrase`; re-export `InteractivePassphrase` under `cfg(not(test))` (hazard 1).
- `src/cli/mod.rs` - `resolve_root` → `pub(crate)`; added cfg-split `acquire_store_passphrase(confirm)` (interactive edge in prod, fixed secret in test — hazard 3).
- `src/cli/keygen.rs` - `--out` now `Option`; added `--persist`/`--base`; extracted `pub fn persist_dkg_shares`; `run` prompts once then delegates; added `write_public_envelope` helper.
- `tests/keygen_persist.rs` - Small-n (default t=2,n=3, env-overridable) test driving `persist_dkg_shares`.

## Decisions Made
- Per-seat root topology (D-03): each seat is its own `<base>/seat-NNNN/` store, keyed 1-based off the stable `BTreeMap` iteration order.
- `persist_dkg_shares` is `pub` (not `pub(crate)`) — the lib target means `tests/` links `cheget` as an external crate, so `pub(crate)` would be unreachable.
- D-07 disposition: `store_checkpoint_n100.rs::persist_reload_100` kept `#[ignore]`d and unchanged as the on-demand at-scale durability harness; the fast small-n test is the criterion-3 vehicle through the real command.

## Deviations from Plan
None - plan executed exactly as written.

## Issues Encountered
None. The scrypt cost (age/scrypt log_n=18, ~1s per op) makes even the small-n test slow (~109s for n=3), as anticipated in the plan; correctness is unaffected.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- 03-02 (`sign` from store) can now reuse `crate::cli::resolve_root` and load persisted shares written here; the per-seat `seat-NNNN` topology and `KeyId::active()`/`Epoch::GENESIS` tagging are the read contract.
- Phase 2 UAT Test 1 is unblocked in principle: `keygen --persist` is the command that finally drives `InteractivePassphrase::for_new_store`. Interactive confirm-twice/no-echo UX still needs a human terminal check (D4, deferred to `/gsd-verify-work 02`).

## Self-Check: PASSED
- All 5 target files exist on disk.
- All 4 task commits present (`6e9e615`, `a1c0dee`, `ec9cb15`, `90f3cff`).
- `cargo test --lib store::passphrase` and `cargo test --test keygen_persist` pass; `cargo build` and `cargo test --no-run` succeed; `src/crypto/` gains no fs/store dependency.

---
*Phase: 03-dkg-at-scale-local*
*Completed: 2026-07-16*
