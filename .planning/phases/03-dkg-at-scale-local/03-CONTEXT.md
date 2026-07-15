# Phase 3: DKG at Scale — Local - Context

**Gathered:** 2026-07-15
**Status:** Ready for planning

<domain>
## Phase Boundary

**Phase 3 is the CLI wiring that connects Phase 1's proven crypto core to Phase 2's proven
storage layer — at the full n=100 share set, in-process, no transport.** It is NOT a
"prove the DKG scales" phase: the scale proof already exists (see below). Its deliverable is
the two commands that make the persisted-share flow real end to end:

1. **`keygen` → store:** run the in-process simulate-all DKG (`run_inprocess_dkg`) and
   **persist the full n=100 secret share set** through the Phase 2 participant store. Today
   `cli/keygen.rs` throws the secret shares away (`let (_shares, group)`) and writes only the
   public envelope — Phase 3 adds the secret-persistence path. This is the **first command that
   creates an encrypted store**, which directly **unblocks the outstanding Phase 2 UAT Test 1**.
2. **`sign` ← store:** load 51 of the persisted stores, drive the signing session, and re-prove
   the crown-jewel **confirmed regtest key-spend from *persisted* shares** (today `cli/sign.rs`
   re-runs a fresh in-process DKG every invocation because nothing is persisted to load).

**Requirement in scope:** KEY-06 (already marked Complete in REQUIREMENTS.md — the correctness
and O(n²) halves were satisfied in Phase 1; Phase 3 delivers the persist/reload-at-scale half
through a real command rather than an ignored test).

**How the three ROADMAP success criteria are met:**
- **Criterion 1** (100 KeyPackages verify to one group key): ALREADY DONE in Phase 1 —
  `run_inprocess_dkg` + `tests/dkg_100_correctness.rs` (runs by default at t=51/n=100).
- **Criterion 2** (O(n²) cost measured locally): ALREADY DONE in Phase 1 — instrumented in
  `dkg_100_correctness.rs`; quick task `260713-jqs` recorded DKG group-key proof 4.41s /
  regtest key-spend 9.90s. **Marked satisfied-by-Phase-1. Phase 3 does NOT re-measure.**
- **Criterion 3** (full set persists/reloads through the Phase 2 stores): DELIVERED by the
  `keygen`→store + `sign`←store wiring above, exercised at n=100.

**Explicitly NOT in this phase:**
- Any re-measurement / benchmark harness / MEASUREMENTS.md — speed is known-acceptable from
  Phase 1 (a known-constant scrypt cost × count is arithmetic, not a discovery).
- A coordinator-store-at-100 proof — "SQLite holds 100 rows" is trivially true and proves
  nothing. Coordinator roster correctness is a small-n concern already covered by Phase 2.
- Any real network transport / coordinator-driven-over-the-wire ceremony (Phase 7).
- Rotation / refresh / enroll / repair (Phase 4), sweep / lifecycle / watch (Phase 5).

**Boundary correction (do not over-rotate):** "everything is already proven, only CLI is
missing" holds for the **DKG + signing + bridge** value (Phase 1 scope) — that is exactly why
Phase 3 is pure wiring. It does NOT hold for later phases: Phase 4 (`refresh`/`repairable`
primitives) is exercised NOWHERE in the codebase yet, and Phases 5/7 have genuinely unbuilt
ceremony logic behind their commands, not merely missing CLI veneer.

</domain>

<decisions>
## Implementation Decisions

### Phase reframe (foundational)
- **D-01:** Phase 3 is reframed from "prove n=100 DKG scales" to "**wire the proven crypto to
  the proven store**." Criteria 1 & 2 are satisfied-by-Phase-1; criterion 3 is delivered via
  the real `keygen`/`sign` commands. **This requires a ROADMAP re-frame of plans 03-01/03-02**
  (their current text says "scale the DKG" / "measure O(n²)") — see Deferred Ideas for the
  `/gsd-phase` action.

### `keygen` → store (the writer)
- **D-02:** `keygen` runs the existing in-process simulate-all DKG (`run_inprocess_dkg`, D-08
  from Phase 1) and **persists the full n=100 secret share set** through the Phase 2
  `ParticipantStore` (encrypted `KeyPackage` per seat + plaintext group public package). The
  existing public-envelope output may remain; the NEW behavior is secret persistence. Flag-vs-
  default surface (e.g. a `--persist` flag vs. persist-by-default) is a planning detail.
- **D-03:** **100 separate store roots** — one real `ParticipantStore` per seat
  (`<base>/seat-NNNN/`), each with its own encrypted share + plaintext group package. This is
  the faithful deployment topology (per-member isolation the store was designed around), and it
  is what makes criterion 3's persist/reload real across independent stores. A single store
  holding 100 tagged shares was rejected (blurs per-member isolation).
- **D-04:** **Prompt-once passphrase, reused for all 100 sim roots.** The interactive
  `InteractivePassphrase::for_new_store` prompt (confirm-twice, no-echo) runs ONCE; that
  passphrase is applied to all 100 simulated roots. This respects D-01/D-03 from Phase 2 (NO
  passphrase env var or CLI flag ships — production surface stays interactive-only) AND
  **exercises the `for_new_store` path, which unblocks the Phase 2 UAT Test 1**. The 100-roots-
  in-one-invocation is a **simulation affordance** — one operator standing in for 100 members;
  the production topology is one member / one machine / one store.

### `sign` ← store (the reader)
- **D-05:** `sign` **loads 51 of the 100 persisted store roots** (prompt-once unlock), loads
  their `KeyPackage`s, drives the `SigningSession` over the PSBT, and re-proves the
  **crown-jewel confirmed regtest key-spend from PERSISTED shares** — the strongest end-to-end
  proof that the store→sign path works, distinct from the existing `inproc_sign_100` (which
  uses a fresh in-process DKG). This replaces `sign`'s current re-simulate-DKG-every-run
  behavior for the persisted-share path.

### Rigor / testing
- **D-06:** **Correctness at small n in the PR gate; one-time `--full` (100) functional smoke.**
  The wiring is generic over the share set, so correctness is fully proven at small n (fast, in
  CI). The full-100 run is a one-time confirmation that the wiring holds at scale (file handles,
  paths, completion) — a **functional smoke, NOT a measurement**. No numbers committed, no
  benchmark, no MEASUREMENTS.md.
- **D-07:** The existing `#[ignore]`d `tests/store_checkpoint_n100.rs::persist_reload_100` is
  now **superseded as the criterion-3 vehicle** by the real command. Whether it is kept as a
  lower-level durability check, run once, or retired is a planning detail — the criterion is met
  by the command, not the ignored test.

### Claude's Discretion
Left to research/planning unless a decision above constrains it:
- Exact command surface for the new persist behavior: `--persist` flag vs. persist-by-default;
  whether the public-envelope output is retained alongside the encrypted shares.
- How `sign` selects which 51 of 100 roots to load (first-51, liveness-poll simulation, or
  configurable) and how it discovers the store roots under `<base>/`.
- Whether/how `keygen` populates the coordinator SQLite roster (D-15 npubs) as part of the
  wiring — **not a Phase 3 at-scale deliverable**; if done at all, small-n correctness only.
- Disposition of the `#[ignore]`d `persist_reload_100` test (D-07).
- Base-dir / `CHEGET_HOME` handling for the 100 simulated roots vs. the real single-root path.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design & spec (authoritative)
- `SPEC-frost-cli.md` — full design. Most relevant to Phase 3: **§5** CLI persona/command
  surface, **§6.5** signing/nonce discipline (the persisted-share sign path must NOT touch the
  nonce-exclusion invariant), and the at-rest age/scrypt storage rules the writer uses.
- `.planning/research/PITFALLS.md` — implementation pitfalls (parity/even-Y, nonce hazards) —
  relevant to the persisted-share sign path.

### Project planning
- `.planning/PROJECT.md` — locked crate stack + pins; Key Decisions table (do not re-litigate).
- `.planning/REQUIREMENTS.md` — KEY-06 (already Complete); STOR-01/02/03 (Phase 2, the store
  this phase wires into).
- `.planning/ROADMAP.md` — Phase 3 success criteria (1 & 2 satisfied-by-Phase-1; 3 delivered
  here). **Note: 03-01/03-02 plan text needs re-framing (D-01) via `/gsd-phase`.**
- `.planning/phases/01-crypto-bridge-in-process-signing/01-CONTEXT.md` — D-02 (t=51/n=100
  acceptance target), D-08 (simulate-all-seats in-process), D-09 (public-on-disk /
  secret-never-persisted — the line Phase 3 now crosses by adding secret persistence).
- `.planning/phases/02-persistence-storage/02-CONTEXT.md` — the store this phase wires into.
  Especially **D-01/D-03** (interactive-only passphrase, no env/flag ships — the constraint
  D-04 above satisfies), **D-05** (file-per-share tree + manifest), **D-06** (decrypt-use-drop),
  **D-07** (atomic writes), **D-15** (coordinator roster npubs).

### Existing code to wire / not break (from codebase scout)
- `src/cli/keygen.rs` — the writer to extend: `run()` currently drops `_shares` and writes only
  the public envelope; add the `ParticipantStore` persist path.
- `src/cli/sign.rs` — the reader to rework: `run()` currently re-runs `run_inprocess_dkg` every
  invocation; change the persisted-share path to load from the stores.
- `src/cli/mod.rs` — persona tree + `resolve_root`; the new passphrase-prompting entry point.
- `src/store/participant.rs` — `ParticipantStore::put_share` / `load_share` / `read_manifest`.
- `src/store/passphrase.rs` — `InteractivePassphrase::for_new_store` (the confirm-twice/no-echo
  path D-04 must invoke; the WR-01 zeroize fix lives here at lines ~80-100).
- `src/crypto/keygen.rs` — `run_inprocess_dkg` (the DKG the writer drives).
- `src/session/mod.rs` — `SigningSession` (the sign path over the in-memory `Transport` stub).
- `tests/inproc_sign_100.rs` — the existing fresh-DKG regtest key-spend at n=100 (contrast: D-05
  proves the SAME from persisted shares).
- `tests/store_checkpoint_n100.rs` — the `#[ignore]`d persist/reload harness superseded by the
  command (D-07).
- `tests/dkg_100_correctness.rs` — the criterion-1/2 proof already satisfying KEY-06.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`run_inprocess_dkg(t, n)`** (`src/crypto/keygen.rs`): the proven, rayon-parallel n=100 DKG
  the `keygen` writer drives — already returns the full `BTreeMap<Identifier, KeyPackage>` +
  group `PublicKeyPackage`. No new crypto needed.
- **`ParticipantStore`** (`src/store/participant.rs`): `put_share(tag, kp, group, state)` /
  `load_share(tag)` — the exact API the writer/reader call. Proven byte-faithful by
  `store_checkpoint_n100`.
- **`InteractivePassphrase::for_new_store`** (`src/store/passphrase.rs`): the confirm-twice/
  no-echo prompt D-04 reuses across all 100 sim roots.
- **`SigningSession`** (`src/session/mod.rs`) + `InMemoryTransport`: the sign pipeline `sign`
  already uses — the change is the *source* of the KeyPackages (store, not fresh DKG).

### Established Patterns
- **CLI routes, never computes** (`cli/mod.rs`): handlers dispatch to the library. The new
  persist/load logic belongs in the store/CLI layer, never in `src/crypto/` (which imports no
  fs/transport code — keep it that way).
- **Public-plaintext / secret-encrypted split** (Phase 1 D-09 → Phase 2 D-05): the writer keeps
  the public package plaintext and encrypts only the secret share.
- **Prompt-once, interactive-only passphrase** (Phase 2 D-01/D-03): no env/flag ships; the sim's
  one-operator-for-100 convenience is expressed as one prompt reused, not a new flag.

### Integration Points
- `keygen` handler → `ParticipantStore::put_share` × (n seats × roots) → **first writer of the
  encrypted store** (unblocks Phase 2 UAT).
- `sign` handler → `ParticipantStore::load_share` × 51 → `SigningSession` → confirmed regtest
  key-spend.
- Regtest fixture (`tests/regtest_fixture.rs`, `tests/common/`) → the from-persisted-shares
  key-spend proof (D-05).

</code_context>

<specifics>
## Specific Ideas

- The persisted-share **confirmed regtest key-spend** (D-05) is the acceptance bar — it proves
  the store→sign path works against the chain, not merely that shares deserialize.
- `keygen`→store being the **first command that creates an encrypted store** is the concrete
  thing that closes the Phase 2 UAT blocker — treat unblocking that UAT as an explicit outcome.
- "We already know the speed is acceptable from Phase 1" — the guiding principle that removed
  the measurement artifact; do not re-derive known constants.

</specifics>

<deferred>
## Deferred Ideas

- **ROADMAP re-frame (roadmap action, not a new capability):** re-frame plans 03-01/03-02 from
  "scale the DKG" / "measure O(n²) + persist/reload" to the wiring scope (keygen→store,
  sign←store, confirmed regtest key-spend from persisted shares). Run **`/gsd-phase`** after this
  discussion — discuss-phase does not edit the roadmap. Captured here so the intent isn't lost.
- **Phase 4+ command wiring** — refresh/enroll/repair (Phase 4), standby/sweep/watch/policy
  (Phase 5), init/transport (Phase 7) commands are all still missing, but each belongs to its
  own phase and has genuine unbuilt ceremony logic behind it (not just CLI). Not Phase 3.
- **Coordinator roster/transcript at scale** — de-scoped from Phase 3 (trivial at volume); the
  coordinator's real value (orchestration/liveness/policy) is inherently a Phase 7 (transport)
  concern.

### Reviewed Todos (not folded)
None — no pending todos matched this phase.

</deferred>

---

*Phase: 3-dkg-at-scale-local*
*Context gathered: 2026-07-15*
