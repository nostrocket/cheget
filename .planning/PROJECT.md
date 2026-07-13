# tsig — 51-of-100 FROST Taproot Signing CLI

## What This Is

`tsig` is a single-binary Rust command-line tool that lets a fixed-threshold, large-membership group (51-of-100) jointly control one Bitcoin Taproot address using FROST threshold Schnorr signatures (RFC 9591, secp256k1, BIP340/341 key-path spend). On-chain, its spends are indistinguishable from single-sig. Three personae — participant, coordinator, watcher — are selected by subcommand. It supports membership rotation with zero on-chain footprint and true revocation via an on-chain sweep to a pre-generated standby key.

## Core Value

A group of 100 can jointly control a single Bitcoin address such that (a) any 51 can spend, (b) no individual ever holds the key, (c) membership can rotate without touching the chain or changing the address, and (d) past compromise is truly revocable by sweeping to a standby key. If everything else fails, the frost↔rust-bitcoin bridge must produce a valid BIP340 key-spend signature over the correct sighash.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] DKG keygen (the only keygen path) producing a FROST `PublicKeyPackage` whose verifying key is the Taproot internal key `P` — run in-process on a single host for fast local testing, then distributed over Nostr at n=100
- [ ] Deterministic frost→rust-bitcoin key bridge (33-byte SEC1 → x-only → `XOnlyPublicKey` → BIP341 P2TR address, merkle root `None`), pinned by a byte-level round-trip test
- [ ] Two-round FROST signing with taproot tweak (`sign_with_tweak` / `aggregate_with_tweak(…, None)`) producing a standard 64-byte BIP340 signature over the key-spend sighash
- [ ] Nostr transport: signed events per message class, NIP-44 v2 for confidential payloads, multi-relay publish/dedup, roster pinning, offline `--in/--out` file fallback
- [ ] DKG at n=100 over self-hosted relays: resumable, idempotent per `(ceremony_id, round, seat)`, relay rate-limit tuning, paced round-2 batches
- [ ] Membership rotation: refresh (removals + proactivize), enroll (repair/RTS + immediate refresh), epoch bookkeeping, mandatory client-side same-key postcondition check
- [ ] Key lifecycle: standby key pre-generation, sweep flow (all UTXOs → standby, standby→active rollover on confirmation), RETIRED state
- [ ] Policy engine + watcher: `value_cap`, `churn_budget`, `max_epochs`, `standby_max_age`; cron/CI-friendly `watch` with nonzero exit + JSON report
- [ ] Nonce discipline: signing nonces live in memory only, never persisted; any session restart generates fresh nonces
- [ ] Display-before-sign: participants recompute the sighash from the PSBT and ack human-readable outputs/amounts/fee (no blind signing)
- [ ] At-rest share encryption (age/scrypt) + in-memory zeroize; checkpointed resumable ceremony state (except nonces)
- [ ] Coordinator SQLite state: roster, ceremony transcripts, session logs, policy, churn ledger
- [ ] Bitcoin Core JSON-RPC integration (watch-only `tr()` descriptor import) with Esplora as a light alternative behind the same trait
- [ ] Reproducible participant binary builds; pinned deps with `cargo audit` / `cargo deny` in CI
- [ ] Adversarial test suite: malicious relay (DoS), mixed-epoch shares, replayed envelopes, nonce-reuse attempts

### Out of Scope

- Changing the threshold `t` — requires new DKG → new key → sweep; supported only *as* a sweep, never in place
- Verifiable erasure / remote attestation of share deletion — assumed impossible on member hardware; no security claim rests on any local deletion
- Script-path spends, multi-address wallet management, coin selection beyond consolidation — not needed for the single key-path address model
- ECDSA of any kind — wrong signature type for Taproot key-spend and infeasible at n=100
- `secp256kfun`/`schnorr_fun`, tss-lib, luxfi/threshold — primitives-level or wrong-scheme or stub; ZF `frost-secp256k1-tr` is the audited packaged path

## Context

- **Cryptography is entirely `frost-secp256k1-tr` ≥3.0** (NCC-reviewed, MIT/Apache): DKG, dealer keygen, refresh-DKG, repair/enroll primitives, `sign_with_tweak`/`aggregate_with_tweak`. The crypto layer of this project *is* that crate's public API.
- **Companion research:** `implementations-resharing.md`; full design in `SPEC-frost-cli.md` (draft v0.1, 2026-07-09).
- **Security model:** untrusted individual participants (assumption: <51 of any single epoch's holders collude/leak); untrusted coordinator (can stall/censor, cannot forge/steal/alter address); relays trusted for liveness only, redundantly. Adversary is mobile (compromises devices over time; refresh resets progress via epoch-mixing) plus retained-share insiders (mitigated only by sweep).
- **Residual risk (normative):** 51 shares from *one* epoch reconstruct the key forever. Rotation defends against external gradual compromise only; the sweep is the real revocation. Value bounds the prize, churn bounds the coalition pool — both trigger sweeps, either alone insufficient.
- **Epoch discipline:** every completed refresh/enroll increments `epoch`; share files tagged `(key_id, epoch, identifier)`; signing sessions bind `(key_id, epoch)` and reject mixed-epoch shares early with a clear error.

## Constraints

- **Tech stack**: Rust — `frost-secp256k1-tr` ≥3.0 (+`frost-core`, `serialization`), `bitcoin` (rust-bitcoin), `bitcoincore-rpc`/`esplora-client`, `nostr-sdk` (NIP-44/42/59), `age`+`zeroize`, `clap` 4, `serde`/`serde_json`/`toml`, `rusqlite`. — the audited/canonical path for each concern.
- **Security**: Nonce discipline is the highest-severity implementation rule — nonces never persisted. Nostr identity keys are transport-only, independently generated, never derived from or reused as FROST material (both live on secp256k1). Same-key check after every refresh is mandatory and client-side.
- **Fixed parameters**: `t = 51`, `n = 100`; threshold never changes.
- **Transport / ops**: Operators MUST run ≥3 dedicated relays (strfry / nostr-rs-relay), NIP-42 AUTH to roster npubs; ceremonies generate on the order of ~10⁴ events (~10 MB) — order-of-magnitude estimate, scaling as O(n²) at n=100 — never point at a public relay. Offline file mode is a first-class fallback.
- **Verifiability**: reproducible builds required — 100 people must be able to verify what they run; library versions pinned; `cargo audit`/`cargo deny` in CI.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Roadmap covers all of M1–M5 | User wants the full spec end-to-end: happy path → transport → rotation → lifecycle → hardening | — Pending |
| DKG is the only keygen path; dealer mode dropped | User wants trustless key generation with no single-party trust event; local in-process DKG on one host covers fast testing | — Pending |
| Crypto bridge proven early via in-process DKG (simulated participants, no transport) before full n=100 DKG transport | De-risk the frost↔rust-bitcoin integration bug class without blocking on O(n²) relay work; M1 keygen is local DKG, not dealer | — Pending |
| Framed as security-reviewable OSS | 100 people must verify what they run — reproducible builds, pinned/audited deps, external review of nonce discipline + bridge are first-class | — Pending |
| `frost-secp256k1-tr` ≥3.0 is the entire crypto layer | Audited (NCC), packaged, provides every primitive needed incl. BIP341 tweak; alternatives are primitives-level or wrong-scheme | — Pending |
| Nostr as transport (not a bespoke relay) | Event model = the message-board/envelope-signature/delivery semantics ceremonies need; multi-relay = redundant liveness by construction | — Pending |
| Sweep is the true revocation, not share erasure | Verifiable deletion assumed impossible on member hardware; no security claim rests on it | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-07-10 after initialization*
