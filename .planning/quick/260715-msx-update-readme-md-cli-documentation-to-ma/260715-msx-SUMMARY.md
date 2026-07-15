---
phase: 260715-msx
plan: 01
subsystem: docs
status: complete
tags: [readme, cli-docs, phase-2-status]
requires: []
provides:
  - README CLI surface documents all 7 leaf subcommands (incl. share-status, roster)
  - README status framing reflects Phase 2 (storage layer) as shipped
affects:
  - README.md
tech-stack:
  added: []
  patterns: []
key-files:
  created: []
  modified:
    - README.md
decisions:
  - "Documented number of CLI commands as 'seven subcommands' (7 leaf commands), matching the plan success criteria, while noting participant/coordinator keygen+sign share one handler."
  - "Attributed the persisted-key custody signing flow to 'later-phase work', NOT Phase 2 — Phase 2 shipped only the storage layer, to avoid overstating."
metrics:
  duration: ~10 min
  completed: 2026-07-15
  tasks: 2
  files: 1
---

# Phase 260715-msx Plan 01: Update README CLI Documentation Summary

Updated README.md so its CLI documentation matches the release binary's actual command/flag surface and corrected the stale "Phase-1-only / no persistence" framing to reflect that Phase 2 (persistence & storage layer) has shipped — without overstating, since no CLI flow yet persists or consumes a secret share.

## What Changed

**Task 1 — CLI surface section:**
- Added `participant share-status` and `coordinator roster` leaves to the clap persona tree with read-only glosses.
- Added a usage bullet for each: `share-status [--home <PATH>]` (no decryption, never prompts for a passphrase, D-05; notes the manifest is not populated by current keygen/sign flows) and `roster [--key-id <ID>] [--home <PATH>]` (reads the coordinator's public SQLite store, STOR-03).
- Corrected the stale "Three commands run end-to-end" sentence to "Seven subcommands run end-to-end", enumerating the leaves and noting participant/coordinator keygen+sign share one handler.

**Task 2 — status framing:**
- Status banner: now states the crypto core (Phase 1) AND the Phase 2 persistence/storage layer are implemented (age/scrypt participant store, encrypted between-round checkpointing, coordinator SQLite store, two read-only inspectors), while keeping the honest caveats (no networking/transport; no CLI flow persists/consumes a secret share).
- "Core value" closing sentence, "Status: what works today" heading (now "Phases 1–2") and intro, mainnet safety blockquote, "Planned" lead-in, and "Security model (current)" closing paragraph all softened from "no persistence / Phase 1 only" to "storage layer exists but not yet wired into a fund-custody signing flow".
- Added a dedicated "Phase 2 — persistence & storage layer (shipped, not yet wired into custody)" paragraph in the what-works area.
- Removed the "Phase 2 — Persistence & storage" bullet from the "Planned (not yet built)" list and removed the "no persistence layer yet" lead-in clause. Phases 3–7 preserved.
- Fixed a would-be contradiction: the `sign` bullet previously said the persisted-key wallet flow "arrives in Phase 2"; changed to attribute it to later-phase work (Phase 2 shipped only the storage layer).

## Mandatory Verification (against the release binary)

Every documented command/flag was re-verified against `./target/release/cheget <...> --help` before writing. All documented flags appear in `--help` output. No invented flags.

Verified leaf surface (7 subcommands):
- `participant keygen --out <OUT> [--ceremony] [--seats] [--threshold] [--full] [--key-id (default active)]`
- `participant sign [--session] [--psbt] [--key (default active)] [--seats] [--threshold] [--full] [--network (default regtest)] [--yes]`
- `participant share-status [--home <PATH>]`
- `coordinator keygen ...` (identical to participant keygen)
- `coordinator sign ...` (identical to participant sign)
- `coordinator roster [--key-id (default active, active|standby)] [--home <PATH>]`
- `watcher address --pubkey <FILE> [--network (default bitcoin)]`

Discrepancies from plan: none. The plan's claimed surface matched the binary exactly. (Note: `sign` also exposes a `--key` flag which the README does not document — omission is allowed and pre-existing; no invented flags were added.)

## Deviations from Plan

None — plan executed as written. One extra consistency fix was applied beyond the literal task steps: the `sign` bullet's parenthetical "(that arrives in Phase 2)" was corrected to avoid contradicting the new framing that Phase 2 shipped only the storage layer (Rule 1 — removing an internal contradiction the reframe would otherwise introduce).

## Precision Guardrails (preserved)

- keygen writes ONLY the public `PublicKeyPackage` envelope — no secret share persisted.
- sign runs an in-process simulate-all-seats DKG; does not consume a persisted secret share.
- No CLI command persists/consumes a secret share yet; `share-status` reads a manifest current flows do not populate.
- Real-funds safety warnings preserved; only mainnet-safe action remains offline address derivation.

## Verification

- Task 1 automated check: PASS (`share-status`/`roster` help succeed; both appear in README; "Three commands run end-to-end" removed).
- Task 2 automated check: PASS (no "Phase 2 — Persistence" bullet; no "no persistence layer yet"; `age/scrypt` and `simulate-all-seats` present).
- No remaining "no persistence" / "Phase 1 only" / "lives in memory" claims.
- No section contradicts another on persistence status.

## Self-Check: PASSED

- README.md modified and exists at `/Users/g/git/threshold/research/README.md`.
- Commit `b9c89b4` exists (verified via `git rev-parse --short HEAD`).
