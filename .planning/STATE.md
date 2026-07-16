---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: 03
current_phase_name: dkg-at-scale-local
status: planning
stopped_at: "Phase 3 context gathered (reframed to CLI wiring: keygen→store, sign←store)"
last_updated: "2026-07-16T06:22:59.120Z"
last_activity: 2026-07-16
last_activity_desc: Phase 03 execution started
progress:
  total_phases: 7
  completed_phases: 3
  total_plans: 12
  completed_plans: 12
  percent: 43
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-10)

**Core value:** A group of 100 can jointly control one Bitcoin address (any 51 can spend, no individual holds the key), rotate membership with zero on-chain cost, and truly revoke past compromise by sweeping to a standby key.
**Current focus:** Phase 03 — dkg-at-scale-local

## Current Position

Phase: 03 (dkg-at-scale-local) — EXECUTING (03-01, 03-02 plans complete)
Verification: human_needed — 02-UAT Test 1 BLOCKED (interactive passphrase prompt); keygen --persist (03-01) + sign --persist (03-02) now drive InteractivePassphrase::for_new_store/for_unlock — re-confirm the interactive UX via /gsd-verify-work
Next: verify Phase 03, then transition
Last activity: 2026-07-16 — 03-02 (sign-from-store) complete: confirmed regtest key-spend from PERSISTED shares (D-05)

Progress: [██████░░░░░░░░] 3/7 phases complete (43%) · 12/12 executed plans

## Performance Metrics

**Velocity:**

- Total plans completed: 10 (Phase 01: 5/5, Phase 02: 5/5)
- Durations below are as recorded per plan; 02-05 (gap-closure) duration was not recorded.

**Per-plan record:**

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 01 P01 | 25 min | 3 tasks | 18 files |
| Phase 01 P02 | 110 min | 3 tasks | 10 files |
| Phase 01 P03 | 40 min | 3 tasks | 7 files |
| Phase 01 P04 | 16 min | 3 tasks | 13 files |
| Phase 01 P05 | 10 min | 2 tasks | 4 files |
| Phase 02 P01 | 11 min | 3 tasks | 7 files |
| Phase 02 P02 | ~30 min | 3 tasks | 6 files |
| Phase 02 P03 | 16 min | 2 tasks | 8 files |
| Phase 02 P04 | 24 min | 3 tasks | 6 files |
| Phase 02 P05 | not recorded | 2 tasks | (gap-closure: passphrase.rs zeroize) |
| Phase 03 P01 | 25 | 3 tasks | 5 files |
| Phase 03 P02 | 12min | 3 tasks | 4 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Init]: Roadmap covers all of M1–M5 (full spec end-to-end)
- [Init]: DKG is the only keygen path; dealer mode dropped — Phase 1 keygen is in-process DKG with simulated participants
- [Init]: Crypto bridge proven early via in-process DKG before n=100 transport (Phase 1 = bridge + regtest key-spend, zero transport)
- [Revision]: Prove the entire system LOCALLY first, real transport LAST. The `Transport` trait + in-memory stub (introduced Phase 1) let every ceremony phase (3–6) run with zero relay code; Phase 7 swaps in real `FileTransport`/`NostrTransport` behind the same trait and re-runs at scale
- [Revision]: Local DKG-at-scale compute proof (KEY-06, Phase 3, n=100 in-process) is separated from the transport-layer relay load test (TRAN-08, Phase 7)
- [Revision]: SEC-03 narrowed to locally-verifiable adversarial tests (mixed-epoch, nonce-reuse-won't-compile) in hardening (Phase 6); new SEC-05 (malicious-relay DoS, replayed-envelope rejection) lives in the final transport phase (Phase 7)
- [Phase ?]: [01-01]: Canonical bridge established; x-only from_slice confined to bridge/taproot.rs; even-Y invariant rejects OddY (D-11)
- [Phase ?]: [01-01]: Public-artifact envelope (D-09) = frost PublicKeyPackage hex in serde_json with key_id + reserved epoch; cheget address --network defaults to bitcoin
- [Phase ?]: [01-01]: Pinned stack committed (frost-secp256k1-tr 3.0.0, bitcoin 0.32.101); corepc-node feature 28_0; toolchain 1.96.0 / MSRV 1.85
- [Phase 01]: [01-05]: Transport trait seam + in-memory stub (D-08); opaque-bytes Envelope shaped for Nostr event kinds; content-derived FNV-1a EnvelopeId seeds Phase-7 dedup; no nostr-sdk type in the seam
- [Phase ?]: [02-01]: MSRV gate branch (b) — rusqlite 0.40.1→0.37.0 (bundled) and home 0.5.12→0.5.9; full dep set builds on 1.85. 02-04 targets rusqlite 0.37.
- [Phase ?]: [02-01]: rpassword APPROVED (Task 1); license Apache-2.0 not MIT (benign). No passphrase env/CLI flag ships (D-01/D-03); CHEGET_HOME is path-override only.
- [Phase ?]: [02-01]: store layer = StoreError (manual idiom) + write_atomic (D-07) + age/scrypt log_n=18 Zeroizing decrypt (D-06) + PassphraseSource seam (interactive/in-code).
- [Phase 02]: 02-02: IdentityKeypair transport key is structurally non-derivable from FROST material (no From/TryFrom), proven by a trybuild compile-fail snapshot (D-13)
- [Phase ?]: 03-01: keygen --persist writes per-seat encrypted seat-NNNN roots via pub persist_dkg_shares under one prompt-once ResolvedPassphrase (D-02/D-03/D-04)
- [Phase ?]: [03-02]: sign --persist loads first-t seat-NNNN roots (prompt-once for_unlock) via pub load_persisted_shares -> load_only_active -> existing SigningSession; confirmed regtest key-spend from PERSISTED shares (D-05), SIGN-05/SIGN-07 preserved

### Pending Todos

None yet.

### Blockers/Concerns

- [Phase 02]: 02-UAT Test 1 BLOCKED (blocked_by: prior-phase) — the interactive new-store passphrase prompt (`InteractivePassphrase::for_new_store`) has no CLI entry point in the current binary, so its runtime no-echo/confirm-twice UX cannot be re-confirmed. The WR-01 zeroize fix itself is verified present in source (src/store/passphrase.rs:80-100). 02-VERIFICATION stays `human_needed` until a later phase wires an encrypted-store-creating command; re-confirm then via `/gsd-verify-work 02`.
- [Roadmap]: Four controls MUST be structural from Phase 1, not retrofitted — non-serializable nonce type (SIGN-05), byte-level bridge round-trip (KEY-03), tweak/aggregate verified against Q (SIGN-03/04), display-before-sign sighash recompute (SIGN-07)
- [Roadmap]: The `Transport` trait + in-memory stub is the load-bearing architectural seam — it MUST be introduced in Phase 1 so DKG-at-scale (Phase 3), rotation (Phase 4), lifecycle (Phase 5), and hardening (Phase 6) all validate locally with zero relay code
- [Roadmap]: n=100 O(n²) DKG over Nostr (TRAN-08) is the highest project unknown — Phase 7 flagged for deeper research on strfry tuning + round-2 pacing; the load test is a gating deliverable, not optional. Keep `FileTransport` + schema before `NostrTransport` within Phase 7
- [Roadmap]: KEY-06 (local n=100 DKG) de-risks the O(n²) compute cost in Phase 3 before any relay code, isolating compute-scaling from transport-scaling

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 260713-itg | Massively speed up the in-process n=100 FROST DKG simulation (rayon-parallel rounds 2/3, O(n²) clone elimination, release-profile tuning) — ~6.6× at t=101/n=200 | 2026-07-13 | 9bc25e4 | [260713-itg-massively-speed-up-the-in-process-n-1000](./quick/260713-itg-massively-speed-up-the-in-process-n-1000/) |
| 260713-jqs | Change fixed FROST parameters t=501/n=1000 → t=51/n=100 across the entire project (live docs, Phase-1 history, source, tests); renamed full-scale tests to `_100`, corrected #[ignore] cost language. Measured full 51/100: crown-jewel regtest key-spend 9.90s, DKG group-key proof 4.41s | 2026-07-13 | 07a0f25 | [260713-jqs-change-fixed-frost-parameters-from-t-501](./quick/260713-jqs-change-fixed-frost-parameters-from-t-501/) |
| 260713-kgv | add a readme | 2026-07-13 | 8858b36 | [260713-kgv-add-a-readme](./quick/260713-kgv-add-a-readme/) |
| 260713-kxi | rename project from tsig to cheget (crate, binary, lib, `CHEGET_*` env prefix, and all prose incl. completed planning history) | 2026-07-13 | f1c14c1 | [260713-kxi-rename-project-from-tsig-to-cheget](./quick/260713-kxi-rename-project-from-tsig-to-cheget/) |
| 260713-pk2 | update README with mainnet and regtest usage, fully current (documented real verified CLI surface; added regtest/mainnet sections; corrected stale keygen/sign and #[ignore] claims) | 2026-07-13 | df44fbb | [260713-pk2-update-readme-with-mainnet-and-regtest-u](./quick/260713-pk2-update-readme-with-mainnet-and-regtest-u/) |
| 260715-msx | Update README CLI docs to current binary surface (added `participant share-status` + `coordinator roster`; corrected "three commands" count; reframed stale no-persistence/Phase-1-only claims now that Phase 2 shipped, without overstating — storage layer + read inspectors only, no persisted-key custody yet) | 2026-07-15 | b9c89b4 | [260715-msx-update-readme-md-cli-documentation-to-ma](./quick/260715-msx-update-readme-md-cli-documentation-to-ma/) |

### Roadmap Evolution

- Phase 3 edited: reframed goal/success-criteria/plans: DKG-at-scale proof (already met by Phase 1) -> CLI wiring keygen->store + sign<-store; unblocks Phase 2 store-creation UAT

## Deferred Items

Items acknowledged and carried forward from previous milestone close:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| *(none)* | | | |

## Session Continuity

Last session: 2026-07-16T06:21:41.139Z
Stopped at: Phase 3 context gathered (reframed to CLI wiring: keygen→store, sign←store)
Resume file: .planning/phases/03-dkg-at-scale-local/03-CONTEXT.md
