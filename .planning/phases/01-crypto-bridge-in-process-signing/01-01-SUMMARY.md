---
phase: 01-crypto-bridge-in-process-signing
plan: 01
subsystem: infra
tags: [rust, cargo, frost-secp256k1-tr, rust-bitcoin, bip341, bip86, taproot, clap, bridge, kat]

# Dependency graph
requires:
  - phase: none
    provides: greenfield project (first plan of the milestone)
provides:
  - Pinned reproducible Cargo scaffold (Cargo.toml + committed Cargo.lock, rust-toolchain 1.96.0, MSRV 1.85)
  - Module seams for cli/crypto/chain/transport/session/bridge (stubs filled by 01-02/03/04/05)
  - clap three-persona CLI skeleton (participant/coordinator/watcher) with real entry points
  - The ONE canonical frost->rust-bitcoin key bridge (address_from_group_key, output_key_q) with defensive even-Y invariant
  - BIP341/BIP86 known-answer-test harness pinning the bridge to published address strings (even-Y + odd-Y-origin)
  - Public-artifact envelope format (D-09) and wired `cheget watcher address --pubkey <file>`
affects: [01-02-keygen, 01-03-chain, 01-04-signing, 01-05-transport, phase-04-rotation]

# Tech tracking
tech-stack:
  added:
    - frost-secp256k1-tr 3.0.0
    - bitcoin (rust-bitcoin) 0.32.101
    - bitcoincore-rpc 0.19.0
    - esplora-client 0.13.0
    - zeroize 1.9.0
    - clap 4.6.1
    - serde 1 / serde_json 1
    - corepc-node 0.12.0 (dev, feature 28_0+download)
    - trybuild 1 (dev)
  patterns:
    - "One canonical bridge function; XOnlyPublicKey::from_slice confined to bridge/taproot.rs"
    - "Defensive even-Y parity invariant at the bridge (reject OddY, never strip prefix)"
    - "KAT pins hard-coded published address strings, not 'it runs'"
    - "Public artifacts are plaintext; no secret material touches disk (D-09)"
    - "Pure crypto/bridge layers with zero I/O dependencies"

key-files:
  created:
    - Cargo.toml
    - Cargo.lock
    - rust-toolchain.toml
    - src/lib.rs
    - src/main.rs
    - src/cli/mod.rs
    - src/cli/address.rs
    - src/bridge/taproot.rs
    - src/bridge/mod.rs
    - tests/bridge_roundtrip.rs
    - tests/vectors/bip341_keyspend.json
  modified: []

key-decisions:
  - "corepc-node feature pinned to 28_0 (confirmed available; 28_0..30_2 exist)"
  - "rust-toolchain pinned to installed 1.96.0 while Cargo rust-version stays at the 1.85 MSRV floor"
  - "Public-artifact envelope stores frost PublicKeyPackage canonical bytes as hex in serde_json with key_id + reserved epoch"
  - "address command gained a --network flag (default bitcoin) mapping to KnownHrp"
  - "odd-Y-origin KAT reuses the published even-Y address since even-Y normalization preserves the x-coordinate"

patterns-established:
  - "Bridge confinement: only bridge/taproot.rs calls the x-only from_slice constructor"
  - "TDD gate on the KAT: failing test committed (RED) before the address implementation (GREEN)"

requirements-completed: [KEY-03, KEY-04]

coverage:
  - id: D1
    description: "Crate compiles reproducibly against the pinned stack with a committed Cargo.lock; clap three-persona skeleton runs"
    verification:
      - kind: integration
        ref: "cargo build (exit 0); cargo tree shows frost-secp256k1-tr v3.0.0 + bitcoin v0.32.101; cargo run -- --help lists 3 personas"
        status: pass
    human_judgment: false
  - id: D2
    description: "Canonical frost->rust-bitcoin bridge: address_from_group_key + output_key_q with defensive even-Y invariant; from_slice confined to bridge/taproot.rs"
    requirement: KEY-03
    verification:
      - kind: unit
        ref: "tests/bridge_roundtrip.rs#even_y_vector_matches_published_address"
        status: pass
      - kind: unit
        ref: "tests/bridge_roundtrip.rs#odd_y_origin_is_rejected_then_normalizes_to_published_address"
        status: pass
    human_judgment: false
  - id: D3
    description: "BIP341/BIP86 KAT pins the bridge to the published address bc1p2wsldez... for even-Y and odd-Y-origin cases"
    requirement: KEY-03
    verification:
      - kind: unit
        ref: "tests/bridge_roundtrip.rs (cargo test --test bridge_roundtrip, 3 passed)"
        status: pass
    human_judgment: false
  - id: D4
    description: "cheget watcher address --pubkey <file> reads a public-artifact envelope and prints the BIP341 P2TR address"
    requirement: KEY-04
    verification:
      - kind: unit
        ref: "tests/bridge_roundtrip.rs#address_command_reads_pubkey_file_and_prints_vector_address"
        status: pass
    human_judgment: false

# Metrics
duration: 25min
completed: 2026-07-10
status: complete
---

# Phase 1 Plan 01: Crypto Bridge Scaffold & Canonical Bridge Summary

**Pinned reproducible `cheget` Cargo scaffold with a clap three-persona skeleton and the ONE canonical frost→rust-bitcoin BIP341 key bridge, pinned to the published BIP341/BIP86 address `bc1p2wsldez...` for both an even-Y and an odd-Y-origin key.**

## Performance

- **Duration:** ~25 min
- **Completed:** 2026-07-10
- **Tasks:** 3 (Task 3 was TDD: RED → GREEN)
- **Files created:** 18

## Accomplishments

- Greenfield crate compiles against the exact pinned stack (`frost-secp256k1-tr 3.0.0`, `bitcoin 0.32.101`, no 0.33.x) with a committed `Cargo.lock` and `rust-version = "1.85"`.
- `src/bridge/taproot.rs` provides the single canonical `address_from_group_key` (VerifyingKey `P` → x-only → `XOnlyPublicKey` → `Address::p2tr(.., None, hrp)`) and `output_key_q` (BIP86 tweaked output key `Q` for verification). The x-only `from_slice` constructor is confined to this module.
- The bridge enforces the D-11 parity invariant: a non-even-Y key returns `BridgeError::OddY` instead of blindly stripping the SEC1 prefix.
- `tests/bridge_roundtrip.rs` pins the bridge to the hard-coded published BIP341/BIP86 address string for the even-Y vector, proves the odd-Y-origin key is rejected then normalizes to the same address, and proves `cheget address --pubkey <file>` prints that address from a public-artifact envelope.
- clap persona tree (participant/coordinator/watcher) with real entry points; `keygen`/`sign` are explicit stubs (wired in 01-02/01-04), `address` fully wired (KEY-04).

## Task Commits

1. **Task 1: Pinned Cargo scaffold + module seams + clap persona skeleton** - `d928740` (feat)
2. **Task 2: Canonical frost→rust-bitcoin bridge with parity invariant** - `4630635` (feat)
3. **Task 3 (TDD RED): BIP341/BIP86 KAT + fixture** - `66a9dfe` (test)
4. **Task 3 (TDD GREEN): public-artifact envelope + wire `cheget address`** - `91b53ff` (feat)

_TDD gate satisfied: `test(...)` commit precedes the `feat(...)` implementation commit._

## Files Created/Modified

- `Cargo.toml` / `Cargo.lock` - pinned dependency stack + committed lockfile
- `rust-toolchain.toml` - channel 1.96.0 (build toolchain pin)
- `.gitignore` - ignores `/target`
- `src/lib.rs` - declares cli/crypto/chain/transport/session/bridge
- `src/main.rs` - persona dispatch entry point
- `src/cli/mod.rs` - clap three-persona tree + dispatch
- `src/cli/keygen.rs`, `src/cli/sign.rs` - stub handlers (D-08; wired 01-02/01-04)
- `src/cli/address.rs` - `PubkeyEnvelope` (D-09), `Network`→`KnownHrp`, `address_from_pubkey_file`, wired handler (KEY-04)
- `src/crypto/mod.rs`, `src/chain/mod.rs`, `src/transport/mod.rs`, `src/session/mod.rs` - documented module-seam stubs
- `src/bridge/mod.rs`, `src/bridge/taproot.rs` - canonical bridge + `BridgeError`
- `tests/bridge_roundtrip.rs` - KEY-03 KAT harness (3 tests)
- `tests/vectors/bip341_keyspend.json` - even-Y + odd-Y-origin published vectors

## Decisions Made

- **corepc-node feature `28_0`:** confirmed via `cargo info corepc-node` (feature set spans `0_17_2`..`30_2`); pinned `28_0` + `download` as the plan sketched.
- **Toolchain pin 1.96.0:** `rust-toolchain.toml` pins the installed/verified toolchain for reproducibility; `Cargo.toml` keeps the 1.85 MSRV floor.
- **Envelope format:** frost `PublicKeyPackage` canonical (postcard) bytes hex-encoded inside a `serde_json` envelope with `key_id` + reserved `epoch` (Q4 / D-09), dependency-free hex helpers (no new crate).
- **`--network` flag on `address`:** defaults to `bitcoin` so the mainnet KAT vector (`bc1p...`) renders; maps to `KnownHrp`.
- **odd-Y-origin reference:** even-Y normalization negates the point and preserves the x-coordinate, so the normalized odd-Y key yields the same output key `Q` and address — the fixture reuses the published even-Y address as the independent post-normalization reference (documented in the fixture; Assumption A7).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Corrected `KnownHrp` variant name**
- **Found during:** Task 3 (GREEN)
- **Issue:** The plan/research examples referenced `KnownHrp::Bitcoin`; rust-bitcoin 0.32.101 names the mainnet variant `KnownHrp::Mainnet`.
- **Fix:** Used `KnownHrp::Mainnet` in the bridge tests and mapped `Network::Bitcoin → KnownHrp::Mainnet` in the address module.
- **Files modified:** tests/bridge_roundtrip.rs, src/cli/address.rs
- **Verification:** `cargo test --test bridge_roundtrip` (3 passed)
- **Committed in:** 66a9dfe / 91b53ff

**2. [Rule 2 - Missing critical] `address --pubkey` made a required arg + added `--network`**
- **Found during:** Task 3
- **Issue:** A P2TR address cannot be rendered without a network HRP, and the address command is meaningless without a pubkey file.
- **Fix:** `--pubkey` is a required `PathBuf`; added a `--network` value-enum (default `bitcoin`) mapping to `KnownHrp`.
- **Files modified:** src/cli/address.rs
- **Verification:** `cargo run -- watcher address --help`; missing `--pubkey` errors cleanly.
- **Committed in:** 91b53ff

---

**Total deviations:** 2 auto-fixed (1 blocking API-name correction, 1 missing-critical CLI argument). No architectural changes; no scope creep.

## Issues Encountered

- None blocking. `cargo test` compiles the `corepc-node` dev-dependency (download feature); it built cleanly in this environment. The regtest harness that consumes it lands in 01-04.

## Known Stubs

All intentional and documented; each names the plan that resolves it:

- `src/cli/keygen.rs`, `src/cli/sign.rs` - handlers return an explicit "not implemented yet" error (D-08). Wired by 01-02 (keygen) and 01-04 (sign).
- `src/crypto/mod.rs` - filled by 01-02 (DKG, tweaked signing, nonce type).
- `src/chain/mod.rs` - filled by 01-03 (`ChainBackend` trait + Core/Esplora).
- `src/transport/mod.rs` - filled by 01-05 (`Transport` trait + in-memory stub).
- `src/session/mod.rs` - filled by 01-04 (two-round signing session).

None of these stubs block this plan's goal (KEY-03/KEY-04): the bridge and `cheget address` are fully functional and test-pinned.

## Next Phase Readiness

- The canonical bridge and its KAT are in place; 01-02 (crypto core / DKG) can normalize keys to even-Y and hand a `PublicKeyPackage` to `address_from_group_key` / `output_key_q`.
- Module seams exist for all later plans; no call-site churn expected when they fill their stubs.
- `output_key_q` is ready for 01-04 to verify aggregate signatures against `Q`.

## Self-Check: PASSED

- All 18 plan files verified present on disk.
- All task commits verified in git history: d928740, 4630635, 66a9dfe, 91b53ff.
- `cargo build` exit 0; `cargo test --test bridge_roundtrip` 3 passed; bridge confinement check passes.

---
*Phase: 01-crypto-bridge-in-process-signing*
*Completed: 2026-07-10*
