---
status: testing
phase: 03-dkg-at-scale-local
source: [03-VERIFICATION.md]
started: 2026-07-16T14:35:00Z
updated: 2026-07-16T14:35:00Z
---

## Current Test

number: 1
name: Confirmed regtest key-spend from PERSISTED shares
expected: |
  On a host with bitcoind/bitcoin-cli on PATH, load_persisted_shares assembles t=51
  shares from disk, the SigningSession broadcasts the key-spend, and confirmation
  depth >= 6 on regtest. This is the phase acceptance bar (ROADMAP criterion 2).
awaiting: user response

## Tests

### Test 1 — Confirmed regtest key-spend from persisted shares

- status: pending
- requirement: KEY-06
- why_human: The confirmed on-chain key-spend is a runtime behavior requiring a live
  regtest node; bitcoind is not installed in the verification sandbox. The
  read/assemble seam and write/reload seam ARE proven behaviorally; only the final
  on-chain confirmation is unexercised.
- how_to_run: |
    On a host with bitcoind/bitcoin-cli on PATH:
      cargo test --test persisted_sign persisted_sign_confirmed_regtest_key_spend_small_n -- --test-threads=1
    Optionally the full-100 functional smoke:
      cargo test --release --test persisted_sign -- --ignored
- expected: Test passes — confirmed key-spend from persisted shares, confirmation depth >= 6.

### Test 2 — Interactive keygen confirm-twice UX (also closes Phase 2 UAT Test 1)

- status: pending
- requirement: KEY-06 (also unblocks Phase 02 UAT Test 1)
- why_human: The `InteractivePassphrase::for_new_store` prompt is `#[cfg(not(test))]`
  and cannot run under automated tests; the confirm-twice / no-echo UX needs a human
  at a terminal.
- how_to_run: |
    At a terminal:
      cheget participant keygen --persist --base <tmp-dir>
- expected: |
    The confirm-twice, no-echo passphrase prompt appears exactly once; the passphrase
    is never echoed; 100 per-seat encrypted store roots are created under <tmp-dir>.
    This is the first command that drives for_new_store, so it also re-confirms the
    Phase 2 store-creation UAT (Test 1), previously BLOCKED for lack of a CLI entry point.

## Gaps

_None recorded — both items are environment/human-terminal limits, not code defects._
