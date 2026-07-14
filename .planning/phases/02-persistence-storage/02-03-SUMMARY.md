---
phase: 02-persistence-storage
plan: 03
subsystem: database
tags: [frost, dkg, checkpoint, age, scrypt, zeroize, trybuild, STOR-02]

# Dependency graph
requires:
  - phase: 02-01
    provides: "store::atomic::write_atomic, store::crypto::{encrypt_secret,decrypt_secret}, StoreError, StoreRoot"
  - phase: 02-02
    provides: "PassphraseSource seam (InCodePassphrase), manifest::seat_hex, ParticipantStore, crypto::types::{KeyId,Epoch,SeatId}"
provides:
  - "CheckpointStore: encrypted between-round DKG checkpoint persistence (STOR-02, DKG-round-secret half)"
  - "CeremonyId newtype with path-traversal validation"
  - "Concrete type-restricted put/load_round1/round2 for dkg::round{1,2}::SecretPackage (no generic persist)"
  - "wipe-on-success / keep-on-abort semantics (D-10)"
  - "Store-side nonce guard: compile-fail proofs that nonce material is a non-expressible checkpoint input"
  - "#[ignore]d n=100 persist/reload harness (persist_reload_100) built for Phase 3"
affects: [phase-03, dkg-ceremony, membership-rotation, resharing]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Type-restricted persistence API (concrete dkg SecretPackage types only; no generic persist) — the structural inverse of the non-serializable EphemeralNonces"
    - "Compile-fail trybuild snapshot as the reviewable structural control (mirrors nonce_no_serialize.rs)"
    - "CeremonyId path-component validation as an input-validation guard against traversal"

key-files:
  created:
    - src/store/checkpoint.rs
    - tests/store_checkpoint_n100.rs
    - tests/ui/checkpoint_no_nonce.rs
    - tests/ui/checkpoint_no_nonce.stderr
    - tests/ui/checkpoint_no_generic_persist.rs
    - tests/ui/checkpoint_no_generic_persist.stderr
  modified:
    - src/store/mod.rs
    - tests/compile_fail.rs

key-decisions:
  - "CeremonyId::new validates a single safe path component ([A-Za-z0-9_-]) to block traversal (Rule 2, T-02-12) — was not spelled out in the plan"
  - "n=100 harness defaults to full t=51/n=100 (CHEGET_N100_T/N overrides) so Phase 3 runs it with no edit, matching the dkg_100_correctness convention"
  - "Two compile-fail proofs (nonce-rejection E0308 + no-generic-persist E0599) chosen over a runtime string check — the control is the API shape, per Pitfall 1"

patterns-established:
  - "Type-restricted store API: concrete-typed methods, no generic persist, proven non-expressible by compile-fail snapshot"
  - "Best-effort-hygiene wipe (never a security control) with keep-on-abort resume"

requirements-completed: [STOR-02]

coverage:
  - id: D1
    description: "DKG round1/round2 SecretPackages checkpoint (encrypted) and reload byte-faithfully via real dkg::part1/part2"
    requirement: "STOR-02"
    verification:
      - kind: unit
        ref: "src/store/checkpoint.rs#store::checkpoint::tests::dkg_roundtrip"
        status: pass
    human_judgment: false
  - id: D2
    description: "wipe-on-success removes a ceremony's checkpoint files; keep-on-abort leaves them for resume"
    requirement: "STOR-02"
    verification:
      - kind: unit
        ref: "src/store/checkpoint.rs#store::checkpoint::tests::wipe_vs_keep"
        status: pass
    human_judgment: false
  - id: D3
    description: "CeremonyId rejects path-traversal ids"
    requirement: "STOR-02"
    verification:
      - kind: unit
        ref: "src/store/checkpoint.rs#store::checkpoint::tests::ceremony_id_rejects_traversal"
        status: pass
    human_judgment: false
  - id: D4
    description: "Checkpoint API exposes no method accepting EphemeralNonces/SigningNonces and no generic persist — a nonce is a non-expressible checkpoint input (highest-severity structural control)"
    requirement: "STOR-02"
    verification:
      - kind: unit
        ref: "tests/compile_fail.rs#checkpoint_rejects_nonce_material (tests/ui/checkpoint_no_nonce.rs)"
        status: pass
      - kind: unit
        ref: "tests/compile_fail.rs#checkpoint_has_no_generic_persist (tests/ui/checkpoint_no_generic_persist.rs)"
        status: pass
    human_judgment: false
  - id: D5
    description: "#[ignore]d n=100 persist/reload harness compiles and is excluded from the default suite, ready to run at 51/100 in Phase 3"
    requirement: "STOR-02"
    verification:
      - kind: integration
        ref: "cargo test --test store_checkpoint_n100 -- --list (persist_reload_100, ignored)"
        status: pass
    human_judgment: false

# Metrics
duration: 16min
completed: 2026-07-14
status: complete
---

# Phase 2 Plan 3: DKG Checkpoint Store Summary

**Type-restricted `CheckpointStore` that persists dkg::round{1,2}::SecretPackage encrypted between rounds (age/scrypt under the store passphrase) with wipe-on-success/keep-on-abort, plus compile-fail proofs that a signing nonce is a non-expressible checkpoint input (STOR-02).**

## Performance

- **Duration:** ~16 min
- **Started:** 2026-07-14T08:57Z
- **Completed:** 2026-07-14T09:12Z
- **Tasks:** 2
- **Files modified:** 8 (6 created, 2 modified)

## Accomplishments
- `CheckpointStore` with concrete `put/load_round1` and `put/load_round2` typed to `dkg::round{1,2}::SecretPackage` only — no generic persist. Each put is `serialize()` → `Zeroizing` → `encrypt_secret` (same store passphrase, D-09) → `write_atomic` (crash-safe, D-07) to `ceremonies/<cid>/<seat>/round-N.age` (D-11); each load reverses it with the plaintext scoped to the call (D-06).
- `CeremonyId` newtype validating a single safe path component, blocking traversal (T-02-12).
- `wipe(cid)` removes a ceremony dir on success (idempotent), labelled best-effort hygiene; keep-on-abort retains files for `(ceremony_id, round, seat)` resume (D-10).
- Two compile-fail trybuild proofs — `checkpoint_no_nonce.rs` (E0308: `put_round1/2` reject `&EphemeralNonces`) and `checkpoint_no_generic_persist.rs` (E0599: no generic `persist`) — the reviewable structural control that a nonce cannot be checkpointed (T-02-10). Mirrors `tests/ui/nonce_no_serialize.rs`.
- `#[ignore]`d `persist_reload_100` harness in `tests/store_checkpoint_n100.rs`, driving a real DKG + full-share-set persist/reload through the participant + checkpoint stores; excluded from the default suite, ready for Phase 3 at 51/100.
- `run_inprocess_dkg` / `src/crypto/keygen.rs` untouched (git diff empty) — persistence stayed out of the pure crypto core (D-08).

## Task Commits

1. **Task 1: CheckpointStore — type-restricted dkg SecretPackage persistence** — `8379392` (feat)
2. **Task 2: Store-side nonce guard + n=100 persist/reload harness stub** — `f1af786` (test)

_Task 1 has `tdd="true"`; TDD mode was inactive for this run (config `tdd_mode: false`, orchestrator passed no TDD/MVP gate), so the module + its passing unit tests landed in one atomic feat commit rather than split RED/GREEN commits._

## Files Created/Modified
- `src/store/checkpoint.rs` - `CheckpointStore` + `CeremonyId`; concrete round1/round2 put/load, wipe; unit tests (dkg_roundtrip, wipe_vs_keep, ceremony_id_rejects_traversal)
- `src/store/mod.rs` - `pub mod checkpoint;` + `pub use checkpoint::{CeremonyId, CheckpointStore};`
- `tests/ui/checkpoint_no_nonce.rs` (+ `.stderr`) - compile-fail: nonce material rejected by the concrete-typed API
- `tests/ui/checkpoint_no_generic_persist.rs` (+ `.stderr`) - compile-fail: no generic persist sink exists
- `tests/compile_fail.rs` - registers the two new trybuild cases
- `tests/store_checkpoint_n100.rs` - `#[ignore]`d `persist_reload_100` at-scale harness

## Decisions Made
- **CeremonyId validation (Rule 2):** the plan said "CeremonyId newtype (String or hex)" without specifying validation; a caller-supplied id flows into a filesystem path, so `new` rejects anything but `[A-Za-z0-9_-]` (non-empty) to prevent traversal. Correctness/security requirement, not scope creep.
- **n=100 harness defaults to full 51/100** with `CHEGET_N100_T/N` env overrides — so Phase 3 runs it verbatim, matching the `CHEGET_DKG_T/N` seam in `dkg_100_correctness.rs`. It is `#[ignore]`d, so the default suite is unaffected regardless.
- **Structural control via two compile-fail snapshots** rather than a runtime assertion — the control is the API shape (Pitfall 1). The `.stderr` snapshots pin the *reason* (type mismatch / missing method), not merely that compilation fails.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] CeremonyId path-traversal validation**
- **Found during:** Task 1 (CheckpointStore)
- **Issue:** A raw `CeremonyId(String)` used directly as a path component would let `/`, `..`, or `\` escape the `ceremonies/` subtree (T-02-12 tampering surface).
- **Fix:** `CeremonyId::new` returns `Result` and rejects any non-`[A-Za-z0-9_-]`/empty id with `StoreError::Io(InvalidInput)`; a `ceremony_id_rejects_traversal` test locks it in.
- **Files modified:** src/store/checkpoint.rs
- **Verification:** `ceremony_id_rejects_traversal` green.
- **Committed in:** `8379392` (Task 1 commit)

**2. [Rule 3 - Blocking] Clippy `needless_borrows_for_generic_args` in the new harness**
- **Found during:** Task 2 (n=100 harness), running the plan's `cargo clippy -- -D warnings` gate.
- **Issue:** rust-1.96.0 clippy flagged `dkg::part1(id, n, t, &mut rng)` (OsRng is Copy) as a needless borrow, failing `-D warnings`.
- **Fix:** pass `rng` by value in `tests/store_checkpoint_n100.rs`.
- **Files modified:** tests/store_checkpoint_n100.rs
- **Verification:** `cargo clippy --test store_checkpoint_n100 --test compile_fail -- -D warnings` clean.
- **Committed in:** `f1af786` (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 missing-critical, 1 blocking)
**Impact on plan:** Both necessary for security/CI-cleanliness. No scope creep; the type-restricted API and D-08/D-10/D-11 invariants are exactly as planned.

## Issues Encountered
- **Pre-existing (out of scope):** `cargo clippy --all-targets -- -D warnings` fails on `tests/dkg_100_correctness.rs:55` (`&mut rng`, same lint) — a Phase 1 file untouched by this plan, already logged in `deferred-items.md` from 02-01 (toolchain drift on clippy 1.96). The lib and all files this plan created/modified are clippy-clean.
- **Slow crypto (expected):** age/scrypt at `log_n = 18` is ~1s per encrypt/decrypt, so the checkpoint unit tests take ~95s in debug and the full `store::` suite times out at 2 min — the prior-wave note warned of this. Checkpoint tests pass in isolation (`cargo test --lib store::checkpoint::tests`, 3 passed).

## Known Stubs
- `tests/store_checkpoint_n100.rs::persist_reload_100` is an intentional `#[ignore]`d harness, BUILT here and RUN at scale in Phase 3 (Phase 3 acceptance criterion 3). It is a complete, compiling test — not a placeholder — deliberately excluded from the default suite for runtime cost.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- STOR-02 DKG-round-secret half complete; the nonce half remains structurally satisfied (non-serializable `EphemeralNonces` + store-side compile-fail guards).
- `persist_reload_100` stands ready for Phase 3 to run at 51/100 with `cargo test --release --test store_checkpoint_n100 persist_reload_100 -- --ignored`.
- Plan 02-04 (coordinator SQLite) is the remaining Phase 2 plan.

## Self-Check

- `src/store/checkpoint.rs` — FOUND
- `tests/store_checkpoint_n100.rs` — FOUND
- `tests/ui/checkpoint_no_nonce.rs` + `.stderr` — FOUND
- `tests/ui/checkpoint_no_generic_persist.rs` + `.stderr` — FOUND
- Commit `8379392` (Task 1) — FOUND
- Commit `f1af786` (Task 2) — FOUND

## Self-Check: PASSED

---
*Phase: 02-persistence-storage*
*Completed: 2026-07-14*
