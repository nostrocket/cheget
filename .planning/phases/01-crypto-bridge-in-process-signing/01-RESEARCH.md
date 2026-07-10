# Phase 1: Crypto Bridge & In-Process Signing - Research

**Researched:** 2026-07-10
**Domain:** FROST threshold Schnorr (RFC 9591) → BIP340/341 Taproot key-path bridge, two-round tweaked signing, in-process DKG at n=1000, regtest end-to-end proof (Rust)
**Confidence:** HIGH (crate versions + core API surface re-verified against docs.rs and the crates.io legitimacy seam this session, 2026-07-10; pitfalls from curated in-repo HIGH-confidence research)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** DKG and signing code is written **generic over `(t, n)`**. Fast unit tests may exercise tiny sizes (2-of-3, 3-of-5) for TDD speed.
- **D-02:** The **real acceptance target is `t=501`, `n=1000`** run fully in-process. The end-to-end proof (DKG → address → sign → aggregate → verify against `Q` → broadcast → confirm on regtest) runs at n=1000. "Always run real where it counts."
- **D-03:** **Phase 3 folds into Phase 1.** Phase 1 absorbs the n=1000 DKG *correctness* proof (all 1000 `KeyPackage`s verify to one group `PublicKeyPackage` — KEY-06) and the **O(n²) timing/memory measurement** (part1/part2/part3 instrumentation across 1000 seats). No persistence needed for either.
- **D-04:** The **persist/reload-at-scale** check moves to Phase 2. Phase 3 ceases to exist standalone. → Requires a ROADMAP edit (deferred, not this phase's work).
- **D-05:** The regtest `bitcoind` is **auto-spawned by the test harness** via the `corepc-node`/`bitcoind` crate (throwaway regtest node on a temp datadir, pinned Core version). Hermetic, reproducible, CI-friendly.
- **D-06:** **Tiered CI.** Every PR gates on: bridge known-answer vector test + a **small-n** end-to-end (DKG→sign→confirm on regtest, e.g. 3-of-5) + build/`cargo audit`. The **full `t=501`/`n=1000`** end-to-end runs **nightly and on-demand**, and MUST pass before Phase 1 sign-off.
- **D-07:** The **Core RPC backend fronts the confirmed-key-spend path** (native regtest mining via `generatetoaddress`). The **Esplora backend is still built to the same `ChainBackend` trait** and covered by trait-conformance/unit tests (mocked or public endpoint) — satisfies STOR-04 — but Esplora is **not** in the n=1000 confirm path.
- **D-08:** Ship the **real subcommand skeleton** (clap persona tree) and wire keygen/sign to run against the **in-memory `Transport` stub** in a "simulate all seats in one process" mode. Commands are real entry points; the stub stands in for the network so Phase 7 can swap in Nostr behind the same seam with no call-site churn.
- **D-09:** **State/key flow without a persistence layer:** public artifacts (`PublicKeyPackage`/group verifying key) written to **plaintext files** (they are public); `tsig address --pubkey <file>` reads one. **Secret share material never touches disk** — lives only in the simulating process for the duration of a run.
- **D-10:** The bridge round-trip test is anchored to the **official BIP341 taproot-tweak / BIP86 key-path published test vectors** (known internal key → known output key → known scriptPubKey/address). Externally auditable against the BIPs.
- **D-11:** **Parity contract:** the crypto core applies frost's `EvenY` so the group key is always even-Y, and **the bridge asserts this invariant** (rejects/normalizes odd-Y defensively rather than blindly stripping the SEC1 prefix). The KAT suite **covers BOTH an even-Y and an odd-Y-origin vector**, each verified end-to-end.

### Claude's Discretion

- The exact mechanism for the **non-serializable nonce type** (SIGN-05) — e.g. a newtype around frost `SigningNonces` with no `Serialize` impl + a `trybuild` compile-fail test.
- The **`Transport` trait contract** (sync/async, message/envelope model) — shaped to fit the later Nostr event model but concretely only needs the in-memory stub now.
- The **`ChainBackend` trait contract** (UTXO listing, fee estimation, broadcast, sighash helpers, watch-only descriptor import).
- **Liveness poll / 501-of-1000 subset selection** logic and how the coordinator drives it over the in-memory transport.
- **Display-before-sign UX** specifics (what's rendered, `--yes` behavior) per SIGN-07.

### Deferred Ideas (OUT OF SCOPE)

- **ROADMAP EDIT** folding Phase 3 into Phase 1 (roadmap action, applied via `/gsd-phase` — not implementation work in this phase).
- **Esplora-over-regtest (electrs) confirm path** — deferred; Core fronts the confirm (D-07).
- Any real network transport (Nostr/file — Phase 7); encrypted at-rest secret storage / SQLite coordinator store (Phase 2); rotation (Phase 4); sweep/lifecycle/policy (Phase 5); hardening/reproducible-build/external review (Phase 6).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| KEY-01 | Generate group key via DKG (`dkg::part1/2/3`) → `KeyPackage`+`PublicKeyPackage`; verifying key is Taproot internal key `P`; DKG-only | DKG signatures verified below (Code Examples §DKG); `into_even_y(None)` normalization for BIP340 (Pattern 2) |
| KEY-02 | Same DKG routines run in-process, single host, all participants simulated, no transport | In-process simulation loop over a `BTreeMap<Identifier, _>` (Code Examples §DKG, §n=1000 loop); runs behind the in-memory `Transport` stub (D-08) |
| KEY-03 | Byte-level round-trip test pins the frost→rust-bitcoin bridge (33-byte SEC1 → 32-byte x-only → `XOnlyPublicKey` → `Address::p2tr(secp, internal, None, network)`), asserting x-only parity and internal-vs-output-key correctness | Bridge mechanics + KAT sourcing (Pitfall 2, Code Examples §Bridge, Validation Architecture) |
| KEY-04 | `tsig address [--key active\|standby]` prints BIP341 P2TR address (`Q = P + H_taproot(P)·G`, merkle root `None`), constant across refresh epochs | `Address::p2tr(..., None, hrp)` (Standard Stack, Code Examples §Bridge); reads public artifact file (D-09) |
| KEY-05 | Each participant confirms group verifying key to coordinator after keygen; mismatch aborts | Client-side confirmation over in-memory transport (Pattern 5); trivial in-process at Phase 1, structurally seated for Phase 7 |
| KEY-06 | Full n=1000 share set in-process, 1000 `KeyPackage`s verify to one `PublicKeyPackage`; validate O(n²) scales locally | n=1000 feasibility + instrumentation (Open Q1, Code Examples §n=1000 loop, Performance section) |
| SIGN-01 | Coordinator runs signing session from PSBT; BIP341 key-spend sighash per input (`taproot_key_spend_signature_hash`, default type) | `ChainBackend` sighash helper (Code Examples §Sighash); `Prevouts::All` (Pitfall 2) |
| SIGN-02 | Coordinator selects 501 live participants via liveness poll; round 1 `round1::commit` collecting `SigningCommitments` | `round1::commit` returns `(SigningNonces, SigningCommitments)` (Code Examples §Signing); over-provision poll (Pitfall 11) |
| SIGN-03 | Round 2 `round2::sign_with_tweak`; aggregate `aggregate_with_tweak(…, merkle_root: None)` → 64-byte BIP340 sig | Tweaked signing pipeline (Code Examples §Signing, Pitfall 7) |
| SIGN-04 | Verify aggregated BIP340 sig against output key `Q`, finalize PSBT, print raw tx | `Q` derivation via `.tweak(None)` + bridge; verify against `Q` not `P` (Pitfall 7, Code Examples §Verify) |
| SIGN-05 | Nonces in memory only, non-serializable type; any restart → fresh nonces | Non-serializable newtype + `trybuild` compile-fail (Pattern 3, Code Examples §Nonce, Validation Architecture) |
| SIGN-06 | Aggregation surfaces 3.0 cheater-detection culprits; timeout aborts → new session, replacement subset, no commitment reuse | `Error::culprits() -> Vec<Identifier>`, cheater detection default-on (State of the Art); new-session-on-abort (Pattern 3, Pitfall 1/11) |
| SIGN-07 | Before signing each participant recomputes sighash from PSBT, shown human-readable outputs/amounts/fee, explicit ack unless `--yes`; no blind signing | Display-before-sign gate (Pattern 4, Pitfall 8); recompute via same `ChainBackend` sighash fn |
| STOR-04 | Chain access behind a trait with Bitcoin Core JSON-RPC backend (watch-only `tr()` descriptor import) and Esplora alternative | `ChainBackend` trait shape (Architecture, Open Q2); Core fronts confirm, Esplora trait-conformance only (D-07) |
</phase_requirements>

## Summary

Phase 1 builds the trusted computing base of `tsig`: a pure crypto-core wrapper over `frost-secp256k1-tr` 3.0.0, the byte-level frost→rust-bitcoin key bridge, a two-round tweaked signing session, and two architectural trait seams (`Transport`, `ChainBackend`) — all proven end-to-end by a **confirmed regtest key-spend at t=501/n=1000**, with zero transport, relays, or encrypted persistence. The entire crypto layer of this project *is* the `frost-secp256k1-tr` public API; there is essentially no bespoke cryptography to write, only correct *wiring* of audited primitives, and correct *conventions* at the one place three convention systems (frost serialization, BIP340 x-only/parity, BIP341 tweak) collide — the bridge.

The dominant risk is not algorithmic but integrational and structural. Two failure classes must be prevented **by construction from the first line of code**, not retrofitted: (1) nonce persistence/reuse (a key-extraction bug class) — prevented by a non-serializable nonce newtype the compiler refuses to persist; and (2) bridge parity/tweak/sighash errors — prevented by a single canonical bridge function pinned to official BIP341/BIP86 known-answer vectors and verified end-to-end against the *output* key `Q` (never the internal key `P`). Two further client-side gates — display-before-sign (recompute sighash from the PSBT) and the same-key check (Phase 4) — encode the "coordinator is untrusted" trust boundary. The four Phase-1 controls map exactly to SIGN-05, KEY-03, SIGN-03/04, and SIGN-07.

Every crate and every load-bearing API was re-verified this session: DKG `part1/2/3` signatures, the `EvenY` and `Tweak` trait method names and implementers, and the `corepc-node` 0.12.0 auto-spawn harness. Rust 1.96 and Docker are present locally; `bitcoind` need not be installed because `corepc-node`'s auto-download feature fetches and hash-verifies a pinned Core binary for the test.

**Primary recommendation:** Build strictly bottom-up along the ARCHITECTURE.md order A→B→C, keeping `crypto/` and `bridge/` pure (no I/O). Land the bridge KAT (KEY-03) and the non-serializable nonce type + `trybuild` test (SIGN-05) *first* — they are the cheapest and highest-leverage structural controls. Then prove in-process 501-of-1000 signing against a real regtest sighash and confirm the spend. Normalize the group key to even-Y with `into_even_y(None)` immediately after DKG, and route the tweak through `sign_with_tweak` / `aggregate_with_tweak(…, None)` exclusively — never expose the untweaked path to app code.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| DKG (part1/2/3), share generation, verify-to-group | Crypto core (`crypto/`, pure) | — | Audited frost primitives; must stay I/O-free for a small auditable TCB |
| Even-Y normalization of group key | Crypto core (`crypto/`) | Bridge (assertion) | `into_even_y(None)` is a frost `keys` op; bridge re-asserts `has_even_y()` (D-11) |
| frost key → x-only → P2TR address | Key bridge (`bridge/`, pure) | Crypto core (supplies `VerifyingKey`) | The one seam between two curve-crate worlds; byte-level only |
| Output-key `Q` derivation for verification | Key bridge (`bridge/`) | Crypto core (`.tweak(None)`) | Signature must verify against `Q`, not internal `P` |
| PSBT parse/finalize, key-spend sighash, broadcast, UTXO/fee | Chain backend (`chain/`, trait) | Bitcoin Core RPC / Esplora impls | Side-effecting; behind `ChainBackend` per STOR-04 |
| Two-round session (commit/sign/aggregate/verify), liveness, display gate | Signing session (`session/`, orchestration) | crypto, bridge, chain, transport | Sequences pure ops + adapters; owns nonce lifetime (memory-only) |
| Signing-nonce lifetime & non-persistence | Signing session (`session/`) | — | Nonces created round 1, consumed round 2, dropped; never in store API |
| Message passing between seats (in-process now) | Transport (`transport/`, trait) | in-memory stub impl | The seam Phase 7 swaps for Nostr; no relay code in Phase 1 |
| CLI persona dispatch, config, `--pubkey`/`--yes` flags, exit codes | CLI (`cli/`, clap 4) | all orchestration | Real entry points (D-08); does no work itself |
| n=1000 DKG simulation + O(n²) instrumentation | Test/bench harness | crypto core | Correctness (KEY-06) + timing/memory measurement (D-03) |
| Regtest node spawn, mine, confirm | Test harness (`corepc-node`) | chain backend (Core RPC) | Hermetic auto-spawn (D-05); not production code |

## Standard Stack

All versions are the project-locked pins from `./.claude/CLAUDE.md` / `.planning/research/STACK.md`, re-verified against the crates.io legitimacy seam this session (2026-07-10). Only Phase-1-relevant crates are listed.

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `frost-secp256k1-tr` | **3.0.0** | Entire crypto layer: DKG, `round1::commit`, `round2::sign_with_tweak`, `aggregate_with_tweak`, `keys::Tweak`/`keys::EvenY` | Audited (NCC), ZcashFoundation-maintained; only packaged Rust FROST exposing the Taproot tweak `[VERIFIED: crates.io legitimacy seam — repo ZcashFoundation/frost, OK]` |
| `frost-core` | **3.0.0** | Trait/type substrate re-exported by `-tr` | Must match `-tr` major; prefer using `-tr` re-exports `[VERIFIED: crates.io seam, OK]` |
| `bitcoin` (rust-bitcoin) | **0.32.101** | Address, PSBT, tx, BIP341 sighash, x-only key types | Canonical; 0.33 is beta and unsupported by the RPC/esplora clients `[VERIFIED: crates.io seam — 165k downloads/wk, rust-bitcoin/rust-bitcoin, OK]` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `bitcoincore-rpc` | **0.19.0** | Core JSON-RPC: watch-only `tr()` import, UTXO listing, fee estimate, broadcast | Default chain backend; fronts the n=1000 confirm (D-07) `[VERIFIED: seam, OK]` |
| `esplora-client` | **0.13.0** | Esplora HTTP client behind the same `ChainBackend` trait | Trait-conformance/unit coverage only in Phase 1 (D-07) `[VERIFIED: seam, OK]` |
| `zeroize` | **1.9.0** | Memory hygiene for the nonce/secret newtypes | Wrap decrypted/ephemeral secrets in `Zeroizing`; nonce newtype's inner state `[VERIFIED: seam — 10.9M downloads/wk, RustCrypto, OK]` |
| `clap` | **4.6.1** | CLI persona tree (derive API) | Real subcommand skeleton (D-08) `[CITED: STACK.md]` |
| `serde`/`serde_json` | **1.x** | (De)serialize public artifacts, config; frost types via their serde impl | Public-artifact file I/O (D-09); NOT on the nonce type `[CITED: STACK.md]` |
| `corepc-node` | **0.12.0** (2026-04-14) | Auto-spawn a throwaway regtest `bitcoind` in tests (D-05) | dev-dependency only; auto-download + hash-verify a pinned Core binary `[VERIFIED: docs.rs crate page + crates.io seam — rust-bitcoin/corepc, OK]` |
| `trybuild` | **1.x** | Compile-fail harness proving the nonce type won't serialize (SIGN-05) | dev-dependency; `tests/ui/*.rs` with expected `.stderr` `[VERIFIED: crates.io seam — dtolnay/trybuild, 940k downloads/wk, OK]` |

**Deferred to later phases (do NOT pull in now):** `nostr-sdk` (Phase 7), `age` (Phase 2), `rusqlite` (Phase 2), `toml` (config polish; optional this phase).

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `corepc-node` auto-download | System `bitcoind` on PATH | Non-hermetic, not CI-reproducible; violates D-05. Auto-download fetches + hash-verifies a pinned Core version |
| `trybuild` compile-fail | A doc-test with `compile_fail` attribute | `trybuild` gives a stable, reviewable `.stderr` snapshot and is the idiomatic choice for "this must not compile" `[ASSUMED]` |
| in-process channel transport | `tokio` mpsc / async | A synchronous in-memory `Vec`/`BTreeMap`-backed stub is simplest now; keep the trait async-compatible for the Nostr swap (Open Q3) |

**Installation (Phase-1 subset of Cargo.toml):**
```toml
[package]
rust-version = "1.85"     # MSRV floor; local toolchain is 1.96 (OK)
edition = "2021"

[dependencies]
frost-secp256k1-tr = "3.0.0"     # serialization on by default
bitcoin            = "0.32.101"   # do NOT bump to 0.33.x-beta
bitcoincore-rpc    = "0.19.0"
esplora-client     = "0.13.0"
zeroize            = { version = "1.9.0", features = ["zeroize_derive"] }
clap               = { version = "4.6.1", features = ["derive"] }
serde              = { version = "1", features = ["derive"] }
serde_json         = "1"

[dev-dependencies]
corepc-node = { version = "0.12.0", features = ["28_0", "download"] }  # confirm exact Core-version feature at plan time
trybuild    = "1"
```

**Version verification (run at plan/execute time):**
```bash
cargo add frost-secp256k1-tr@3.0.0 --dry-run
cargo search corepc-node          # confirm 0.12.0 latest + feature flags for the pinned Core version
```
The `corepc-node` Core-version feature (e.g. `28_0`, `27_1`, `25_1`) selects which Bitcoin Core binary the build script downloads — pin one explicitly for reproducibility. `[VERIFIED: docs.rs — feature is a version string like `25_1`/`24_0_1`; `Node` killed on drop; picks free ports with 3 spawn retries]`

## Package Legitimacy Audit

Verdicts from `gsd-tools query package-legitimacy check --ecosystem crates …` (2026-07-10). Versions/dates cross-checked with STACK.md (crates.io fetch, same day).

| Package | Registry | First Published | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----------------|-----------|-------------|---------|-------------|
| `frost-secp256k1-tr` | crates | 2025-01-15 (3.0.0: 2026-04-23) | ~1.6k/wk | github.com/ZcashFoundation/frost | OK | Approved |
| `frost-core` | crates | 2023-03-09 | ~18k/wk | github.com/ZcashFoundation/frost | OK | Approved |
| `bitcoin` | crates | 2015-09-20 | ~165k/wk | github.com/rust-bitcoin/rust-bitcoin | OK | Approved |
| `bitcoincore-rpc` | crates | 2018-11-09 | ~33k/wk | github.com/rust-bitcoin/rust-bitcoincore-rpc | OK | Approved |
| `esplora-client` | crates | 2022-09-26 | ~22k/wk | github.com/bitcoindevkit/rust-esplora-client | OK | Approved |
| `corepc-node` | crates | 2024-11-14 | ~7k/wk | github.com/rust-bitcoin/corepc | OK | Approved |
| `trybuild` | crates | 2019-05-06 | ~941k/wk | github.com/dtolnay/trybuild | OK | Approved |
| `zeroize` | crates | 2018-10-03 | ~10.9M/wk | github.com/RustCrypto/utils | OK | Approved |
| `clap` | crates | (mature) | (very high) | github.com/clap-rs/clap | OK | Approved |

**Packages removed due to [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none. (A first-pass lookup transiently returned SUS for `bitcoin` with `exists: null` — a registry API miss, not a real signal; the retry returned OK with 165k weekly downloads and the canonical rust-bitcoin repo. No checkpoint needed.)
**Note on `frost-secp256k1-tr` download count (~1.6k/wk):** low volume is expected for a specialized, recently-major-bumped audited crypto crate; source repo is the ZcashFoundation reference implementation. Not a suspicion signal in context.

## Architecture Patterns

### System Architecture Diagram

Phase-1 data flow (in-process; the network is the in-memory `Transport` stub):

```
  tsig CLI (clap: keygen | address | session sign)
        │
        ▼
  ┌─────────────────────── Orchestration (L3) ───────────────────────┐
  │                                                                    │
  │  Ceremony (keygen)              Signing session                    │
  │  simulate n seats  ──part1/2/3──►  liveness poll → pick 501        │
  │        │                            │                              │
  │        │  (all seats in one process)│  round1::commit  ─┐          │
  │        ▼                            ▼                   │ nonces   │
  │   BTreeMap<Id, KeyPackage>     SigningCommitments       │ stay in  │
  │        │  into_even_y(None)         │                   │ RAM only │
  │        ▼                            ▼                   │ (never   │
  │   PublicKeyPackage (group)     SigningPackage (per input)│ stored)  │
  │        │                            │  DISPLAY tx summary◄┘         │
  │        │                            │  recompute sighash from PSBT  │
  │        │                            │  human ack (--yes bypass)     │
  │        │                            ▼                              │
  │        │                       round2::sign_with_tweak             │
  │        │                            │                              │
  │        │                       aggregate_with_tweak(…, None)       │
  │        │                            │  → 64-byte BIP340 sig         │
  └────────┼────────────────────────────┼──────────────────────────────┘
           │                            │
           ▼ (via Transport stub)       ▼
  ┌─ Key bridge (L0.5, pure) ─┐   verify sig against Q
  │ VerifyingKey P (even-Y)   │        │
  │  33B SEC1 → 32B x-only     │        ▼
  │  assert has_even_y()       │   finalize PSBT
  │  Address::p2tr(P,None,hrp) │        │
  │  derive Q via .tweak(None) │        ▼
  └───────────┬────────────────┘   ChainBackend trait (L2)
              │                      ├─ Core RPC ── regtest bitcoind
              ▼                      │    (corepc-node auto-spawn):
        P2TR address                 │    fund addr, generatetoaddress,
        (KEY-04)                     │    broadcast, confirm  ◄── the proof
                                     └─ Esplora (trait-conformance only)
```

Entry points: the CLI subcommands. Processing stages flow top-to-bottom: simulate DKG → normalize even-Y → bridge to address → sign (with the display gate and nonce-in-RAM invariant) → aggregate → verify against `Q` → finalize → broadcast+confirm on the auto-spawned regtest node. Decision/branch points: the liveness poll selects a 501-subset; any timeout aborts to a *new* session (never reuse commitments); the display gate blocks round 2 until ack. File-to-implementation mapping is in the Component Responsibilities table of ARCHITECTURE.md (do not duplicate here).

### Recommended Project Structure
```
tsig/
├── Cargo.toml                # Phase-1 pins; commit Cargo.lock
├── src/
│   ├── main.rs               # persona dispatch only
│   ├── cli/                  # L4 — clap persona tree (real entry points, D-08)
│   ├── crypto/               # L0 — PURE frost wrapper (no I/O)
│   │   ├── keygen.rs         #   dkg part1/2/3 + into_even_y(None)
│   │   ├── sign.rs           #   round1::commit, round2::sign_with_tweak, aggregate_with_tweak
│   │   ├── nonce.rs          #   non-serializable SigningNonces newtype (SIGN-05)
│   │   └── types.rs          #   (key_id, epoch, identifier) newtypes
│   ├── bridge/               # L0.5 — VerifyingKey → x-only → XOnlyPublicKey → P2TR + Q
│   │   └── taproot.rs        #   the ONE canonical bridge fn; asserts has_even_y()
│   ├── chain/                # L2 — ChainBackend trait + Core/Esplora impls, PSBT, sighash
│   ├── transport/            # L2 — Transport trait + in-memory stub (no relay code)
│   └── session/              # L3 — signing session; owns nonce lifetime (RAM only)
└── tests/
    ├── bridge_roundtrip.rs   # KEY-03 KAT: BIP341/BIP86 vectors, even-Y AND odd-Y-origin
    ├── inproc_sign.rs        # small-n (3-of-5) end-to-end on regtest (PR gate, D-06)
    ├── inproc_sign_1000.rs   # t=501/n=1000 end-to-end (nightly gate, D-02/D-06) [ignore]
    ├── dkg_1000_correctness.rs # KEY-06: 1000 KeyPackages → one PublicKeyPackage + O(n²) bench
    └── ui/nonce_no_serialize.rs # trybuild compile-fail (SIGN-05)
```

### Pattern 1: One canonical bridge function, pinned by a known-answer vector
**What:** Exactly one function converts `VerifyingKey → XOnlyPublicKey → Address`, and exactly one derives the output key `Q`. Nothing else calls `XOnlyPublicKey::from_slice`.
**When to use:** All key→address and verification paths (KEY-03/04, SIGN-04).
**Example:**
```rust
// Source: synthesized from bitcoin 0.32 API [CITED: STACK.md] + frost EvenY trait [VERIFIED docs.rs]
// bridge/taproot.rs
use bitcoin::{Address, XOnlyPublicKey, KnownHrp};
use bitcoin::secp256k1::Secp256k1;
use frost_secp256k1_tr as frost;
use frost::keys::EvenY;

/// Convert a FROST group verifying key (already even-Y) into the P2TR address.
/// Panics/errors if the key is not even-Y — the parity invariant (D-11).
pub fn address_from_group_key(
    vk: &frost::VerifyingKey,
    hrp: KnownHrp,
) -> Result<Address, BridgeError> {
    // D-11: defensive parity assertion, do NOT blindly strip the SEC1 prefix.
    if !vk.has_even_y() {
        return Err(BridgeError::OddY);
    }
    let sec1: [u8; 33] = vk.serialize()?.try_into().map_err(|_| BridgeError::Len)?;
    // even-Y ⇒ SEC1 prefix is 0x02; x-only is the trailing 32 bytes.
    debug_assert_eq!(sec1[0], 0x02);
    let xonly = XOnlyPublicKey::from_slice(&sec1[1..])?;   // internal key P
    let secp = Secp256k1::verification_only();
    // merkle_root = None ⇒ BIP86 key-only output Q = P + H_taproot(P)·G
    Ok(Address::p2tr(&secp, xonly, None, hrp))
}
```
*Note: `VerifyingKey::serialize()` returns the 33-byte compressed SEC1 point; confirm the exact return type (`Vec<u8>` vs `[u8; 33]`) at implementation time — see Assumptions.*

### Pattern 2: Even-Y normalization immediately after DKG
**What:** After `part3`, call `.into_even_y(None)` on both `KeyPackage` and `PublicKeyPackage` so the group key is canonically even-Y before it ever reaches the bridge or signing (D-11).
**When to use:** Once, at the end of keygen, before persisting the public artifact or building an address.
**Example:**
```rust
// Source: frost EvenY trait [VERIFIED: docs.rs/frost-secp256k1-tr/3.0.0 keys::EvenY]
use frost::keys::EvenY;
let (key_package, pubkey_package) = frost::keys::dkg::part3(&r2_secret, &r1_pkgs, &r2_pkgs)?;
let key_package    = key_package.into_even_y(None);       // Option<bool>: None = auto-detect
let pubkey_package = pubkey_package.into_even_y(None);
assert!(pubkey_package.verifying_key().has_even_y());     // invariant now holds
```

### Pattern 3: Nonce-exclusion by type (structural nonce discipline)
**What:** Wrap `frost::round1::SigningNonces` in a newtype that (a) does not derive/impl `Serialize`/`Deserialize`, (b) holds `zeroize::Zeroizing` state, (c) is created in round 1, consumed by value in round 2, and never enters any store API. A `trybuild` test proves serialization won't compile.
**When to use:** The signing session only. This is the single highest-severity structural control in the project.
**Example:**
```rust
// Source: SPEC §6.5 + PITFALLS Pitfall 1 [CITED: .planning/research/PITFALLS.md]
// crypto/nonce.rs  — NO derive(Serialize), NO serde impl, NO Clone that outlives the round.
pub struct EphemeralNonces(frost::round1::SigningNonces);  // move-only; consumed in round 2
impl EphemeralNonces {
    pub fn commit(share: &frost::keys::SigningShare, rng: &mut impl RngCore)
        -> (Self, frost::round1::SigningCommitments) {
        let (n, c) = frost::round1::commit(share, rng);
        (Self(n), c)
    }
    pub fn sign(self, pkg: &frost::SigningPackage, kp: &frost::keys::KeyPackage)
        -> Result<frost::round2::SignatureShare, frost::Error> {
        frost::round2::sign_with_tweak(pkg, &self.0, kp)   // self consumed ⇒ nonce dropped
    }
}
```

### Pattern 4: Client-side verification gates (never trust the coordinator)
**What:** Display-before-sign — each participant recomputes the sighash from the PSBT locally (via the same `ChainBackend` sighash fn the coordinator uses) and is shown human-readable outputs/amounts/fee; signs only after ack (unless `--yes`). The coordinator sends the *PSBT*, never a precomputed sighash.
**When to use:** Every round 2 (SIGN-07). Build it in the very first signing flow, even in-process — retrofitting into a coordinator-authoritative flow is error-prone.

### Pattern 5: Trait seam for every side effect
**What:** `ChainBackend` and `Transport` are traits; orchestration depends only on the trait; the in-memory `Transport` stub and the Core/Esplora `ChainBackend` impls are injected. Keep `crypto/`+`bridge/` pure (no I/O deps).
**When to use:** Always — it is the mechanism that lets Phase 1 prove the crypto value in-process and lets Phase 7 swap in Nostr with zero call-site churn.

### Anti-Patterns to Avoid
- **Building transport before proving the bridge:** you can burn weeks on plumbing before discovering the signature doesn't verify against the address. Bridge + in-process sign first (order A→B→C).
- **Persisting/resuming signing nonces:** any `derive(Serialize)` transitively reaching nonce material, or any "resume"/"checkpoint" verb in the signing module, is the key-extraction bug class. New session on any restart/timeout.
- **Exposing the untweaked `aggregate`/`sign` to app code:** mixing tweaked/untweaked, or passing `Some(merkle_root)`, silently yields an invalid or wrong-key signature. Wire only `sign_with_tweak` + `aggregate_with_tweak(…, None)`.
- **Verifying against internal key `P` instead of output key `Q`:** passes in a unit test, fails on-chain. Always verify the aggregate against `Q`.
- **Blind-signing a coordinator-supplied sighash:** enables fund theft by a compromised coordinator. Recompute from the PSBT client-side.
- **Coupling `crypto/` to chain/transport/store:** bloats the audit/reproducible-build surface and breaks in-process testability.
- **Reusing round-1 commitments across sessions after a timeout:** breaks FROST security (nonce reuse). New nonces, possibly new subset.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| BIP341 taproot tweak / parity sign-flip | Custom `Q = P + t·G` + parity bookkeeping | `keys::Tweak::tweak(None)` (frost side) + `Address::p2tr(…, None, …)` (bitcoin side) | "A notorious implementation footgun — get it from BIP 445 / secp256k1-zkp, don't hand-roll" (scheme survey via PITFALLS). The two sides agree *by construction* |
| Even-Y normalization | Manual point-negation on parity | `keys::EvenY::into_even_y(None)` | frost tracks the `gacc` sign-flip internally so signatures still verify |
| Key-spend sighash | Manual BIP341 sighash serialization | `SighashCache::taproot_key_spend_signature_hash(idx, &Prevouts::All(&prevouts), TapSighashType::Default)` | Taproot signs the whole prevout set; a hand-rolled hash changes the challenge `e` |
| DKG / VSS | Any custom Feldman/Pedersen VSS | `keys::dkg::part1/2/3` | Audited; the whole point of choosing ZF frost |
| Cheater detection at aggregation | Custom share-validity checks | `aggregate_with_tweak` (detection default-on in 3.0) → `Error::culprits()` | Built-in identifiable abort returning `Vec<Identifier>` (SIGN-06) |
| Nonce zeroization | Manual `memset`/drop | `zeroize::Zeroizing` inside the nonce newtype | FROST 3.0 already makes `SigningKey: ZeroizeOnDrop`; use the same discipline for nonces |
| Regtest node lifecycle | Shell scripts spawning bitcoind | `corepc-node` `Node::from_downloaded()` | Auto-download + hash-verify pinned Core; killed on drop; free-port selection; hermetic (D-05) |
| "This must not compile" test | Fragile `cfg`/macro tricks | `trybuild` compile-fail with `.stderr` snapshot | Idiomatic, reviewable proof that the nonce type is non-serializable (SIGN-05) |
| Compressed-point serialization | Manual SEC1 encoding | `VerifyingKey::serialize()` | Canonical frost output; the bridge only slices the known 33-byte form |

**Key insight:** In this phase there is essentially *no* cryptography to author — only correct wiring of audited primitives and correct handling of three convention systems at one seam (the bridge). Every "clever" custom crypto path here is a documented catastrophic-bug vector. The engineering value is in the *tests and structural types*, not the algorithms.

## Common Pitfalls

These are the Phase-1-relevant entries from `.planning/research/PITFALLS.md` (HIGH confidence, spec-derived). Read that file in full before planning.

### Pitfall 1: Persisting or reusing signing nonces (key-extraction bug class)
**What goes wrong:** Reusing one committed nonce pair across two different sighashes gives an adversary (including a malicious coordinator who sees every partial) two linear equations in the same unknowns → solves for the share. 501 extracted shares reconstruct the group key forever.
**Why it happens:** Every *other* ceremony state is deliberately persisted for resumability; the nonce is the one violent exception, living in the same code paths. "Make signing resumable like everything else" is exactly the bug.
**How to avoid:** Non-serializable nonce newtype with `Zeroizing` inner state (Pattern 3); separate signing state from ceremony state; new session (fresh nonces, possibly new subset) on any restart/timeout; never reuse `SigningCommitments`.
**Warning signs:** `derive(Serialize)` transitively over nonce material; the words "resume"/"checkpoint" in the signing module; a session re-enterable with the same id.

### Pitfall 2: frost→rust-bitcoin bridge errors (x-only parity, wrong sighash, `P` vs `Q`)
**What goes wrong:** (1) x-only truncation dropping parity → sig doesn't verify under even-Y key; (2) building the address from `Q` but verifying against `P` (or importing the wrong one) → silent unspendable/invalid; (3) wrong sighash type (`SIGHASH_ALL` instead of `SIGHASH_DEFAULT`) or a missing prevout → wrong challenge `e`; (4) using non-`-tr` `frost-secp256k1` → RFC-9591 hash, never verifies on-chain.
**Why it happens:** Three convention systems meet at one function; rust-bitcoin happily builds an address from any x-only key and hashes any sighash type — silent until a real spend.
**How to avoid:** One canonical bridge fn (Pattern 1) pinned to BIP341/BIP86 KAT on day one (KEY-03); `sign_with_tweak`/`aggregate_with_tweak(…, None)`; verify against `Q`; a confirmed regtest key-spend at n=1000 is the only test that proves all four strands at once.
**Warning signs:** `from_slice` outside the one bridge fn; any non-default sighash type; the round-trip test asserts "it runs" not a hard-coded expected address string.

### Pitfall 7: Taproot tweak applied inconsistently during aggregation
**What goes wrong:** Aggregating with plain `aggregate` while participants used `sign_with_tweak` (or vice versa), or passing `Some(merkle_root)` when the design is key-only (`None`), or verifying against `P` — all silently produce an invalid or wrong-key signature. The untweaked path may "pass" a unit test that verifies against the internal key.
**How to avoid:** Single pipeline that always uses the tweaked path with `merkle_root: None` hard-wired; don't expose untweaked functions to app code; verify the aggregate against `Q` before finalizing.

### Pitfall 8: Blind signing — trusting the coordinator's sighash
**What goes wrong:** If participants sign the sighash the coordinator hands them, a compromised coordinator gets a 501-quorum to blind-sign an arbitrary tx (drain to attacker) while showing a benign summary.
**How to avoid:** Display-before-sign (Pattern 4); coordinator sends the PSBT, not a sighash; `--yes` is for automated/regtest only, loudly flagged, never the default.

### Pitfall 11: FROST is not robust — one dropout aborts a 501-way session
**What goes wrong:** FROST gives identifiable abort, not robustness; a single dropped/malformed partial aborts the whole session. At exactly-501 selection this thrashes.
**How to avoid:** Over-provision the liveness poll (poll a margin, finalize 501 from those who actually commit); on abort, start a *new* session (fresh nonces, possibly different subset) — never retry the same set/commitments. (In-process at Phase 1 there are no real dropouts, but the *session/abort semantics* must be built now so Phase 7 inherits them.)

### Pitfall 18 (partial): zeroize gaps
**What goes wrong:** Secret/ephemeral material left in memory after use.
**How to avoid this phase:** the nonce newtype uses `Zeroizing`; frost 3.0 already makes `SigningKey: ZeroizeOnDrop`. (At-rest share encryption is Phase 2, not now — D-09.)

## Code Examples

Verified against docs.rs (frost 3.0.0) and STACK.md (bitcoin 0.32.101) this session.

### DKG (in-process simulation, KEY-01/02) `[VERIFIED: docs.rs/frost-secp256k1-tr/3.0.0/keys/dkg]`
```rust
use std::collections::BTreeMap;
use frost_secp256k1_tr as frost;
use frost::keys::EvenY;

let mut rng = rand::rngs::OsRng;
let (max_signers, min_signers) = (1000u16, 501u16);   // D-02 acceptance target

// Round 1
let mut r1_secret = BTreeMap::new();
let mut r1_pkgs   = BTreeMap::new();
for i in 1..=max_signers {
    let id = i.try_into().expect("nonzero");
    let (secret, pkg) = frost::keys::dkg::part1(id, max_signers, min_signers, &mut rng)?;
    r1_secret.insert(id, secret);
    r1_pkgs.insert(id, pkg);           // "broadcast"
}

// Round 2 — each seat consumes its own r1 secret + everyone else's r1 packages
let mut r2_secret = BTreeMap::new();
let mut r2_pkgs_by_recipient: BTreeMap<_, BTreeMap<_, _>> = BTreeMap::new();
for (id, secret) in r1_secret {
    let others: BTreeMap<_, _> = r1_pkgs.iter().filter(|(k, _)| **k != id)
        .map(|(k, v)| (*k, v.clone())).collect();
    let (secret2, sent) = frost::keys::dkg::part2(secret, &others)?;  // sent: BTreeMap<Id, round2::Package>
    r2_secret.insert(id, secret2);
    for (recipient, pkg) in sent {
        r2_pkgs_by_recipient.entry(recipient).or_default().insert(id, pkg);
    }
}

// Round 3 — each seat produces its KeyPackage; all must verify to one PublicKeyPackage (KEY-06)
let mut key_packages = BTreeMap::new();
let mut group_pubkey: Option<frost::keys::PublicKeyPackage> = None;
for (id, secret2) in &r2_secret {
    let r1_others: BTreeMap<_, _> = r1_pkgs.iter().filter(|(k, _)| *k != id)
        .map(|(k, v)| (*k, v.clone())).collect();
    let (kp, pubkeys) = frost::keys::dkg::part3(secret2, &r1_others, &r2_pkgs_by_recipient[id])?;
    let kp = kp.into_even_y(None);                     // D-11
    let pubkeys = pubkeys.into_even_y(None);
    if let Some(g) = &group_pubkey {
        assert_eq!(g.verifying_key(), pubkeys.verifying_key(), "KEY-06: all seats agree on P");
    } else { group_pubkey = Some(pubkeys); }
    key_packages.insert(*id, kp);
}
```
Signatures: `part1(id, max, min, rng) -> (round1::SecretPackage, round1::Package)`; `part2(round1::SecretPackage, &BTreeMap<Id, round1::Package>) -> (round2::SecretPackage, BTreeMap<Id, round2::Package>)`; `part3(&round2::SecretPackage, &BTreeMap<Id, round1::Package>, &BTreeMap<Id, round2::Package>) -> (KeyPackage, PublicKeyPackage)`.

### Signing — two rounds, tweaked (SIGN-02/03) `[VERIFIED: docs.rs README round1::commit + CLAUDE.md sign_with_tweak/aggregate_with_tweak]`
```rust
// Round 1: each of the 501 chosen seats commits (nonces stay in RAM — Pattern 3)
let mut commitments = BTreeMap::new();
let mut nonces = BTreeMap::new();
for id in chosen_501 {
    let (n, c) = EphemeralNonces::commit(key_packages[&id].signing_share(), &mut rng);
    commitments.insert(id, c);
    nonces.insert(id, n);          // held in-memory only; never serialized
}

// Coordinator builds SigningPackage (message = the BIP341 key-spend sighash for this input)
let signing_package = frost::SigningPackage::new(commitments, &sighash_bytes);

// Round 2: each seat displays + acks (SIGN-07), then signs with the taproot tweak
let mut shares = BTreeMap::new();
for (id, n) in nonces {                      // consumes each nonce by value
    shares.insert(id, n.sign(&signing_package, &key_packages[&id])?);
}

// Aggregate with tweak, merkle_root = None (BIP86 key-only)
let group_sig = frost::aggregate_with_tweak(
    &signing_package, &shares, &group_pubkey_pkg, None::<&[u8]>,
)?;   // 64-byte BIP340 signature; cheater detection default-on → Error::culprits() on failure
```

### Verify against output key `Q` (SIGN-04) `[VERIFIED: frost Tweak/EvenY traits, docs.rs]`
```rust
use frost::keys::{Tweak, EvenY};
// Derive Q (tweaked output key) from the group public key package.
let tweaked = group_pubkey_pkg.clone().into_even_y(None).tweak(None::<&[u8]>);
let q: frost::VerifyingKey = *tweaked.verifying_key();
// The aggregate signature must verify against Q, NOT against internal key P.
q.verify(&sighash_bytes, &group_sig)?;       // last line before PSBT finalize
```

### Key-spend sighash from a PSBT (SIGN-01, SIGN-07) `[CITED: STACK.md — bitcoin 0.32.101]`
```rust
use bitcoin::sighash::{SighashCache, Prevouts, TapSighashType};
let mut cache = SighashCache::new(&unsigned_tx);
let sighash = cache.taproot_key_spend_signature_hash(
    input_index,
    &Prevouts::All(&all_prevout_txouts),   // Taproot signs ALL prevouts
    TapSighashType::Default,               // SIGHASH_DEFAULT, not SIGHASH_ALL
)?;
// `sighash` (32 bytes) is the message fed to SigningPackage::new and recomputed client-side.
```

### Non-serializable nonce compile-fail (SIGN-05) `[ASSUMED — idiomatic trybuild pattern]`
```rust
// tests/ui/nonce_no_serialize.rs  (must FAIL to compile)
fn main() {
    let n: tsig::crypto::EphemeralNonces = unimplemented!();
    let _ = serde_json::to_vec(&n);   // EphemeralNonces: !Serialize  → E0277
}
// tests/compile_fail.rs
#[test] fn nonce_is_not_serializable() {
    trybuild::TestCases::new().compile_fail("tests/ui/nonce_no_serialize.rs");
}
```

### Regtest auto-spawn + confirm (D-05) `[VERIFIED: docs.rs corepc-node 0.12.0]`
```rust
// tests/inproc_sign.rs
let node = corepc_node::Node::from_downloaded().unwrap();  // pinned Core via feature flag
let client = node.client;                                   // JSON-RPC client
// fund the P2TR address, build+sign the key-spend via the in-process session,
// broadcast the finalized tx, then mine to confirm:
let addr = /* bridge-derived regtest P2TR address */;
client.generate_to_address(101, &addr)?;                    // mature coinbase / confirm
// ... broadcast finalized tx, then client.generate_to_address(6, &miner_addr)? to confirm depth
// Node is killed automatically when `node` drops.
```
*Confirm the exact client method names (`generate_to_address` vs `generatetoaddress`) and the version-feature flag at plan time — see Assumptions A5.*

## State of the Art

| Old Approach | Current Approach | When Changed | Impact on Phase 1 |
|--------------|------------------|--------------|-------------------|
| `refresh_dkg_part_1`, `repair_share_step_1/2/3` | `refresh_dkg_part1`, `repair_share_part1/2/3` | frost 3.0 | Not used this phase (rotation is Phase 4); SPEC already uses new spellings |
| Cheater detection behind a feature flag | Default-on in `aggregate`/`aggregate_with_tweak` | frost 3.0 | SIGN-06 is free: aggregation returns culprits automatically; opt out only via `aggregate_custom(…, CheaterDetection::Disabled)` (don't) |
| `Error::culprit()` (single) | `Error::culprits() -> Vec<Identifier>` | frost 3.0 | Error handling must expect *multiple* culprits |
| `SigningKey: Copy` | `SigningKey: !Copy, ZeroizeOnDrop` | frost 3.0 | Aligns with nonce hygiene; adjust any code assuming `Copy` |
| `std`/`nightly` cargo features | All crates `no_std` (alloc); features removed | frost 3.0 | A std binary is unaffected; just don't toggle a `std` feature |
| `bitcoind` crate | Renamed/superseded by `corepc-node` | 2024–25 | Use `corepc-node` 0.12.0 (D-05); the old `bitcoind` crate still exists but development moved |
| `Address::p2tr(…, Network)` | `Address::p2tr(…, impl Into<KnownHrp>)` | bitcoin 0.32 | `Network` still coerces via `Into<KnownHrp>`; prefer `KnownHrp::Regtest` in node-less contexts |

**Deprecated/outdated — do NOT use:**
- `frost-secp256k1` (non-`-tr`): RFC-9591 challenge, not BIP340 — signatures never verify on-chain.
- `bitcoin 0.33.0-beta`: unsupported by `bitcoincore-rpc`/`esplora-client`.
- `secp256kfun`/`schnorr_fun`, tss-lib, luxfi/threshold: primitives-level / wrong-scheme / stub.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `VerifyingKey::serialize()` returns the 33-byte compressed SEC1 encoding (even-Y prefix `0x02`), sliceable to a 32-byte x-only key | Pattern 1, Bridge | LOW — if it returns a different length/format, the bridge slice indices change; caught immediately by the KEY-03 KAT. Verify exact return type at implementation |
| A2 | `into_even_y(None)` auto-detects and normalizes parity (the `Option<bool>` is `None`=auto) | Pattern 2 | LOW — method + signature verified on docs.rs; the `None` semantics ("auto") is inferred from the doc text "make sure the group public key has an even Y" |
| A3 | `sign_with_tweak`/`aggregate_with_tweak` internally apply BIP340/EvenY so the pipeline needs `into_even_y` only on the *stored/bridged* key, not re-applied per sign | Signing, Verify | MEDIUM — if the tweaked fns require a pre-normalized package, the crypto core must normalize before each call. The n=1000 confirmed spend (D-02) proves the real interplay; add a 01-02 spike |
| A4 | Verifying the aggregate against `Q` via `pubkeys.into_even_y(None).tweak(None).verifying_key()` yields the same key the P2TR address commits to | Verify, KEY-03 | MEDIUM — this equality IS the KAT (KEY-03) and the on-chain confirm (SIGN-04); if it fails the bridge is wrong. That's the whole point of the test |
| A5 | `corepc-node` 0.12.0 exposes `Node::from_downloaded()` and a JSON-RPC client with a `generate_to_address`-style method; version selected by a feature like `28_0` | Regtest example | LOW — auto-download + free-port + kill-on-drop verified on docs.rs; exact method names/feature string confirmed at plan time via `cargo doc`/crate README |
| A6 | `trybuild` compile-fail with a `.stderr` snapshot is the chosen SIGN-05 mechanism | Code Examples §Nonce | LOW — this is a Claude's-discretion area (D-08 discretion list); alternative is a `#[doc = compile_fail]` test. Either satisfies SIGN-05 |
| A7 | BIP341's `scriptPubKey` test-vector entries with `scriptTree: null` are valid key-only (merkle root `None`) KATs, and the set includes at least one odd-Y internal key for the D-11 odd-Y-origin case | Validation Architecture, KEY-03 | MEDIUM — needs confirmation against the actual BIP341 vector JSON; if no null-tree odd-Y entry exists, construct one from a known internal key and independently compute `Q` (BIP86 algorithm) as the reference |

**These assumptions need confirmation during 01-01/01-02 implementation spikes before becoming locked.** None blocks planning; all are caught by the KEY-03 KAT or the n=1000 confirmed-spend gate.

## Open Questions

1. **n=1000 in-process DKG feasibility (O(n²) cost).**
   - What we know: part2 produces `n−1` packages per seat → ~10⁶ `round2::Package` objects total held in `BTreeMap`s; part3 verifies ~999 round1 + ~999 round2 packages per seat. Round1 packages carry `t=501` commitment points (~33 B each ≈ 16 KB/pkg ≈ 16 MB for 1000). Round2 packages are small (~a scalar). Rough memory: low hundreds of MB. Compute is the concern: part1 is O(t) point-muls/seat; part3 verification is ~O(n·t) point-ops/seat → O(n²·t) ≈ 5×10⁸ group operations overall.
   - What's unclear: wall-clock (could be minutes to tens of minutes single-threaded in debug); peak RSS.
   - Recommendation: run the n=1000 DKG in `--release`; instrument each part (wall-clock + peak RSS) as the KEY-06/O(n²) deliverable (D-03); consider parallelizing the *simulation loop* with `rayon` (the crypto is per-seat independent) while keeping per-seat calls deterministic; gate the full run as nightly/on-demand (D-06), not per-PR. Measure before optimizing.

2. **`ChainBackend` trait shape (sync vs async).** `bitcoincore-rpc` 0.19 is synchronous; `esplora-client` 0.13 offers both blocking and async. Recommendation: define `ChainBackend` **synchronous** for Phase 1 (Core RPC is sync; the confirm path is Core per D-07) and use `esplora-client`'s blocking API for trait-conformance. Revisit async only if a later phase needs it — a sync trait keeps the in-process proof simple.

3. **`Transport` trait shape (sync/async, envelope model).** Only the in-memory stub is needed now, but the trait must fit the later Nostr event model (signed envelope, per-message-class kinds, directed vs broadcast, dedup by id). Recommendation: model an `Envelope { class, ceremony/session id, round, seat, recipient: Option<..>, payload: bytes }` and a `publish`/`subscribe(filter)` pair; keep it synchronous for the stub but design the payload as opaque bytes so NIP-44 encryption slots in at Phase 7 without touching orchestration. Do not leak `nostr-sdk` types into the trait.

4. **Public-artifact file format (D-09).** `PublicKeyPackage` serialized how — frost's default `serialization` (postcard) or `serde_json`? Recommendation: use frost's `serialize()`/`deserialize()` for the canonical bytes wrapped in a small JSON/TOML envelope carrying `key_id` + (future) `epoch`, so `tsig address --pubkey <file>` is stable and human-inspectable. Confirm the frost serialization API shape at implementation.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | Everything | ✓ | rustc/cargo **1.96.0** | — (MSRV floor is 1.85; well satisfied) |
| `bitcoind` (Bitcoin Core) | regtest confirm path (D-05, SIGN-04) | ✗ on PATH | — | **`corepc-node` auto-download** fetches + hash-verifies a pinned Core binary at build time — no system install needed |
| Docker | not required Phase 1 (Phase 7 load test) | ✓ | present | — |
| Network egress (crates.io + Core download) | first build + `corepc-node` download | assumed ✓ | — | Pre-download/cache the Core binary for offline CI; vendor crates via `cargo vendor` if air-gapped |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** `bitcoind` — covered by `corepc-node`'s auto-download feature (the intended D-05 mechanism). Ensure CI has egress on first run or pre-seed the download cache.

## Validation Architecture

Nyquist validation is enabled. Every Phase-1 success criterion has a concrete automated proof.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `cargo test`; `trybuild` for compile-fail; `corepc-node` for regtest integration |
| Config file | none — Cargo convention (`tests/`, `#[test]`, `#[ignore]` for the heavy n=1000 run) |
| Quick run command | `cargo test` (unit + small-n + bridge KAT + trybuild) |
| Full suite command | `cargo test -- --include-ignored` (adds the t=501/n=1000 end-to-end + O(n²) bench) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| KEY-03 | Bridge byte-level round-trip vs BIP341/BIP86 KAT (even-Y AND odd-Y-origin), assert exact address string | unit (KAT) | `cargo test --test bridge_roundtrip` | ❌ Wave 0 |
| KEY-01/02 | In-process DKG (small-n) → `KeyPackage`+`PublicKeyPackage`; verifying key = `P` | unit | `cargo test dkg_small` | ❌ Wave 0 |
| KEY-04 | `tsig address` prints correct P2TR from a `PublicKeyPackage` file | unit + CLI | `cargo test address_from_pubkey` | ❌ Wave 0 |
| KEY-05 | Every seat's verifying key matches; mismatch aborts | unit | `cargo test keygen_confirm_mismatch_aborts` | ❌ Wave 0 |
| KEY-06 | 1000 `KeyPackage`s all verify to one `PublicKeyPackage` | integration (ignored) | `cargo test --test dkg_1000_correctness -- --ignored` | ❌ Wave 0 |
| KEY-06/D-03 | O(n²) timing + peak-RSS instrumentation across part1/2/3 at n=1000 | bench/report | `cargo test --test dkg_1000_correctness -- --ignored --nocapture` | ❌ Wave 0 |
| SIGN-01 | Key-spend sighash per input from a PSBT (default type, all prevouts) | unit | `cargo test sighash_key_spend` | ❌ Wave 0 |
| SIGN-02 | Round 1 commit collects `SigningCommitments`; over-provisioned poll finalizes 501 | unit | `cargo test round1_commit_subset` | ❌ Wave 0 |
| SIGN-03 | Round 2 `sign_with_tweak` + `aggregate_with_tweak(…,None)` → 64-byte sig | unit | `cargo test aggregate_tweaked` | ❌ Wave 0 |
| SIGN-04 | Aggregate verifies against `Q` (not `P`); PSBT finalizes | unit | `cargo test verify_against_Q` | ❌ Wave 0 |
| SIGN-04 | **Confirmed regtest key-spend** small-n (PR) and t=501/n=1000 (nightly) | integration | `cargo test --test inproc_sign` / `--test inproc_sign_1000 -- --ignored` | ❌ Wave 0 |
| SIGN-05 | Nonce type does not compile if serialized | compile-fail | `cargo test --test compile_fail` (trybuild) | ❌ Wave 0 |
| SIGN-05 | Session restart mints fresh nonces + new session id (not a resume) | unit | `cargo test session_restart_fresh_nonces` | ❌ Wave 0 |
| SIGN-06 | Aggregation surfaces `culprits()`; timeout → new session, no commitment reuse | unit | `cargo test cheater_culprits`, `cargo test abort_new_session` | ❌ Wave 0 |
| SIGN-07 | Recompute sighash from PSBT client-side; mismatched-summary refuses; `--yes` bypass | unit | `cargo test display_before_sign_mismatch_refuses` | ❌ Wave 0 |
| STOR-04 | `ChainBackend` trait; Core RPC + Esplora both satisfy trait-conformance | unit/integration | `cargo test chain_backend_conformance` | ❌ Wave 0 |

### Property / adversarial checks (structural controls)
- **Parity backstop (D-11):** KAT covers even-Y AND an odd-Y-origin vector, each verified end-to-end (produce a signature that verifies against `Q`). Optionally a property test over random DKG runs asserting `has_even_y()` post-normalization and that a spend confirms.
- **Nonce non-reuse:** a test that reuses `SigningCommitments` across two `SigningPackage`s with different sighashes must be rejected before any signature share is emitted.
- **Blind-sign refusal:** a "malicious coordinator" test where the displayed summary differs from the PSBT outputs → participant refuses.

### Sampling Rate
- **Per task commit:** `cargo test` (small-n end-to-end + bridge KAT + trybuild) — fast PR feedback (D-06).
- **Per wave merge:** `cargo test` full + `cargo audit`.
- **Phase gate:** `cargo test -- --include-ignored` green, including the **t=501/n=1000 confirmed regtest key-spend** and the O(n²) report (D-02/D-06), before `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `tests/bridge_roundtrip.rs` — KEY-03 KAT harness + committed BIP341/BIP86 vectors (even-Y + odd-Y-origin)
- [ ] `tests/inproc_sign.rs` — small-n regtest end-to-end via `corepc-node`
- [ ] `tests/inproc_sign_1000.rs` — `#[ignore]` t=501/n=1000 end-to-end (nightly gate)
- [ ] `tests/dkg_1000_correctness.rs` — `#[ignore]` KEY-06 + O(n²) instrumentation
- [ ] `tests/ui/nonce_no_serialize.rs` + `tests/compile_fail.rs` — trybuild SIGN-05
- [ ] Test fixtures: a funded regtest PSBT builder helper; `Prevouts` assembly helper
- [ ] Dev-dependency install: `corepc-node` (with the pinned Core-version feature) + `trybuild`

## Security Domain

`security_enforcement: true`, ASVS level 1, block-on: high. This is a security-critical crypto phase; the "attack surface" is key material and signature correctness, not web I/O.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (Phase 1) | Roster/npub auth is Phase 7 transport; no auth surface in-process |
| V3 Session Management | partial | "Session" here = FROST signing session: fresh id + fresh nonces on every start; never resumable (Pattern 3) |
| V4 Access Control | no (Phase 1) | Coordinator-untrusted model enforced by client-side gates, but no multi-user access control yet |
| V5 Input Validation | yes | PSBT parsed and sighash **recomputed** client-side (never trust coordinator-supplied hash); prevout set validated; even-Y invariant asserted at the bridge |
| V6 Cryptography | yes (core) | Never hand-roll: DKG/tweak/parity/sighash all via audited `frost-secp256k1-tr` + `bitcoin`; verify against `Q`; nonces `Zeroizing` + non-serializable; `SigningKey: ZeroizeOnDrop` |
| V7 Error Handling & Logging | yes | Fail early with clear errors (odd-Y rejected, culprits surfaced); never log secret share/nonce material |
| V14 Configuration | yes | Pinned versions + committed `Cargo.lock`; `corepc-node` hash-verifies the downloaded Core binary |

### Known Threat Patterns for this stack
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Nonce persistence/reuse → key extraction | Information Disclosure / Elevation | Non-serializable `Zeroizing` nonce newtype; new session on restart; trybuild compile-fail proof (SIGN-05) |
| Bridge parity/tweak/sighash error → unspendable or invalid/wrong sig | Tampering | One canonical bridge fn pinned to BIP341/BIP86 KAT; verify against `Q`; confirmed regtest spend (KEY-03, SIGN-04) |
| Blind-signing a coordinator-supplied sighash → fund theft | Spoofing / Tampering | Display-before-sign; recompute sighash from PSBT client-side (SIGN-07) |
| Untweaked/`Some(merkle_root)` aggregation → wrong-key sig | Tampering | Single tweaked pipeline, `merkle_root: None` hard-wired; untweaked path not exposed (Pitfall 7) |
| Secret material lingering in memory | Information Disclosure | `zeroize::Zeroizing`; frost 3.0 `ZeroizeOnDrop`; no `Serialize` on secrets |
| Supply-chain: malicious/typosquat crate or tampered Core binary | Tampering | All crates verdict OK (legitimacy audit); `cargo audit`/`cargo deny` (SEC-01, gated per D-06); `corepc-node` verifies Core binary hash |
| FROST non-robustness → session-abort DoS at 501 | Denial of Service | Over-provision liveness poll; new session on abort (Pitfall 11) — semantics built now, stress-tested Phase 7 |

**Out of scope this phase (later):** roster pinning / non-roster event rejection (Phase 7), Nostr↔FROST key separation (Phase 7), same-key-after-refresh check (Phase 4), at-rest share encryption (Phase 2), mixed-epoch rejection (Phase 4 schema / Phase 6 test). The nonce-reuse-won't-compile and bridge controls are the Phase-1 security deliverables.

## Sources

### Primary (HIGH confidence)
- docs.rs `frost-secp256k1-tr` 3.0.0 — `keys::dkg::part1/2/3` signatures + example; `keys::EvenY` (`has_even_y`, `into_even_y(Option<bool>)`, 6 implementers); `keys::Tweak` (`tweak<T: AsRef<[u8]>>(Option<T>)`, KeyPackage/PublicKeyPackage) — fetched this session
- docs.rs / crates.io `corepc-node` 0.12.0 (2026-04-14) — auto-download version features, `Node::from_downloaded`, kill-on-drop, free-port selection — fetched this session
- `gsd-tools query package-legitimacy check --ecosystem crates` — all 9 Phase-1 crates verdict OK with repos + download counts (2026-07-10)
- `.planning/research/PITFALLS.md` — Pitfalls 1, 2, 7, 8, 11, 18 (HIGH, spec-derived) — highest-value read
- `SPEC-frost-cli.md` §3, §6.5, §9, §11 — normative cryptography, signing/nonce discipline, bridge, security
- `.planning/research/STACK.md` / `ARCHITECTURE.md` / `./.claude/CLAUDE.md` — locked pins, API-surface confirmations, module seams, build order (crates.io/docs.rs verified 2026-07-10)

### Secondary (MEDIUM confidence)
- WebSearch — `corepc-node` is the successor to the `bitcoind` crate; auto-download + hash-verify behavior (cross-checked against docs.rs)

### Tertiary (LOW confidence)
- Training knowledge for idiomatic `trybuild` usage and BIP341/BIP86 vector structure (flagged in Assumptions A6/A7 for confirmation during implementation)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — versions + API surface re-verified against docs.rs and the crates.io legitimacy seam this session
- Architecture / patterns: HIGH — grounded in SPEC + ARCHITECTURE.md + PITFALLS.md (all HIGH, spec-derived)
- DKG / signing / bridge APIs: HIGH — exact signatures fetched from docs.rs 3.0.0 this session (few interplay details in Assumptions A3/A4, resolved by the confirmed-spend gate)
- Pitfalls: HIGH — curated in-repo threat literature
- n=1000 O(n²) cost: MEDIUM — order-of-magnitude reasoning; exact wall-clock/RSS is the KEY-06 measurement deliverable

**Research date:** 2026-07-10
**Valid until:** ~2026-08-10 (stable pinned stack; re-verify `corepc-node` feature flags and any frost 3.0.x patch before execution)
</content>
</invoke>
