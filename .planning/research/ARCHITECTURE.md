# Architecture Research

**Domain:** Threshold-signature Bitcoin CLI (51-of-100 FROST Taproot; single Rust binary, three personae)
**Researched:** 2026-07-10
**Confidence:** HIGH

> Grounded in `SPEC-frost-cli.md` (draft v0.1) and `implementations-resharing.md`. All crate
> APIs referenced (`frost-secp256k1-tr` ≥3.0, `rust-bitcoin`, `nostr-sdk`) are pinned in the SPEC
> and source-verified (✅) in the companion research. External web providers are disabled in
> config; this document synthesizes the SPEC into component boundaries, trust boundaries, data
> flows, and a dependency-driven build order. It does not invent new technology choices.

---

## Standard Architecture

The system is a **layered, trait-seamed monolith**. One binary, three personae (participant /
coordinator / watcher) selected by subcommand, but internally a strict dependency stack: pure
crypto at the bottom, side-effecting adapters (chain, transport, storage) behind traits in the
middle, orchestration above them, CLI on top. Security-critical logic is concentrated in the
lowest two layers so the auditable, reproducible surface is small.

### System Overview

```
┌──────────────────────────────────────────────────────────────────────────┐
│  L4  CLI / personae (clap 4)                                               │
│      participant · coordinator · watcher   — config + relay/key resolution │
├──────────────────────────────────────────────────────────────────────────┤
│  L3  Orchestration                                                         │
│  ┌────────────────┐  ┌───────────────────┐  ┌───────────────────────────┐ │
│  │ Ceremony engine│  │ Signing session   │  │ Lifecycle + Policy         │ │
│  │ keygen/refresh │  │ psbt→sighash→2rnd │  │ standby · sweep · watch    │ │
│  │ enroll/repair  │  │ display · agg+ver │  │ policy engine (4 knobs)    │ │
│  │ resumable/idem │  │ nonces in-mem only│  │ state machine A→S→RETIRED  │ │
│  └───────┬────────┘  └─────────┬─────────┘  └──────────────┬─────────────┘ │
├──────────┼─────────────────────┼───────────────────────────┼──────────────┤
│  L2  Side-effecting adapters (all behind traits)                           │
│  ┌───────▼─────────┐  ┌─────────▼──────────┐  ┌────────────▼─────────────┐ │
│  │ Transport trait │  │ Storage            │  │ ChainBackend trait        │ │
│  │  ├ NostrTransport│  │ ├ ParticipantStore │  │  ├ CoreRpc (bitcoincore) │ │
│  │  └ FileTransport │  │ └ CoordinatorStore │  │  └ Esplora (esplora)     │ │
│  │ msg schema/NIP44│  │  age-enc · SQLite  │  │ PSBT·sighash·utxo·bcast  │ │
│  └───────┬─────────┘  └─────────┬──────────┘  └────────────┬─────────────┘ │
├──────────┴─────────────────────┴───────────────────────────┴──────────────┤
│  L0.5  Key bridge  ── frost VerifyingKey → x-only → XOnlyPublicKey → P2TR   │
│        (merkle_root None) · pinned by a byte-level round-trip test          │  ◄── highest-risk seam
├──────────────────────────────────────────────────────────────────────────┤
│  L0  Crypto core  — thin wrapper over frost-secp256k1-tr ≥3.0 (PURE, no I/O)│
│      dkg part1/2/3 · generate_with_dealer · refresh_dkg · repair/enroll     │  ◄── trusted compute base
│      round1::commit · round2::sign_with_tweak · aggregate_with_tweak(…,None) │
└──────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility (owns) | Depends on | Never touches |
|-----------|-----------------------|------------|---------------|
| **Crypto core** (`crypto/`) | The entire cryptographic layer: DKG (part1/2/3), dealer keygen, refresh-DKG, repair/enroll (RTS), round1 commit, round2 `sign_with_tweak`, `aggregate_with_tweak(…,None)`, verify; epoch/identifier tagging on our own newtypes | `frost-secp256k1-tr`, `frost-core` | I/O, network, disk, clock |
| **Key bridge** (`bridge/`) | `VerifyingKey` (33-byte SEC1) → 32-byte x-only → `bitcoin::XOnlyPublicKey` → `Address::p2tr(…, None, network)`; the Q output-key derivation used for verification | crypto-core types, `rust-bitcoin` | transport, storage |
| **ChainBackend** (`chain/`) | PSBT parse/finalize, per-input BIP341 key-spend sighash (`SighashCache::taproot_key_spend_signature_hash`), UTXO listing, fee estimate, broadcast, `tr()` watch-only descriptor import — behind one trait | `rust-bitcoin`, `bitcoincore-rpc` \| `esplora-client` | FROST secrets |
| **ParticipantStore** (`store/participant`) | `~/.cheget/` file store: identity keypair, per-`(key_id,epoch)` `KeyPackage`+`PublicKeyPackage` age/scrypt-encrypted, checkpointed inter-round ceremony state, zeroize on drop | `age`, `zeroize`, `serde` | signing nonces (structurally excluded) |
| **CoordinatorStore** (`store/coordinator`) | SQLite: roster (identifier↔npub↔status↔join/leave epoch), ceremony transcripts (event ids), session logs, policy, churn ledger | `rusqlite` | share secrets |
| **Transport** (`transport/`) | Message send/recv per message class behind one trait; `NostrTransport` (multi-relay pool, NIP-44 v2, roster pinning, NIP-42 AUTH, dedup by event id) and `FileTransport` (`--in/--out`) | `nostr-sdk`, `serde` | FROST/crypto logic |
| **Ceremony engine** (`ceremony/`) | Sequences keygen/refresh/enroll/repair rounds; resumable + idempotent per `(ceremony_id, round, seat)`; epoch bookkeeping; same-key postcondition; checkpoints every round *except* nonces | crypto, transport, store | rust-bitcoin |
| **Signing session** (`session/`) | PSBT→sighash→liveness poll→round1→display-before-sign→round2→aggregate+verify+finalize; new session per timeout; nonces memory-only | crypto, bridge, chain, transport | persisting nonces |
| **Lifecycle + Policy** (`lifecycle/`, `policy/`) | ACTIVE/STANDBY/RETIRED state machine, sweep build+rollover, `watch` evaluation, policy engine (`value_cap`, `churn_budget`, `max_epochs`, `standby_max_age`) | chain, store, session | crypto internals |
| **CLI** (`cli/`) | clap 4 subcommand dispatch, config/relay/passphrase resolution, human-readable output, exit codes (`watch` → 0/2) | all L3 | doing work itself |

---

## Recommended Project Structure

Single Cargo binary crate with library-style internal modules (so the crypto/bridge layers are
unit-testable and, ideally, extractable into an audited sub-crate later).

```
cheget/
├── Cargo.toml              # pinned deps; workspace-lock committed
├── src/
│   ├── main.rs             # persona dispatch only
│   ├── cli/                # L4 — clap definitions, config, output, exit codes
│   │   ├── participant.rs  #   init, keygen/refresh/enroll/repair/sign join, share status
│   │   ├── coordinator.rs  #   ceremony *, session sign, sweep, standby new
│   │   └── watcher.rs      #   address, watch, policy show|set
│   ├── crypto/             # L0 — PURE frost-secp256k1-tr wrapper (no I/O)
│   │   ├── keygen.rs       #   dkg part1/2/3, generate_with_dealer
│   │   ├── refresh.rs      #   refresh_dkg_part1/2, refresh_dkg_shares
│   │   ├── enroll.rs       #   repairable::repair_share_part1/2/3 (RTS/enroll)
│   │   ├── sign.rs         #   round1::commit, round2::sign_with_tweak, aggregate_with_tweak
│   │   └── types.rs        #   epoch/identifier newtypes, (key_id,epoch,identifier) tagging
│   ├── bridge/             # L0.5 — verifying key → x-only → XOnlyPublicKey → P2TR
│   │   └── taproot.rs      #   + the byte-level round-trip test lives here
│   ├── chain/              # L2 — ChainBackend trait + Core/Esplora impls, PSBT, sighash
│   ├── store/              # L2 — participant file store + coordinator SQLite
│   ├── transport/          # L2 — Transport trait, message schema, Nostr + File impls
│   │   ├── schema.rs       #   event kinds / envelope types per message class
│   │   ├── nostr.rs        #   nostr-sdk pool, NIP-44, roster pin, NIP-42, dedup
│   │   └── file.rs         #   --in/--out air-gapped mode (same envelope JSON)
│   ├── ceremony/           # L3 — resumable/idempotent round engine, epoch bookkeeping
│   ├── session/            # L3 — signing session state machine, nonces in-memory only
│   └── lifecycle/          # L3 — standby/sweep/rollover state machine + policy engine
└── tests/
    ├── bridge_roundtrip.rs # byte-level pin (must exist from day one)
    ├── inproc_sign.rs      # 51-of-100 simulated end-to-end sign on regtest sighash
    └── adversarial/        # malicious relay, mixed-epoch, replay, nonce-reuse
```

### Structure Rationale

- **`crypto/` and `bridge/` are pure and I/O-free** so they can be audited and reproducibly
  built in isolation, and unit-tested without relays, nodes, or disk. This is the trusted
  computing base; keeping it small is a security property, not a style choice.
- **`chain/`, `transport/`, `store/` are trait seams** so the same orchestration code runs
  in-process (simulated), over files (offline), or over Nostr/Core — which is exactly what the
  build order below exploits to prove the bridge before any relay work exists.
- **Orchestration never imports a concrete adapter** — only the traits — so ceremonies/sessions
  are testable against in-memory fakes.

---

## Trust Boundaries

These are the load-bearing boundaries. Component design exists to enforce them.

| Boundary | Rule | Enforced by |
|----------|------|-------------|
| **TCB = crypto core + bridge** | All key security lives here; small, pure, audited, reproducible | Layering: no I/O deps allowed in `crypto/`/`bridge/` |
| **Nonce boundary** | Signing nonces are memory-only, never serialized, never resumed | `SigningNonces` held only in `session/`; not `Serialize`; store API physically cannot accept them |
| **Nostr key ≠ FROST material** | Transport identity keys independently generated, never derived from / reused as share or group-key material (both on secp256k1) | Separate key types + generation paths; no conversion fn exists between them |
| **Coordinator untrusted for key security** | Can stall/censor; cannot forge, learn shares, or alter the address | Client-side sighash recompute (display-before-sign) + client-side same-key check after refresh |
| **Relays trusted for liveness only** | Roster-pinned event signatures verified client-side; relays never trusted to filter | `transport/nostr` discards events from non-roster npubs; publish to ≥3, merge+dedup |
| **Dealer (optional) sees full secret momentarily** | Documented trust event, air-gapped ceremony, recorded in transcript | dealer mode isolated to `crypto/keygen` + `store` transcript entry; DKG mode is the trustless default |
| **At-rest boundary** | Share files age/scrypt-encrypted; **no** security claim rests on deletion | `store/participant` encrypt-always; sweep (not erasure) is the revocation path |
| **Epoch boundary** | Sessions bind `(key_id, epoch)`; mixed-epoch shares rejected early with a clear error | `crypto/types` tagging checked before any round begins |

---

## Data Flow

### Flow 1 — Signing session (`session sign`)

```
Coordinator                              Participant (×51)                 Chain
    │  parse PSBT (chain::parse)                │                             │
    │  sighash per input                        │                             │
    │  liveness poll ─────────────────────────► │                             │
    │  ceremony-open/session-control (transport)│                             │
    │ ◄──── round1: SigningCommitments ──────── │  round1::commit (nonces     │
    │       (public, plaintext event)           │    stay in RAM only)        │
    │  build SigningPackage (per input)         │                             │
    │  ─── round2 request (transport) ────────► │  DISPLAY tx summary,        │
    │                                           │  recompute sighash from     │
    │                                           │  PSBT, human ack (--yes?)   │
    │ ◄──── signature-share ─────────────────── │  round2::sign_with_tweak    │
    │  aggregate_with_tweak(…, None)            │                             │
    │  verify BIP340 sig against Q  ───(bridge derives Q)                     │
    │  finalize PSBT → raw tx                    │                             │
    │  broadcast (optional --broadcast) ───────────────────────────────────► │
    │  any timeout → ABORT, NEW session, fresh subset (never reuse commits)   │
```

Direction: coordinator pulls commitments (round 1), pushes signing packages, pulls signature
shares (round 2), aggregates locally, verifies via the bridge-derived output key `Q`, finalizes.
Secrets never leave the participant; only public commitments and signature shares transit.

### Flow 2 — DKG ceremony (keygen, n=100)

```
Coordinator                               Participant seat i (×100)
   │ ceremony-open (roster hash pinned) ──────►│
   │                                           │ dkg::part1 → round1 package (small)
   │ ◄──── round1-package (broadcast, public) ─│      100 broadcasts total
   │ collect + fan out all round-1 packages ──►│
   │                                           │ dkg::part2 → 99 per-recipient packages
   │                                           │   NIP-44-encrypt each to recipient npub,
   │                                           │   upload as ONE batched bundle (paced)
   │ ◄──── round2-bundle (directed, enc) ──────│   ≈10⁴ directed envelopes across group
   │ route directed pkgs to each recipient ───►│ dkg::part3 → (KeyPackage, PublicKeyPackage)
   │ ◄──── confirmation (verifying key) ───────│   checkpoint KeyPackage encrypted to disk
   │ collect all 100 vk confirmations;        │
   │ any mismatch → ABORT ceremony             │
   │ epoch := 1; roster committed              │
```

Resumability: every arrow is an idempotent event keyed by `(ceremony_id, round, seat)`; a
restarted participant re-reads its own last checkpoint and re-emits (dedup by event id).
**Exception:** DKG round secrets are checkpointed *between* rounds; signing nonces are never.
Refresh, enroll, and repair reuse this exact engine and transport path (§6.3–6.4) — refresh is
another O(n²) round set, enroll is a repair-to-new-identifier immediately chased by a refresh so
helper deltas are proactivized away before the epoch boundary closes.

### Flow 3 — Sweep (true revocation)

```
watch (cron/CI) ── evaluate policy over chain balance + churn ledger + epochs
   │  balance>value_cap OR churn>budget OR epochs>max → exit 2 + JSON report
   ▼
sweep --to standby
   │  chain: list ALL utxos of ACTIVE
   │  bridge: derive STANDBY P2TR address (must already exist via `standby new`)
   │  build one RBF consolidation tx: all inputs → single STANDBY output, feerate arg/estimate
   │  run Flow 1 (signing session) against ACTIVE ── aggregate+verify+finalize+broadcast
   ▼
on confirmation depth ≥ 6
   │  state machine: ACTIVE → RETIRED, STANDBY → ACTIVE
   └─ watch nags until a new STANDBY exists (`standby new`)
```

---

## Architectural Patterns

### Pattern 1: Trait seam for every side effect (chain / transport / store)

**What:** `ChainBackend`, `Transport`, and store access are traits; orchestration depends only on
the trait. Concrete impls (`CoreRpc`/`Esplora`, `NostrTransport`/`FileTransport`) are injected.
**When:** Always in this project — it is the mechanism that lets the crypto value be proven
in-process before transport exists, and gives offline file mode "for free" behind the same interface.
**Trade-offs:** A little boilerplate; in return, deterministic tests and a decoupled build order.

```rust
trait Transport {
    fn publish(&self, env: &Envelope) -> Result<EventId>;
    fn subscribe(&self, filter: MsgFilter) -> Result<Stream<Envelope>>;
}
// InMemoryTransport (tests) · FileTransport (--in/--out) · NostrTransport (relays)
```

### Pattern 2: Resumable, idempotent round engine keyed by `(ceremony_id, round, seat)`

**What:** Ceremony state is a checkpointed transcript of idempotent events; a restart replays from
the last durable checkpoint and dedups by event id.
**When:** All ceremonies (keygen/refresh/enroll/repair). Not signing rounds.
**Trade-offs:** Requires disciplined "checkpoint-between-rounds, never mid-round-secret-for-nonces."

### Pattern 3: Nonce-exclusion by type (structural nonce discipline)

**What:** `SigningNonces` are created inside the session, held in RAM, and the store API has no
method that accepts them; they are not `Serialize`. Crash → fresh nonces on the new session.
**When:** Signing only.
**Trade-offs:** No crash-resume of an in-flight signing round — by design; the alternative is a
key-extraction bug class. This is the single highest-severity rule in the spec.

### Pattern 4: Client-side verification gates (never trust the coordinator)

**What:** Two mandatory client-side checks — (a) display-before-sign: recompute sighash from the
PSBT and ack outputs/amounts/fee; (b) same-key postcondition: new `PublicKeyPackage` verifying key
`==` old after every refresh, else abort and discard.
**When:** Signing (a) and refresh (b).
**Trade-offs:** More work per participant; it is precisely the point at 51 signers.

---

## Scaling Considerations

The scaling axis is **n (membership)**, not user traffic. Everything is single-group.

| Scale | Architecture behavior |
|-------|-----------------------|
| n=2–5 (dev/test) | In-process or single-machine; validates schema, resumption, bridge, sign correctness |
| n=100 signing | Signing cost is O(t)=51, not O(n²) — cheap; one round-trip of commitments + shares |
| n=100 DKG/refresh | **O(n²)**: ≈10⁴ events × ~1 KB ≈ **10 MB per ceremony**, ~99 directed envelopes per sender in round 2 — the real scaling wall |

### Scaling Priorities

1. **First wall — DKG round-2 fan-out.** Mitigate with self-hosted strfry/nostr-rs-relay (≥3),
   raised rate limits + retention for ceremony kinds, **paced round-2 batches** from clients, and
   NIP-44 padding to hide share sizes. Never point a ceremony at a public relay.
2. **Second wall — resumption under partial relay failure.** Publish every event to all relays;
   readers merge+dedup by event id; any one honest reachable relay suffices. Offline file mode is
   the ultimate fallback.
3. **Non-issues.** Coordinator SQLite and watcher are trivial scale; signing throughput is trivial.

---

## Anti-Patterns

### Anti-Pattern 1: Building transport before proving the bridge
**What people do:** Start with the exciting distributed Nostr/relay layer.
**Why it's wrong:** The frost↔rust-bitcoin key bridge is the classic integration-bug surface *and*
the whole value proposition; you can spend weeks on O(n²) relay tuning before discovering the
signature doesn't verify against the address.
**Do this instead:** Prove the bridge + in-process 51-of-100 sign first (build order Phase A/B),
with zero transport. This is exactly milestone M1.

### Anti-Pattern 2: Persisting or resuming signing nonces
**What people do:** Checkpoint the whole signing session for crash-resume.
**Why it's wrong:** Reusing a nonce across signing attempts leaks the share (key extraction).
**Do this instead:** Nonces memory-only, non-`Serialize`; crash → new session, fresh nonces, fresh subset.

### Anti-Pattern 3: Reusing / deriving the Nostr identity key from FROST material
**What people do:** "They're both secp256k1, share one key."
**Why it's wrong:** Couples transport identity to key security; forbidden by the spec.
**Do this instead:** Independent generation; no conversion function between the two key domains.

### Anti-Pattern 4: Trusting the coordinator's hash or same-key claim
**What people do:** Sign the sighash the coordinator sends; accept "refresh kept the key."
**Why it's wrong:** A hostile coordinator could get a quorum to blind-sign an arbitrary tx, or
swap the key.
**Do this instead:** Recompute sighash from the PSBT client-side; recompute + compare verifying key client-side.

### Anti-Pattern 5: Coupling the crypto core to chain/transport/store
**What people do:** Let `crypto/` reach for a node client or serialize straight to Nostr.
**Why it's wrong:** Bloats the audit/reproducible-build surface and breaks in-process testability.
**Do this instead:** Keep `crypto/`+`bridge/` pure; all I/O behind traits above them.

### Anti-Pattern 6: Mixed-epoch shares / reused commitments
**What people do:** Combine shares tagged with different epochs, or reuse round-1 commitments after a timeout.
**Why it's wrong:** Mixed epochs produce garbage; reused commitments break FROST security.
**Do this instead:** Bind `(key_id, epoch)` and reject early with a clear error; new session per timeout.

---

## Integration Points

### External Services / Libraries

| Service | Integration pattern | Gotchas |
|---------|---------------------|---------|
| `frost-secp256k1-tr` ≥3.0 | Direct API = the crypto layer; `serialization` feature for wire types | Confirm exact `refresh_dkg_*` / `repairable::repair_share_*` names against 3.x during Phase A smoke test |
| `rust-bitcoin` | PSBT, `SighashCache::taproot_key_spend_signature_hash`, `Address::p2tr(secp, internal, None, network)` | Merkle root **must** be `None` (BIP86-style); x-only parity is the classic bridge bug — pin it |
| `bitcoincore-rpc` / `esplora-client` | Behind `ChainBackend` trait; `tr()` watch-only descriptor import | Core needs descriptor import to track the address; keep Esplora feature-parity minimal |
| `nostr-sdk` (NIP-44/42/59) | Behind `Transport` trait; multi-relay pool, dedup, roster-pinned event verification | Custom event kinds per message class; NIP-42 AUTH to roster; batch/pace round-2 publishes |
| strfry / nostr-rs-relay (ops) | Not a dependency; ≥3 self-hosted, raised limits/retention | ~10 MB per ceremony; never a public relay |
| `age` + `zeroize` | Encrypt-always at-rest share files; zeroize in memory | No security claim on deletion — sweep is revocation |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| orchestration ↔ adapters | trait objects / generics | Only traits imported; enables in-memory/file/Nostr swap |
| crypto ↔ bridge | typed values (`VerifyingKey`) | Pure; the round-trip test lives at this seam |
| ceremony/session ↔ store | checkpoint API | Nonces excluded from the API by construction |

---

## Dependency-Driven Build Order

Ordered by dependency and risk. The rule: **prove the crypto bridge and end-to-end signing
in-process before any transport exists**, then layer transport → rotation → lifecycle → hardening.
Maps onto SPEC milestones M1–M5 but finer-grained.

| Order | Deliverable | Proves / unblocks | Depends on | SPEC milestone |
|-------|-------------|-------------------|------------|----------------|
| **A** | **Key bridge + byte-level round-trip test** | verifying key → x-only → `XOnlyPublicKey` → P2TR is byte-correct (the classic integration bug) | crypto types, rust-bitcoin | M1 |
| **B** | Crypto core + **in-process 51-of-100 sign** (dealer keygen first for speed, then DKG in-process) | full crypto value: `aggregate_with_tweak(…,None)` → valid BIP340 sig over a regtest key-spend sighash verified against Q — **zero transport** | A, frost-secp256k1-tr | M1 |
| **C** | ChainBackend trait + Core/Esplora + PSBT/sighash on regtest | real PSBTs and sighashes feed the signing session | rust-bitcoin, node | M1→M2 |
| **D** | Storage: participant file store (age, epoch tag, nonce-exclusion) + coordinator SQLite | durable shares + resumable checkpoints; enforces nonce boundary | serde, age, rusqlite | M2 |
| **E** | Transport trait + **FileTransport first**, then message schema, then NostrTransport | out-of-process ceremonies; file mode de-risks the schema before relay/network work | D | M2 / M5 |
| **F** | Ceremony engine: resumable DKG **small-scale (n=5)** → then **n=100 load test** over strfry | idempotency, epoch bookkeeping, relay rate-limit tuning, ~10⁴-event ceremony | crypto, E, D | M2 |
| **G** | Distributed signing session over transport (liveness poll, display-before-sign) | real 51-of-100 sign across the relay set | B, C, E | M2 |
| **H** | Rotation: refresh (removals+proactivize), enroll (repair→refresh), same-key postcondition | membership change, same address, epoch mixing | F | M3 |
| **I** | Lifecycle: standby key, sweep + rollover, watch + policy engine | true revocation path, cron/CI policy | C, H, store | M4 |
| **J** | Hardening: offline file mode polish, reproducible builds, adversarial suite, external review of §6.5 + bridge | ships trust: 100 people verify what they run | all | M5 |

**Why A/B before everything:** the bridge is the highest-risk, lowest-infrastructure component.
Proving it plus an in-process 51-of-100 signature needs no relays, no node RPC, no persistence —
just the crypto crate and rust-bitcoin. If that fails, nothing else matters; if it passes, the
remaining work is "plumbing at scale," which is real but lower-risk. **Why FileTransport before
NostrTransport (E):** identical `Transport` interface, no network/relay complexity — it validates
the message schema and resumption logic deterministically before the O(n²) relay tuning of F.

---

## Sources

- `SPEC-frost-cli.md` (draft v0.1, 2026-07-09) — §2 system model, §3 cryptography, §4 lifecycle,
  §6 flows, §7 transport, §8 storage, §9 Bitcoin, §11 security, §12 libraries, §13 milestones. HIGH.
- `implementations-resharing.md` — source-verified (✅) confirmation of `frost-secp256k1-tr`
  Taproot support, `aggregate_with_tweak()`, refresh/repair/enroll primitives, same-`PublicKeyPackage`
  invariant. HIGH.
- Known crate APIs (`rust-bitcoin` P2TR/sighash, `nostr-sdk` NIP-44/42), knowledge cutoff Jan 2026. HIGH.

---
*Architecture research for: 51-of-100 FROST Taproot signing CLI (cheget)*
*Researched: 2026-07-10*
