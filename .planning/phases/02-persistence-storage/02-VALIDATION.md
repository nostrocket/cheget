---
phase: 2
slug: persistence-storage
status: approved
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-14
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from 02-RESEARCH.md § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `cargo test` (unit + integration) + `trybuild` (existing, compile-fail) |
| **Config file** | none — `#[test]` in modules + `tests/` dir |
| **Quick run command** | `cargo test --lib store::` (or `coordinator::`) |
| **Full suite command** | `cargo test` (excludes `#[ignore]` n=100) |
| **Estimated runtime** | ~sub-second per quick run after first bundled-SQLite build; full suite seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib store::` (or `coordinator::`) — sub-second, no bundled-SQLite rebuild after first
- **After every plan wave:** Run `cargo test` (full non-ignored suite) + `cargo clippy -- -D warnings`
- **Before `/gsd-verify-work`:** Full suite green + a manual `cheget participant share status` from a freshly-created store (no unlock)
- **Max feedback latency:** ~5 seconds (quick run)

---

## Per-Task Verification Map

| Req ID | Behavior | Test Type | Automated Command | File Exists | Status |
|--------|----------|-----------|-------------------|-------------|--------|
| STOR-01 | KeyPackage persist→reload roundtrip (encrypt, reload, decrypt, assert equal) via in-code passphrase | unit | `cargo test --lib store::participant::tests::share_roundtrip` | ❌ W0 | ⬜ pending |
| STOR-01 | Wrong passphrase fails to decrypt (no partial leak) | unit | `cargo test --lib store::crypto::tests::wrong_passphrase_fails` | ❌ W0 | ⬜ pending |
| STOR-01 | Identity keypair persist→reload; npub is stable + starts `npub1` | unit | `cargo test --lib store::identity::tests::identity_roundtrip_npub` | ❌ W0 | ⬜ pending |
| STOR-01 | Unix perms: store dir 0700, files 0600 (`#[cfg(unix)]`) | unit | `cargo test --lib store::atomic::tests::perms` | ❌ W0 | ⬜ pending |
| STOR-01 | `(key_id, epoch, seat)` tagging survives roundtrip; manifest indexes correctly | unit | `cargo test --lib store::manifest::tests::tags` | ❌ W0 | ⬜ pending |
| STOR-02 | dkg round1/round2 SecretPackage checkpoint persist→reload (D-08) | unit | `cargo test --lib store::checkpoint::tests::dkg_roundtrip` | ❌ W0 | ⬜ pending |
| STOR-02 | Wipe-on-success removes checkpoint files; keep-on-abort leaves them (D-10) | unit | `cargo test --lib store::checkpoint::tests::wipe_vs_keep` | ❌ W0 | ⬜ pending |
| STOR-02 | Nonce-exclusion preserved: no store API accepts `EphemeralNonces`/`SigningNonces` | compile-fail / structural | existing `tests/ui/nonce_no_serialize.rs` + review | ✅ (nonce) / ❌ store guard | ⬜ pending |
| STOR-02 | Atomic write: crash-simulated (leftover tmp) never yields a truncated/corrupt share; manifest points only to complete files | unit | `cargo test --lib store::atomic::tests::atomic_no_partial` | ❌ W0 | ⬜ pending |
| STOR-03 | Coordinator DB opens, migrates (user_version 0→1), WAL on | unit | `cargo test --lib coordinator::tests::open_migrate` | ❌ W0 | ⬜ pending |
| STOR-03 | Roster insert/query roundtrip with real npub (D-15) | unit | `cargo test --lib coordinator::tests::roster_roundtrip` | ❌ W0 | ⬜ pending |
| STOR-03 | Transcript / session_log / policy default / churn insert+query | unit | `cargo test --lib coordinator::tests::tables` | ❌ W0 | ⬜ pending |
| D-03 | Headless CI path: store built with in-code `PassphraseSource`, full persist/reload with no prompt | integration | `cargo test --test store_headless` | ❌ W0 | ⬜ pending |
| D-13 | Structural separation: no fn converts FROST↔identity | structural/review | code review + optional `tests/ui/` compile-fail | ❌ W0 | ⬜ pending |
| (Phase 3) | n=100 persist/reload of full share set through these stores | integration (`#[ignore]`) | `cargo test --release persist_reload_100 -- --ignored` | ❌ (built here, exercised Phase 3) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/store/crypto.rs` tests — age roundtrip + wrong-passphrase (covers STOR-01)
- [ ] `src/store/participant.rs` tests — share roundtrip, tagging (STOR-01)
- [ ] `src/store/identity.rs` tests — identity roundtrip + npub format (STOR-01, D-15)
- [ ] `src/store/atomic.rs` tests — perms + atomic-no-partial (STOR-01, D-07)
- [ ] `src/store/manifest.rs` tests — schema/versioning (D-05)
- [ ] `src/store/checkpoint.rs` tests — dkg roundtrip + wipe/keep (STOR-02, D-08/D-10)
- [ ] `src/coordinator/` tests — open/migrate/WAL + table roundtrips (STOR-03)
- [ ] `tests/store_headless.rs` — in-code PassphraseSource CI seam (D-03)
- [ ] Store-side nonce guard (no API accepts nonce material) — complements existing trybuild snapshot
- [ ] `#[ignore]` n=100 persist/reload harness stub (built here; run in Phase 3)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Interactive no-echo passphrase prompt on every secret-touching command (D-01) | STOR-01 | Terminal TTY interaction not automatable in unit tests; the `PassphraseSource` trait is exercised in-code, but the *interactive wiring* is manual | Run `cheget participant` command that touches a secret; confirm no-echo prompt appears, wrong passphrase rejects, no passphrase in env/history |
| Create-store confirm-twice + "lost passphrase = unrecoverable" warning (D-04) | STOR-01 | Interactive confirmation flow | Create a store; confirm it prompts twice, rejects mismatch, and prints the unrecoverable-loss warning |
| `cheget watcher address` / `share status` work from store alone, no unlock (D-05) | STOR-01 | End-to-end CLI behavior from a fresh store | From a freshly-created store, run address/status with no `--pubkey` file and no passphrase prompt |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 5s
- [x] `nyquist_compliant: true` set in frontmatter
- [ ] Wave 0 test stubs written (execution — flips `wave_0_complete: true` during /gsd-execute-phase)

**Approval:** approved 2026-07-14 (plan-design level; `wave_0_complete` flips at execution)
