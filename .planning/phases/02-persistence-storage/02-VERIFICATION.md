---
phase: 02-persistence-storage
verified: 2026-07-14T14:55:15Z
status: human_needed
score: 16/16 must-haves verified
behavior_unverified: 1
overrides_applied: 0
re_verification:
  previous_status: human_needed
  previous_score: 15/15
  gaps_closed:
    - "WR-01: the interactive rpassword passphrase reads no longer land the plaintext in an un-zeroized String — all three reads are now wrapped in Zeroizing<String> and the owned copy is consumed directly by the zeroize-on-drop SecretString (src/store/passphrase.rs:84,93,94,85,98)."
  gaps_remaining: []
  regressions: []
  resolved_human_items:
    - "Prior human item #1 (interactive no-echo prompt): human-verified PASS in 02-UAT.md test 1. Re-flagged below because the gap-closure (c2c3a83) modified those exact prompt lines — a light re-confirmation on the post-change code."
    - "Prior human item #3 (WR-03 pre-existing dir perms): resolved by maintainer decision ACCEPT in 02-UAT.md test 3 — files stay 0600 regardless; no code change. Not re-flagged."
behavior_unverified_items:
  - truth: "New-store creation still prompts twice with no echo, rejects a mismatch, and prints the unrecoverability warning; the unlock path still prompts once — all preserved after the Zeroizing wrapping (02-05 T2)."
    test: "Run cheget triggering new-store passphrase creation (InteractivePassphrase::for_new_store) at a real terminal, then the unlock path (for_unlock)."
    expected: "No echo while typing; new-store prompts twice with the 'a lost passphrase makes them unrecoverable — there is no reset' warning printed first; a mismatch is rejected with 'passphrases did not match'; unlock prompts once. Behavior byte-for-byte identical to the pre-fix UX (only the transient buffer is now zeroized on drop)."
    why_human: "InteractivePassphrase is #[cfg(not(test))] — it cannot be linked or driven in any test build, and no-echo is a runtime TTY property grep/headless tests cannot observe. The gap-closure commit c2c3a83 modified these exact lines, so the prior UAT PASS (which covered pre-fix code) warrants a one-time re-confirmation."
human_verification:
  - test: "Re-confirm the interactive no-echo confirm-twice passphrase prompt on the post-fix code (src/store/passphrase.rs, commit c2c3a83)."
    expected: "No echo; new-store prompts twice + mismatch rejected + unrecoverability warning first; unlock prompts once. UX unchanged from the prior UAT PASS — the only change is the transient plaintext buffer is now wiped on drop."
    why_human: "#[cfg(not(test))] TTY path; no-echo and confirm-twice are runtime terminal properties. The exact lines were modified by the WR-01 gap-closure, so a light re-confirmation is warranted."
---

# Phase 2: Persistence & Storage — Verification Report (Re-verification)

**Phase Goal:** Lay down the durable-state foundation the ceremony and transport layers build on — age/scrypt participant storage with nonce-exclusion and epoch tagging, encrypted between-round ceremony checkpointing, and the coordinator SQLite store for roster/transcripts/logs/policy/churn — so no durable state is retrofitted later.
**Verified:** 2026-07-14T14:55:15Z
**Status:** human_needed
**Re-verification:** Yes — after gap closure (02-05 closed UAT finding WR-01)

## Re-verification Summary

The single actionable gap from the prior cycle — **WR-01** (the interactive `rpassword` reads landing the passphrase in an un-zeroized `String`, contradicting the module's own doc invariant) — is now **genuinely closed** in the codebase, not merely claimed. I read `src/store/passphrase.rs` directly and confirmed the fix independently of the SUMMARY:

- **All three** `rpassword::prompt_password(...)` reads are wrapped in `Zeroizing::new(...)` (grep count == 3, lines 84, 93, 94).
- `use zeroize::Zeroizing;` is imported (line 20).
- The owned copy handed to `SecretString` is produced via `.as_str().to_owned()` and **moved directly into** `SecretString::from(...)` (lines 85, 98). `SecretString` is zeroize-on-drop, so the moved heap buffer is wiped on drop and the `Zeroizing` wrapper wipes the transient rpassword buffer — **no un-zeroized binding survives, and no new residual copy is introduced.**
- The confirm-twice mismatch check is preserved as `*first != *second` (line 95); the warning text (lines 89–92) and mismatch error (line 96) are byte-for-byte unchanged.
- Fix committed cleanly as `c2c3a83`; working tree clean; no debt markers introduced.

**All 15 prior must-have truths regression-confirmed** by re-running the suite (see Behavioral Spot-Checks). No regressions. The two prior maintainer-decision warnings are resolved: WR-01 fixed (this cycle), WR-03 accepted (02-UAT.md test 3, no code change).

The status is `human_needed` (not `passed`) solely because the `#[cfg(not(test))]` interactive no-echo/confirm-twice prompt is a runtime TTY property that cannot be exercised headlessly by design, and the gap-closure modified those exact lines — warranting a one-time re-confirmation of the (behavior-preserving) UX change. This does not block goal achievement.

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | age/scrypt encrypt→decrypt round-trips; wrong passphrase → Err; decrypt returns Zeroizing | ✓ VERIFIED | `crypto::tests::age_roundtrip` + `wrong_passphrase_fails` pass (re-ran full lib suite) |
| 2 | Atomic write leaves no partial file on crash; 0700 dir / 0600 file on Unix | ✓ VERIFIED | `store::atomic::tests::atomic_no_partial` + `interrupted_write_does_not_truncate_existing` + `perms` pass |
| 3 | PassphraseSource seam: interactive (prod) + in-code (test) behind one trait; no passphrase env var or CLI flag ships | ✓ VERIFIED | `src/store/passphrase.rs` trait + `InCodePassphrase` + `#[cfg(not(test))] InteractivePassphrase`; no env/flag passphrase source (grep) |
| 4 | Full pinned dependency set builds on MSRV 1.85 | ✓ VERIFIED | `cargo build` clean; MSRV pins recorded (regression — unchanged by 02-05) |
| 5 | KeyPackage persists age-encrypted, reloads byte-equal, tagged (key_id, epoch, seat) | ✓ VERIFIED | `store::participant::tests::share_roundtrip` + `store_headless` pass |
| 6 | Plaintext PublicKeyPackage envelope readable with NO unlock, reusing PubkeyEnvelope | ✓ VERIFIED | `share_roundtrip` reads under wrong passphrase; reuses `cli::address::PubkeyEnvelope` |
| 7 | Transport identity persists/reloads; npub stable, starts npub1, Bech32 (not Bech32m) | ✓ VERIFIED | `store::identity::tests::identity_roundtrip_npub` passes |
| 8 | No FROST↔identity conversion (reuse non-expressible) | ✓ VERIFIED | `compile_fail` trybuild suite passes (4/4) |
| 9 | dkg round1/round2 SecretPackage checkpoint encrypted, reload byte-faithful via real part1/part2 | ✓ VERIFIED | `store::checkpoint::tests::dkg_roundtrip` passes |
| 10 | wipe-on-success removes files; keep-on-abort retains for resume | ✓ VERIFIED | `store::checkpoint::tests::wipe_vs_keep` passes |
| 11 | Checkpoint store exposes NO generic persist and NO nonce method — nonce non-expressible | ✓ VERIFIED | `compile_fail` guards `checkpoint_rejects_nonce_material` + `checkpoint_has_no_generic_persist` pass |
| 12 | Coordinator SQLite opens, WAL + foreign_keys, migrates user_version 0→1 | ✓ VERIFIED | `coordinator::tests::open_migrate` passes |
| 13 | roster/transcripts/session_logs/policy(defaults)/churn roundtrip; real npubs; no secret column | ✓ VERIFIED | `coordinator::tests::roster_roundtrip` + `tables` pass |
| 14 | `cheget participant share-status` lists shares with NO unlock | ✓ VERIFIED | Regression — routes to `read_manifest`, no PassphraseSource constructed (unchanged by 02-05) |
| 15 | Headless integration test drives persist→reload with in-code PassphraseSource, no TTY | ✓ VERIFIED | `cargo test --test store_headless` → `headless_persist_reload_no_prompt` passes |
| 16 | **(WR-01 closure)** The transient rpassword plaintext buffer is zeroized on drop — the passphrase never persists in an un-zeroized String | ✓ VERIFIED | `src/store/passphrase.rs` lines 20, 84, 93–94, 85, 98: all 3 reads `Zeroizing`-wrapped (grep==3), owned copy moved into zeroize-on-drop `SecretString`; `cargo build` clean; `store::passphrase` unit test passes; commit c2c3a83 |
| — | New-store confirm-twice / no-echo / warning UX preserved after the Zeroizing change (02-05 T2) | ⚠️ PRESENT_BEHAVIOR_UNVERIFIED | Control flow byte-for-byte preserved (warning 89–92, mismatch `*first != *second` 95–96, confirm flag) — but no-echo is a `#[cfg(not(test))]` TTY property; exact lines changed since prior UAT. See Human Verification. |

**Score:** 16/16 must-have truths verified (1 present, behavior-unverified — the interactive TTY UX)

### Deferred Items

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | n=100 at-scale persist/reload of the full share set through the Phase 2 stores | Phase 3 | `tests/store_checkpoint_n100.rs::persist_reload_100` is `#[ignore]`d and built here; Phase 3 SC #3 and KEY-06 (marked Complete in REQUIREMENTS.md). |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/store/passphrase.rs` | PassphraseSource seam + zeroized transient reads | ✓ VERIFIED | Trait + InCode + `#[cfg(not(test))]` Interactive; all 3 rpassword reads Zeroizing-wrapped (WR-01 closed) |
| `src/store/mod.rs` | StoreError + StoreRoot substrate | ✓ VERIFIED | Regression — `store::tests` pass |
| `src/store/atomic.rs` | Crash-safe write + perms | ✓ VERIFIED | Regression — 3 tests pass |
| `src/store/crypto.rs` | age/scrypt encrypt/decrypt | ✓ VERIFIED | Regression — Zeroizing boundary, tests pass |
| `src/store/manifest.rs` | Versioned share index | ✓ VERIFIED | Regression |
| `src/store/participant.rs` | Encrypted shares + plaintext public path | ✓ VERIFIED | Regression — `share_roundtrip` passes |
| `src/store/identity.rs` | IdentityKeypair + npub | ✓ VERIFIED | Regression — `identity_roundtrip_npub` passes |
| `src/store/checkpoint.rs` | Type-restricted DKG checkpoints | ✓ VERIFIED | Regression — `dkg_roundtrip`, `wipe_vs_keep` pass |
| `src/coordinator/schema.rs` | SCHEMA_V1 (5 public tables) | ✓ VERIFIED | Regression |
| `src/coordinator/mod.rs` | CoordinatorStore CRUD | ✓ VERIFIED | Regression — `open_migrate`, `roster_roundtrip`, `tables` pass |
| `tests/store_headless.rs` | Headless persist/reload | ✓ VERIFIED | Re-ran, passes |
| `tests/store_checkpoint_n100.rs` | #[ignore]d n=100 harness | ✓ VERIFIED | Present, ignored (deferred to Phase 3) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `InteractivePassphrase::passphrase` | `SecretString` | `Zeroizing::new(rpassword read)` → `.as_str().to_owned()` → `SecretString::from` | ✓ WIRED | Owned copy moved into zeroize-on-drop SecretString; transient buffer wiped by Zeroizing — no un-zeroized intermediate survives (WR-01) |
| `store::participant` | `cli::address::PubkeyEnvelope` | `PubkeyEnvelope::from_package` | ✓ WIRED | Regression — single address-derivation path preserved |
| `store::participant`/`checkpoint`/`identity` | `store::crypto` + `store::atomic` | encrypt_secret + write_atomic | ✓ WIRED | Regression — all secret writes through same seam |
| `decrypt_secret` | caller drop | `Zeroizing<Vec<u8>>` | ✓ WIRED | Regression |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| WR-01 marker: all 3 rpassword reads Zeroizing-wrapped | `grep -c 'Zeroizing::new(rpassword::prompt_password'` | 3 | ✓ PASS |
| WR-01 marker: zeroize import present | `grep 'use zeroize::Zeroizing;'` | line 20 | ✓ PASS |
| Build clean (production, no cfg-test warning) | `cargo build` | Finished, 0 warnings | ✓ PASS |
| Passphrase unit round-trip | `cargo test --lib store::passphrase` | 1 passed | ✓ PASS |
| Full lib suite (regression) | `cargo test --lib` | 20 passed, 0 failed | ✓ PASS |
| Structural guards (nonce/identity/generic-persist non-expressible) | `cargo test --test compile_fail` | 4 passed | ✓ PASS |
| Headless persist/reload | `cargo test --test store_headless` | 1 passed | ✓ PASS |
| WR-01 fix committed cleanly | `git show --stat c2c3a83` | 1 file, +7/-6, tree clean | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| STOR-01 | 02-01, 02-02, 02-05 | Participant storage: identity + per-key-per-epoch KeyPackage+PublicKeyPackage age/scrypt encrypted, zeroized, tagged | ✓ SATISFIED | Truths 1–8, 16; WR-01 closure completes the "zeroized in memory after use" clause for the passphrase acquisition path |
| STOR-02 | 02-03 | Ceremony round secrets checkpointed encrypted between rounds; signing nonces structurally excluded | ✓ SATISFIED | Truths 9–11; trybuild proves nonce non-expressible |
| STOR-03 | 02-04 | Coordinator SQLite: roster/transcripts/logs/policy/churn | ✓ SATISFIED | Truths 12–13; all tables roundtrip; public-only schema |

All three declared requirement IDs (STOR-01/02/03) accounted for. REQUIREMENTS.md maps exactly these to Phase 2 (STOR-04 is Phase 1) and marks all three **Complete** — no orphaned requirements.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/store/passphrase.rs` | 83, 92–97 (prior WR-01) | un-zeroized rpassword String | RESOLVED | Fixed in c2c3a83 — no longer present |
| `src/store/atomic.rs` | 43–56 (WR-03) | `create_dir_secure` doesn't tighten perms on pre-existing dir | ℹ️ Info | Maintainer ACCEPTED in 02-UAT.md test 3 (files stay 0600; only dir-listing metadata leaks if store pre-exists loose). No code change. |
| `src/store/passphrase.rs` | 20 | `use zeroize::Zeroizing;` flagged unused **in test builds only** | ℹ️ Info | Cosmetic: the import is used inside the `#[cfg(not(test))]` block, so production builds are warning-free (verified via `cargo build`). Optionally gate the import `#[cfg(not(test))]` to silence the test-build warning. Not a gap. |
| `Cargo.toml` | rusqlite 0.37 | pinned below documented 0.40.1 | ℹ️ Info | INTENTIONAL — MSRV-1.85-driven; recorded in 02-01-SUMMARY. Not a gap. |
| `src/coordinator/mod.rs` | foreign_keys pragma | no FK constraints in schema (IN-01) | ℹ️ Info | Harmless; reserved for later phases. |

### Human Verification Required

**1. Re-confirm interactive no-echo confirm-twice passphrase prompt (post-fix code, D5)**
- **Test:** Run `cheget` triggering new-store passphrase creation (`InteractivePassphrase::for_new_store`) at a real terminal, then the unlock path (`for_unlock`).
- **Expected:** No echo while typing; new-store prompts twice with the unrecoverability warning printed first; a mismatch is rejected ("passphrases did not match"); unlock prompts once. UX byte-for-byte identical to the prior UAT PASS — the only change is the transient buffer is now wiped on drop.
- **Why human:** `InteractivePassphrase` is `#[cfg(not(test))]`; no-echo is a runtime TTY property grep/headless tests cannot observe. The prior UAT PASS covered pre-fix code, and commit c2c3a83 modified these exact lines — a one-time re-confirmation of the behavior-preserving change is warranted.

### Gaps Summary

**No gaps.** The prior cycle's only actionable gap (WR-01) is genuinely closed in the codebase and regression-confirmed. All 16 must-have truths are VERIFIED with executed evidence; all three prior maintainer-decision items are resolved (WR-01 fixed, WR-03 accepted, interactive-UX re-flagged for a light post-change re-confirmation only). The durable-state foundation is functionally complete and behaviorally proven. Status is `human_needed` purely because the `#[cfg(not(test))]` no-echo TTY UX cannot be verified programmatically and its lines changed since the last human test — not because anything is missing or broken.

---

_Verified: 2026-07-14T14:55:15Z_
_Verifier: Claude (gsd-verifier)_
