# Project Research Summary

**Project:** tsig — 501-of-1000 FROST Taproot Signing CLI
**Domain:** Rust CLI — FROST threshold-Schnorr, Bitcoin Taproot key-path signing, Nostr transport, large-membership (t=501/n=1000)
**Researched:** 2026-07-10
**Confidence:** HIGH

## Executive Summary

`tsig` is a single-binary Rust CLI that lets a fixed 501-of-1000 group jointly control one Bitcoin Taproot address via FROST threshold Schnorr (RFC 9591 / BIP340/341 key-path). On-chain its spends are indistinguishable from single-sig. The way experts build this class of tool is now settled: the entire cryptographic layer is the NCC-audited `frost-secp256k1-tr` crate (>=3.0), the address/signature bridge goes through `rust-bitcoin`, and ceremony traffic rides `nostr-sdk` over self-hosted relays. Research confirmed the SPEC's stack is correct and current at pinned versions — no correction needed — with one structural fact that drives the whole build: **FROST and rust-bitcoin do not share a curve crate.** FROST uses pure-Rust `k256`; rust-bitcoin uses the `secp256k1` C bindings. There is no Cargo version to align between them, so the frost->bitcoin key bridge is strictly **byte-level** (33-byte SEC1 -> x-only 32B -> `XOnlyPublicKey`), pinned by a mandatory round-trip test.

The recommended approach is a **layered, trait-seamed monolith** built **bridge-first**: prove the frost<->rust-bitcoin bridge and an in-process 501-of-1000 signature — with zero transport, node RPC, or persistence — before any relay work exists. This directly de-risks the highest-value integration bug class and matches the PROJECT decision to prove the crypto bridge early. The crypto core and bridge are pure and I/O-free (the small, auditable, reproducible trusted computing base); chain, transport, and storage sit behind traits so orchestration runs identically in-process (tests), over files (air-gapped), or over Nostr/Core. That single seam is what lets the value be proven before the O(n^2) DKG transport is even attempted.

The key risks are concentrated and severe: (1) **nonce persistence/reuse** is a direct key-extraction bug class and must be prevented *structurally* (a non-`Serialize`, zeroizing nonce newtype) from the first line of signing code; (2) **bridge errors** (x-only parity, internal key `P` vs output key `Q`, wrong sighash, wrong crate) yield unspendable addresses or invalid signatures and are prevented by one canonical bridge fn + a known-answer round-trip vector + a confirmed regtest spend; (3) **blind signing** lets a compromised coordinator drain funds, prevented by mandatory client-side sighash recompute (display-before-sign); (4) **skipping the client-side same-key check after refresh** can lock funds in a dead key. Rotation is explicitly *not* revocation — 501 shares from any one past epoch reconstruct the key forever — so the on-chain **sweep to a pre-generated standby** is the real revocation mechanism. These four structural rules must exist from M1, not be retrofitted.

## Key Findings

### Recommended Stack

The SPEC section 12 stack is verified correct and current against crates.io/docs.rs/CHANGELOGs as of 2026-07-10 (not training data). Every claimed `frost-secp256k1-tr` primitive exists at 3.0.0, and the SPEC already uses the post-3.0 rename spellings (`refresh_dkg_part1`, `repair_share_part1/2/3`). Two non-blocking facts to internalize: the FROST/rust-bitcoin curve split makes the bridge byte-level (no version to align), and the Bitcoin client layer pins you to `bitcoin = 0.32.x` (both `bitcoincore-rpc` and `esplora-client` require it — do not adopt 0.33.0-beta). MSRV floor is Rust 1.85 (driven by `toml` 1.1.x). Commit `Cargo.lock`; run `cargo audit`/`cargo deny` in CI with a deliberate duplicate-`secp256k1` allow-list.

**Core technologies:**
- `frost-secp256k1-tr` **3.0.0** (+ `frost-core` 3.0.x): entire crypto layer (DKG, dealer keygen, refresh-DKG, repair/enroll, `sign_with_tweak`/`aggregate_with_tweak(..., None)`) — the only audited, packaged Rust FROST exposing every primitive incl. the BIP-341 tweak. Uses `k256`.
- `bitcoin` (rust-bitcoin) **0.32.101**: address, PSBT, BIP341 sighash, x-only keys — canonical; pin 0.32.x (RPC/esplora clients require it). Uses `secp256k1` C bindings.
- `nostr-sdk` **0.44.1** (`nip44`, `nip59` features; NIP-42 needs no flag): event build/sign/publish/subscribe, multi-relay pool + dedup, NIP-44 v2 — replaces a bespoke relay stack. 0.45 is alpha; stay on 0.44.1 stable.
- Supporting: `bitcoincore-rpc` 0.19 / `esplora-client` 0.13 (chain, behind one trait), `age` 0.11.3 + `zeroize` 1.9 (at-rest + memory hygiene), `clap` 4.6, `rusqlite` 0.40 (`bundled`), `serde`/`serde_json`/`toml`.

**FROST 3.0 breaking changes to account for:** cheater detection is now default (keep it on at t=501); `Error::culprit()` -> `culprits()` returns a `Vec`; `PublicKeyPackage::new()` now requires `min_signers`; `SigningKey` is no longer `Copy` and is `ZeroizeOnDrop`; crates are `no_std` (alloc), `std`/`nightly` features removed. One rust-bitcoin note: `Address::p2tr`'s 4th arg is now `impl Into<KnownHrp>` (a `Network` still satisfies it — existing calls compile).

### Expected Features

The feature set is fixed by SPEC-frost-cli.md v0.1; research categorized it against how comparable tooling (ZF frost, Chainflip, DFINITY chain-key, Frostsnap, Sparrow/Nunchuk/Liana) actually behaves. The distinctive product is essentially "ZF frost's rotation/repair primitives, operationalized at n=1000 with a Nostr transport and a sweep-based revocation lifecycle" — a combination no existing OSS tool offers.

**Must have (table stakes — a signer is broken without these):**
- Keygen (dealer + DKG modes) -> `PublicKeyPackage`/`KeyPackage`
- Address derivation (frost->rust-bitcoin bridge, BIP86-style, merkle root `None`)
- Two-round FROST signing with taproot tweak -> 64-byte BIP340 key-spend sig
- PSBT parse + BIP341 sighash; at-rest share storage + memory hygiene
- Nonce discipline (in-memory only); resumable/idempotent ceremonies (table stakes *at n=1000*)
- Repair (recover a lost share); share status; coordinator SQLite state; Bitcoin chain integration

**Should have (differentiators — this design's distinctive bets):**
- Membership rotation, same address (refresh + enroll) — zero on-chain footprint (the headline bet)
- Sweep-as-revocation to a pre-generated standby — the *real* revocation mechanism
- Standby key lifecycle + policy-driven sweep triggers (`value_cap`, `churn_budget`, `max_epochs`, `standby_max_age`) + watcher
- Nostr transport (signed events, multi-relay, NIP-44, roster pinning) + offline file fallback
- Display-before-sign; mandatory client-side same-key postcondition; epoch discipline / mixed-epoch rejection
- Verifiability posture (reproducible builds, pinned/audited deps)

**Defer / out of scope (SPEC non-goals):** in-place threshold change (only via sweep), verifiable erasure, script-path spends, multi-address wallet mgmt, coin selection beyond consolidation, any ECDSA, alt crypto libraries.

### Architecture Approach

A layered, trait-seamed monolith: one binary, three personae (participant/coordinator/watcher) by subcommand, with a strict internal dependency stack — pure crypto at the bottom, side-effecting adapters (chain/transport/store) behind traits in the middle, orchestration above, CLI on top. Security-critical logic is concentrated in the lowest two layers (crypto core + bridge = the trusted computing base) so the auditable, reproducible surface stays small. Orchestration imports only traits, so the same ceremony/session code runs in-process, over files, or over Nostr — which is exactly what the bridge-first build order exploits.

**Major components:**
1. **Crypto core** (`crypto/`, L0) — pure `frost-secp256k1-tr` wrapper (DKG, dealer, refresh, repair/enroll, commit, `sign_with_tweak`, `aggregate_with_tweak`); no I/O. Trusted compute base.
2. **Key bridge** (`bridge/`, L0.5) — `VerifyingKey` -> x-only -> `XOnlyPublicKey` -> P2TR + `Q` derivation; the byte-level round-trip test lives here. Highest-risk seam.
3. **Adapters** (L2, all behind traits) — `ChainBackend` (Core/Esplora, PSBT, sighash), `Transport` (Nostr/File), `Storage` (participant age-encrypted file store + coordinator SQLite).
4. **Orchestration** (L3) — ceremony engine (resumable/idempotent, epoch bookkeeping, same-key check), signing session (nonces memory-only), lifecycle + policy (standby/sweep/rollover, watcher).
5. **CLI** (`cli/`, L4) — clap dispatch, config/relay/passphrase resolution, exit codes.

### Critical Pitfalls

1. **Persisting/reusing signing nonces (key extraction).** The single highest-severity rule. Two signatures under one committed nonce pair solve for the share. Prevent *structurally*: a non-`Serialize`, `Zeroizing` nonce newtype the store API physically cannot accept; separate signing-session state (ephemeral) from ceremony state (resumable); new session + fresh nonces on any restart/timeout — never reuse commitments. (M1 structural, M5 review.)
2. **frost->rust-bitcoin bridge errors** — x-only parity drop, internal key `P` vs output key `Q` confusion, wrong sighash type, or using non-`-tr` crate -> unspendable address or invalid sig. Prevent: one canonical bridge fn + one canonical sighash fn, a byte-level round-trip test against a known-answer vector, and a confirmed regtest key-spend broadcast in M1. Verify aggregate against `Q`, not `P`. (M1.)
3. **Blind signing** — participants trusting the coordinator's sighash lets a compromised coordinator get a quorum to sign an arbitrary tx. Prevent: participants recompute the sighash from the PSBT locally and display-before-sign; coordinator sends the PSBT, not a hash; `--yes` is automated/regtest only. Build the gate in M1 — do not retrofit. (M1 gate, M5 test.)
4. **Skipping the client-side same-key check after refresh** — a buggy/malicious refresh can produce a different verifying key; deleting old shares after that = permanent fund loss. Prevent: every client asserts `new verifying_key == pinned old` before persisting/deleting, using the *local* pinned key. (M3.)
5. **Deletion-as-revocation fallacy + n=1000 O(n^2) transport wall.** No security claim rests on share deletion — 501 one-epoch shares reconstruct forever; the **sweep to standby** is the real revocation (M4). And a DKG at n=1000 is ~10^6 events / ~1 GB — never point it at a public relay; require >=3 self-hosted strfry relays with raised limits/retention, paced round-2 batches, and idempotent resume keyed by `(ceremony_id, round, seat)` (M2).

## Implications for Roadmap

Research strongly endorses a bridge-first, trait-seamed build order that maps cleanly onto the SPEC's M1-M5 milestones (PROJECT decision: roadmap covers all of M1-M5). The architecture's finer-grained order A-J collapses into these five phases.

### Phase 1 (M1): Prove the crypto bridge + in-process signing
**Rationale:** The frost<->rust-bitcoin bridge is the highest-risk, lowest-infrastructure component and the whole value proposition. It needs no relays, node RPC, or persistence — just the crypto crate + rust-bitcoin. If it fails, nothing else matters. Building transport first is the #1 anti-pattern.
**Delivers:** Byte-level bridge + round-trip test (known-answer vector); crypto core; dealer keygen then in-process DKG; two-round signing with taproot tweak -> BIP340 sig over a regtest key-spend sighash, verified against `Q`, broadcast and confirmed on regtest with 501 simulated participants. PSBT/sighash via `ChainBackend`. Display-before-sign gate and the non-serializable nonce type exist from day one.
**Addresses:** Keygen, address bridge, two-round signing, PSBT/sighash, nonce discipline, display-before-sign.
**Avoids:** Pitfalls 1 (nonce), 2 (bridge), 7 (tweak/aggregate against `Q`), 8 (blind signing) — all structural from M1.

### Phase 2 (M2): Real transport + DKG at n=1000
**Rationale:** With the bridge proven, layer transport. Build `FileTransport` before `NostrTransport` (identical interface, no network complexity) to validate the message schema + resumption deterministically before O(n^2) relay tuning. Establish the `(key_id, epoch)` session-binding schema, Nostr key separation, and roster pinning here so they need no retrofit.
**Delivers:** Storage (age-encrypted participant store enforcing nonce-exclusion + coordinator SQLite); Transport trait + File then Nostr (multi-relay, NIP-44 per-class, roster pin, NIP-42, dedup); resumable/idempotent ceremony engine; small-n DKG then n=1000 load test over self-hosted strfry with paced batches; distributed signing over transport; version handshake in ceremony-open.
**Uses:** `nostr-sdk` 0.44.1, `age`/`zeroize`, `rusqlite`, `bitcoincore-rpc`/`esplora-client`.
**Implements:** Transport, Storage, ChainBackend adapters; ceremony engine.
**Avoids:** Pitfalls 5 (roster pinning), 6 (Nostr key != FROST), 10 (public relay / O(n^2)), 14 (NIP-44 misuse), 15 (version skew), 20 (replay).

### Phase 3 (M3): Membership rotation
**Rationale:** Rotation depends on the ceremony engine + epoch bookkeeping from M2. Enroll and refresh are effectively one deliverable — enroll must atomically chain a refresh to proactivize helper delta-knowledge before the epoch boundary closes.
**Delivers:** Refresh (removals + proactivize), enroll (repair/RTS -> immediate refresh), repair, epoch bookkeeping, mandatory client-side same-key postcondition, mixed-epoch rejection, monotonic identifier allocation.
**Avoids:** Pitfalls 3 (mixed-epoch), 4 (same-key check), 12 (enroll without refresh), 16 (identifier reuse).

### Phase 4 (M4): Key lifecycle + revocation
**Rationale:** The sweep is the true revocation and depends on a working rotation + chain integration. Standby must be pre-generated and kept refreshed so a sweep is a routine signing session, not an emergency ceremony.
**Delivers:** Standby key lifecycle (`standby new`), sweep flow (all UTXOs -> standby, standby->active rollover on >=6 confs, ACTIVE->RETIRED), policy engine (4 knobs), cron/CI `watch` with exit 2 + JSON report.
**Avoids:** Pitfalls 9 (deletion-as-revocation), 13 (standby neglect).

### Phase 5 (M5): Hardening + security-reviewable release
**Rationale:** Ships the trust: 1000 people must verify what they run. Adversarial tests re-verify the structural rules laid down earlier.
**Delivers:** Offline file-mode polish, reproducible builds, pinned/audited deps (`cargo audit`/`cargo deny`), adversarial suite (malicious relay, mixed-epoch, replay, nonce-reuse-won't-compile), external review of section 6.5 nonce discipline + bridge, threat-model doc accuracy (static-only security claim).
**Avoids:** Re-verifies Pitfalls 1, 2, 5, 8; closes 18 (at-rest/zeroize audit), 19 (adaptive over-claim).

### Phase Ordering Rationale

- **Dependency-driven:** address requires keygen; signing is the convergence of keygen + bridge + chain + PSBT; rotation requires the ceremony engine + epochs; sweep requires rotation + standby + chain; watch feeds the sweep decision. This forces the A-J / M1-M5 order.
- **Risk-driven:** the bridge is highest-risk / lowest-infra, so it goes first (in-process, zero transport). Everything after M1 is "plumbing at scale" — real but lower-risk.
- **Structural-from-day-one:** four rules (non-serializable nonce type, byte-level bridge round-trip, tweak/aggregate verified against `Q`, display-before-sign recompute) must exist in M1's first signing code, because retrofitting them into an established coordinator-authoritative flow is far more error-prone.
- **Cross-cutting schema early:** resumable/idempotent infra, `(key_id, epoch)` binding, roster pinning, and Nostr key separation are laid down in M2 so M3+ don't retrofit them.

### Research Flags

Phases likely needing deeper research (`/gsd-plan-phase --research-phase <N>`) during planning:
- **Phase 2 (M2):** The n=1000 O(n^2) DKG over Nostr is the real engineering wall — needs concrete strfry/nostr-rs-relay config research (rate limits, retention, event-kind policy), round-2 batching/pacing strategy, and resumption-under-partial-relay-failure design. Highest unknown in the project.
- **Phase 4 (M4):** Sweep tx construction (RBF consolidation, feerate estimation), rollover state machine, and policy-trigger semantics warrant a focused pass.

Phases with well-documented patterns (lighter research):
- **Phase 1 (M1):** APIs are source-verified at pinned versions; the bridge/sighash/tweak conventions are enumerated in PITFALLS.md. Execute against the known-answer vectors.
- **Phase 3 (M3):** ZF frost refresh/repair/enroll primitives are confirmed present; the same-key check pattern mirrors coinbase/cb-mpc. Well-specified.
- **Phase 5 (M5):** Standard hardening/reproducible-build practices; the adversarial cases are already enumerated.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Versions + API surface verified against crates.io/docs.rs/CHANGELOGs on 2026-07-10 (not training data). Every FROST primitive confirmed at 3.0.0. |
| Features | HIGH | Feature set fixed by authoritative SPEC v0.1 + PROJECT.md; competitor grounding from source-verified implementations-resharing.md. |
| Architecture | HIGH | Synthesized from SPEC + companion research; component boundaries and build order follow directly from the trait-seam pattern and dependency graph. |
| Pitfalls | HIGH | Spec-derived (section 11) + curated threshold-Schnorr / Taproot failure-mode literature; each pitfall maps to a normative rule and a milestone. |

**Overall confidence:** HIGH

### Gaps to Address

- **NIP-42 AUTH confirmation is MEDIUM-HIGH:** documented in `nostr-sdk` but not a separate cargo feature flag — validate the exact API (`ClientMessage::auth` / relay `Auth`) during M2 planning.
- **`bitcoincore-rpc` 0.19 staleness (May 2024):** still targets `bitcoin 0.32` (our pin) so acceptable now, but abstract chain access behind the trait (already planned) so a future client swap/fork is cheap. Watch upstream.
- **n=1000 relay tuning is theoretical until load-tested:** the ~10^6-event / ~1 GB ceremony behavior, exact strfry rate-limit/retention settings, and round-2 pacing thresholds are unproven until the M2 containerized load test. Treat M2 as the empirical de-risking milestone.
- **`age` labelled BETA upstream:** API stable across 0.11.x and de-facto standard; acceptable, but pin exactly and note in the reproducible-build audit.

## Sources

### Primary (HIGH confidence)
- `.planning/research/STACK.md` — verified versions/API surface via crates.io + docs.rs + upstream CHANGELOGs (2026-07-10): `frost-secp256k1-tr` 3.0.0 primitives, FROST 3.0 breaking changes, `bitcoin` 0.32.101 sighash/p2tr, `nostr-sdk` 0.44.1 NIPs, version-compat matrix.
- `.planning/research/FEATURES.md` — table-stakes/differentiator/anti-feature categorization, dependency graph, MVP -> M1-M5 mapping, competitor analysis (grounded in implementations-resharing.md).
- `.planning/research/ARCHITECTURE.md` — layered trait-seamed monolith, component/trust boundaries, data flows, dependency-driven build order A-J.
- `.planning/research/PITFALLS.md` — 20 pitfalls (spec section 11 + threshold-Schnorr/Taproot literature), pitfall-to-milestone map, "looks done but isn't" checklist, recovery strategies.
- `SPEC-frost-cli.md` (draft v0.1, 2026-07-09) — authoritative design contract; `.planning/PROJECT.md` — Core Value, requirements, key decisions (DKG-first, bridge-proven-early, security-reviewable OSS).

### Secondary (MEDIUM confidence)
- `implementations-resharing.md` — source-verified survey of FROST/threshold-ECDSA rotation tooling (ZF frost, Chainflip, DFINITY, Frostsnap, cb-mpc Q-check pattern).
- `schemes/02-threshold-schnorr-frost.md` — curated academic survey (ROS/Drijvers, BIP340 footguns, FROST non-robustness, Arctic nonce-reuse, Meier adaptive barrier).
- Domain knowledge of conventional Bitcoin multisig UX (Sparrow/Nunchuk/Liana) — not re-verified this run (external search disabled).

---
*Research completed: 2026-07-10*
*Ready for roadmap: yes*
