---
task: 260713-jqs
title: Change fixed FROST parameters from t=501/n=1000 to t=51/n=100
subsystem: crypto-parameters
tags: [frost, threshold, params, tests, docs]
status: complete
requires: []
provides: ["fixed FROST parameters t=51/n=100 across docs, source, and tests"]
affects: [docs, source, tests]
key-files:
  created:
    - tests/inproc_sign_100.rs
    - tests/dkg_100_correctness.rs
  modified:
    - .claude/CLAUDE.md
    - .planning/PROJECT.md
    - .planning/ROADMAP.md
    - .planning/REQUIREMENTS.md
    - SPEC-frost-cli.md
    - .planning/research/*.md
    - .planning/phases/01-crypto-bridge-in-process-signing/*
    - .planning/quick/260713-itg-massively-speed-up-the-in-process-n-1000/*.md
    - src/cli/keygen.rs
    - src/cli/sign.rs
    - tests/inproc_sign.rs
metrics:
  duration_min: 14
  completed: 2026-07-13
  commits: 4
---

# Quick Task 260713-jqs: Change fixed FROST parameters t=501/n=1000 → t=51/n=100 Summary

Permanently rescaled the project's fixed FROST parameters from t=501/n=1000 to t=51/n=100 across live docs, completed planning history, source, and tests — surgically preserving fee math, crate/version/MSRV numbers, the BIP341/86 KAT vector, and the itg quick-task's GSD state-identifier directory — then renamed the two full-scale test binaries to `_100`, corrected the now-false multi-CPU-hour `#[ignore]` rationale, and ran the full verification suite (both renamed 51/100 tests pass in seconds).

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 | `8ecd8dd` | docs(params): rescale fixed FROST parameters t=501/n=1000 to t=51/n=100 in live docs |
| 2 | `0791214` | docs(params): rewrite Phase-1 history and itg quick-task prose to t=51/n=100 |
| 3 | `625125d` | refactor(params): set fixed FROST parameters to t=51/n=100 in source |
| 4 | `07a0f25` | test(params): rename full-scale tests to _100 and set 51/100 defaults |

Task 5 (verification) produced no code change — the `#[ignore]` decision was to keep both attributes (see below), so no file edit was required.

## What Changed

- **Task 1 — live docs:** t=51/n=100 across CLAUDE.md, PROJECT.md, ROADMAP.md, REQUIREMENTS.md, SPEC-frost-cli.md, and the five `research/*.md`. Rewrote n-derived prose (group-of-100, any-51, "100 people verify", identifier space `1..=100`, `--seats 100 --threshold 51`, `generate_with_dealer(100, 51, …)`). Rescaled the O(n²) ceremony figure `~10⁶ events (~1 GB)` → `~10⁴ events (~10 MB)` and marked it an order-of-magnitude estimate in CLAUDE.md/PROJECT.md; also rescaled derived figures (`999` per-sender → `99`, `10⁶` → `10⁴`, GB → MB) consistently.
  - Note: `SPEC-frost-cli.md` was previously untracked; committing Task 1 adds it to the repo for the first time (it is in Task 1's file list).
- **Task 2 — planning history:** rescaled every Phase-01 artifact and the itg quick-task `.md` prose. Renamed embedded test-binary/fn references (`inproc_sign_1000`→`inproc_sign_100`, `dkg_1000_correctness`→`dkg_100_correctness`, `*_501_of_1000`→`*_51_of_100`). `01-UAT.md` run commands now point at the renamed `_100` binaries and read "51-of-100".
- **Task 3 — source:** `FULL_THRESHOLD=51` / `FULL_SEATS=100` in `cli/keygen.rs` and `cli/sign.rs`; all doc-comments and `--full` help text set to 51/100 (lib, crypto/keygen, crypto/nonce, session/liveness, transport/envelope, chain/mod, chain/esplora, cli/mod).
- **Task 4 — tests:** `git mv` renamed both full-scale files; crown-jewel fns renamed to `inproc_sign_confirmed_regtest_key_spend_51_of_100` and `dkg_100_all_shares_verify_to_one_group_key`; env defaults `TSIG_SIGN_T`/`TSIG_DKG_T`=51, `TSIG_SIGN_N`/`TSIG_DKG_N`=100. Recomputed `over_provisioned_poll_size` assertions: `(51,100)→57` (6-seat margin) and cap case `(51,55)→55`. Doc cross-references fixed in `inproc_sign.rs`, `dkg_small.rs`, `common/mod.rs`, `chain_backend_conformance.rs`, `regtest_fixture.rs`. `#[ignore]` strings corrected from the false "multi-CPU-hour / nightly" language to the real n=100 on-demand cost.

## Verification (Task 5) — real captured output

- `cargo build` — clean (1.98s). `cargo build --release` — clean (14.83s). `cargo clippy --lib` — clean, no warnings.
- **Fast suite — all pass:** dkg_small (2), inproc_sign (7, incl. small-n confirmed regtest key-spend), bridge_roundtrip, sign_adversarial (3), transport_stub (4), chain_backend_conformance (2), compile_fail (1).
- **Full-scale 51/100 tests — both PASS in release:**

| Test | Result | Test-body wall-clock | Detail |
|------|--------|----------------------|--------|
| `inproc_sign_100::…_51_of_100` | PASS | **9.90s** (30s incl. release compile of the test binary + regtest node spawn) | crown-jewel: n=100 DKG → P2TR → fund → sign → aggregate-with-tweak → verify vs Q → broadcast → CONFIRMED on regtest |
| `dkg_100_correctness::dkg_100_all_shares_verify_to_one_group_key` | PASS | **4.41s** (11s incl. compile) | KEY-06: 100 KeyPackages verify to ONE group key; part1=175ms, part2=98ms, part3=4.13s (11 workers); RSS 16.8 MiB; O(n²): 99 peer pkgs/seat, round-3 verify O(t=51) |

## #[ignore] Decision — KEPT on both (justified)

Both tests complete far under the ~90s release threshold (9.90s and 4.41s), which confirms n=100 is emphatically **not** the multi-CPU-hour job the old n=1000 was — so the false cost language is corrected. I nonetheless **kept `#[ignore]` on both** because:
1. The default `cargo test` PR gate runs in **debug**, where the n=100 `k256`/frost crypto is materially slower than the release figures above (release part3 alone is 4.13s optimized). Removing `#[ignore]` would run them in debug and bloat the quick gate.
2. `inproc_sign_100` additionally spawns a real regtest `bitcoind` node.
3. The crown-jewel pipeline is **already covered in the PR gate at small n** (`inproc_sign_confirmed_regtest_key_spend_small_n`, passing); the `_100` variants are the full-scale acceptance/instrumentation deliverables, correctly run on demand via `--release --ignored`.

The corrected `#[ignore]` strings now read "on-demand: full-scale n=100 …" rather than "multi-CPU-hour / nightly".

## Notes for Human Review

- **itg directory NOT renamed (intentional):** `.planning/quick/260713-itg-massively-speed-up-the-in-process-n-1000/` keeps its slug — it is a GSD state identifier referenced by STATE.md. Only prose *inside* its `.md` files was rescaled to n=100. The STATE.md link text/path `…in-process-n-1000` is deliberately preserved (audit survivor, legitimate).
- **bridge_roundtrip `Some(501)`→`Some(51)` (orchestrator override applied):** `PublicKeyPackage::new(shares, vk, Some(51))` at `tests/bridge_roundtrip.rs:125`. That argument is FROST `min_signers` (= threshold t), not a KAT byte value; the code comment confirms address derivation reads only the group verifying key, so the KAT (hardcoded bc1p… address / pubkey / hash bytes) is unchanged and the round-trip still passes. No other `Some(501)`/`Some(1000)`/`min_signers` references existed.
- **`inproc_sign_100.rs` rename recorded as add/delete, not rename:** `git mv` was used as instructed, but the 41-line file was then fully rewritten (new doc block + `#[ignore]` language), dropping blob similarity below git's 50% rename threshold at commit time. `dkg_100_correctness.rs` retained rename detection (R089). History-tracing via `--follow` may not cross the inproc rename; cosmetic only.
- **Historical measurement narrative left as-is (flagged):** `01-02-SUMMARY.md:182` retains empirical wall-clock narrative from the *old* full-scale run ("~13 CPU-hours; ~70 min wall … n=150/300/500 completing runs"). The parameter tokens there were rescaled (t=51, n=100, 10⁴), but the CPU-hour / core-count / intermediate-scale numbers are historical measurements, not FROST parameters — rewriting them would fabricate benchmarks. Left for human review; the narrative's premise (compute intractability) no longer applies at n=100 (Task 5 measured 4.4s).
- **`01-RESEARCH.md:532`** derived memory/op figures were rescaled by formula (t×33B pkg size → ~1.7 KB/pkg ≈ 170 KB for 100; O(n²·t) ≈ 5×10⁵), since those are deterministic derivations from t and n, not empirical measurements.

## Deliberately-kept 501/1000 survivors (final audit)

Every remaining `\b(501|1000)\b` hit across src/, tests/, and touched docs is a legitimate non-membership number:

| Location | Literal | Reason kept |
|----------|---------|-------------|
| `src/chain/core_rpc.rs:100` | `to_sat() / 1000` | Fee math: sat/kvB → sat/vB (millisat/kilo), not membership |
| `.planning/phases/01-.../01-REVIEW.md:97,98,101,110,111,114` | `/ 1000`, `1000 sat/kvB`, `* 1000 / 4`, `(… + 999) / 1000` | Fee-math discussion of the WR-01 finding — same `/1000` conversion |
| `.planning/STATE.md:97` | `…in-process-n-1000` (link text + path) | itg quick-task directory slug (GSD state identifier) — must not be renamed |

No changes to `Cargo.toml` / `Cargo.lock` / toolchain; no new dependencies.

## Self-Check: PASSED

- `tests/inproc_sign_100.rs` — FOUND; `tests/inproc_sign_1000.rs` — removed.
- `tests/dkg_100_correctness.rs` — FOUND; `tests/dkg_1000_correctness.rs` — removed.
- Commits `8ecd8dd`, `0791214`, `625125d`, `07a0f25` — all present in git history.
