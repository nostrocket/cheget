# Roadmap: tsig — 501-of-1000 FROST Taproot Signing CLI

## Overview

tsig is built bottom-up as a layered, trait-seamed monolith that proves the entire system
LOCALLY before any relay code exists, then swaps in real transport as the final step. The
architecture is trait-seamed: a `Transport` trait (introduced with an in-memory/in-process stub
in Phase 1) lets every ceremony — DKG, refresh, enroll, repair, sweep — run against an in-memory
transport with zero relays, zero network, and zero Nostr code. That single seam is what makes this
ordering possible.

Phase 1 nails the frost↔rust-bitcoin key bridge and an in-process 501-of-1000 signature to a
confirmed regtest key-spend, with zero transport and no persistence — the whole value proposition,
de-risked first — and introduces the `Transport` trait plus its in-memory stub so every later
ceremony phase runs locally. Phase 2 lays down the durable-state foundation: age/scrypt participant
storage, encrypted between-round ceremony checkpointing, and the coordinator SQLite store. Phase 3
scales the in-process DKG to the full n=1000 share set on a single host with no transport, proving
the O(n²) computation cost is tractable locally (distinct from the later transport load test).
Phase 4 adds membership rotation (refresh, enroll, repair) run over the in-memory stub, with the
mandatory client-side same-key check and epoch discipline. Phase 5 delivers the real revocation
path — standby key, sweep, and policy watcher — with ceremonies over the in-memory stub and chain
access via the `ChainBackend` trait. Phase 6 ships the trust: reproducible builds, pinned/audited
deps, locally-verifiable adversarial tests, and external review — all verifiable without real
transport. Phase 7 (FINAL) implements the real `Transport` impls behind the same trait — offline
`FileTransport` first, then `NostrTransport` — and re-runs the whole, already-proven system over
real relays at scale, gated by the containerized n=1000 DKG load test and the transport-dependent
adversarial suite.

Four security controls (non-serializable nonce type, byte-level bridge round-trip, tweak/aggregate
verified against Q, display-before-sign) are structural from Phase 1, never retrofitted.

## Phases

**Phase Numbering:**

- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Crypto Bridge & In-Process Signing** - Prove the frost↔rust-bitcoin bridge and a confirmed regtest 501-of-1000 key-spend with zero transport; introduce the `Transport` trait + in-memory stub
- [ ] **Phase 2: Persistence & Storage** - Age/scrypt participant store, encrypted between-round checkpointing, and the coordinator SQLite store — the durable-state foundation
- [ ] **Phase 3: DKG at Scale — Local** - Scale the in-process DKG to the full n=1000 share set on one host with no transport; prove the O(n²) computation cost locally
- [ ] **Phase 4: Membership Rotation** - Refresh, enroll, and repair over the in-memory stub with client-side same-key check and epoch discipline
- [ ] **Phase 5: Key Lifecycle & Revocation** - Standby key, sweep + rollover, and the policy-driven watcher over the in-memory stub (the true revocation path)
- [ ] **Phase 6: Hardening & Security-Reviewable Release** - Reproducible builds, pinned/audited deps, locally-verifiable adversarial suite, external review — all without real transport
- [ ] **Phase 7: Transport & Transport at Scale** - Real `Transport` impls (offline file mode then Nostr), the gating n=1000 DKG relay load test, and transport-dependent adversarial tests

## Phase Details

### Phase 1: Crypto Bridge & In-Process Signing

**Goal**: Prove the entire cryptographic value in-process — DKG → BIP341 address → two-round tweaked signing → a confirmed regtest key-spend — with zero transport, relays, or persistence, and the four structural security controls present from the first line of signing code. Introduce the `Transport` trait and its in-memory/in-process stub so every later ceremony phase (3–6) runs against it with no relay code.
**Depends on**: Nothing (first phase)
**Requirements**: KEY-01, KEY-02, KEY-03, KEY-04, KEY-05, SIGN-01, SIGN-02, SIGN-03, SIGN-04, SIGN-05, SIGN-06, SIGN-07, STOR-04
**Success Criteria** (what must be TRUE):

  1. `tsig address` prints a BIP341 P2TR address (merkle root `None`) derived from a DKG-generated group key, and a committed byte-level round-trip test pins the frost→rust-bitcoin bridge (33-byte SEC1 → x-only → `XOnlyPublicKey` → P2TR) against a hard-coded known-answer vector (KEY-03, KEY-04)
  2. An in-process ceremony with 501 simulated participants and no transport produces a `KeyPackage`+`PublicKeyPackage` whose verifying key is the Taproot internal key `P`, and every participant confirms the key back — any mismatch aborts the ceremony (KEY-01, KEY-02, KEY-05)
  3. A coordinator signing session over a regtest PSBT computes the per-input key-spend sighash, runs round 1/round 2 with `sign_with_tweak`, aggregates with `aggregate_with_tweak(…, None)` into a 64-byte BIP340 signature that verifies against the output key `Q`, finalizes the PSBT, and broadcasts a confirmed key-spend on regtest (SIGN-01, SIGN-02, SIGN-03, SIGN-04, STOR-04)
  4. Signing nonces are a type that cannot be serialized/persisted (won't compile if attempted); a session restart or timeout mints fresh nonces in a new session and never reuses commitments, with the 3.0 cheater-detection culprits list surfaced on abort (SIGN-05, SIGN-06)
  5. Before round 2, each participant recomputes the sighash locally from the PSBT and is shown human-readable outputs/amounts/fee, requiring an explicit ack unless `--yes` — no blind signing of a coordinator-supplied hash (SIGN-07)

**Plans**: 1/5 plans executed

Plans (waves: W1=01-01 → W2={01-02,01-03,01-05} parallel → W3=01-04):

- [x] 01-01 [W1]: Pinned Cargo scaffold + clap persona skeleton + canonical bridge (`bridge/taproot.rs`) + BIP341/BIP86 KAT (even-Y AND odd-Y-origin) + `tsig address` (KEY-03, KEY-04)
- [ ] 01-02 [W2]: Crypto core over `frost-secp256k1-tr` — in-process DKG generic over (t,n), even-Y, client-side confirmation, non-serializable nonce type + trybuild, n=1000 correctness + O(n²) measurement (KEY-01/02/05/06, SIGN-05)
- [ ] 01-03 [W2]: `ChainBackend` trait + Core RPC + Esplora impls, key-spend sighash helper, auto-spawned regtest fixture (STOR-04)
- [ ] 01-04 [W3]: Signing session — liveness/round1/round2 over Transport, display-before-sign gate, `aggregate_with_tweak(None)` + verify against `Q`, cheater culprits, confirmed regtest key-spend at 501/1000 (SIGN-01/02/03/04/06/07)
- [ ] 01-05 [W2]: `Transport` trait + in-memory/in-process stub — the architectural seam every later ceremony phase runs against (no relay code)

### Phase 2: Persistence & Storage

**Goal**: Lay down the durable-state foundation the ceremony and transport layers build on — age/scrypt participant storage with nonce-exclusion and epoch tagging, encrypted between-round ceremony checkpointing, and the coordinator SQLite store for roster/transcripts/logs/policy/churn — so no durable state is retrofitted later.
**Depends on**: Phase 1
**Requirements**: STOR-01, STOR-02, STOR-03
**Success Criteria** (what must be TRUE):

  1. Participant storage (`~/.tsig/`) holds the identity keypair and per-key-per-epoch `KeyPackage`+`PublicKeyPackage` age/scrypt-encrypted at rest and zeroized in memory after use, tagged `(key_id, epoch, identifier)` (STOR-01)
  2. Ceremony round secrets (DKG parts) are checkpointed encrypted between rounds of the same ceremony, and signing nonces are structurally excluded from persistence — the sole never-persisted exception (STOR-02)
  3. Coordinator state persists in SQLite (rusqlite): roster (identifier ↔ npub ↔ status ↔ join/leave epochs), ceremony transcripts, session logs, policy config, and churn ledger (STOR-03)

**Plans**: 2 plans

Plans:

- [x] 01-01-PLAN.md
- [ ] 01-02-PLAN.md
- [ ] 01-03-PLAN.md
- [ ] 01-04-PLAN.md
- [ ] 01-05-PLAN.md
- [ ] 02-01: Participant store (age/scrypt, zeroize, nonce-exclusion, epoch tagging) + encrypted between-round ceremony checkpointing
- [ ] 02-02: Coordinator SQLite store (roster/transcripts/session logs/policy/churn ledger)

### Phase 3: DKG at Scale — Local

**Goal**: Scale the in-process DKG to the full n=1000 share set on a single host with no transport, proving the O(n²) computation cost is tractable locally — the compute-scaling proof, cleanly separated from the later transport-layer load test.
**Depends on**: Phase 2
**Requirements**: KEY-06
**Success Criteria** (what must be TRUE):

  1. An in-process DKG generates the full n=1000 share set on one host with no transport, producing 1000 `KeyPackage`s that all verify to a single group `PublicKeyPackage` (KEY-06)
  2. The O(n²) computation cost is measured locally (part1/part2/part3 timing and memory across 1000 seats), demonstrating the computation scales on one machine independent of any relay (KEY-06)
  3. The generated n=1000 share set persists to and reloads from the Phase 2 participant/coordinator stores at scale, confirming the durable-state layer holds at full size

**Plans**: 2 plans

Plans:

- [ ] 03-01: Scale the in-process DKG to n=1000 on one host (no transport); all 1000 KeyPackages verify to one group key
- [ ] 03-02: Measure the O(n²) computation cost locally + persist/reload the full n=1000 share set through the Phase 2 stores

### Phase 4: Membership Rotation

**Goal**: Change membership at a constant on-chain address via refresh, enroll, and repair — all run over the in-memory transport stub — with the mandatory client-side same-key postcondition, epoch bookkeeping, and early mixed-epoch rejection.
**Depends on**: Phase 3
**Success Criteria** (what must be TRUE):

  1. `tsig ceremony refresh --remove <ids>` (over the in-memory stub) runs refresh-DKG removing any excluded identifier, increments `epoch`, and every participant verifies client-side that the new `PublicKeyPackage` verifying key equals the pinned old one — aborting and discarding the new share on mismatch, never trusting the coordinator (ROT-01, ROT-02)
  2. `tsig ceremony enroll --seat <id> --new-member <pubkey>` issues a share to a fresh identifier via repair/RTS and atomically chains an immediate refresh in the same ceremony window so helper delta-knowledge is proactivized away (ROT-03)
  3. `tsig ceremony repair --seat <id>` recovers a lost share with ≥501 helpers, and the recovering member verifies the result against the group `PublicKeyPackage` (ROT-04)
  4. Signing sessions bind `(key_id, epoch)` and reject mixed-epoch shares early with a clear, seat-identifiable error before any partial reaches aggregation (ROT-05)
  5. `tsig share status` lists held shares (key_id, epoch, state); at steady state each participant holds exactly two — one ACTIVE, one STANDBY (ROT-06)

**Requirements**: ROT-01, ROT-02, ROT-03, ROT-04, ROT-05, ROT-06
**Plans**: 4 plans

Plans:

- [ ] 04-01: Refresh (removals + proactivize) over the in-memory stub + mandatory client-side same-key postcondition (verify → persist → delete order)
- [ ] 04-02: Enroll (repair/RTS to fresh identifier → atomic chained refresh) + monotonic identifier allocation from the roster authority
- [ ] 04-03: Repair for an existing seat with ≥501 helpers + recovering-member verification
- [ ] 04-04: Epoch bookkeeping, `(key_id, epoch)` session binding + early mixed-epoch rejection, `share status`

### Phase 5: Key Lifecycle & Revocation

**Goal**: Deliver the true revocation path — a pre-generated, refreshed standby key; a sweep of all UTXOs to standby with confirmation-driven rollover; and a cron/CI-friendly policy watcher — with ceremonies run over the in-memory stub and chain access via the `ChainBackend` trait from Phase 1. Rotation is defense; the sweep is the actual revocation.
**Depends on**: Phase 4
**Requirements**: LIFE-01, LIFE-02, LIFE-03, LIFE-04, POL-01, POL-02, POL-03
**Success Criteria** (what must be TRUE):

  1. `tsig standby new` pre-generates the next (STANDBY) key via a full ceremony over the in-memory stub and keeps it refreshed on the same cadence as ACTIVE (LIFE-01)
  2. `tsig sweep [--to standby] [--feerate]` builds one RBF-enabled consolidation tx spending ALL active UTXOs to the standby address (via the `ChainBackend` trait) and signs it through the signing session against ACTIVE (LIFE-02)
  3. On sweep confirmation at depth ≥6 the state machine rolls ACTIVE→RETIRED and STANDBY→ACTIVE, and `tsig watch` nags until a new STANDBY exists (LIFE-03, LIFE-04)
  4. `tsig policy show|set` manages `value_cap`, `churn_budget` (default 50), `max_epochs` (default 24), and `standby_max_age` (default 90d), and a STANDBY older than `standby_max_age` forces regeneration (POL-01, POL-03)
  5. `tsig watch --node <rpc-url>` is cron/CI-friendly: exit 0 ok / exit 2 sweep-due, emitting a JSON report when balance > value_cap OR distinct former holders since last DKG > churn_budget OR epochs since DKG > max_epochs (POL-02)

**Plans**: 4 plans

Plans:

- [ ] 05-01: Standby key lifecycle (`standby new`, same-cadence refresh) + ACTIVE/STANDBY/RETIRED state machine
- [ ] 05-02: Sweep flow — RBF consolidation tx build, feerate estimation, sign against ACTIVE, broadcast
- [ ] 05-03: Rollover on confirmation depth ≥6 + post-sweep standby-regeneration nag
- [ ] 05-04: Policy engine (4 knobs) + `watch` with exit codes and JSON report

### Phase 6: Hardening & Security-Reviewable Release

**Goal**: Ship the trust — 1000 people must be able to verify what they run — via reproducible builds, pinned/audited dependencies, a locally-verifiable adversarial test suite that re-verifies the structural rules, and external review of the nonce discipline and bridge. Everything here is verifiable without real transport.
**Depends on**: Phase 5
**Requirements**: SEC-01, SEC-02, SEC-03, SEC-04
**Success Criteria** (what must be TRUE):

  1. The participant binary has a reproducible build so members can independently verify the artifact they run (SEC-02)
  2. `Cargo.lock` is committed and `cargo audit` / `cargo deny` run in CI with documented allow-lists (duplicate secp256k1, age label) (SEC-01)
  3. A locally-verifiable adversarial test suite covers mixed-epoch share rejection and a nonce-reuse attempt that fails to compile / is rejected before any partial signature is emitted — no transport required (SEC-03)
  4. External review targets the §6.5 nonce discipline and the §9 bridge code specifically, with findings tracked to closure (SEC-04)

**Plans**: 3 plans

Plans:

- [ ] 06-01: Reproducible participant build + pinned deps, `cargo audit`/`cargo deny` in CI with documented allow-lists
- [ ] 06-02: Locally-verifiable adversarial suite (mixed-epoch rejection, nonce-reuse-won't-compile) — no transport required
- [ ] 06-03: External review package (nonce discipline §6.5 + bridge §9) + threat-model accuracy (static-only claim)

### Phase 7: Transport & Transport at Scale

**Goal**: Take the whole system — already proven locally end-to-end — and run it over real transport. Implement the real `Transport` impls behind the same trait the local stub used (offline `FileTransport` first, then `NostrTransport`), lay down Nostr↔FROST key separation, the signed-event schema with `(key_id, epoch)` binding, roster pinning, and NIP-44 v2 confidentiality; then prove the O(n²) DKG at n=1000 over self-hosted strfry (the gating load test) and pass the transport-dependent adversarial suite.
**Depends on**: Phase 6
**Requirements**: TRAN-01, TRAN-02, TRAN-03, TRAN-04, TRAN-05, TRAN-06, TRAN-07, TRAN-08, SEC-05
**Success Criteria** (what must be TRUE):

  1. `tsig init` generates a dedicated Nostr identity keypair independently of (and never derived from) FROST material and prints the npub for out-of-band roster registration (TRAN-01)
  2. The same ceremonies proven locally now run end-to-end over both `FileTransport` (`--in/--out`) and `NostrTransport` behind the one `Transport` trait, using one signed Nostr event kind per message class tagged for ceremony/session/round/seat binding, with confidential payloads (DKG round-2 shares, enroll/repair deltas) NIP-44 v2 encrypted and public classes in the clear (TRAN-02, TRAN-03, TRAN-07)
  3. Every event is published to all configured relays (≥3), readers merge and dedup by event id, and events from npubs outside the pinned roster (whose hash is committed in every ceremony-open event) are discarded client-side even when a relay delivers them (TRAN-04, TRAN-05)
  4. The gating containerized n=1000 DKG completes over self-hosted strfry with paced round-2 batches (~10⁶ events / ~1 GB), and a ceremony interrupted mid-run resumes idempotently per `(ceremony_id, round, seat)` (TRAN-06, TRAN-08)
  5. Transport-dependent adversarial tests pass: a malicious relay cannot break a ceremony as long as one honest relay is reachable, and replayed envelopes are rejected via event-id / `(ceremony_id, round, seat)` dedup (SEC-05)

**Plans**: 5 plans

Plans:

- [ ] 07-01: `tsig init` + Nostr↔FROST key separation, real `Transport` impls behind the trait + message schema (one event kind per class, `(key_id, epoch)` binding), offline `FileTransport` FIRST
- [ ] 07-02: `NostrTransport` — multi-relay pool, NIP-44 v2 per class, roster pinning + hash commit, NIP-42, dedup
- [ ] 07-03: Resumable/idempotent ceremonies over real transport — re-run DKG/rotation/lifecycle flows end-to-end over the wire
- [ ] 07-04: Gating containerized n=1000 DKG load test — strfry rate-limit/retention tuning, paced batches, interrupt-and-resume (TRAN-08)
- [ ] 07-05: Transport-dependent adversarial tests (SEC-05: malicious-relay DoS, replayed-envelope rejection)

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6 → 7

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Crypto Bridge & In-Process Signing | 1/5 | In Progress|  |
| 2. Persistence & Storage | 0/2 | Not started | - |
| 3. DKG at Scale — Local | 0/2 | Not started | - |
| 4. Membership Rotation | 0/4 | Not started | - |
| 5. Key Lifecycle & Revocation | 0/4 | Not started | - |
| 6. Hardening & Security-Reviewable Release | 0/3 | Not started | - |
| 7. Transport & Transport at Scale | 0/5 | Not started | - |
