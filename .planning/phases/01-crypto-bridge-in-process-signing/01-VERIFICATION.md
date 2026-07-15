---
phase: 01-crypto-bridge-in-process-signing
verified: 2026-07-10T13:10:49Z
reverified: 2026-07-15T05:51:29Z
status: passed
score: 5/5 must-haves verified
behavior_unverified: 0
overrides_applied: 0
human_verification: []
resolution:
  - note: "The two items originally flagged human_needed (full-scale 51/100 confirmed key-spend and n=100 DKG correctness) are RESOLVED. After the post-completion DKG speedup (commit 9bc25e4 rayon parallelization + quick-task 260713-itg ~6.6×), commit c537bf0 un-ignored both full-scale tests; they now run by default on a plain `cargo test`. Re-verified 2026-07-15: `dkg_100_correctness` passes at t=51/n=100 in 4.32s (part1=160ms, part2=99ms, part3=4.04s; peak RSS 16.9 MiB); `inproc_sign_100` confirmed regtest key-spend passes in 8.93s. No `#[ignore]`, no `--ignored`, no nightly/on-demand run is needed. (KEY-06 maps to Phase 3 in REQUIREMENTS.md; the correctness gate was folded forward here per D-03.)"
---

# Phase 1: Crypto Bridge & In-Process Signing Verification Report

**Phase Goal:** Prove the entire cryptographic value in-process — DKG → BIP341 address → two-round tweaked signing → a confirmed regtest key-spend — with zero transport, relays, or persistence, and the four structural security controls present from the first line of signing code. Introduce the `Transport` trait and its in-memory/in-process stub so every later ceremony phase runs against it with no relay code.
**Verified:** 2026-07-10T13:10:49Z (re-verified 2026-07-15T05:51:29Z)
**Status:** passed
**Re-verification:** Yes — 2026-07-15: the two former human-needed full-scale gates now run by default and pass (see Resolution in frontmatter)

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
| --- | ------- | ---------- | -------------- |
| 1 | `cheget address` prints a BIP341 P2TR address (merkle root `None`) from a DKG-generated group key; a committed byte-level round-trip test pins the frost→rust-bitcoin bridge against a hard-coded KAT (KEY-03, KEY-04) | ✓ VERIFIED | `src/bridge/taproot.rs` is the sole bridge; `cargo test --test bridge_roundtrip` → 3/3 pass (even-Y + odd-Y-origin hard-coded address strings + address-command read-back). CLI spot-check: `watcher address --pubkey` printed `bc1p…w8l6n`. `XOnlyPublicKey::from_slice` confined to `bridge/taproot.rs` (grep confirmed). |
| 2 | An in-process ceremony (51 simulated participants, no transport) produces a `KeyPackage`+`PublicKeyPackage` whose verifying key is the internal key `P`, and every participant confirms the key back — mismatch aborts (KEY-01, KEY-02, KEY-05) | ✓ VERIFIED (at proven scale) | `src/crypto/keygen.rs::run_inprocess_dkg` (pure `dkg::part1/2/3`, even-Y normalized, group-key equality check) + `confirm_group_key`. `dkg_small` → 2/2 pass incl. `corrupted_seat_fails_confirmation_and_aborts`. Generic over (t,n); full 51/100 correctness now runs by default and passes — `dkg_100_correctness` → 1/1 pass in 4.32s (re-verified 2026-07-15). |
| 3 | A coordinator signing session over a regtest PSBT computes the per-input key-spend sighash, runs round1/round2 with `sign_with_tweak`, aggregates with `aggregate_with_tweak(…, None)` into a 64-byte BIP340 sig verifying against output key `Q`, finalizes the PSBT, and broadcasts a confirmed regtest key-spend (SIGN-01..04, STOR-04) | ✓ VERIFIED (at proven scale) | `src/session/mod.rs` full two-round orchestration; `crypto/sign.rs` tweaked-only aggregate + `verify_against_q`; `chain/sighash.rs` fixed `Default`/`Prevouts::All`. `inproc_sign` → 7/7 pass incl. `round2_run_signs_and_verifies_against_q_not_p` and `inproc_sign_confirmed_regtest_key_spend_small_n` (broadcasts + confirms on corepc-node regtest). Full 51/100 crown-jewel now runs by default and passes — `inproc_sign_100` → 1/1 pass in 8.93s, confirmed regtest key-spend (re-verified 2026-07-15). |
| 4 | Signing nonces are a type that cannot be serialized/persisted (won't compile); a restart/timeout mints fresh nonces in a new session, never reusing commitments, with 3.0 cheater-detection culprits surfaced on abort (SIGN-05, SIGN-06) | ✓ VERIFIED | `src/crypto/nonce.rs::EphemeralNonces` (move-only, no Serialize/Clone, `Zeroizing`, consumed by-value in `sign`). trybuild `compile_fail` → `nonce_is_not_serializable` passes (committed `.stderr`). `sign_adversarial` → 3/3 pass: `nonce_reuse_is_rejected…`, `abort_yields_fresh_commitments_never_the_reused_set`; culprits via `AggregateError::Culprits` + `round2_aggregate_surfaces_culprits_on_invalid_share`. |
| 5 | Before round 2, each participant recomputes the sighash locally from the PSBT and is shown human-readable outputs/amounts/fee, requiring explicit ack unless `--yes` — no blind signing (SIGN-07) | ✓ VERIFIED | `src/session/display.rs::display_and_ack` recomputes via the one canonical `key_spend_sighash`, rejects `BlindSignMismatch`, requires ack. Tests: `round2_display_gate_refuses_blind_sign`, `malicious_coordinator_sighash_is_refused_even_with_yes` (the `--yes` bypass does NOT skip the recompute check). |

**Score:** 5/5 truths verified (0 present, behavior-unverified)

All behavior-dependent truths (state transitions / cancellation invariants: KEY-05 abort, SIGN-05 nonce consume-on-sign, SIGN-06 spent-session/fresh-commitments, SIGN-07 blind-sign refusal) have passing behavioral tests. The full acceptance-scale (t=51, n=100) runs — the same generic code paths — now execute by default on `cargo test` and pass (re-verified 2026-07-15); they are no longer cost-gated or human-run.

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | ----------- | ------ | ------- |
| `Cargo.toml` + `Cargo.lock` | Exact pinned stack, committed lockfile | ✓ VERIFIED | `rust-version = "1.85"`, `frost-secp256k1-tr = "3.0.0"`, `bitcoin = "0.32.101"` (tree confirms v0.32.101, no 0.33.x), bitcoincore-rpc 0.19, esplora 0.13, zeroize 1.9, clap 4.6.1. `Cargo.lock` tracked in git. |
| `src/bridge/taproot.rs` | Canonical key→address + output-key-Q, even-Y invariant | ✓ VERIFIED | `address_from_group_key`, `internal_key_xonly`, `output_key_q`; `BridgeError::OddY` rejects odd-Y (no blind prefix strip). Pure module. |
| `src/crypto/keygen.rs` + `nonce.rs` | In-process DKG + non-serializable nonce | ✓ VERIFIED | See truths 2 & 4. |
| `src/chain/` (trait + core_rpc + esplora + sighash) | `ChainBackend` trait, 2 backends, sighash helper | ✓ VERIFIED | `chain_backend_conformance` → 2/2 pass (Core RPC + Esplora conform to same contract). |
| `src/session/` (mod + liveness + display) | Two-round session orchestration | ✓ VERIFIED | Wired end-to-end; consumes bridge/crypto/chain/transport by trait. |
| `src/transport/` (trait + envelope + inmemory) | `Transport` trait + in-memory stub | ✓ VERIFIED | `InMemoryTransport` publish/subscribe with id-based dedup; `transport_stub` → 4/4 pass. Orchestration depends only on the trait (generic `T: Transport`). |
| `tests/bridge_roundtrip.rs` + `tests/vectors/bip341_keyspend.json` | KEY-03 KAT | ✓ VERIFIED | Committed; 3/3 pass. |
| `tests/ui/nonce_no_serialize.{rs,stderr}` | trybuild SIGN-05 proof | ✓ VERIFIED | Committed; compile-fail snapshot passes. |
| clap three-persona skeleton | Real entry points | ✓ VERIFIED | `--help` lists participant/coordinator/watcher; keygen/sign/address all dispatch to real handlers. |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | --- | --- | ------ | ------- |
| `VerifyingKey` | `XOnlyPublicKey::from_slice` | Confined to `bridge/taproot.rs` | ✓ WIRED | grep: only occurrence in crate is `taproot.rs:106`. |
| bridge | `Address::p2tr(secp, internal, None, hrp)` | merkle root `None` → BIP86 Q | ✓ WIRED | `address_from_group_key` line 79. |
| DKG part3 | `into_even_y(None)` → group key = bridge input | even-Y normalization | ✓ WIRED | `keygen.rs:182-183`; CLI keygen→address round-trip produces a valid address. |
| `EphemeralNonces` | consumed by-value in `sign()` | nonce dropped after share | ✓ WIRED | `nonce.rs::sign(self,…)`; session `round2` moves nonces. |
| session | `aggregate_with_tweak(None)` → `output_key_q` verify | tweaked-only path | ✓ WIRED | `crypto/sign.rs` + `session/mod.rs:385-388`. |
| coordinator | distributes PSBT, seat recomputes sighash | SIGN-07 gate | ✓ WIRED | `display.rs` recompute; session sends PSBT-derived sighash, gate re-derives independently. |
| `Transport::publish/subscribe` | in-memory stub → signing rounds | architectural seam | ✓ WIRED | Session is generic `T: Transport`; no concrete transport leaks into orchestration. |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Full suite (re-verified 2026-07-15) | `cargo test --release` | 49 pass, 0 fail, 1 ignored — the two former Phase-1 full-scale gates now run by default and pass; the sole remaining `#[ignore]` is the Phase-2 `store_checkpoint_n100` harness stub | ✓ PASS |
| CLI persona tree | `cheget --help` | participant/coordinator/watcher listed | ✓ PASS |
| keygen→address round-trip | `coordinator keygen --out … ; watcher address --pubkey …` | valid `bc1p…` P2TR printed | ✓ PASS |
| No secret material persisted (D-09) | grep envelope for share/secret/KeyPackage | 0 matches | ✓ PASS |
| Full-scale 51/100 runs pass by default | `cargo test --release --test inproc_sign_100 --test dkg_100_correctness` | both run and pass (key-spend 8.93s, DKG proof 4.32s) | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Status | Evidence |
| ----------- | ---------- | ------ | -------- |
| KEY-01 | 01-02 | ✓ SATISFIED | `run_inprocess_dkg` → KeyPackage+PublicKeyPackage, verifying key = internal P; `dkg_small`. |
| KEY-02 | 01-02 | ✓ SATISFIED | Fully in-process, all seats simulated, no transport. |
| KEY-03 | 01-01 | ✓ SATISFIED | `bridge_roundtrip` KAT (even-Y + odd-Y), hard-coded address strings. |
| KEY-04 | 01-01 | ✓ SATISFIED | `cheget … address` prints P2TR (CLI spot-check + test). |
| KEY-05 | 01-02 | ✓ SATISFIED | `confirm_group_key` + `corrupted_seat_fails_confirmation_and_aborts`. |
| SIGN-01 | 01-04 | ✓ SATISFIED | Per-input key-spend sighash from PSBT; `round1_builds_signing_package_from_psbt_sighash`. |
| SIGN-02 | 01-04, 01-05 | ✓ SATISFIED | Liveness poll + `t`-subset select over Transport; `round1_over_provisioned_poll_selects_exactly_t`. |
| SIGN-03 | 01-04 | ✓ SATISFIED | `sign_with_tweak` + `aggregate_with_tweak(None)` → 64-byte BIP340. |
| SIGN-04 | 01-04 | ✓ SATISFIED | `verify_against_q`, PSBT finalize, confirmed regtest key-spend — verified at small n and at full 51/100 (`inproc_sign_100` passes by default, 8.93s). |
| SIGN-05 | 01-02 | ✓ SATISFIED | Non-serializable `EphemeralNonces` + trybuild compile-fail. |
| SIGN-06 | 01-04 | ✓ SATISFIED | Culprits surfaced; spent-session/fresh-nonce; `sign_adversarial`. |
| SIGN-07 | 01-04 | ✓ SATISFIED | Recompute + display + ack; blind-sign refused even with `--yes`. |
| STOR-04 | 01-03 | ✓ SATISFIED | `ChainBackend` trait + Core RPC + Esplora; `chain_backend_conformance`. |

All 13 phase requirement IDs are declared in plan frontmatter and satisfied. No orphaned requirements. (Plan 01-02 additionally declares KEY-06, which REQUIREMENTS.md maps to Phase 3; it was folded forward here per D-03. The full 51/100 correctness run now passes by default — `dkg_100_correctness`, 4.32s — so it is fully verified here, not deferred.)

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| `src/cli/mod.rs` | 5 | Stale doc comment: "keygen and sign are explicit `unimplemented` stubs" | ℹ️ Info | Comment only — the handlers are fully wired (`keygen::run`/`sign::run` dispatch real logic; no `unimplemented!`/`todo!` macros anywhere in `src/`). Cosmetic; recommend correcting the comment. |

No debt-marker blockers (`TODO`/`FIXME`/`XXX`/`TBD`) in phase source. No empty/placeholder implementations. No hollow data flow.

### Human Verification Required

None — both items previously listed here are RESOLVED (re-verified 2026-07-15). After the post-completion DKG speedup they were un-ignored (commit c537bf0) and now run by default:

1. ~~Full-scale 51-of-100 confirmed regtest key-spend~~ → **resolved**: `cargo test --release --test inproc_sign_100` runs by default and passes (8.93s, confirmed regtest key-spend at t=51/n=100). No `--ignored`.
2. ~~Full-scale n=100 DKG correctness + O(n²) instrumentation~~ → **resolved**: `cargo test --release --test dkg_100_correctness` runs by default and passes (4.32s; 100 KeyPackages → one group key; part1=160ms/part2=99ms/part3=4.04s, peak RSS 16.9 MiB). No `--ignored`.

### Gaps Summary

No gaps. All five ROADMAP success criteria and all thirteen phase requirement IDs are verified in the codebase, and the four structural security controls (non-serializable nonce type, byte-level bridge KAT, verify-against-Q, display-before-sign) are each backed by a passing test. The two items originally deferred as `#[ignore]`d full-scale (t=51, n=100) gates are now RESOLVED: after the post-completion DKG speedup they were un-ignored (commit c537bf0) and run by default. Re-verified 2026-07-15 — the full-scale crown-jewel confirmed key-spend (`inproc_sign_100`, 8.93s) and the n=100 DKG correctness/instrumentation run (`dkg_100_correctness`, 4.32s) both pass on a plain `cargo test`. Status is therefore `passed`.

---

_Verified: 2026-07-10T13:10:49Z; re-verified 2026-07-15T05:51:29Z (full-scale gates run by default and pass; status human_needed → passed)_
_Verifier: Claude (gsd-verifier)_
