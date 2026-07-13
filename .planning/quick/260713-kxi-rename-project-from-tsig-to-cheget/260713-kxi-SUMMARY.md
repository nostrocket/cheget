---
phase: quick-260713-kxi
plan: 01
subsystem: project-identity
tags: [rename, refactor, docs, cargo, cli]
status: complete
requires: []
provides:
  - "crate/binary/lib name `cheget`"
  - "CHEGET_* override env-var interface"
affects:
  - Cargo.toml
  - Cargo.lock
  - src/
  - tests/
  - .planning/ history
tech-stack:
  added: []
  patterns: []
key-files:
  created: []
  modified:
    - Cargo.toml
    - Cargo.lock
    - src/main.rs
    - src/lib.rs
    - src/cli/mod.rs
    - src/cli/keygen.rs
    - src/crypto/nonce.rs
    - tests/*.rs
    - tests/common/mod.rs
    - tests/ui/nonce_no_serialize.rs
    - tests/ui/nonce_no_serialize.stderr
    - README.md
    - SPEC-frost-cli.md
    - .claude/CLAUDE.md
    - .planning/PROJECT.md
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
    - .planning/research/*.md
    - .planning/phases/01-crypto-bridge-in-process-signing/*.md
    - .planning/quick/{itg,jqs,kgv}/*.md
decisions:
  - "STATE.md rename left unstaged for the orchestrator docs commit (workflow-owned artifact)"
  - "Task-dir slug 260713-kxi-rename-project-from-tsig-to-cheget NOT renamed (content-only task; slug flagged)"
metrics:
  tasks_completed: 2
  files_modified: 48
  completed: 2026-07-13
---

# Quick Task 260713-kxi: Rename project tsig → cheget Summary

Mechanical, decision-free rename of the crate/binary/lib name, the override
env-var prefix, and all prose (including completed `.planning/` history) from
`tsig` to `cheget`; the project builds, tests, and lints clean under the new
name and produces a `cheget` binary.

## What Was Done

### Task 1 — crate/binary identity, crate-path imports, CLI name, env interface (commit b0b0adf)

- `Cargo.toml`: `[package]`/`[[bin]]`/`[lib]` `name = "cheget"`. Path, version,
  edition, rust-version, deps, dev-deps and the description string left intact.
- `Cargo.lock`: regenerated via `cargo build`; only the local package entry
  changed (`name = "tsig"` → `name = "cheget"` at line 301). No dependency
  entries or hashes were hand-edited.
- Crate-path imports: every `use tsig::…` / bare `tsig::…` in `src/main.rs` and
  all `tests/*.rs` + `tests/common/mod.rs` → `cheget::`.
- CLI: `#[command(name = "cheget", …)]` in `src/cli/mod.rs`; the `eprintln!`
  error prefix in `src/main.rs` → `cheget: error:`.
- Env override interface: `TSIG_SIGN_T/N` and `TSIG_DKG_T/N` → `CHEGET_SIGN_T/N`
  and `CHEGET_DKG_T/N` at the `std::env::var`/`env_u16` call sites
  (`tests/inproc_sign_100.rs`, `tests/dkg_100_correctness.rs`) and their `//!`
  doc-comments.
- trybuild fixture: `tests/ui/nonce_no_serialize.rs` →
  `cheget::crypto::EphemeralNonces`; `tests/ui/nonce_no_serialize.stderr`
  regenerated with `TRYBUILD=overwrite cargo test --test compile_fail`, then
  re-run WITHOUT the env — passes.
- Doc-comment prose in the touched source files updated to `cheget`.

### Task 2 — remaining prose across live docs, SPEC, README, planning history (commit f1c14c1)

- Rewrote `README.md`, `SPEC-frost-cli.md`, `.claude/CLAUDE.md`,
  `.planning/PROJECT.md`, `REQUIREMENTS.md`, `ROADMAP.md`, `research/*.md`, all
  phase-01 artifacts, and the itg/jqs/kgv quick-task docs.
- `TSIG_*` env-var names in prose → `CHEGET_*`.
- `.planning/STATE.md` left unstaged (its rename is carried into the
  orchestrator's docs commit); the `260713-kxi` task directory is untracked and
  owned by the orchestrator.

## Verification

- `cargo build` clean; `target/debug/cheget` present and executable.
- `cargo build --release` clean; `cargo clippy --lib` clean.
- `cargo run -- --help` → `Usage: cheget <COMMAND>` and the
  `51-of-100 FROST Taproot signing CLI …` about line.
- Fast suite passes: `compile_fail` (1, regenerated .stderr), `dkg_small` (2),
  `inproc_sign` (7), `bridge_roundtrip` (3), `sign_adversarial` (3),
  `transport_stub` (4), `chain_backend_conformance` (2).
- Negative greps under `src/`/`tests/`: no `TSIG_`, no `tsig::`.

### Final audit (authoritative — tracked files)

- `git grep -in 'tsig' -- ':!Cargo.lock' | grep -vi scriptsig` → **empty (CLEAN)**.
- `git grep -n 'TSIG_'` → **empty (CLEAN)**.
- `Cargo.lock` local package = `cheget` (only line 301); no `tsig` remains.

## Flagged Items

- **Task-directory slug contains `tsig`:**
  `.planning/quick/260713-kxi-rename-project-from-tsig-to-cheget/`. Per the task
  constraints this is a content-only rename — the GSD task-dir slug was NOT
  renamed. It is currently untracked (orchestrator-owned) so it does not appear
  in the tracked-file audit. The slug legitimately names the rename task.
- **This task's own PLAN.md (`260713-kxi-PLAN.md`)** necessarily contains the
  string `tsig`/`TSIG_` because it describes the rename. It is untracked and
  excluded from the `git grep` audit; when the orchestrator commits docs it is
  an expected, documented survivor (a plan describing a `tsig`→`cheget` rename
  must reference the old name).

## Deviations from Plan

None — plan executed exactly as written. (The bulk in-place edits were applied
one-file-per-`perl` invocation after multi-file invocations misbehaved under the
sandbox; this is a mechanical detail, not a scope/content deviation.)

## Known Stubs

None.

## Self-Check: PASSED

- `Cargo.toml` name = cheget — FOUND (3 entries)
- `Cargo.lock` name = "cheget" — FOUND (line 301)
- `target/debug/cheget` — FOUND (executable)
- `tests/ui/nonce_no_serialize.stderr` references `cheget::` — FOUND
- Commit b0b0adf (Task 1) — present
- Commit f1c14c1 (Task 2) — present
- `git grep -in tsig` (minus Cargo.lock/scriptSig) — CLEAN
- `git grep -n TSIG_` — CLEAN
