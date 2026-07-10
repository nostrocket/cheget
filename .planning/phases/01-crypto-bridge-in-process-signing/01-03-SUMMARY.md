---
phase: 01-crypto-bridge-in-process-signing
plan: 03
subsystem: infra
tags: [rust, bitcoin, bitcoincore-rpc, esplora-client, corepc-node, chain-backend, bip341, sighash, regtest, taproot]

# Dependency graph
requires:
  - phase: 01-01
    provides: "Cargo scaffold + pinned stack (bitcoincore-rpc 0.19, esplora-client 0.13, corepc-node 0.12 dev-dep); src/chain module seam"
provides:
  - "Sync `trait ChainBackend` (import tr() descriptor, list UTXOs, estimate fee, broadcast, confirmation depth) speaking only rust-bitcoin 0.32 types (STOR-04)"
  - "`ChainError` + `Utxo` backend-agnostic types"
  - "`key_spend_sighash` helper: TapSighashType::Default over Prevouts::All, the one message a FROST key-path sig commits to (SIGN-01 support; reused by client-side recompute in 01-04)"
  - "`CoreRpcBackend` over bitcoincore-rpc (fronts the confirm path, D-07)"
  - "`EsploraBackend` over esplora-client blocking API (conformance-only, D-07)"
  - "`tests/common::spawn_regtest()` — hermetic auto-spawned regtest node + watch-only + funding wallets (D-05), reusable by 01-04 e2e"
affects: [01-04-signing, phase-05-sweep-watch]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Every side effect behind a sync trait (ChainBackend); orchestration depends on the trait, backends are injected"
    - "One canonical key-spend sighash helper; TapSighashType::Default + Prevouts::All hard-wired, no sighash-type parameter"
    - "Watch-only tr() descriptor lives in a private-keys-disabled wallet (Core rejects keyless import into a key-holding wallet)"
    - "Esplora is address-indexed: descriptor import is a documented no-op; Core fronts the confirm path (D-07)"
    - "corepc-node used only to spawn/expose the node; the production CoreRpcBackend is a plain bitcoincore-rpc client on the node's wallet endpoint"
    - "Hermetic Esplora conformance via an in-process mock HTTP server (no network, no public endpoint)"

key-files:
  created:
    - src/chain/sighash.rs
    - src/chain/core_rpc.rs
    - src/chain/esplora.rs
    - tests/common/mod.rs
    - tests/regtest_fixture.rs
    - tests/chain_backend_conformance.rs
  modified:
    - src/chain/mod.rs

key-decisions:
  - "Watch-only tr() descriptor imported into a dedicated disable_private_keys wallet; Core v28 rejects keyless descriptor import into a key-holding wallet (real error surfaced during Task 3)"
  - "CoreRpcBackend is a bitcoincore-rpc client pointed at the corepc-node wallet endpoint (cookie auth); corepc-node's own corepc_client is used only for spawning — keeps the production backend on the pinned bitcoincore-rpc 0.19"
  - "confirmation_depth uses wallet gettransaction (watch-only wallet knows txs paying to/from the group address); returns 0 for negative/unknown"
  - "estimate_fee returns Option<FeeRate> (regtest → Ok(None)) so callers can fall back to a floor; Core BTC/kvB converted to sat/vB"
  - "Esplora conformance runs against an in-process mock HTTP server for hermeticity (D-07 permits mock); the shared regtest fixture lives in tests/common and is re-exported from tests/regtest_fixture.rs"

requirements-completed: [STOR-04]

coverage:
  - id: D1
    description: "Chain access behind a sync ChainBackend trait with a Core RPC backend and an Esplora backend, both driven through one conformance contract (STOR-04)"
    requirement: STOR-04
    verification:
      - kind: integration
        ref: "tests/chain_backend_conformance.rs (cargo test --test chain_backend_conformance: core_rpc_backend_conforms + esplora_backend_conforms, 2 passed)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Core RPC backend imports a watch-only tr() descriptor, lists UTXOs, broadcasts, and confirms on an auto-spawned regtest node (D-05, D-07)"
    requirement: STOR-04
    verification:
      - kind: integration
        ref: "tests/regtest_fixture.rs#regtest_fund_watch_broadcast_confirm_smoke (cargo test --test regtest_fixture, 1 passed; node auto-downloaded/spawned, killed on drop)"
        status: pass
    human_judgment: false
  - id: D3
    description: "BIP341 key-spend sighash computed with SIGHASH_DEFAULT over the full prevout set (SIGN-01 support)"
    requirement: STOR-04
    verification:
      - kind: unit
        ref: "src/chain/sighash.rs (cargo build --lib exit 0; grep TapSighashType::Default present; grep SIGHASH_ALL/TapSighashType::All count 0)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Esplora built to the same trait, conformance-covered but not in the n=1000 confirm path (D-07)"
    requirement: STOR-04
    verification:
      - kind: integration
        ref: "tests/chain_backend_conformance.rs#esplora_backend_conforms (in-process mock: import no-op, list_utxos depth 51, estimate_fee, broadcast, confirmation_depth 51)"
        status: pass
    human_judgment: false

# Metrics
duration: ~40min
completed: 2026-07-10
status: complete
---

# Phase 1 Plan 03: Chain Backend Trait, Core RPC + Esplora Backends, Regtest Fixture Summary

**Side-effecting chain access now sits behind a synchronous `ChainBackend` trait (rust-bitcoin 0.32 types only) with a `bitcoincore-rpc` backend fronting a hermetic auto-spawned regtest confirm path and an `esplora-client` backend covered by the same conformance contract, plus the one canonical BIP341 key-spend sighash helper (SIGHASH_DEFAULT over all prevouts) that 01-04's client-side recompute will reuse (STOR-04).**

## Performance

- **Duration:** ~40 min
- **Completed:** 2026-07-10
- **Tasks:** 3
- **Files created:** 6; modified: 1

## Accomplishments

- `src/chain/mod.rs` defines the synchronous `trait ChainBackend` (import watch-only `tr()` descriptor, list UTXOs, estimate fee, broadcast, confirmation depth) plus `ChainError` and a backend-agnostic `Utxo` type. No backend-specific type leaks through the trait — the coordinator, watcher (Phase 5), and tests depend only on the trait.
- `src/chain/sighash.rs` provides `key_spend_sighash(tx, input_index, prevouts)` hard-wiring `TapSighashType::Default` and `Prevouts::All` — the single place the project computes the message a FROST key-path signature commits to (SIGN-01 support). The gate confirms no legacy sighash constant appears in the file.
- `src/chain/core_rpc.rs` — `CoreRpcBackend` over `bitcoincore-rpc` 0.19: checksummed `importdescriptors` of a watch-only `tr(<x-only>)` descriptor, `listunspent`→`Utxo`, `estimatesmartfee` (BTC/kvB → sat/vB `FeeRate`, `None` on regtest), `sendrawtransaction`, and `gettransaction`-based confirmation depth. Fronts the confirm path (D-07).
- `src/chain/esplora.rs` — `EsploraBackend` over `esplora-client` 0.13 blocking API: `get_address_utxos`, `get_fee_estimates`, `broadcast`, and `get_tx_status`+tip-height confirmation depth. Descriptor import is a documented no-op (Esplora is address-indexed). Not in the confirm path (D-07).
- `tests/common/mod.rs` — `spawn_regtest()` auto-downloads and spawns a throwaway regtest `bitcoind` (corepc-node `28_0`+`download`, killed on drop, free port), creates a dedicated **watch-only** wallet for the group descriptor and a key-holding **funding** wallet for test plumbing, and wires a `CoreRpcBackend` to the watch-only wallet. Reusable by 01-04's end-to-end confirmed key-spend.
- `tests/regtest_fixture.rs` — fund → import watch-only descriptor → confirm the address is watched (list UTXO) → broadcast a wallet-signed tx **through the trait** → mine → assert ≥6 confirmations, all against the auto-spawned node with no system `bitcoind`.
- `tests/chain_backend_conformance.rs` — drives **both** backends through one `assert_query_surface` helper; Core against the regtest node, Esplora against an in-process mock Esplora HTTP server (hermetic — no network), with Esplora also exercising broadcast + confirmation-depth math (tip 150, confirmed at 100 → depth 51).

## Task Commits

1. **Task 1: ChainBackend trait + key-spend sighash helper** — `0888095` (feat)
2. **Task 2: Core RPC + Esplora backends + conformance tests** — `0be08de` (feat)
3. **Task 3: auto-spawned regtest broadcast/confirm smoke** — `1f691e8` (test)

## Files Created/Modified

- `src/chain/mod.rs` — `ChainBackend` trait, `ChainError`, `Utxo`, module wiring (was a doc-only seam stub)
- `src/chain/sighash.rs` — `key_spend_sighash` (SIGHASH_DEFAULT + Prevouts::All)
- `src/chain/core_rpc.rs` — `CoreRpcBackend` (bitcoincore-rpc 0.19)
- `src/chain/esplora.rs` — `EsploraBackend` (esplora-client 0.13 blocking)
- `tests/common/mod.rs` — `spawn_regtest()` + `RegtestFixture` (D-05)
- `tests/regtest_fixture.rs` — broadcast/confirm smoke; re-exports `spawn_regtest`
- `tests/chain_backend_conformance.rs` — two-backend conformance + in-process Esplora mock

## Decisions Made

- **Watch-only wallet is mandatory for the tr() import.** Core v28 rejects importing a keyless descriptor into a wallet that holds private keys (`-4: Cannot import descriptor without private keys to a wallet with private keys enabled`). The fixture creates a dedicated `disable_private_keys`+`blank` "watch" wallet for the descriptor and a separate key-holding "default" wallet for funding/signing — which is also the correct production separation.
- **CoreRpcBackend is a plain `bitcoincore-rpc` client** pointed at the corepc-node wallet endpoint (cookie auth). corepc-node's bundled `corepc_client` is used only to spawn the node, keeping the production backend on the pinned `bitcoincore-rpc` 0.19 (STOR-04). Verified a single `bitcoin v0.32.101` in the dependency graph — no cross-crate type mismatch.
- **`estimate_fee` returns `Option<FeeRate>`** so a data-starved backend (regtest) yields `Ok(None)` and callers fall back to a floor rather than erroring; Core's BTC/kvB is converted to sat/vB.
- **Esplora conformance uses an in-process mock HTTP server** (D-07 permits "mock or public endpoint") to stay hermetic — no public endpoint, no flakiness, no network in CI.
- **Fixture lives in `tests/common`** and is re-exported from `tests/regtest_fixture.rs` so both the smoke test and the conformance test (and 01-04's e2e) share one `spawn_regtest`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Watch-only tr() descriptor requires a private-keys-disabled wallet**
- **Found during:** Task 3 (first regtest run)
- **Issue:** Importing the keyless `tr()` descriptor into corepc-node's key-holding "default" wallet failed with Core error `-4: Cannot import descriptor without private keys to a wallet with private keys enabled`.
- **Fix:** `spawn_regtest` now creates a dedicated `disable_private_keys`+`blank` "watch" wallet for the descriptor (the production-correct pattern) and keeps the key-holding "default" wallet for test funding/signing; the smoke test pays the broadcast tx to the watched address so the watch-only wallet can report its confirmation depth.
- **Files modified:** tests/common/mod.rs, tests/regtest_fixture.rs
- **Verification:** `cargo test --test regtest_fixture` (1 passed); `cargo test --test chain_backend_conformance` (2 passed)
- **Committed in:** 0be08de (fixture) / 1f691e8 (smoke)

**2. [Rule 3 - Blocking] Fixture factored into `tests/common/mod.rs`**
- **Found during:** Task 2 (Core conformance test needs the same spawn helper as Task 3)
- **Issue:** Rust compiles each `tests/*.rs` as a separate crate, so the conformance test could not call a `spawn_regtest` defined only in `tests/regtest_fixture.rs` without double-running that file's `#[test]`s.
- **Fix:** Placed `spawn_regtest`/`RegtestFixture` in `tests/common/mod.rs` (included via `mod common;` in both test crates) and re-exported it from `tests/regtest_fixture.rs` so the plan's named file still exposes the symbol.
- **Files modified:** tests/common/mod.rs, tests/chain_backend_conformance.rs, tests/regtest_fixture.rs
- **Verification:** both test binaries compile and pass.
- **Committed in:** 0be08de / 1f691e8

---

**Total deviations:** 2 auto-fixed (1 real Core-behavior bug on watch-only import, 1 test-crate organization). No architectural changes; no scope creep. The `ChainBackend` surface and confirm path are exactly as planned.

## Issues Encountered

- `cargo clippy --tests` recompiles the full dependency graph (frost, bitcoin, corepc-node with the download feature) under its lint pass and is slow in this sandbox; `cargo clippy --lib` was clean and each acceptance-required `cargo test --test …` command passed individually. A full-suite clippy run can be recorded on the nightly/on-demand CI tier.

## Known Stubs

- `EsploraBackend::import_tr_descriptor` is an intentional no-op (returns `Ok(())`): Esplora is address-indexed and watches every address implicitly, so there is nothing to import. This is documented in-source and does not block the plan goal — the confirm path is Core-only by design (D-07).

## Next Phase Readiness

- 01-04 can `mod common;` and call `spawn_regtest()` to run the end-to-end confirmed key-spend, and reuse `key_spend_sighash` for both the coordinator's `SigningPackage` message and each participant's client-side recompute-before-sign gate (SIGN-07).
- The `ChainBackend` trait is the injection seam for Phase 5's sweep/watch personae; both backends are ready.

## Self-Check: PASSED

- All 6 created files + 1 modified file verified present on disk.
- All 3 task commits verified in git history: 0888095, 0be08de, 1f691e8.
- `cargo build --lib` exit 0; sighash gate green (Default present, legacy count 0); `cargo test --test chain_backend_conformance` 2 passed; `cargo test --test regtest_fixture` 1 passed.

---
*Phase: 01-crypto-bridge-in-process-signing*
*Completed: 2026-07-10*
