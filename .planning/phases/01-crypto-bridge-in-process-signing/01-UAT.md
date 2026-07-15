---
status: complete
phase: 01-crypto-bridge-in-process-signing
source: [01-VERIFICATION.md]
started: 2026-07-10T13:12:44Z
updated: 2026-07-15T05:51:29Z
---

## Current Test

[testing complete — both full-scale tests now run by default and pass (re-verified 2026-07-15)]

## Tests

### 1. Full-scale 51-of-100 in-process confirmed regtest key-spend
expected: |
  `cargo test --release --test inproc_sign_100 -- --nocapture`
  `inproc_sign_confirmed_regtest_key_spend_51_of_100` completes: DKG over 100
  seats → group address → two-round tweaked sign at t=51 → aggregate_with_tweak(None)
  → 64-byte BIP340 sig verifies against Q → PSBT finalized → tx broadcast and confirmed.
result: pass
source: automated
note: |
  Runs by default (no longer `#[ignore]`d — un-ignored in c537bf0 after the
  260713-itg DKG speedup). Re-verified 2026-07-15: passes in 9.27s at t=51/n=100,
  confirmed regtest key-spend. No `--ignored` needed. What was originally flagged as
  a human/on-demand run is now an ordinary automated test.

### 2. Full-scale n=100 DKG correctness + O(n^2) instrumentation gate
expected: |
  `cargo test --release --test dkg_100_correctness -- --nocapture`
  `dkg_100_all_shares_verify_to_one_group_key` completes: 100 KeyPackages all
  verify to one group PublicKeyPackage; part1/part2/part3 timing and peak-RSS reported.
result: pass
source: automated
note: |
  Runs by default (no longer `#[ignore]`d). Re-verified 2026-07-15: passes in 4.31s
  at t=51/n=100 — part1=160ms, part2=99ms, part3=4.04s, peak RSS 16.9 MiB; 100
  KeyPackages verify to one group key. No `--ignored` needed. (KEY-06 maps to Phase 3;
  the gate was folded forward here per D-03.)

## Summary

total: 2
passed: 2
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

None — both tests pass by default (re-verified 2026-07-15).
