---
phase: 02-persistence-storage
plan: 05
subsystem: infra
tags: [zeroize, rpassword, secretstring, memory-hygiene, passphrase]

# Dependency graph
requires:
  - phase: 02-persistence-storage
    provides: the passphrase seam (src/store/passphrase.rs) and store crypto layer
provides:
  - "Zeroize-wrapped transient rpassword plaintext buffers — the store passphrase never survives in an un-zeroized String (WR-01 closed)"
affects: [store, crypto, identity, keygen]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "rpassword read -> Zeroizing<String> -> SecretString: transient plaintext buffer wiped on drop, owned copy consumed directly by zeroize-on-drop SecretString"

key-files:
  created: []
  modified:
    - src/store/passphrase.rs

key-decisions:
  - "FIX (not accept-and-document) the WR-01 gap: wrap the three interactive prompt reads in Zeroizing so the code honors its own module-doc invariant."
  - "Compare dereferenced Zeroizing values (*first != *second) to preserve the confirm-twice mismatch check on the wrapped Strings."

patterns-established:
  - "Interactive secret reads land in Zeroizing<String> before being handed (by owned copy) to SecretString; no un-zeroized intermediate binding survives."

requirements-completed: [STOR-01]

coverage:
  - id: D1
    description: "All three interactive rpassword::prompt_password reads are wrapped in Zeroizing<String> so the transient plaintext buffer is zeroized on drop before conversion to SecretString"
    requirement: STOR-01
    verification:
      - kind: other
        ref: "grep -c 'Zeroizing::new(rpassword::prompt_password' src/store/passphrase.rs == 3 && grep -q 'use zeroize::Zeroizing;'"
        status: pass
      - kind: unit
        ref: "src/store/passphrase.rs#in_code_source_drives_roundtrip_headlessly (cargo test --lib store::passphrase)"
        status: pass
      - kind: integration
        ref: "cargo build"
        status: pass
  - id: D2
    description: "Confirm-twice UX preserved: new-store path prompts twice, rejects mismatch, prints unrecoverability warning; unlock path prompts once"
    requirement: STOR-01
    verification: []
    human_judgment: true
    rationale: "The interactive terminal-prompt behavior is #[cfg(not(test))] and cannot be exercised by the headless test suite; the mismatch/warning/confirm UX flow requires a human at a TTY to fully verify. Code review confirms the branch logic, warning text, and mismatch error are byte-for-byte unchanged."

# Metrics
duration: 2 min
completed: 2026-07-14
status: complete
---

# Phase 2 Plan 05: Zeroize Transient Passphrase Buffers Summary

**All three interactive rpassword reads in the store passphrase seam now land in `Zeroizing<String>` and are wiped on drop, closing WR-01 so the code honors its own "never lands in a plain String" module-doc invariant.**

## Performance

- **Duration:** ~2 min
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Added `use zeroize::Zeroizing;` to `src/store/passphrase.rs`, mirroring the established store-layer pattern (identity.rs, crypto.rs, participant.rs, checkpoint.rs).
- Wrapped the unlock-path read in `Zeroizing::new(...)`, returning `SecretString::from(entered.as_str().to_owned())` so no un-zeroized binding survives.
- Wrapped both confirm-twice reads (`first`, `second`) in `Zeroizing::new(...)`, compared dereferenced values (`*first != *second`), and returned `SecretString::from(first.as_str().to_owned())` on match.
- Preserved the confirm flag semantics, unrecoverability warning text, and mismatch error exactly.

## Task Commits

Each task was committed atomically:

1. **Task 1: Zeroize the transient rpassword plaintext buffers** - `c2c3a83` (fix)

## Files Created/Modified
- `src/store/passphrase.rs` - Wrapped the three transient `rpassword::prompt_password` reads in `Zeroizing<String>`; added the `zeroize::Zeroizing` import.

## Decisions Made
- FIX rather than accept-and-document: the maintainer chose to make the code truthful to its invariant. This passphrase unlocks the identity key AND every share (D-02), so the memory hygiene is load-bearing.
- Used `.as_str().to_owned()` to hand an owned copy to the zeroize-on-drop `SecretString` while the `Zeroizing` wrapper wipes the transient buffer — no un-zeroized intermediate binding.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Verification Results
- `grep -c 'Zeroizing::new(rpassword::prompt_password' src/store/passphrase.rs` == 3 — PASS
- `use zeroize::Zeroizing;` present — PASS
- `cargo build` compiles cleanly — PASS
- `cargo test --lib store::passphrase` — 1 passed, 0 failed — PASS

## Next Phase Readiness
- WR-01 gap closed. The store passphrase seam no longer leaves an un-zeroized plaintext String in memory.
- No blockers.

## Self-Check: PASSED
- `src/store/passphrase.rs` exists on disk — FOUND
- Commit `c2c3a83` present in git log — FOUND

---
*Phase: 02-persistence-storage*
*Completed: 2026-07-14*
