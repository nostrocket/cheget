# Phase 3: DKG at Scale — Local - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-15
**Phase:** 03-dkg-at-scale-local
**Areas discussed:** Phase scope / "done" bar, Measurement artifact, Persist/reload rigor, CLI vs test-only boundary

---

## Opening frame (from codebase scout)

Scouting established that Phase 3's original scope was largely pre-built: `run_inprocess_dkg`
(rayon-parallel) + `tests/dkg_100_correctness.rs` satisfy criteria 1 & 2 by default at
t=51/n=100; quick task `260713-jqs` already recorded the timings; `store_checkpoint_n100.rs`
exists but is `#[ignore]`d. KEY-06 was already marked Complete. The user's guidance progressively
reframed the phase away from "prove scale" toward "wire the proven crypto to the proven store."

---

## Phase scope / "done" bar

| Option | Description | Selected |
|--------|-------------|----------|
| Run-measure-document only | Accept existing tests for criteria 1&2; run the ignored persist/reload; record results; close. Minimal new code. | |
| Also harden the proof | Same + first-class committed benchmark + strengthen criterion 3 to all-100 checkpoints. | |
| Add real DKG ceremony command | Treat the missing CLI entry point as Phase 3 scope; wire a real n=100 DKG command. | ✓ |

**User's choice:** Add real DKG ceremony command.
**Notes:** Sharpened by the user's later observation ("what's missing are CLI commands") — the
crypto (Phase 1) and store (Phase 2) are both proven but unconnected: `keygen` drops its secret
shares and `sign` re-simulates a DKG every run. Phase 3's true content is that wiring.

## Command shape (given in-process / no-transport boundary)

| Option | Description | Selected |
|--------|-------------|----------|
| In-process simulate-all command | One command runs all 100 seats over the in-memory stub and persists the whole set. | ✓ |
| Single-participant join command | Per-seat command; true multi-party needs coordinator+transport (Phase 7). | |
| Coordinator ceremony command | The Phase 7 surface; crosses the transport-last boundary. | |

**User's choice:** In-process simulate-all command.

## Store layout

| Option | Description | Selected |
|--------|-------------|----------|
| 100 separate store roots | Write 100 distinct ParticipantStore dirs; faithful to deployment; proves criterion 3 across independent stores. | ✓ |
| One store, 100 tagged shares | Simpler, but blurs per-member isolation. | |
| You decide | Defer to planning. | |

**User's choice:** 100 separate store roots.

## Passphrase handling

| Option | Description | Selected |
|--------|-------------|----------|
| Prompt once, reuse for all 100 | Interactive `for_new_store` once, applied to all 100 sim roots; respects D-01/D-03; unblocks Phase 2 UAT. | ✓ |
| Hidden/dev-gated command | Gate behind cargo feature/hidden flag so it never expands production surface. | |
| You decide | Defer to planning. | |

**User's choice:** Prompt once, reuse for all 100.
**Notes:** Explicitly unblocks the Phase 2 UAT Test 1 by exercising the `for_new_store` path.

## Coordinator store (clarification, no formal selection)

**User's challenge:** "What problem does the coordinator solve in production?" then "There is no
question SQLite can hold 100 rows." Resolved in discussion: the coordinator is an
orchestration/bookkeeping party, deliberately untrusted for integrity. In the in-process sim
there is no real orchestration, and "SQLite holds 100 rows" proves nothing. **Coordinator-at-100
de-scoped**; roster correctness is a small-n concern already covered by Phase 2.

## Sign path (persisted-share proof depth)

| Option | Description | Selected |
|--------|-------------|----------|
| Load 51 stores → confirmed regtest key-spend | Load 51 persisted roots, sign, re-prove the crown-jewel confirmed regtest key-spend from PERSISTED shares. | ✓ |
| Load 51 stores → valid sig, no broadcast | Produce a sig verifying against Q; leave confirmed-spend to existing inproc_sign_100 (fresh DKG). | |
| You decide | Defer to planning. | |

**User's choice:** Load 51 stores → confirmed regtest key-spend.

## Measurement artifact

**User's challenge:** "We already know the speed is acceptable from Phase 1." Resolved in
discussion: criterion 2 is satisfied-by-Phase-1 (quick task `260713-jqs`: DKG 4.41s, key-spend
9.90s), and the added persist cost is a known scrypt constant × count — arithmetic, not a
discovery. **Measurement artifact dropped entirely.**

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — pure wiring, no measurement | Pure CLI wiring; small-n CI gate + one-time --full functional smoke; no committed numbers. | ✓ |
| Mostly — keep a one-line record | Same, but a one-liner wall-clock breadcrumb in the SUMMARY. | |

**User's choice:** Yes — pure wiring, no measurement.

---

## Claude's Discretion

- `--persist` flag vs. persist-by-default; retention of the public-envelope output.
- How `sign` selects/discovers the 51 of 100 store roots.
- Whether/how `keygen` populates the coordinator roster (small-n only if at all — not an
  at-scale deliverable).
- Disposition of the `#[ignore]`d `store_checkpoint_n100::persist_reload_100` test.
- Base-dir / `CHEGET_HOME` handling for the 100 simulated roots.

## Deferred Ideas

- **ROADMAP re-frame:** re-frame plans 03-01/03-02 to the wiring scope via `/gsd-phase` (discuss
  does not edit the roadmap).
- **Phase 4+ command wiring:** refresh/enroll/repair, standby/sweep/watch/policy, init/transport
  — each belongs to its own phase with genuine unbuilt ceremony logic behind it.
- **Coordinator roster/transcript at scale:** de-scoped; the coordinator's real value is a
  Phase 7 (transport) concern.
