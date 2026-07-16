---
phase: 03-dkg-at-scale-local
plan: 02
subsystem: cli
tags: [frost, sign, store, persistence, key-spend, regtest, taproot, key-06]

# Dependency graph
requires:
  - phase: 03-dkg-at-scale-local
    provides: "keygen --persist -> per-seat seat-NNNN encrypted roots; pub persist_dkg_shares(base,t,n,&dyn PassphraseSource); ResolvedPassphrase; pub(crate) resolve_root/acquire_store_passphrase"
  - phase: 02-participant-store
    provides: "ParticipantStore::load_share/load_public_envelope/load_manifest, PassphraseSource/InCodePassphrase, ShareTag/ShareState, manifest::seat_hex"
  - phase: 01-foundation
    provides: "run_inprocess_dkg, SigningSession, address_from_group_key, PubkeyEnvelope::decode_package, KeyId/Epoch/SeatId newtypes, regtest common fixture"
provides:
  - "ParticipantStore::load_only_active(): single-call per-root read of the sole Active genesis (SeatId, KeyPackage)"
  - "cheget sign --persist/--base: loads t persisted seat roots (prompt-once unlock) + plaintext group package, drives the existing SigningSession to a confirmed key-spend from PERSISTED shares (D-05)"
  - "sign::load_persisted_shares(base,t,&dyn PassphraseSource): pub, test-drivable non-interactive read glue (discover seat-* -> sort -> first t -> load_only_active -> BTreeMap + group)"
  - "common::run_confirmed_key_spend_from_shares(key_packages,group,t): chain-proof body taking pre-loaded shares"
affects: [phase-04-rotation, phase-02-uat-test-1]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "single-call per-root read: load_only_active reads the plaintext manifest's sole Active entry, reconstructs the tag (seat_from_hex inverse of seat_hex, KeyId::new re-validates path component), then load_share"
    - "test-drivable read helper: pub fn load_persisted_shares takes &dyn PassphraseSource so tests inject InCodePassphrase without a terminal; interactive prompt stays at the thin CLI edge (hazard 3)"
    - "chain-proof body split: run_confirmed_key_spend_from_shares(pre-loaded) is the reusable acceptance body; run_confirmed_key_spend keeps its signature and delegates after its own DKG"

key-files:
  created:
    - tests/persisted_sign.rs
  modified:
    - src/store/participant.rs
    - src/cli/sign.rs
    - tests/common/mod.rs

key-decisions:
  - "load_persisted_shares uses first-t root selection (roots sorted by name); in production t=51 so first-t == first-51 (D-05). The SigningSession still selects t via its own liveness poll."
  - "The group PublicKeyPackage is read WITHOUT unlock via load_public_envelope(...).decode_package() — the public path never touches the passphrase."
  - "load_persisted_shares and load_only_active are pub (not pub(crate)) so integration tests linking cheget externally can drive the real read glue instead of re-implementing it."
  - "Fresh-DKG sign path preserved when --persist is absent (Phase-1 compatibility); only the SOURCE of key_packages/group changes, SIGN-05 nonces and SIGN-07 display gate untouched."

patterns-established:
  - "Read-glue mirrors the 03-01 write-glue seam: prompt once at the CLI edge, pass a passphrase SOURCE into the passphrase-source-generic loop."

requirements-completed: [KEY-06]

coverage:
  - id: T1
    description: "load_only_active returns the store's sole Active (seat, KeyPackage) byte-equal to the stored one; seat_from_hex inverts seat_hex for seats 1/7/51/100; no-active/malformed-hex return StoreError not a panic (T-03-08)."
    requirement: "KEY-06"
    verification:
      - kind: unit
        ref: "src/store/participant.rs#load_only_active_roundtrip_and_hex_inverse"
        status: pass
    human_judgment: false
  - id: T2
    description: "sign --persist/--base surface exists; run() prompts once (for_unlock) then delegates to load_persisted_shares; fresh-DKG fallback and SIGN-07/SIGN-05 controls intact."
    requirement: "KEY-06"
    verification:
      - kind: other
        ref: "cargo run -q -- coordinator sign --help (lists --persist/--base); grep load_persisted_shares/preview/prompt_ack/run_inprocess_dkg src/cli/sign.rs"
        status: pass
    human_judgment: false
  - id: T3
    description: "Confirmed regtest key-spend produced BY load_persisted_shares from PERSISTED shares at small n (t=3,n=5), depth>=6; full-100 smoke #[ignore]d and env-overridable; no MEASUREMENTS.md."
    requirement: "KEY-06"
    verification:
      - kind: integration
        ref: "tests/persisted_sign.rs#persisted_sign_confirmed_regtest_key_spend_small_n"
        status: pass
    human_judgment: false

# Metrics
duration: 12min
completed: 2026-07-16
status: complete
---

# Phase 3 Plan 02: sign-from-store Summary

**`cheget sign --persist` now loads `t` of the persisted `seat-NNNN` store roots (prompt-once unlock) plus the plaintext group package and drives the existing signing session to a confirmed regtest key-spend from PERSISTED shares — delivering the KEY-06 read half via the test-drivable `load_persisted_shares` read glue, proven end to end at small n.**

## Performance

- **Duration:** ~12 min (dominated by scrypt log_n=18: lib test ~152s for 2 tests, persisted_sign small-n ~153s including a live regtest node)
- **Started:** 2026-07-16T06:08:54Z
- **Completed:** 2026-07-16T06:20:30Z
- **Tasks:** 3
- **Files:** 3 modified + 1 created

## Accomplishments
- Added `ParticipantStore::load_only_active()` — a single call that reads a per-seat root's plaintext manifest, selects its sole `Active` genesis entry, reconstructs the `(key_id, epoch, seat)` tag (`seat_from_hex` inverse of `manifest::seat_hex`; `KeyId::new` re-validates the id as a safe path component, T-02-12/T-03-08), and decrypts the share via `load_share`.
- Added a private `seat_from_hex` helper: hex → bytes → `frost::Identifier::deserialize`, mapping malformed input to `StoreError` (never a panic).
- Extracted `sign::load_persisted_shares(base, t, &dyn PassphraseSource)` — the ENTIRE non-interactive read glue (discover `seat-*` → sort → error if `< t` → take first `t` → `load_only_active` per root → assemble `BTreeMap` + load the group via `load_public_envelope(...).decode_package()` with no unlock), `pub` so integration tests drive it directly with `InCodePassphrase`.
- Rewired `sign::run`: `--persist` resolves the base, prompts ONCE via `acquire_store_passphrase(false)` (no-echo `for_unlock`), then delegates; without `--persist` the Phase-1 fresh-DKG path is preserved. Only the SOURCE of the key material changes — the `SigningSession`, in-memory-only nonces (SIGN-05), and display gate (SIGN-07) are untouched.
- Extracted `common::run_confirmed_key_spend_from_shares(key_packages, group, t)` (the chain-proof body taking pre-loaded shares); `run_confirmed_key_spend` keeps its signature and delegates after its own DKG (no behavior change for existing callers).
- New `tests/persisted_sign.rs`: a small-n (t=3, n=5) PR gate that sets up the fixture with the 03-01 writer `persist_dkg_shares`, then DRIVES `load_persisted_shares` into the confirmed key-spend — proving the crown-jewel spend is produced BY the store→load glue from PERSISTED shares — plus a `#[ignore]`d full-100 functional smoke (env-overridable `CHEGET_PERSIST_T`/`CHEGET_PERSIST_N`).

## Task Commits

Each task was committed atomically:

1. **Task 1: `load_only_active` + `seat_from_hex` inverse** (TDD) — `649fced` (test RED) → `172bb41` (feat GREEN)
2. **Task 2: `load_persisted_shares` read helper + rewire `sign::run`** — `cd662d5` (feat)
3. **Task 3: persisted-share confirmed key-spend (small-n gate + full-100 smoke)** — `83ad27c` (test)

_Note: Task 1 was TDD (failing test → implementation)._

## Files Created/Modified
- `src/store/participant.rs` — added `pub fn load_only_active`, private `fn seat_from_hex`, and the `load_only_active_roundtrip_and_hex_inverse` unit test.
- `src/cli/sign.rs` — `SignArgs` gains `--persist`/`--base`; added `pub fn load_persisted_shares`; `run` branches on `--persist` (prompt-once + delegate) while keeping the fresh-DKG fallback and the SIGN-07/SIGN-05 controls.
- `tests/common/mod.rs` — extracted `run_confirmed_key_spend_from_shares`; `run_confirmed_key_spend` now delegates to it.
- `tests/persisted_sign.rs` — small-n PR gate + `#[ignore]`d full-100 smoke driving the real read glue.

## Decisions Made
- First-`t` root selection (sorted `seat-*` names); production `t=51` makes first-`t` the D-05 first-51. The session still finalizes exactly `t` via its own liveness poll.
- Group package read with no unlock (`load_public_envelope` → `decode_package`).
- Read helpers are `pub` for external test linkage, mirroring 03-01's `persist_dkg_shares` decision.

## Deviations from Plan
None - plan executed exactly as written.

## Issues Encountered
None. As anticipated by the plan and the prior-wave note, age/scrypt log_n=18 is costly in this sandbox; tests were run single-threaded / scoped to a single test to avoid the parallel-scrypt anti-DoS guard. Correctness is unaffected.

## Threat Coverage
- **T-03-05 (nonces):** only the SOURCE of `key_packages` changed; `session.run` and the in-memory-only nonce type are untouched (SIGN-05).
- **T-03-06 (decrypted plaintext):** loads go through `load_share`'s decrypt-into-`Zeroizing`-drop path (D-06); `load_only_active` returns only the parsed `KeyPackage`.
- **T-03-07 (blind signing):** `session.preview()` + `--yes`/`prompt_ack` display gate preserved; `round2` still recomputes the sighash from the PSBT (SIGN-07).
- **T-03-08 (discovery / hex→SeatId):** roots discovered by `seat-` prefix under a resolved base; `seat_from_hex` validates via `Identifier::deserialize` and `KeyId::new` re-validates the manifest key_id; malformed entries return `StoreError`, not a panic.
- **T-03-09 (fewer than t roots):** a clear "insufficient persisted seat roots" error before any session starts.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- The store→sign path is proven end to end: `keygen --persist` (03-01) writes, `sign --persist` (this plan) reads, and the confirmed regtest key-spend is produced from persisted shares (ROADMAP criterion 2 met).
- Phase 4 (rotation) can reuse `load_only_active` / `load_persisted_shares` as the read seam; epoch handling currently binds `Epoch::GENESIS` and will generalize when refresh advances epochs.
- On demand, the full 51/100 functional smoke runs via `cargo test --release --test persisted_sign -- --ignored`.

## Self-Check: PASSED
- All 4 target files exist on disk (participant.rs, sign.rs, common/mod.rs, persisted_sign.rs).
- All 4 task commits present (`649fced`, `172bb41`, `cd662d5`, `83ad27c`).
- `cargo build`, `cargo test --no-run` (all targets), `cargo test --lib store::participant`, and the small-n `cargo test --test persisted_sign` pass; `sign --help` lists `--persist`/`--base`; no MEASUREMENTS.md added.

---
*Phase: 03-dkg-at-scale-local*
*Completed: 2026-07-16*
