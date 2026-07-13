---
phase: quick
plan: 260713-kgv
subsystem: docs
status: complete
tags: [readme, documentation]
requires: []
provides: [README.md]
affects: []
tech-stack:
  added: []
  patterns: []
key-files:
  created:
    - README.md
  modified: []
decisions:
  - "README written fresh (not copied from Cargo.toml description); fixed threshold stated as 51-of-100 (t=51/n=100) exclusively"
  - "Phase 1 crypto core described in present tense; transport/persistence/rotation/lifecycle/Nostr marked as planned (Phases 2-7)"
  - "No install/distribution claims beyond cargo build/test; local timings attributed as measurements, not benchmarks"
metrics:
  duration: 3 min
  completed: 2026-07-13
---

# Quick Task 260713-kgv: Add a README Summary

Added a repo-root `README.md` for `tsig` that gives an accurate, non-overclaiming
orientation — what the project is (a 51-of-100 FROST Taproot signing CLI), what is
actually implemented today (the Phase 1 in-process crypto core), and what is planned
but not yet built (transport, persistence, rotation, lifecycle, Nostr).

## What Was Built

**Task 1 — Write repo-root README.md** (commit `8858b36`)

Created `README.md` with the eight sections specified in the plan:

1. Title + one-line description at the fixed 51-of-100 threshold.
2. "Core value" framed as the project goal (any 51 spend / no individual holds the
   key / rotation with zero on-chain footprint / sweep revocation), immediately
   followed by a status caveat that only the crypto core is built.
3. "Status: what works today" — a table of the Phase 1 DONE items (bridge,
   in-process DKG, in-process tweaked signing to a confirmed regtest key-spend,
   `Transport` stub, `ChainBackend` trait), each with source module and pinning
   test, plus an honest CLI-status note (only `watcher address` wired end-to-end)
   and the structural security controls.
4. "Planned (not yet built)" — Phases 2-7 marked clearly as future, stating plainly
   that no transport/relay/Nostr code and no persistence layer exist yet.
5. "Architecture" — the layered module map from `src/lib.rs` and the load-bearing
   `Transport` trait seam idea.
6. "Building & testing" — `cargo build --release` / `cargo test`, the `#[ignore]`
   crown-jewel test name, MSRV 1.85, committed `Cargo.lock`; no invented install steps.
7. "Security model (current)" — only properties the code enforces today.
8. "License" — MIT OR Apache-2.0.

## Verification

- `test -f README.md` — passes.
- `grep -q "51-of-100" README.md` — passes.
- No `501` / `1000` threshold references (stricter grep for `501`/`1000` anywhere) — passes.
- No `cargo install tsig` / `crates.io` / homebrew claims — passes.
- `cargo build` — succeeds (README add did not break the build).

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- README.md exists: FOUND (`/Users/g/git/threshold/research/README.md`)
- Commit exists: FOUND (`8858b36`)
