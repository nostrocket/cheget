---
phase: 1
slug: crypto-bridge-in-process-signing
status: approved
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-10
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust built-in) + `trybuild` (compile-fail) + `corepc-node`/`bitcoind` (auto-spawn regtest) |
| **Config file** | none — Wave 0 creates `Cargo.toml`, `Cargo.lock`, pinned deps |
| **Quick run command** | `cargo test --lib` (unit + bridge KAT + trybuild; no regtest) |
| **Full suite command** | `cargo test` (adds small-n end-to-end on auto-spawned regtest) |
| **Estimated runtime** | quick ~30–60s · full ~2–5 min (regtest node spawn) · nightly n=100 e2e separate |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib` (quick — unit, KAT, trybuild)
- **After every plan wave:** Run `cargo test` (full — includes regtest small-n e2e)
- **Before `/gsd-verify-work`:** Full suite green; the `t=51/n=100` nightly/on-demand e2e must pass before Phase 1 sign-off (D-06)
- **Max feedback latency:** ~60 seconds (quick), ~300 seconds (full)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 1-01-xx | 01 | 1 | KEY-03, KEY-04 | T-1-bridge | Bridge maps 33-byte SEC1 → x-only → `XOnlyPublicKey` → P2TR matching BIP341/BIP86 KAT (even-Y AND odd-Y-origin) | unit (KAT) | `cargo test --lib bridge::` | ❌ W0 | ⬜ pending |
| 1-02-xx | 02 | 2 | KEY-01, KEY-02, KEY-05, (KEY-06) | T-1-dkg | 51-of-100 in-process DKG yields one group key = internal key `P`; every seat confirms; mismatch aborts | unit + e2e | `cargo test dkg::` / nightly n=100 | ❌ W0 | ⬜ pending |
| 1-03-xx | 03 | 2 | STOR-04 | T-1-chain | `ChainBackend` trait conformance; key-spend sighash, PSBT finalize, broadcast+confirm on regtest | unit + regtest e2e | `cargo test chain::` | ❌ W0 | ⬜ pending |
| 1-04-xx | 04 | 3 | SIGN-01..07 | T-1-nonce, T-1-blindsign | Non-serializable nonce (won't compile if persisted); tweaked sig verifies against `Q`; display-before-sign ack; cheater culprits on abort | unit + trybuild + e2e | `cargo test sign::` + `cargo test --test compile_fail` | ❌ W0 | ⬜ pending |
| 1-05-xx | 05 | 1 | (architectural seam) | — | `Transport` trait + in-memory stub; no relay concretes leak into ceremony/session logic | unit | `cargo test transport::` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky. Exact task IDs finalized by the planner.*

---

## Wave 0 Requirements

- [ ] `Cargo.toml` + committed `Cargo.lock` — pinned crate stack (frost-secp256k1-tr 3.0.0, bitcoin 0.32.101, bitcoincore-rpc 0.19, esplora-client 0.13, corepc-node); `rust-version = "1.85"`
- [ ] `dev-dependencies`: `trybuild`, `corepc-node`/`bitcoind`
- [ ] `tests/` harness scaffold: bridge KAT fixtures (BIP341/BIP86 vectors + constructed odd-Y case), regtest fixture helper (auto-spawn node), compile-fail test dir

*Greenfield — Wave 0 installs the entire framework; no existing infrastructure to reuse.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Human-readable display-before-sign rendering | SIGN-07 | Visual UX (rendered outputs/amounts/fee) is asserted structurally in tests, but the human-legibility of the rendering is a manual review | Run `cheget sign` against a regtest PSBT without `--yes`; confirm outputs, amounts, and fee are shown and an explicit ack is required before round 2 |
| n=100 O(n²) timing/memory measurement | KEY-06 (folded in) | The measurement *is* the deliverable — pass/fail is feasibility judgement, not a fixed threshold | Run nightly n=100 e2e in `--release`; record per-part wall-clock + RSS |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 300s (full suite)
- [x] `nyquist_compliant: true` set in frontmatter

Every task's `<automated>` verify now propagates cargo's real exit status (`set -o pipefail;` prefix on all `| tail`-piped cargo commands), so a failing build/test gates the task rather than being masked by tail's exit 0 — including the crown-jewel bridge KAT (01-01/T3) and the confirmed regtest key-spend (01-04/T3).

**Approval:** approved 2026-07-10
