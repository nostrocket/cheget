---
status: testing
phase: 01-crypto-bridge-in-process-signing
source: [01-VERIFICATION.md]
started: 2026-07-10T13:12:44Z
updated: 2026-07-13T06:39:41Z
---

## Current Test

number: 1
name: Full-scale 51-of-100 in-process confirmed regtest key-spend
expected: |
  Run `cargo test --release --test inproc_sign_100 -- --ignored --nocapture`.
  `inproc_sign_confirmed_regtest_key_spend_51_of_100` completes: DKG over 100
  seats → group address → two-round tweaked sign at t=51 → aggregate_with_tweak(None)
  → 64-byte BIP340 sig verifies against Q → PSBT finalized → tx broadcast and
  confirmed on the auto-spawned regtest node.
awaiting: user response

## Tests

### 1. Full-scale 51-of-100 in-process confirmed regtest key-spend
expected: |
  `cargo test --release --test inproc_sign_100 -- --ignored --nocapture`
  `inproc_sign_confirmed_regtest_key_spend_51_of_100` completes: DKG over 100
  seats → group address → two-round tweaked sign at t=51 → aggregate_with_tweak(None)
  → 64-byte BIP340 sig verifies against Q → PSBT finalized → tx broadcast and confirmed.
why_human: |
  #[ignore]d off the default PR gate (which builds debug, and this test spawns a
  regtest bitcoind); the small-n key-spend already covers the pipeline per-PR. At
  n=100 this is a fast on-demand run (~10s in release) — not the former multi-CPU-hour
  n=1000 job — that observes the crown-jewel key-spend at the real 51/100 acceptance
  scale.
result: [pending]

### 2. Full-scale n=100 DKG correctness + O(n^2) instrumentation gate
expected: |
  `cargo test --release --test dkg_100_correctness -- --ignored --nocapture`
  `dkg_100_all_shares_verify_to_one_group_key` completes: 100 KeyPackages all
  verify to one group PublicKeyPackage; part1/part2/part3 timing and peak-RSS reported.
why_human: |
  #[ignore]d off the default PR gate (KEY-06/D-06); a fast on-demand run (~4s in
  release) that records the n=100 DKG correctness + O(n²) instrumentation at the real
  acceptance scale. (KEY-06 maps to Phase 3; the gate was folded forward here per D-03.)
result: [pending]

## Summary

total: 2
passed: 0
issues: 0
pending: 2
skipped: 0
blocked: 0

## Gaps
