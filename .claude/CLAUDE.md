<!-- GSD:project-start source:PROJECT.md -->

## Project

**cheget — 51-of-100 FROST Taproot Signing CLI**

`cheget` is a single-binary Rust command-line tool that lets a fixed-threshold, large-membership group (51-of-100) jointly control one Bitcoin Taproot address using FROST threshold Schnorr signatures (RFC 9591, secp256k1, BIP340/341 key-path spend). On-chain, its spends are indistinguishable from single-sig. Three personae — participant, coordinator, watcher — are selected by subcommand. It supports membership rotation with zero on-chain footprint and true revocation via an on-chain sweep to a pre-generated standby key.

**Core Value:** A group of 100 can jointly control a single Bitcoin address such that (a) any 51 can spend, (b) no individual ever holds the key, (c) membership can rotate without touching the chain or changing the address, and (d) past compromise is truly revocable by sweeping to a standby key. If everything else fails, the frost↔rust-bitcoin bridge must produce a valid BIP340 key-spend signature over the correct sighash.

### Constraints

- **Tech stack**: Rust — `frost-secp256k1-tr` ≥3.0 (+`frost-core`, `serialization`), `bitcoin` (rust-bitcoin), `bitcoincore-rpc`/`esplora-client`, `nostr-sdk` (NIP-44/42/59), `age`+`zeroize`, `clap` 4, `serde`/`serde_json`/`toml`, `rusqlite`. — the audited/canonical path for each concern.
- **Security**: Nonce discipline is the highest-severity implementation rule — nonces never persisted. Nostr identity keys are transport-only, independently generated, never derived from or reused as FROST material (both live on secp256k1). Same-key check after every refresh is mandatory and client-side.
- **Fixed parameters**: `t = 51`, `n = 100`; threshold never changes.
- **Transport / ops**: Operators MUST run ≥3 dedicated relays (strfry / nostr-rs-relay), NIP-42 AUTH to roster npubs; ceremonies generate on the order of ~10⁴ events (~10 MB) — order-of-magnitude estimate, scaling as O(n²) at n=100 — never point at a public relay. Offline file mode is a first-class fallback.
- **Verifiability**: reproducible builds required — 100 people must be able to verify what they run; library versions pinned; `cargo audit`/`cargo deny` in CI.

<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->

## Technology Stack

## Verdict

## Recommended Stack

### Core Technologies

| Technology | Version (pin) | Purpose | Why Recommended |
|------------|---------------|---------|-----------------|
| `frost-secp256k1-tr` | **3.0.0** (2026-04-23) | Entire crypto layer: DKG, dealer keygen, refresh-DKG, repair/enroll, BIP-341 tweak sign/aggregate | Audited (NCC), ZcashFoundation-maintained, MIT/Apache. The only packaged Rust FROST implementation exposing all required primitives incl. the Taproot tweak. `>= 3.0` from SPEC satisfied by the current stable 3.0.0. |
| `frost-core` | **3.0.0** (2026-04-23) | Trait/type substrate re-exported by `frost-secp256k1-tr` | Must match the `-tr` crate's major (it depends on `frost-core ^3.0`). Pin to the same 3.0.x. You rarely depend on it directly — the ciphersuite crate re-exports what you need. |
| `bitcoin` (rust-bitcoin) | **0.32.101** (stable, 2026-06-24) | Address, PSBT, tx, BIP341 sighash, x-only key types | Canonical. 0.33.0-beta exists but is beta *and* unsupported by the RPC/esplora clients — stay on 0.32.x. |
| `nostr-sdk` (rust-nostr) | **0.44.1** (stable) | Event build/sign/publish/subscribe, multi-relay pool + dedup, NIP-44 v2, NIP-42 AUTH, NIP-59 gift-wrap | Replaces a bespoke relay/envelope stack. 0.45.0 is alpha only — use the 0.44.1 stable line. |

### Supporting Libraries

| Library | Version (pin) | Purpose | When to Use |
|---------|---------------|---------|-------------|
| `bitcoincore-rpc` | **0.19.0** (2024-05-15) | Bitcoin Core JSON-RPC: watch-only `tr()` descriptor import, UTXO listing, fee estimation, broadcast | Default chain backend (`watch`/`sweep`). Note staleness below. Uses `bitcoin 0.32`. |
| `esplora-client` | **0.13.0** (2026-07-02) | Async/blocking Esplora HTTP client behind the same chain trait | Light alternative to a full node. Depends on `bitcoin ^0.32` — aligns cleanly. |
| `age` | **0.11.3** (2026-04-22) | At-rest share encryption, scrypt passphrase recipient (`age::scrypt::Recipient` / `Identity`) | Encrypt `KeyPackage`/`PublicKeyPackage` and checkpointed ceremony state. Still labelled BETA upstream but is the de-facto standard; API stable across 0.11.x. |
| `zeroize` | **1.9.0** (2026-06-12) | Memory hygiene for secret material | Wrap decrypted shares / nonces in `Zeroizing<_>`. Note FROST 3.0 already makes `SigningKey: ZeroizeOnDrop`. |
| `clap` | **4.6.1** (2026-04-15) | CLI argument parsing (derive API) | Three-persona subcommand tree. |
| `rusqlite` | **0.40.1** (2026-06-06) | Coordinator SQLite state (roster, transcripts, churn ledger, policy) | Coordinator persona only. Prefer the `bundled` feature for reproducible builds. |
| `serde` / `serde_json` | **1.x** (latest 1.x) | (De)serialize event payloads, config, FROST types via their serde impls | Everywhere. Pin `1` with a `Cargo.lock` for reproducibility. |
| `toml` | **1.1.2** (2026-04-01) | Human-editable config file (relays, policy) | Config load/save. |

### Development / CI Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo audit` | RUSTSEC advisory scan | Required by SPEC §11.8; run in CI. |
| `cargo deny` | License + duplicate-dep + advisory gate | Also flags the expected duplicate `secp256k1` (see compat notes) so you can allow-list it deliberately. |
| `Cargo.lock` committed | Pinned, reproducible builds | Mandatory — "100 people must verify what they run" (SPEC §11.8). MSRV floor is **Rust 1.85** (driven by `toml` 1.1.x; `frost-core` needs only 1.81). |

## FROST API surface — confirmed present at `frost-secp256k1-tr` 3.0.0

| Claimed API | Status | Exact path / signature at 3.0.0 |
|-------------|--------|---------------------------------|
| DKG rounds | ✅ present | `keys::dkg::part1`, `keys::dkg::part2`, `keys::dkg::part3` |
| Dealer keygen | ✅ present | `keys::generate_with_dealer(max_signers, min_signers, IdentifierList, rng)` |
| Refresh-DKG | ✅ present | `keys::refresh::refresh_dkg_part1`, `refresh_dkg_part2`, `refresh_dkg_shares` (also `compute_refreshing_shares`, `refresh_share`) |
| Repair / enroll (RTS) | ✅ present | `keys::repairable::repair_share_part1`, `repair_share_part2`, `repair_share_part3` |
| Tweaked signing | ✅ present | `round2::sign_with_tweak(...)` ("same as `sign()` but with BIP-341 Taproot tweak") |
| Tweaked aggregation | ✅ present | `aggregate_with_tweak(signing_package, signature_shares, public_key_package, merkle_root: Option<&[u8]>)` — **`merkle_root: None` is valid** and gives the BIP86-style key-only output |
| BIP-341 key traits | ✅ present | `keys::Tweak`, `keys::EvenY` traits |
| Serialization | ✅ present | `serialization` feature exists and is **on by default** (postcard-backed); `serde` is a separate opt-in feature |

### FROST 3.0 breaking changes vs 2.x (what the roadmap must account for)

- **Renames (already reflected in the SPEC):** `refresh_dkg_part_1` → `refresh_dkg_part1`; `repair_share_step_1/2/3` → `repair_share_part1/2/3`. The SPEC uses the new names — no action.
- **Cheater detection is now default behavior**, not a feature flag. `aggregate()` / `aggregate_with_tweak()` identify malicious shares automatically and return culprits; opt out only via `aggregate_custom(..., CheaterDetection::Disabled)`. Beneficial at t=51 — keep it on.
- **`Error::culprit()` → `culprits()`** returning a `Vec<Identifier>` (and `InvalidSignatureShare::culprit` → `culprits`). Error-handling code must expect multiple culprits.
- **`PublicKeyPackage::new()` now requires a `min_signers` argument.** Relevant if you reconstruct packages manually.
- **`SigningKey` is no longer `Copy` and now `ZeroizeOnDrop`.** Aligns with the SPEC's hygiene goals; adjust any code that assumed `Copy`.
- **All crates are now `no_std` (alloc);** the `std` and `nightly` features were removed. A std binary is unaffected — you just no longer toggle a `std` feature.

## Bitcoin API surface — confirmed at `bitcoin` 0.32.101

| Claimed API | Status | Exact signature at 0.32.101 |
|-------------|--------|------------------------------|
| Key-spend sighash | ✅ present | `SighashCache::taproot_key_spend_signature_hash<T: Borrow<TxOut>>(&mut self, input_index: usize, prevouts: &Prevouts<T>, sighash_type: TapSighashType) -> Result<TapSighash, TaprootError>` |
| P2TR address | ✅ present, **minor param note** | `Address::p2tr<C: Verification>(secp: &Secp256k1<C>, internal_key: UntweakedPublicKey, merkle_root: Option<TapNodeHash>, hrp: impl Into<KnownHrp>) -> Address` |
| X-only internal key | ✅ present | `UntweakedPublicKey` is a type alias for `XOnlyPublicKey`; build from 32 bytes with `XOnlyPublicKey::from_slice(&[u8; 32])` |

## Nostr API surface — confirmed at `nostr-sdk` 0.44.1

| Claimed capability | Status | Notes |
|--------------------|--------|-------|
| NIP-44 v2 encryption | ✅ | `nip44` feature (also under `all-nips`); versioned encrypted payloads. |
| NIP-59 gift-wrap | ✅ | `nip59` feature. Optional per SPEC (metadata privacy only). |
| NIP-42 AUTH | ✅ | Built into the client/relay message layer (`ClientMessage::auth` / relay `Auth`), not a separate cargo feature — that is why it is absent from the feature table. The SDK can authenticate to roster-restricted relays. |
| Multi-relay pool + dedup | ✅ | Core `Client`/relay-pool behavior; publishes to all relays, merges/dedups by event id. |
| Custom event kinds | ✅ | `Kind::Custom(u16)` / `Kind::from(<n>)` — use the addressable/regular custom ranges per SPEC §7. |

## Version Compatibility (read before writing `Cargo.toml`)

| Concern | Resolution | Notes |
|---------|-----------|-------|
| FROST curve vs rust-bitcoin curve | **No alignment needed — they are different crates** | FROST → `k256` (pure Rust). rust-bitcoin → `secp256k1` (C libsecp). The frost→bitcoin key bridge is **byte-level only**: FROST `VerifyingKey` → 33-byte SEC1 → strip parity → 32-byte x-only → `XOnlyPublicKey::from_slice`. Pin this with the round-trip test (SPEC §9, PROJECT requirement). You cannot pass a `k256` point into rust-bitcoin. |
| `bitcoin` major/minor | **Pin `bitcoin = "0.32.101"`; do not use 0.33.0-beta** | `bitcoincore-rpc 0.19` and `esplora-client 0.13` both require `bitcoin ^0.32`. 0.33 is beta and unsupported by both clients — adopting it breaks the chain layer. |
| `frost-core` vs `frost-secp256k1-tr` | Pin both to **3.0.x** | The `-tr` crate depends on `frost-core ^3.0` and `frost-rerandomized ^3.0`; a `frost-core` mismatch across majors will not compile. Prefer depending only on `frost-secp256k1-tr` and using its re-exports. |
| Duplicate `secp256k1` (rust-bitcoin ⟷ nostr-sdk) | **Benign; allow-list in `cargo deny`** | Both use the C-bindings `secp256k1` crate; if their required versions differ you get two copies in the graph. Harmless because Nostr keys and Bitcoin keys never cross the type boundary. Expect a `cargo deny` "duplicate" note and allow it deliberately. |
| `bitcoincore-rpc` staleness | Acceptable now, watch upstream | 0.19.0 is from May 2024 and has not tracked newer rust-bitcoin releases; it still targets `bitcoin 0.32`, which is exactly our pin. Abstract chain access behind a trait (SPEC §9) so a future client swap (or a maintained fork) is cheap. |
| MSRV | **Rust 1.85** | Highest floor comes from `toml` 1.1.x (`rusqlite`/`bitcoin` are lower; `frost-core` needs 1.81). Set `rust-version = "1.85"` and enforce in CI for reproducibility. |

## What NOT to Use (rejections confirmed still valid)

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `secp256kfun` / `schnorr_fun` | Excellent but primitives-level; you would reimplement DKG, refresh, repair, and the audited packaging yourself | `frost-secp256k1-tr` 3.0.0 (audited, packaged) |
| `tss-lib` / any ECDSA threshold stack | Wrong signature type (ECDSA, not Schnorr/BIP340) — cannot produce a Taproot key-path spend; also infeasible at n=100 | `frost-secp256k1-tr` (Schnorr, BIP340/341) |
| `luxfi/threshold` | Stub-quality per companion research (`implementations-resharing.md` §3.6); unaudited | `frost-secp256k1-tr` |
| `bitcoin 0.33.0-beta` | Pre-release; unsupported by `bitcoincore-rpc` / `esplora-client` | `bitcoin 0.32.101` (stable) |
| `nostr-sdk 0.45.0-alpha.*` | Alpha; unstable API | `nostr-sdk 0.44.1` (stable) |
| Persisting signing nonces / a nonce-store crate | Persisted nonces are a key-extraction bug class (SPEC §6.5) | Keep nonces in memory only; regenerate on any restart. No crate should touch them. |

## Installation (Cargo.toml sketch)

# regtest integration harness for the frost↔rust-bitcoin bridge round-trip test

## Sources

- crates.io API (`/api/v1/crates/<name>` + `/<version>/dependencies`) — current versions, release dates, dependency graphs for all crates above (fetched 2026-07-10) — HIGH
- docs.rs `frost_secp256k1_tr` 3.0.0 — module tree (`keys::dkg`, `keys::refresh`, `keys::repairable`), `round2::sign_with_tweak`, `aggregate_with_tweak` signature, `keys::Tweak`/`EvenY`, features — HIGH
- ZcashFoundation/frost `frost-core/CHANGELOG.md` (main) — 3.0.0 / 3.0.0-rc.0 breaking changes and renames — HIGH
- docs.rs `bitcoin` 0.32.101 — `SighashCache::taproot_key_spend_signature_hash`, `Address::p2tr` signatures — HIGH
- docs.rs `age` 0.11.3 `scrypt` module — passphrase Recipient/Identity — HIGH
- rust-nostr book (rust-nostr.org/sdk/messages) + docs.rs `nostr-sdk` 0.44.1 — NIP-44/42/59 support, features — HIGH (NIP-42 confirmation MEDIUM-HIGH: documented but not feature-gated)

<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->

## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->

## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase.
<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->

## Project Skills

No project skills found. Add skills to any of: `.claude/skills/`, `.agents/skills/`, `.cursor/skills/`, `.github/skills/`, or `.codex/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->

## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:

- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->

## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
