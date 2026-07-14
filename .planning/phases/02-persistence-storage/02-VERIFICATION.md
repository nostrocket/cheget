---
phase: 02-persistence-storage
verified: 2026-07-14T13:13:03Z
status: human_needed
score: 15/15 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification:
  previous_status: none
  previous_score: none
  note: "Initial verification. Incorporates 02-REVIEW.md: 2 Critical blockers confirmed FIXED (CR-01 KeyId traversal, CR-02 atomic migration), 3 Warnings + 4 Info remain (see below)."
human_verification:
  - test: "Run `cheget` on a fresh store at a real terminal, triggering the interactive new-store passphrase prompt (InteractivePassphrase::for_new_store), and observe the prompt."
    expected: "The passphrase is NOT echoed as you type; you are prompted twice and a mismatch is rejected; the 'a lost passphrase makes them unrecoverable — there is no reset' warning prints before the prompts."
    why_human: "The InteractivePassphrase impl is #[cfg(not(test))] — it cannot be linked or driven in any test build. No-echo terminal behavior and the confirm-twice UX are runtime TTY properties grep and headless tests cannot observe. Flagged human_judgment in 02-01 coverage item D5."
  - test: "Review WR-01 (open warning): decide whether the un-zeroized rpassword String in src/store/passphrase.rs:83,92-97 is acceptable, or must be wrapped in Zeroizing before the phase is considered closed."
    expected: "A decision recorded: either fix (wrap the rpassword reads in Zeroizing so the transient plaintext buffer is wiped on drop) or accept-and-document (drop the 'never lands in a plain String' claim in the module doc). This is the passphrase that unlocks the identity key AND every share."
    why_human: "Security memory-hygiene judgment call on cfg-gated production code; contradicts the module's own stated invariant. Not a must-have failure, but a security policy decision the maintainer should make."
  - test: "Review WR-03 (open warning): decide whether create_dir_secure (src/store/atomic.rs:43-56) must tighten permissions on a pre-existing store directory."
    expected: "A decision recorded: either enforce 0700 on an existing dir on Unix, or accept the metadata-listing exposure (files stay 0600 regardless; only directory listings leak if ~/.cheget pre-exists with loose perms)."
    why_human: "Threat-model judgment on directory-metadata exposure; freshly-created dirs are correctly 0700 (verified by the perms test)."
---

# Phase 2: Persistence & Storage — Verification Report

**Phase Goal:** Lay down the durable-state foundation the ceremony and transport layers build on — age/scrypt participant storage with nonce-exclusion and epoch tagging, encrypted between-round ceremony checkpointing, and the coordinator SQLite store for roster/transcripts/logs/policy/churn — so no durable state is retrofitted later.
**Verified:** 2026-07-14T13:13:03Z
**Status:** human_needed
**Re-verification:** No — initial verification (after code-review gap closure)

## Goal Achievement

The durable-state foundation is present, wired, and behaviorally proven in the codebase. All three ROADMAP success criteria (STOR-01, STOR-02, STOR-03) are met by real, tested implementations — not stubs. The two Critical blockers raised in 02-REVIEW.md have been fixed and are now locked in by passing regression tests I executed directly. The single reason the status is `human_needed` rather than `passed` is the interactive no-echo passphrase prompt, which is `#[cfg(not(test))]` and can only be validated at a real TTY (flagged human_judgment in 02-01/D5), plus two open review warnings that call for a maintainer decision.

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | age/scrypt encrypt→decrypt round-trips; wrong passphrase → Err, no partial plaintext; decrypt returns Zeroizing | ✓ VERIFIED | `src/store/crypto.rs` — `decrypt_secret` returns `Zeroizing<Vec<u8>>`, scrypt sole recipient, `set_work_factor(18)` before encrypt; tests `age_roundtrip`/`wrong_passphrase_fails` (suite green per context) |
| 2 | Atomic write leaves no partial/truncated file on crash; 0700 dir / 0600 file on Unix | ✓ VERIFIED | `src/store/atomic.rs` temp(create_new,0600)→file fsync→rename→**dir fsync**; ran `perms`, `atomic_no_partial`, `interrupted_write_does_not_truncate_existing` — all pass |
| 3 | PassphraseSource seam: interactive (prod) + in-code (test) behind one trait; no passphrase env var or CLI flag ships | ✓ VERIFIED | `src/store/passphrase.rs` trait + `InCodePassphrase` + `#[cfg(not(test))] InteractivePassphrase`; CLI `--help` shows only `--home` (no passphrase flag); no env var beyond `CHEGET_HOME` (path only) |
| 4 | Full pinned dependency set builds on MSRV 1.85 | ✓ VERIFIED | `cargo +1.85.0 check --all-targets` green (context); rusqlite 0.37/home 0.5.9 MSRV pins recorded in 02-01-SUMMARY |
| 5 | KeyPackage persists age-encrypted, reloads byte-equal, tagged (key_id, epoch, seat) | ✓ VERIFIED | `src/store/participant.rs put_share/load_share`; `store_headless` integration test (I ran it — passes) proves byte-equal persist/reload under in-code passphrase |
| 6 | Plaintext PublicKeyPackage envelope readable with NO unlock, reusing PubkeyEnvelope | ✓ VERIFIED | `put_public_envelope`/`load_public_envelope` (no passphrase); reuses `crate::cli::address::PubkeyEnvelope`; `share_roundtrip` asserts read under WRONG passphrase succeeds |
| 7 | Transport identity persists/reloads; npub stable, starts npub1, Bech32 (not Bech32m) | ✓ VERIFIED | `src/store/identity.rs` `bech32::encode::<Bech32>`; test `identity_roundtrip_npub` asserts Bech32 validates + Bech32m fails (suite green per context) |
| 8 | No FROST↔identity conversion (reuse non-expressible) | ✓ VERIFIED | trybuild `identity_has_no_frost_conversion` — I ran `cargo test --test compile_fail`, passes; no From/TryFrom to/from FROST types in `identity.rs` |
| 9 | dkg round1/round2 SecretPackage checkpoint encrypted, reload byte-faithful via real part1/part2 | ✓ VERIFIED | `src/store/checkpoint.rs` concrete `put/load_round1/2`; `dkg_roundtrip` drives real `dkg::part1`→checkpoint→reload→`dkg::part2` (suite green per context) |
| 10 | wipe-on-success removes files; keep-on-abort retains for resume | ✓ VERIFIED | `CheckpointStore::wipe`; ran `wipe_vs_keep` — passes (state-cleanup invariant behaviorally exercised) |
| 11 | Checkpoint store exposes NO generic persist and NO nonce method — nonce is non-expressible | ✓ VERIFIED | trybuild `checkpoint_rejects_nonce_material` + `checkpoint_has_no_generic_persist` — I ran both, pass; only concrete `dkg::round{1,2}::SecretPackage` methods exist |
| 12 | Coordinator SQLite opens, WAL + foreign_keys, migrates user_version 0→1 | ✓ VERIFIED | `src/coordinator/mod.rs open/migrate`; ran `open_migrate` — passes (WAL, foreign_keys=1, user_version=1) |
| 13 | roster/transcripts/session_logs/policy(defaults)/churn roundtrip; real npubs; no secret column | ✓ VERIFIED | ran `roster_roundtrip` (real `IdentityKeypair::npub()`) + `tables` (policy defaults 50/24/7776000) — pass; `schema.rs` grep confirms no secret column |
| 14 | `cheget participant share-status` lists shares with NO unlock, no --pubkey file | ✓ VERIFIED | Ran `CHEGET_HOME=<tmp> cheget participant share-status` → "no shares held", exit 0, no prompt; reads via static `ParticipantStore::read_manifest` (no PassphraseSource constructed) |
| 15 | Headless integration test drives persist→reload with in-code PassphraseSource, no TTY | ✓ VERIFIED | Ran `cargo test --test store_headless` → `headless_persist_reload_no_prompt` passes |

**Score:** 15/15 truths verified (0 present, behavior-unverified)

### Deferred Items

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | n=100 at-scale persist/reload of the full share set through the Phase 2 stores | Phase 3 | `tests/store_checkpoint_n100.rs::persist_reload_100` is `#[ignore]`d and built here; Phase 3 SC #3: "The generated n=100 share set persists to and reloads from the Phase 2 participant/coordinator stores at scale". Confirmed harness present, ignored, defaults to t=51/n=100. |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/store/mod.rs` | StoreError + StoreRoot substrate | ✓ VERIFIED | Manual Debug/Display/Error idiom (no thiserror); CHEGET_HOME path-only resolution |
| `src/store/atomic.rs` | Crash-safe write + perms | ✓ VERIFIED | temp+fsync+rename+dir-fsync; 3 tests pass |
| `src/store/crypto.rs` | age/scrypt encrypt/decrypt | ✓ VERIFIED | Zeroizing boundary, scrypt sole recipient, log_n=18 |
| `src/store/passphrase.rs` | PassphraseSource seam | ✓ VERIFIED | interactive (cfg-gated) + in-code impls |
| `src/store/manifest.rs` | Versioned share index | ✓ VERIFIED | schema_version reject-unknown; add/lookup/remove; ran `tags`/`rejects_unknown_schema_version` |
| `src/store/participant.rs` | Encrypted shares + plaintext public path | ✓ VERIFIED | manifest-written-last ordering; multi-epoch coexistence |
| `src/store/identity.rs` | IdentityKeypair + npub | ✓ VERIFIED | independent OsRng, Zeroizing secret, no FROST conversion |
| `src/store/checkpoint.rs` | Type-restricted DKG checkpoints | ✓ VERIFIED | concrete-typed API, CeremonyId traversal guard, wipe/keep |
| `src/coordinator/schema.rs` | SCHEMA_V1 (5 public tables) | ✓ VERIFIED | roster/transcripts/logs/policy/churn; no secret column; 0 FOREIGN KEY (IN-01) |
| `src/coordinator/mod.rs` | CoordinatorStore CRUD | ✓ VERIFIED | WAL+pragmas, atomic migration, insert/query for every table |
| `cheget participant share-status` | Unlock-free CLI | ✓ VERIFIED | Routes to `read_manifest`; ran, no prompt |
| `cheget coordinator roster` | Roster listing | ✓ VERIFIED | Routes to `CoordinatorStore::list_roster` |
| `tests/store_headless.rs` | Headless persist/reload | ✓ VERIFIED | Ran, passes |
| `tests/store_checkpoint_n100.rs` | #[ignore]d n=100 harness | ✓ VERIFIED | Present, ignored, defaults t=51/n=100 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `store::participant` | `cli::address::PubkeyEnvelope` | `PubkeyEnvelope::from_package` | ✓ WIRED | Single address-derivation path preserved (D-05) |
| `store::participant`/`checkpoint`/`identity` | `store::crypto` + `store::atomic` | encrypt_secret + write_atomic | ✓ WIRED | All secret writes go through the same encrypt→atomic seam |
| `coordinator` roster | `store::IdentityKeypair::npub()` | real npub in `roster_roundtrip` | ✓ WIRED | D-15 real, testable npubs |
| `decrypt_secret` | caller drop | `Zeroizing<Vec<u8>>` | ✓ WIRED | Plaintext wiped at op end (D-06) |
| `run_inprocess_dkg` (`crypto/keygen.rs`) | (unchanged) | — | ✓ WIRED | Persistence stays out of pure crypto core (D-08); checkpoint drives real part1/part2 without modifying keygen |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Fast lib tests incl. both critical-fix regressions | `cargo test --lib -- coordinator:: store::manifest:: store::atomic:: crypto::types::tests ...` | 12 passed | ✓ PASS |
| CR-01 fix: KeyId rejects traversal | `crypto::types::tests::key_id_rejects_traversal` | pass | ✓ PASS |
| CR-02 fix: migration atomic on failure | `coordinator::tests::migrate_is_atomic_on_failure` | pass | ✓ PASS |
| Structural guards (nonce/identity/generic-persist) | `cargo test --test compile_fail` | 4 passed | ✓ PASS |
| Headless persist/reload | `cargo test --test store_headless` | 1 passed | ✓ PASS |
| CLI share-status unlock-free | `CHEGET_HOME=<tmp> cheget participant share-status` | "no shares held", exit 0, no prompt | ✓ PASS |
| Build lib + bins | `cargo build --lib --bins` | Finished clean | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| STOR-01 | 02-01, 02-02 | Participant storage: identity + per-key-per-epoch KeyPackage+PublicKeyPackage age/scrypt encrypted, zeroized, tagged (key_id, epoch, identifier) | ✓ SATISFIED | Truths 1-8; headless test proves persist/reload; Zeroizing on decrypt path |
| STOR-02 | 02-03 | Ceremony round secrets checkpointed encrypted between rounds; signing nonces structurally excluded | ✓ SATISFIED | Truths 9-11; trybuild proves nonce non-expressible; EphemeralNonces non-serializable |
| STOR-03 | 02-04 | Coordinator SQLite: roster/transcripts/logs/policy/churn | ✓ SATISFIED | Truths 12-13; all tables roundtrip; public-only schema |

All three declared requirement IDs accounted for. No orphaned requirements: REQUIREMENTS.md maps exactly STOR-01/02/03 to Phase 2 (STOR-04 is Phase 1). REQUIREMENTS.md marks all three Complete.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (phase files) | — | Debt markers (TODO/FIXME/XXX/unimplemented!) | ℹ️ Info | NONE found in src/store, src/coordinator, src/cli/mod.rs |
| `src/store/passphrase.rs` | 83, 92-97 | rpassword returns plaintext `String`, dropped un-zeroized (WR-01, OPEN) | ⚠️ Warning | Transient passphrase buffer not wiped; contradicts module doc. cfg(not(test)) prod path. Does not fail a must-have. Human decision requested. |
| `src/store/atomic.rs` | 43-56 | `create_dir_secure` early-returns on pre-existing dir without tightening perms (WR-03, OPEN) | ⚠️ Warning | Directory-listing metadata leak if store dir pre-exists loose; files stay 0600. Fresh dirs correctly 0700. Human decision requested. |
| `Cargo.toml` | 43 | rusqlite pinned 0.37 vs documented 0.40.1 (WR-02) | ℹ️ Info | INTENTIONAL — MSRV-1.85-driven (libsqlite3-sys 0.38.1 needs 1.88); recorded in 02-01-SUMMARY key-decisions. Not a gap. |
| `src/coordinator/mod.rs` | 145 | `foreign_keys=ON` no-op — no FK constraints in schema (IN-01) | ℹ️ Info | Harmless; grep confirms 0 FOREIGN KEY. Reserved for later phases. |

### Human Verification Required

**1. Interactive no-echo passphrase prompt (UX/security, D5 human_judgment)**
- **Test:** Run `cheget` triggering new-store passphrase creation at a real terminal.
- **Expected:** No echo while typing; prompted twice with mismatch rejected; unrecoverability warning printed first.
- **Why human:** `InteractivePassphrase` is `#[cfg(not(test))]` — cannot be exercised headlessly; no-echo is a runtime TTY property.

**2. WR-01 open warning — un-zeroized rpassword String (maintainer decision)**
- **Test/Decide:** Fix (wrap rpassword reads in Zeroizing) or accept-and-document. This passphrase unlocks the identity key and every share.
- **Why human:** Security memory-hygiene policy judgment on cfg-gated code contradicting its own doc invariant.

**3. WR-03 open warning — pre-existing store dir perms (maintainer decision)**
- **Test/Decide:** Enforce 0700 on an existing dir, or accept the directory-listing metadata exposure.
- **Why human:** Threat-model judgment; files remain 0600 regardless.

### Gaps Summary

No gaps. All 15 must-have truths are VERIFIED against the codebase with executed test/behavioral evidence, and both Critical review blockers are fixed and regression-tested:

- **CR-01 (fixed):** `KeyId` (src/crypto/types.rs) now has a private inner field and is constructible only via fallible `KeyId::new`/`TryFrom`, which reject any non-`[A-Za-z0-9_-]` component — invalid (traversal) state is non-representable, mirroring `CeremonyId`. Test `key_id_rejects_traversal` passes.
- **CR-02 (fixed):** `migrate` (src/coordinator/mod.rs) now wraps `execute_batch(SCHEMA_V1)` + `user_version` bump in one `unchecked_transaction`, so a crash mid-migration rolls back cleanly instead of bricking the DB. Test `migrate_is_atomic_on_failure` passes.

The status is `human_needed` (not `passed`) solely because the interactive passphrase path's no-echo UX cannot be verified programmatically, plus two open review warnings (WR-01, WR-03) request an explicit maintainer decision. These do not block goal achievement — the durable-state foundation is functionally complete and behaviorally proven.

---

_Verified: 2026-07-14T13:13:03Z_
_Verifier: Claude (gsd-verifier)_
