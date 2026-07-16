---
created: 2026-07-16T06:59:47.783Z
title: Verify interactive confirm-twice keygen UX
area: testing
files:
  - src/store/passphrase.rs
  - src/cli/keygen.rs
  - .planning/phases/03-dkg-at-scale-local/03-UAT.md
---

## Problem

The `InteractivePassphrase::for_new_store` confirm-twice, no-echo passphrase
prompt is `#[cfg(not(test))]` and cannot be exercised by automated tests — it
needs a human at a terminal. Phase 3's `keygen --persist` is the first command
that drives this path, so a manual run both verifies the new-store UX and
**closes the previously-BLOCKED Phase 02 UAT Test 1** (which was blocked only
because no command existed to create an encrypted store).

This is a human-terminal verification, **not a code defect**.

## Solution

At an interactive terminal:

```
cheget participant keygen --persist --base <tmp-dir>
```

Expected:
- The confirm-twice, no-echo passphrase prompt appears **exactly once**.
- The passphrase is never echoed to the terminal.
- 100 per-seat encrypted store roots (`seat-NNNN`) are created under `<tmp-dir>`,
  each with its `KeyPackage` age/scrypt-encrypted plus the plaintext group
  package.

After confirming, re-run `/gsd-verify-work 3` (Test 2 → resolved) and
`/gsd-verify-work 02` to close the Phase 02 store-creation UAT Test 1.
