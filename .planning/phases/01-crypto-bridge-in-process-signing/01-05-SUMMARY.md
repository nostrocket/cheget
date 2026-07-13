---
phase: 01-crypto-bridge-in-process-signing
plan: 05
subsystem: transport
tags: [rust, transport, trait-seam, in-memory-stub, envelope, dedup, nostr-swap, d-08]

# Dependency graph
requires:
  - phase: 01-01
    provides: "transport/ module seam stub; pinned Phase-1 crate stack"
provides:
  - "transport-agnostic `Transport` trait (sync publish/subscribe) — the load-bearing seam every ceremony phase (3–6) runs against with zero relay code"
  - "opaque-bytes `Envelope` model + `MessageClass` enum shaped for the future Nostr event kinds (SPEC §7)"
  - "content-derived `EnvelopeId` (deterministic FNV-1a) seeding Phase-7 event-id dedup / replay defense (Pitfall 20)"
  - "`Filter` (Nostr-filter-shaped) with directed-vs-broadcast delivery semantics"
  - "`InMemoryTransport` stub (BTreeMap-backed, id-deduping) — 01-04's signing session publishes commitments/packages/shares over it"
affects: [01-04-signing, phase-03-dkg-at-scale, phase-04-rotation, phase-07-transport]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Transport is a trait seam; orchestration depends only on the trait (D-08, Pattern 5)"
    - "Payload is opaque Vec<u8> so NIP-44 v2 encryption slots in at Phase 7 above the seam"
    - "Content-derived stable envelope id → id-based dedup for free (BTreeMap key)"
    - "No nostr-sdk / relay type leaks into the trait, Envelope, or Filter"
    - "Transport addresses seats by a plain u16 Seat — free of both FROST and Nostr types"

key-files:
  created:
    - src/transport/envelope.rs
    - src/transport/inmemory.rs
    - tests/transport_stub.rs
  modified:
    - src/transport/mod.rs

key-decisions:
  - "Envelope id is a deterministic hand-rolled FNV-1a hash over a length-prefixed field encoding (no new dependency, stable across runs/machines unlike DefaultHasher) — a faithful in-process stand-in for the Nostr event id"
  - "Seat modeled as a transport-local `Seat(u16)` newtype, decoupled from `frost::Identifier`, so the seam stays agnostic; the session layer maps FROST id ↔ Seat (↔ npub at Phase 7)"
  - "`publish(&self, Envelope) -> EnvelopeId` (returns the dedup key) via `Mutex<BTreeMap>` interior mutability — usable across threads if a later test simulates seats on multiple threads"
  - "Directed delivery rule: an envelope with `recipient: Some(r)` is delivered ONLY to a filter selecting `recipient == Some(r)`; broadcast (`None`) ignores the filter's recipient field"

patterns-established:
  - "Trait-seam-first transport: every side effect behind a trait; the stub is injected, the real impl swaps in later with no call-site churn"
  - "TDD gate: failing test committed (RED) before the InMemoryTransport implementation (GREEN)"

requirements-completed: []

coverage:
  - id: D1
    description: "Transport trait (publish/subscribe) + opaque-payload Envelope + MessageClass exist; no relay-SDK import under src/transport/; lib builds against only the pinned Phase-1 stack"
    requirement: SIGN-02
    verification:
      - kind: integration
        ref: "cargo build --lib (exit 0); grep -rl 'nostr_sdk|use nostr' src/transport/ is empty"
        status: pass
    human_judgment: false
  - id: D2
    description: "In-memory stub realizes the seam: publish→subscribe delivery, filter exclusion (class/round/seat/id), id-based dedup, directed recipient-scoped delivery"
    requirement: SIGN-02
    verification:
      - kind: unit
        ref: "tests/transport_stub.rs (cargo test --test transport_stub, 4 passed)"
        status: pass
    human_judgment: false

# Metrics
duration: 10min
completed: 2026-07-10
status: complete
---

# Phase 1 Plan 05: Transport Seam (trait + in-memory stub) Summary

**A transport-agnostic `Transport` trait plus an id-deduping in-memory stub — the load-bearing architectural seam (D-08) every later ceremony/signing phase runs against with zero relay code, with an opaque-bytes `Envelope` shaped for the future Nostr event model and no relay type leaking into orchestration.**

## Performance

- **Duration:** ~10 min
- **Completed:** 2026-07-10
- **Tasks:** 2 (Task 2 was TDD: RED → GREEN)
- **Files:** 3 created, 1 modified

## Accomplishments

- `src/transport/envelope.rs` defines the transport-agnostic message unit: an `Envelope { class, ceremony_or_session_id, round, seat, recipient: Option<Seat>, payload: Vec<u8> }` with an **opaque** payload (so NIP-44 v2 encryption slots in at Phase 7 above the seam), a `MessageClass` enum with one variant per SPEC §7 event kind (`CeremonyOpen`, `Round1Package`, `Round2Bundle`, `Commitments`, `SignatureShare`, `Confirmation`, `SessionControl`), and a `broadcast`/`directed` constructor pair.
- A stable, content-derived `EnvelopeId` (hand-rolled deterministic FNV-1a over a length-prefixed field encoding) — the in-process seed of the Phase-7 Nostr event-id dedup / replay defense (Pitfall 20). Identical envelopes share an id; re-publishing is a no-op.
- `src/transport/mod.rs` defines the synchronous `Transport` trait (`publish(&self, Envelope) -> EnvelopeId`, `subscribe(&self, &Filter) -> Vec<Envelope>`) and a Nostr-filter-shaped `Filter` (by class / ceremony-session id / round / seat / recipient) with builder helpers and explicit directed-vs-broadcast matching semantics. No `nostr-sdk` / relay type appears anywhere in the seam.
- `src/transport/inmemory.rs` implements `InMemoryTransport`, a `Mutex<BTreeMap<EnvelopeId, Envelope>>`-backed "simulate all seats in one process" store; keying by id gives dedup for free and deterministic ordering. This is what 01-04's signing session publishes commitments/packages/shares over (SIGN-02 support).
- `tests/transport_stub.rs` pins the four load-bearing behaviors: publish→matching-subscribe delivery, filter exclusion (wrong class/round/seat/id), duplicate-id single-delivery dedup, and directed envelopes reaching only the addressed recipient's filter.

## Task Commits

1. **Task 1: `Transport` trait + opaque-bytes envelope model** — `be6462c` (feat)
2. **Task 2 (TDD RED): failing in-memory transport tests** — `238351f` (test)
3. **Task 2 (TDD GREEN): in-memory stub with id-based dedup** — `3e2ab60` (feat)

_TDD gate satisfied: the `test(...)` RED commit precedes the `feat(...)` GREEN implementation commit._

## Files Created/Modified

- `src/transport/envelope.rs` (created) — `Envelope`, `MessageClass`, `Seat`, `EnvelopeId`, `broadcast`/`directed` constructors, deterministic `id()`.
- `src/transport/inmemory.rs` (created) — `InMemoryTransport` (BTreeMap + Mutex), dedup `publish`, filtering `subscribe`.
- `tests/transport_stub.rs` (created) — 4 behavior tests.
- `src/transport/mod.rs` (modified) — replaced the placeholder stub with the `Transport` trait, `Filter`, and re-exports.

## Decisions Made

- **Deterministic FNV-1a envelope id (no new dependency):** the project has no hash crate in its pinned deps, and `std`'s `DefaultHasher` is not stable across runs/machines. A hand-rolled FNV-1a over a length-prefixed canonical field encoding is trivial, dependency-free, and genuinely stable — a faithful in-process stand-in for the Nostr event id, so it seeds the Phase-7 event-id dedup rather than being throwaway.
- **Transport-local `Seat(u16)` newtype:** rather than reuse `crypto::types::SeatId` (`frost::Identifier`), the seam addresses seats by a plain integer so it stays free of BOTH FROST and Nostr types. The session layer owns the `frost::Identifier ↔ Seat` mapping (and `Seat ↔ npub` at Phase 7). `u16` covers `n = 100`.
- **`publish` returns the `EnvelopeId` and uses `Mutex<BTreeMap>` interior mutability:** `&self` matches the trait and the future shared-relay-client shape; returning the id lets callers learn the dedup key; `Mutex` keeps the stub usable if a later test simulates seats across threads.
- **Directed-vs-broadcast matching rule made explicit in `Filter::matches`:** a directed envelope (`recipient: Some(r)`) matches only a filter selecting that recipient; a broadcast envelope (`None`) ignores the filter's recipient field. This is asserted directly in the tests.

## Deviations from Plan

None — the plan executed exactly as written. No auto-fixes (Rules 1–3) were needed; no architectural decisions (Rule 4) arose. The `Filter` type and builder helpers, while not enumerated field-by-field in the plan's artifact sketch, are the natural realization of the plan's `subscribe(filter)` contract and RESEARCH Open Q3's "publish/subscribe(filter) pair" recommendation.

## Issues Encountered

- None. `cargo build --lib` exit 0; `cargo test --test transport_stub` 4 passed; the `nostr` leak check under `src/transport/` is clean.

## Known Stubs

None that block this plan's goal. `InMemoryTransport` is itself the intended Phase-1 stub (D-08): it is fully functional and test-pinned. The real `FileTransport` / `NostrTransport` impls are Phase 7, explicitly behind this same trait; the opaque `payload` and absence of any relay type in the seam are what make that swap call-site-free. Envelope authenticity (the Nostr event's BIP340 signature) and payload confidentiality (NIP-44 v2) are Phase 7 concerns layered above the opaque payload — documented in the threat register as `accept` for Phase 1's in-process-only stub.

## Threat Surface

No new security surface beyond the plan's `<threat_model>`. The two `mitigate` dispositions are both satisfied structurally:

- **T-01-transportleak (trait seam purity):** no `nostr-sdk` / relay concrete leaks into the trait, `Envelope`, or `Filter` — verified by the automated `grep` check.
- **T-01-replay (duplicate delivery):** `InMemoryTransport` dedups by the stable content-derived `EnvelopeId`, seeding the Phase-7 event-id dedup.

`T-01-transport-conf` remains `accept` for Phase 1 (in-process only; NIP-44 v2 / roster pinning are Phase 7).

## Next Phase Readiness

- 01-04's signing session can now collect round-1 commitments and distribute the signing package over `Transport` (SIGN-02), running entirely in-process against `InMemoryTransport`.
- Phase 3 (DKG-at-scale, n=100 in-process), Phase 4 (rotation), and Phase 5 (lifecycle) all validate locally against this seam with zero relay code.
- Phase 7 swaps in `FileTransport` / `NostrTransport` behind the unchanged trait; the opaque payload is where NIP-44 v2 encryption lands, and `MessageClass` is where the per-class custom event kinds map.

## Self-Check: PASSED

- All 4 plan files verified present on disk (`src/transport/mod.rs`, `envelope.rs`, `inmemory.rs`, `tests/transport_stub.rs`).
- All 3 task commits verified in git history: `be6462c`, `238351f`, `3e2ab60`.
- `cargo build --lib` exit 0; `cargo test --test transport_stub` 4 passed; no `nostr` reference under `src/transport/`.

---
*Phase: 01-crypto-bridge-in-process-signing*
*Completed: 2026-07-10*
