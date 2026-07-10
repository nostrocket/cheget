---
gsd_state_version: '1.0'  # placeholder; syncStateFrontmatter overwrites on first state.* call
status: planning
progress:
  total_phases: 7
  completed_phases: 0
  total_plans: 25
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-10)

**Core value:** A group of 1000 can jointly control one Bitcoin address (any 501 can spend, no individual holds the key), rotate membership with zero on-chain cost, and truly revoke past compromise by sweeping to a standby key.
**Current focus:** Phase 1 — Crypto Bridge & In-Process Signing

## Current Position

Phase: 1 of 7 (Crypto Bridge & In-Process Signing)
Plan: 0 of 5 in current phase
Status: Ready to plan
Last activity: 2026-07-10 — Roadmap reordered: prove the whole system LOCALLY first (in-process + in-memory transport stub), real transport LAST. Now 7 phases, 43 v1 requirements mapped

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**
- Total plans completed: 0
- Average duration: - min
- Total execution time: 0.0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| - | - | - | - |

**Recent Trend:**
- Last 5 plans: -
- Trend: -

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Init]: Roadmap covers all of M1–M5 (full spec end-to-end)
- [Init]: DKG is the only keygen path; dealer mode dropped — Phase 1 keygen is in-process DKG with simulated participants
- [Init]: Crypto bridge proven early via in-process DKG before n=1000 transport (Phase 1 = bridge + regtest key-spend, zero transport)
- [Revision]: Prove the entire system LOCALLY first, real transport LAST. The `Transport` trait + in-memory stub (introduced Phase 1) let every ceremony phase (3–6) run with zero relay code; Phase 7 swaps in real `FileTransport`/`NostrTransport` behind the same trait and re-runs at scale
- [Revision]: Local DKG-at-scale compute proof (KEY-06, Phase 3, n=1000 in-process) is separated from the transport-layer relay load test (TRAN-08, Phase 7)
- [Revision]: SEC-03 narrowed to locally-verifiable adversarial tests (mixed-epoch, nonce-reuse-won't-compile) in hardening (Phase 6); new SEC-05 (malicious-relay DoS, replayed-envelope rejection) lives in the final transport phase (Phase 7)

### Pending Todos

None yet.

### Blockers/Concerns

- [Roadmap]: Four controls MUST be structural from Phase 1, not retrofitted — non-serializable nonce type (SIGN-05), byte-level bridge round-trip (KEY-03), tweak/aggregate verified against Q (SIGN-03/04), display-before-sign sighash recompute (SIGN-07)
- [Roadmap]: The `Transport` trait + in-memory stub is the load-bearing architectural seam — it MUST be introduced in Phase 1 so DKG-at-scale (Phase 3), rotation (Phase 4), lifecycle (Phase 5), and hardening (Phase 6) all validate locally with zero relay code
- [Roadmap]: n=1000 O(n²) DKG over Nostr (TRAN-08) is the highest project unknown — Phase 7 flagged for deeper research on strfry tuning + round-2 pacing; the load test is a gating deliverable, not optional. Keep `FileTransport` + schema before `NostrTransport` within Phase 7
- [Roadmap]: KEY-06 (local n=1000 DKG) de-risks the O(n²) compute cost in Phase 3 before any relay code, isolating compute-scaling from transport-scaling

## Deferred Items

Items acknowledged and carried forward from previous milestone close:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| *(none)* | | | |

## Session Continuity

Last session: 2026-07-10
Stopped at: Major roadmap reorder — local-first / transport-last, 7 phases; ROADMAP.md, STATE.md, and REQUIREMENTS.md traceability updated (43/43 mapped)
Resume file: None
