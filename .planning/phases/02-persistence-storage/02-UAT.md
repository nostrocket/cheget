---
status: complete
phase: 02-persistence-storage
source: [02-VERIFICATION.md]
started: 2026-07-14T13:15:32Z
updated: 2026-07-14T13:20:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Interactive no-echo passphrase prompt on a fresh store
expected: |
  Run `cheget` on a fresh store at a real terminal, triggering the interactive
  new-store passphrase prompt (InteractivePassphrase::for_new_store). The
  passphrase is NOT echoed while typing; you are prompted twice and a mismatch
  is rejected; the "a lost passphrase makes them unrecoverable — there is no
  reset" warning prints before the prompts.
why_human: |
  The InteractivePassphrase impl is #[cfg(not(test))] — it cannot be linked or
  driven in any test build. No-echo terminal behavior and the confirm-twice UX
  are runtime TTY properties grep and headless tests cannot observe (D5
  human_judgment, 02-01).
result: pass

### 2. WR-01 decision — un-zeroized rpassword String
expected: |
  Decide whether the un-zeroized rpassword `String` in
  src/store/passphrase.rs:83,92-97 is acceptable, or must be wrapped in
  Zeroizing before the phase is closed. Record either: fix (wrap the rpassword
  reads in Zeroizing so the transient plaintext buffer is wiped on drop) OR
  accept-and-document (drop the "never lands in a plain String" claim in the
  module doc). This passphrase unlocks the identity key AND every share.
why_human: |
  Security memory-hygiene judgment call on cfg-gated production code that
  contradicts the module's own stated invariant. Not a must-have failure, but a
  security policy decision the maintainer should make.
result: issue
reported: "Decision: fix — wrap the rpassword reads in Zeroizing so the transient plaintext buffer is wiped on drop."
severity: minor

### 3. WR-03 decision — create_dir_secure perms on a pre-existing directory
expected: |
  Decide whether create_dir_secure (src/store/atomic.rs:43-56) must tighten
  permissions on a pre-existing store directory. Record either: enforce 0700 on
  an existing dir on Unix, OR accept the metadata-listing exposure (files stay
  0600 regardless; only directory listings leak if ~/.cheget pre-exists with
  loose perms).
why_human: |
  Threat-model judgment on directory-metadata exposure. Freshly-created dirs are
  correctly 0700 (verified by the perms test); only a pre-existing loosely-permed
  store root is at issue.
result: pass
note: "Decision: accept — files stay 0600 regardless; only a directory listing leaks if ~/.cheget pre-exists with loose perms. Accepted risk, no code change."

## Summary

total: 3
passed: 2
issues: 1
pending: 0
skipped: 0
blocked: 0

## Gaps

- truth: "The passphrase never lands in a plain, un-zeroized String (per passphrase.rs module doc)."
  status: failed
  reason: "User decided: fix — wrap the rpassword reads (src/store/passphrase.rs:83,92-97) in Zeroizing so the transient plaintext buffer is wiped on drop."
  severity: minor
  test: 2
  root_cause: ""
  artifacts:
    - path: "src/store/passphrase.rs"
      issue: "rpassword read into un-zeroized String at lines 83, 92-97"
  missing:
    - "Wrap rpassword passphrase reads in Zeroizing<String>"
  debug_session: ""
