---
phase: 03-dkg-at-scale-local
verified: 2026-07-16T14:30:00Z
status: human_needed
score: 2/3 must-haves verified
behavior_unverified: 1 # confirmed regtest key-spend from persisted shares — code present + wired, requires bitcoind to exercise (unavailable in this sandbox)
overrides_applied: 0
behavior_unverified_items:
  - truth: "`cheget sign --persist` produces a CONFIRMED regtest key-spend from PERSISTED shares (ROADMAP criterion 2, the acceptance bar)."
    test: "On a host with bitcoind/bitcoin-cli on PATH, run `cargo test --test persisted_sign persisted_sign_confirmed_regtest_key_spend_small_n` (single test / --test-threads=1 for scrypt). Optionally the full-100 smoke: `cargo test --release --test persisted_sign -- --ignored`."
    expected: "Test passes: load_persisted_shares assembles t shares from disk, the SigningSession broadcasts the key-spend, and confirmation depth >= 6 on regtest."
    why_human: "The confirmed on-chain key-spend is a runtime behavior requiring a live regtest node; bitcoind is not installed in this verification sandbox, so the end-to-end spend cannot be exercised here. The read/assemble seam and write/reload seam ARE proven behaviorally (see below); only the final on-chain confirmation is unexercised."
human_verification:
  - test: "Run the persisted-share confirmed key-spend test on a host with bitcoind (see behavior_unverified_items[0])."
    expected: "Confirmed key-spend from persisted shares, depth >= 6."
    why_human: "Requires a live regtest node (external service) absent from this sandbox."
  - test: "Interactively run `cheget participant keygen --persist --base <tmp>` at a terminal (Phase 2 UAT Test 1)."
    expected: "The `InteractivePassphrase::for_new_store` confirm-twice, no-echo prompt appears exactly once; passphrase never echoed; store roots created."
    why_human: "The interactive prompt is `#[cfg(not(test))]` and cannot run under automated tests; the confirm-twice/no-echo UX needs a human at a terminal (SUMMARY D4, deferred to /gsd-verify-work 02)."
---

# Phase 3: DKG at Scale (Local) Verification Report

**Phase Goal:** Wire Phase 1's proven in-process crypto to Phase 2's persistent store at full n=100 scale — a `keygen` that runs the in-process DKG and persists the whole share set through the participant store (first command to create an encrypted store), and a `sign` that loads persisted shares to produce a confirmed regtest key-spend.
**Verified:** 2026-07-16T14:30:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
| --- | ------- | ---------- | -------------- |
| 1 | `cheget keygen --persist` runs the in-process DKG and persists the full share set — per-seat encrypted `seat-NNNN` roots (KeyPackage age/scrypt-encrypted + plaintext group package) under one prompt-once passphrase; first command creating an encrypted store. | ✓ VERIFIED | `persist_dkg_shares` (src/cli/keygen.rs:104-122) runs `run_inprocess_dkg`, resolves passphrase ONCE, loops all `n` shares into `seat-{:04}` roots via `put_share` (encrypts). `keygen::run` (147-162) prompts once via `acquire_store_passphrase(true)` (confirm-twice `for_new_store`). Behavioral test `keygen_persist` PASSED here (109s): per-seat byte-equal reload, decodable public envelope, one-entry Active manifest. n-generic loop; n=100 correctness is Phase-1's. |
| 2 | `cheget sign --persist` loads t persisted roots, drives the session, and produces a CONFIRMED regtest key-spend from PERSISTED shares (not a fresh DKG). | ⚠️ PRESENT_BEHAVIOR_UNVERIFIED | Load/assemble half VERIFIED: `load_persisted_shares` (src/cli/sign.rs:122-173) discovers `seat-*`, sorts, errors if `<t`, takes first t, `load_only_active` per root, loads group WITHOUT unlock. `sign::run` (200-211) branches on `--persist` → no fresh DKG. Read seam behaviorally proven: `load_only_active_roundtrip_and_hex_inverse` unit test PASSED here (37s). BUT the confirmed on-chain key-spend requires bitcoind (unavailable in sandbox); `persisted_sign` small-n test exists & is correctly structured (depth>=6 assertion) but could not be executed here → human/CI. |
| 3 | Wiring verified at small n in the PR gate and run once at full n=100 as a functional smoke; no re-measurement. | ✓ VERIFIED | `tests/persisted_sign.rs`: small-n (t=3,n=5) PR-gate test is NOT `#[ignore]`d; full-100 smoke is `#[ignore]`d, env-overridable (`CHEGET_PERSIST_T`/`CHEGET_PERSIST_N`). No `MEASUREMENTS.md` (D-06). `store_checkpoint_n100::persist_reload_100` retained `#[ignore]`d. Disposition structurally correct. (Regtest execution of the gate shares truth #2's bitcoind dependency.) |

**Score:** 2/3 truths verified (1 present, behavior-unverified — the confirmed on-chain key-spend)

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | ----------- | ------ | ------- |
| `src/store/passphrase.rs` | `ResolvedPassphrase` prompt-once reuse seam | ✓ VERIFIED | `pub struct ResolvedPassphrase(SecretString)` + `impl PassphraseSource` (66-83); not cfg-gated; unit test present. |
| `src/store/mod.rs` | re-exports | ✓ VERIFIED | `ResolvedPassphrase` re-exported (39); `InteractivePassphrase` re-exported under `#[cfg(not(test))]` (41). |
| `src/cli/mod.rs` | pub(crate) resolve_root + cfg-split acquire_store_passphrase | ✓ VERIFIED | `pub(crate) fn resolve_root` (92); cfg-split `acquire_store_passphrase` — `for_new_store`/`for_unlock` under not(test) (107-116), fixed secret under test (124). |
| `src/cli/keygen.rs` | persist_dkg_shares + --persist/--base wiring | ✓ VERIFIED | `pub fn persist_dkg_shares` (104); `run` delegates after prompt-once (147-162); `--out` public-envelope path intact. |
| `src/cli/sign.rs` | load_persisted_shares + --persist wiring | ✓ VERIFIED | `pub fn load_persisted_shares` (122); `run` branches on `--persist`, fresh-DKG fallback at 210; display gate at 235-244. |
| `src/store/participant.rs` | load_only_active + seat_from_hex | ✓ VERIFIED | `pub fn load_only_active` (129); private `fn seat_from_hex` (254); unit round-trip PASSED here. |
| `tests/keygen_persist.rs` | small-n writer-correctness | ✓ VERIFIED | Drives `persist_dkg_shares`; PASSED here. |
| `tests/persisted_sign.rs` | small-n PR gate + full-100 smoke | ⚠️ PRESENT (exec blocked) | Drives `load_persisted_shares` → `run_confirmed_key_spend_from_shares`; small-n not ignored, full-100 ignored. Requires bitcoind to run. |
| `tests/common/mod.rs` | run_confirmed_key_spend_from_shares | ✓ VERIFIED | Extracted (122); `run_confirmed_key_spend` delegates to it (107-111); depth>=6 assert (192). |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | --- | --- | ------ | ------- |
| `keygen::run --persist` | `persist_dkg_shares` | prompt-once → per-seat put_share loop | ✓ WIRED | keygen.rs:151-155. |
| `persist_dkg_shares` | `ParticipantStore::put_share` | per-seat `seat-NNNN` root, encrypted | ✓ WIRED | keygen.rs:114-118. |
| `sign::run --persist` | `load_persisted_shares` | prompt-once unlock → discover/select-t/assemble | ✓ WIRED | sign.rs:204-206; no fresh DKG on persist path. |
| `load_persisted_shares` | `SigningSession::new` → `session.run` | assembled key_packages + group | ✓ WIRED | sign.rs:224-244; display gate preserved. |
| `load_persisted_shares` | `load_only_active` / `load_public_envelope` | per-root active share + no-unlock group | ✓ WIRED | sign.rs:159, 168-170. |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Build all targets incl. tests | `cargo build --tests` | exit 0 (1 unused-import warning) | ✓ PASS |
| keygen persist→reload byte-equal | `cargo test --test keygen_persist` (t=2,n=3, single-thread) | 1 passed, 109s | ✓ PASS |
| load_only_active read seam round-trip | `cargo test --lib store::participant::...load_only_active_roundtrip_and_hex_inverse` | 1 passed, 37s | ✓ PASS |
| keygen CLI surface | `keygen --help` | lists `--persist`, `--base` | ✓ PASS |
| sign CLI surface | `sign --help` | lists `--persist`, `--base` | ✓ PASS |
| confirmed regtest key-spend from persisted shares | `cargo test --test persisted_sign ...small_n` | bitcoind not on PATH | ? SKIP (→ human) |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| ----------- | ---------- | ----------- | ------ | -------- |
| KEY-06 | 03-01, 03-02 | Persist/reload-through-real-commands half of the n=100 DKG (correctness + O(n²) already Phase 1). | ◑ SATISFIED (write+read seams) / confirmed-spend NEEDS HUMAN | Write half + read half verified via source and passing behavioral tests; the final confirmed-spend acceptance bar requires regtest execution (human/CI). REQUIREMENTS.md:17,123. No orphaned Phase-3 requirements. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| src/store/passphrase.rs | 20 | unused import `zeroize::Zeroizing` | ℹ️ Info | Compiler warning only; no functional impact. No TBD/FIXME/XXX debt markers anywhere in the phase files. |

### Human Verification Required

1. **Confirmed regtest key-spend from persisted shares** — On a host with bitcoind on PATH, run `cargo test --test persisted_sign persisted_sign_confirmed_regtest_key_spend_small_n` (single test, scrypt is costly), and optionally the full-100 smoke `cargo test --release --test persisted_sign -- --ignored`.
   - Expected: load_persisted_shares assembles t shares from disk, broadcasts a key-spend, confirmation depth >= 6.
   - Why human: live regtest node (external service) is absent from this sandbox; the surrounding read/write seams are already behaviorally proven here.

2. **Interactive keygen confirm-twice UX (Phase 2 UAT Test 1)** — Run `cheget participant keygen --persist --base <tmp>` at a real terminal.
   - Expected: `InteractivePassphrase::for_new_store` confirm-twice / no-echo prompt appears exactly once; passphrase never echoed; encrypted `seat-NNNN` roots created.
   - Why human: the prompt is `#[cfg(not(test))]` — unreachable from automated tests (SUMMARY D4).

### Gaps Summary

No code gaps. Every planned artifact exists, is substantive, and is wired; the build is clean; crypto core stays I/O-free; the display gate (SIGN-07), fresh-DKG fallback, and in-memory nonce discipline (SIGN-05) are intact. Two of three roadmap criteria are fully verified, including behavioral proof of both the persist/write seam (`keygen_persist` passed) and the reload/read seam (`load_only_active` round-trip passed).

The single open item is the phase's acceptance bar — the CONFIRMED on-chain key-spend from persisted shares (criterion 2). Its integration test exists and is correctly structured, but bitcoind is unavailable in this verification sandbox, so the end-to-end spend was not exercised here. Because the persisted shares reload byte-equal to the DKG output (proven) and the fresh-DKG→confirmed-spend path is Phase-1-proven, the composition is strongly implied — but goal-backward verification will not certify a runtime on-chain confirmation it could not observe. This routes to human/CI verification rather than a gap: the code is present and wired, only the external-service execution is pending.

---

_Verified: 2026-07-16T14:30:00Z_
_Verifier: Claude (gsd-verifier)_
