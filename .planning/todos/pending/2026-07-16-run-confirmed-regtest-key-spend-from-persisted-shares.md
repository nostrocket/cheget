---
created: 2026-07-16T06:59:47.783Z
title: Run confirmed regtest key-spend from persisted shares
area: testing
files:
  - tests/persisted_sign.rs
  - src/cli/sign.rs
  - src/store/participant.rs
  - .planning/phases/03-dkg-at-scale-local/03-UAT.md
---

## Problem

Phase 3 verification is `human_needed` because its acceptance bar (ROADMAP
criterion 2, KEY-06) — a **confirmed regtest key-spend produced from PERSISTED
shares** — could not be exercised in the CI/dev sandbox: **bitcoind/bitcoin-cli
is not installed** there. The code is present and wired (`sign --persist` →
`load_persisted_shares` → `SigningSession` → broadcast), and the surrounding
seams are proven behaviorally (write/reload byte-equal, `load_only_active`
round-trip), but the final on-chain confirmation is the last unexercised step.
Until it passes on a real regtest node, Phase 3 stays pending.

This is an environment/tooling limitation, **not a code defect**.

## Solution

On a host with `bitcoind`/`bitcoin-cli` on PATH:

```
cargo test --test persisted_sign persisted_sign_confirmed_regtest_key_spend_small_n -- --test-threads=1
```

Optional one-time full-100 functional smoke (D-06):

```
cargo test --release --test persisted_sign -- --ignored
```

Expected: `load_persisted_shares` assembles t=51 shares from disk, the
`SigningSession` broadcasts the key-spend, and confirmation depth >= 6 on
regtest.

Note: run single-threaded (`--test-threads=1`) — scrypt `log_n=18` is expensive
(~75s/KDF) and age's anti-DoS guard ("Excessive work parameter") trips when
multiple `log_n=18` KDFs run in parallel.

After it passes, re-run `/gsd-verify-work 3` to move Phase 3 verification from
`human_needed` to `passed` (updates `03-UAT.md` Test 1 → resolved).
