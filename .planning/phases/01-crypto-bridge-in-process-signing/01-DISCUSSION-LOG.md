# Phase 1: Crypto Bridge & In-Process Signing - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-10
**Phase:** 1-crypto-bridge-in-process-signing
**Areas discussed:** Proof group size (t/n), Regtest proof & CI, CLI vs test-harness boundary, Bridge KAT provenance

---

## Proof group size (t/n)

### Q1 — Group size / parameterization strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Generic + small dev, 51 gate | Generic over (t,n); tiny dev tests; one t=51 acceptance | |
| Always real t=51, n=100 | Production params from line one; slow tests; overlaps Phase 3 | ✓ |
| Always t=51, n=51 | Minimal real-threshold group; no liveness-subset slack | |

**User's choice:** Always real t=51, n=100.
**Notes:** Flagged that n=100 in-process duplicates Phase 3's compute proof — led to Q2.

### Q2 — Phase 1 vs Phase 3 boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Ph1 proves it works; Ph3 measures it | Ph1 correctness, Ph3 narrowed to timing/persist | |
| Fold Phase 3 into Phase 1 | Correctness + scaling both land in Ph1 | ✓ |
| Keep Phase 3 as-is, accept overlap | Both run n=100, duplicated compute | |

**User's choice:** Fold Phase 3 into Phase 1.
**Notes:** Surfaced the Phase 2 dependency of Phase 3's persist/reload criterion — led to Q3.

### Q3 — Persist/reload-at-scale placement

| Option | Description | Selected |
|--------|-------------|----------|
| Move it into Phase 2 | Ph1 = correctness + O(n²); persist/reload → Ph2; Ph3 gone | ✓ |
| Keep a thin Phase 3 | Minimal Ph3 after Ph2 for persist/reload only | |
| Decide during roadmap edit | Just record intent, resolve later | |

**User's choice:** Move it into Phase 2.
**Notes:** KEY-06 → Phase 1; Phase 3 disappears; a ROADMAP edit is required afterward.

### Q4 — Hardcode vs generic (t,n)

| Option | Description | Selected |
|--------|-------------|----------|
| Generic code, n=100 acceptance | Params take (t,n); tiny unit tests fast; n=100 gate | ✓ |
| Hardcode 51/100 | Fixed constants; every test pays full cost | |
| Generic, but no tiny tests | Generic code, suite only ever runs n=100 | |

**User's choice:** Generic code, n=100 acceptance.
**Notes:** Reconciles "always run real" with practical TDD iteration speed.

---

## Regtest proof & CI

### Q1 — Regtest node provisioning

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-spawn via corepc-node | Throwaway regtest bitcoind on temp datadir, pinned Core | ✓ |
| External node via env var | Expect running node; more setup friction | |
| Docker-compose bitcoind | Compose brings up node; heavier, Docker dependency | |

**User's choice:** Auto-spawn via corepc-node.
**Notes:** Hermetic and CI-friendly; standard rust-bitcoin testing pattern.

### Q2 — CI gating

| Option | Description | Selected |
|--------|-------------|----------|
| Tiered: fast PR gate + nightly n=100 | PR = KAT + small-n e2e + build/audit; nightly full n=100 | ✓ |
| Full n=100 on every PR | Max confidence, 20–40+ min CI | |
| n=100 gate only on main | Fast PR; full e2e gates main merges | |

**User's choice:** Tiered — fast PR gate + nightly/on-demand full n=100 (must pass before phase sign-off).
**Notes:** Keeps PRs fast while still proving the real thing before merge to main/release.

### Q3 — Chain backend for the confirm path

| Option | Description | Selected |
|--------|-------------|----------|
| Core for confirm; Esplora built + unit-tested | Core native regtest mining; Esplora to same trait, unit-tested | ✓ |
| Core only; defer Esplora | STOR-04 not fully met in Phase 1 | |
| Both against regtest (add electrs) | Fullest coverage; adds electrs to harness | |

**User's choice:** Core for confirm; Esplora built to the same trait + unit/conformance-tested.
**Notes:** Satisfies STOR-04 without standing up electrs/esplora over regtest.

---

## CLI vs test-harness boundary

### Q1 — CLI scope

| Option | Description | Selected |
|--------|-------------|----------|
| Real skeleton over in-memory transport | Real clap tree wired to in-memory Transport stub | ✓ |
| Library + harness, only `tsig address` real | Crypto lib + integration harness; minimal CLI | |
| Full clap tree, Phase 1 paths only functional | Whole persona tree stubbed; rest "not yet implemented" | |

**User's choice:** Real subcommand skeleton over the in-memory transport stub.
**Notes:** Most forward-compatible — Phase 7 swaps the stub for Nostr behind the same seam.

### Q2 — Key/state flow without persistence

| Option | Description | Selected |
|--------|-------------|----------|
| Plaintext public artifacts + in-mem secrets | Public packages to plaintext files; secrets in-memory only | ✓ |
| Single-process whole ceremony | One invocation runs DKG→address→sign→confirm inline | |
| Ephemeral session dir | Temp dir holds public + unencrypted secret material | |

**User's choice:** Plaintext public artifacts + in-memory secrets; `tsig address --pubkey <file>`.
**Notes:** Draws the clean line for Phase 2 (public on disk OK; encrypted secret store is Phase 2).

---

## Bridge KAT provenance

### Q1 — KAT source

| Option | Description | Selected |
|--------|-------------|----------|
| Official BIP341/BIP86 vectors | Anchor to published spec vectors; externally auditable | ✓ |
| Self-generate and pin | rust-bitcoin verifying itself; weaker trust | |
| Cross-validate against descriptor/bitcoin-cli | External oracle at test time; couples to bitcoind | |

**User's choice:** Official BIP341/BIP86 vectors.
**Notes:** Strongest "100 people must verify" story — auditable against the BIPs themselves.

### Q2 — Parity / even-Y handling

| Option | Description | Selected |
|--------|-------------|----------|
| Bridge asserts even-Y; KAT covers both | EvenY in core; bridge asserts; KAT even-Y + odd-Y vectors | ✓ |
| Rely on EvenY, even-Y vector only | Trust normalization; odd-Y path untested | |
| You decide during research | Leave parity contract to researcher/planner | |

**User's choice:** Bridge asserts even-Y invariant; KAT covers both even-Y and odd-Y-origin vectors, each verified end-to-end.
**Notes:** Turns the classic silent parity bug into an explicitly covered adversarial case.

---

## Claude's Discretion

Deferred to research/planning against frost 3.0 + rust-bitcoin 0.32 (grounded in SPEC + PITFALLS.md):
- Non-serializable nonce type mechanism (SIGN-05) — likely newtype + `trybuild` compile-fail test.
- `Transport` trait contract (sync/async, message/envelope model).
- `ChainBackend` trait contract (UTXO list, fee estimate, broadcast, sighash, descriptor import).
- Liveness poll / 51-of-100 subset selection driven over the in-memory transport.
- Display-before-sign UX specifics (SIGN-07): rendered fields, `--yes` behavior.

## Deferred Ideas

- **ROADMAP EDIT REQUIRED:** fold former Phase 3 into Phase 1 (KEY-06 + O(n²) measurement),
  move persist/reload-at-scale to Phase 2, delete Phase 3, renumber Phases 4→3, 5→4, 6→5, 7→6.
  Apply via `/gsd-phase` after this discussion.
- Esplora-over-regtest (electrs) confirm path — deferred; Core fronts the confirm in Phase 1.
