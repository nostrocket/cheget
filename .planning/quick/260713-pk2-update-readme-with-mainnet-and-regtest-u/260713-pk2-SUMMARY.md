---
phase: quick-260713-pk2
plan: 01
subsystem: docs
status: complete
tags: [readme, documentation, cli, regtest, mainnet]
requires: []
provides: [README.md]
affects: [README.md]
tech-stack:
  added: []
  patterns: []
key-files:
  created: []
  modified:
    - README.md
decisions:
  - "Documented keygen/sign as in-process simulate-all-seats (no transport, no persistence), correcting the stale 'test-only' claim."
  - "Mainnet section documents ONLY offline address derivation; full signing/custody explicitly flagged as not-yet-wired (Phases 2-7)."
  - "Full-scale tests documented as running by default (not #[ignore]), release recommended (~9s vs ~90s local)."
metrics:
  duration_min: 6
  tasks: 2
  files: 1
  completed: "2026-07-13"
---

# Quick Task 260713-pk2: Update README with mainnet and regtest usage — Summary

Rewrote the repo-root `README.md` to accurately reflect the current `cheget`
Phase-1 CLI surface, adding grounded "How to use on regtest" and "How to use on
mainnet" sections. Every documented command was run against the freshly built
binary before being written into the README.

## What changed

- **CLI-status correction.** The README previously claimed only `watcher address`
  was wired and that `keygen`/`sign` were test-only. Updated to document that
  `participant keygen` / `coordinator keygen` and `participant sign` /
  `coordinator sign` are now real CLI commands — while being precise that they run
  an **in-process simulate-all-seats DKG** (no transport, no persistence): keygen
  writes only the public `PublicKeyPackage` envelope; sign is a self-contained
  demonstration of the two-round pipeline over a supplied PSBT that must spend the
  in-process-derived address.
- **"How to use on regtest"** (new section): the end-to-end confirmed key-spend via
  the `inproc_sign_100` test harness (auto-spawns a throwaway `bitcoind` via
  `corepc-node`, no manual node), offline regtest address derivation
  (`--network regtest` → `bcrt1p…`), and the in-process `sign` pipeline demo.
- **"How to use on mainnet"** (new section): documents ONLY offline mainnet P2TR
  address derivation (`--network bitcoin` → `bc1p…`) as usable today, with a
  prominent safety/status note that the full mainnet signing/custody ceremony
  (multi-party rounds, PSBT signing, broadcast, watch, sweep) is NOT wired and
  arrives in later phases — Phase 1 only, not production-audited, do not entrust
  real funds to flows that do not exist.
- **Test-suite correction.** Removed the false `#[ignore]` / `--ignored` claim.
  Documented that both full-scale tests (`inproc_sign_100`, `dkg_100_correctness`)
  run by default under `cargo test`, release recommended (~9s vs ~90s local
  measurements), with `CHEGET_SIGN_T`/`CHEGET_SIGN_N` and `CHEGET_DKG_T`/`CHEGET_DKG_N`
  override vars.
- **Currency refresh.** Name `cheget` throughout, fixed threshold 51-of-100, no
  `tsig`, no `501`/`1000`, no `cargo install`/crates.io/homebrew (build from source).

## CLI surface verified by running the binary

All captured from a fresh `cargo build` (`target/debug/cheget`):

- `cheget --help`, `--version` (0.1.0) — three personas: participant, coordinator, watcher.
- `participant/coordinator/watcher --help` — subcommand lists.
- `watcher address --help` — flags `--pubkey <FILE>` (required), `--network` with
  exactly `bitcoin` (default) | `testnet` | `signet` | `regtest`.
- `participant keygen --help` / `coordinator keygen --help` — shared arg struct:
  `--ceremony --seats --threshold --full --key-id (default active) --out (required)`.
- `participant sign --help` — shared arg struct: `--session --psbt --key --seats
  --threshold --full --network (default regtest) --yes`.
- End-to-end: `coordinator keygen --key-id readme-demo --out pk.json` wrote a JSON
  envelope (`key_id`, `epoch`, `pubkey_package_hex`); `watcher address` on that file
  produced a real `bcrt1p…` (regtest) and `bc1p…` (mainnet/default) address.

## Verification gates (all pass)

- `grep -in 'tsig' README.md` → empty.
- `grep -nE '\b(501|1000)\b' README.md` → empty. Remaining `51`/`100` hits are all
  legitimate threshold references (`51-of-100`, `t=51`, `n=100`, `3-of-5`).
- Fenced code blocks balanced (even fence count).
- Contains both `--network regtest` and `--network bitcoin`.
- Contains `CHEGET_SIGN_T` and `CHEGET_DKG_T`.
- No `cargo install` / `crates.io` / `homebrew`.
- `cargo build` succeeds after the change.

## Deviations from Plan

None — plan executed exactly as written.

## Commits

- `df44fbb`: docs(260713-pk2): update README with mainnet and regtest usage

## Self-Check: PASSED

- README.md exists and is committed at `df44fbb` (verified: `git show --stat df44fbb`
  shows `README.md | 149 insertions, 20 deletions`, 1 file changed).
- Commit staged README.md only; unrelated untracked files (`.DS_Store`,
  `implementations-resharing.md`, `schemes/`) were NOT committed.
