# Phase 1: Crypto Bridge & In-Process Signing - Context

**Gathered:** 2026-07-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Prove the entire cryptographic value of `tsig` **in-process** — DKG → BIP341 address →
two-round tweaked signing → a **confirmed regtest key-spend** — with zero transport, zero
relays, and zero encrypted-at-rest persistence, and with the four structural security
controls present from the first line of signing code:

1. Non-serializable signing-nonce type (won't compile if persisted)
2. Byte-level frost→rust-bitcoin bridge round-trip pinned by a known-answer vector
3. Tweak/aggregate output verified against the output key `Q`
4. Display-before-sign gate (each signer recomputes the sighash from the PSBT)

Also introduces the two architectural trait seams that every later phase depends on:
- **`Transport` trait** + an in-memory/in-process stub (the seam that lets ceremonies run
  locally with no relay code; Phase 7 swaps in real `FileTransport`/`NostrTransport`).
- **`ChainBackend` trait** + a Bitcoin Core RPC backend and an Esplora backend.

**Scope-expanding decision (this discussion):** Phase 1 now runs the **full n=100 DKG**,
absorbing the correctness + O(n²) compute-measurement portions of the former Phase 3. See
Deferred Ideas for the required ROADMAP edit.

**Requirements in scope:** KEY-01, KEY-02, KEY-03, KEY-04, KEY-05, KEY-06 (moved in),
SIGN-01…SIGN-07, STOR-04.

**Explicitly NOT in this phase:** any real network transport (Nostr/file — Phase 7);
encrypted at-rest secret storage / SQLite coordinator store (Phase 2); membership rotation
(Phase 4); sweep/lifecycle/policy (Phase 5); hardening/reproducible-build/external review
(Phase 6).

</domain>

<decisions>
## Implementation Decisions

### Group size, parameterization & phase scope
- **D-01:** The DKG and signing code is written **generic over `(t, n)`**. Fast unit tests
  may exercise tiny sizes (e.g. 2-of-3, 3-of-5) for TDD speed.
- **D-02:** The **real acceptance target is `t=51`, `n=100`** run fully in-process. The
  end-to-end proof (DKG → address → sign → aggregate → verify against `Q` → broadcast →
  confirm on regtest) runs at n=100. "Always run real where it counts."
- **D-03:** **Phase 3 folds into Phase 1.** Phase 1 absorbs the n=100 DKG *correctness*
  proof (all 100 `KeyPackage`s verify to one group `PublicKeyPackage` — KEY-06) and the
  **O(n²) timing/memory measurement** (part1/part2/part3 instrumentation across 100 seats).
  No persistence is needed for either, so both fit Phase 1's "zero persistence" constraint.
- **D-04:** The **persist/reload-the-n=100-set-at-scale** check (former Phase 3 criterion 3,
  which requires the Phase 2 stores) **moves to Phase 2**. Phase 3 ceases to exist as a
  standalone phase. → **Requires a ROADMAP edit** (see Deferred Ideas).

### Regtest proof & CI
- **D-05:** The regtest `bitcoind` is **auto-spawned by the test harness** via the
  `corepc-node` / `bitcoind` crate (throwaway regtest node on a temp datadir, pinned Core
  version). Hermetic, reproducible, no external node setup, CI-friendly.
- **D-06:** **Tiered CI.** Every PR gates on: the bridge known-answer vector test + a
  **small-n** end-to-end (DKG→sign→confirm on regtest, e.g. 3-of-5) + build/`cargo audit`.
  The **full `t=51`/`n=100`** end-to-end runs **nightly and on-demand**, and MUST pass
  before Phase 1 sign-off. Keeps PR feedback fast without weakening the "prove it real" bar.
- **D-07:** The **Core RPC backend fronts the confirmed-key-spend path** (native regtest
  mining via `generatetoaddress`). The **Esplora backend is still built to the same
  `ChainBackend` trait** and covered by trait-conformance/unit tests (mocked or against a
  public endpoint) — this satisfies STOR-04 — but Esplora is **not** in the n=100 confirm
  path (no electrs/esplora-over-regtest stack stood up in this phase).

### CLI surface vs test-harness boundary
- **D-08:** Ship the **real subcommand skeleton** (clap persona tree) and wire keygen/sign
  to run against the **in-memory `Transport` stub** in a "simulate all seats in one process"
  mode. Commands are real entry points; the stub stands in for the network so Phase 7 can
  swap in Nostr behind the same seam with no call-site churn.
- **D-09:** **State/key flow without a persistence layer:** public artifacts (the
  `PublicKeyPackage` / group verifying key) are written to **plaintext files** (they are
  public, not secret); `tsig address --pubkey <file>` reads one. **Secret share material
  never touches disk** — it lives only in the simulating process for the duration of a run.
  This draws the clean line for Phase 2: public artifacts on disk are fine now; the
  age/scrypt-encrypted *secret* store is what Phase 2 adds.

### Bridge known-answer vector (KEY-03) & parity
- **D-10:** The bridge round-trip test is anchored to the **official BIP341 taproot-tweak /
  BIP86 key-path published test vectors** (known internal key → known output key → known
  scriptPubKey/address). Externally auditable by anyone against the BIPs — the strongest
  trust story for "100 people must verify what they run."
- **D-11:** **Parity contract:** the crypto core applies frost's `EvenY` so the group key is
  always even-Y, and **the bridge asserts this invariant** (rejects/normalizes odd-Y
  defensively rather than blindly stripping the SEC1 prefix). The KAT suite **covers BOTH an
  even-Y and an odd-Y-origin vector**, each verified end-to-end (produce a signature that
  verifies against `Q`) — turning the classic silent parity bug into an explicitly covered
  case.

### Claude's Discretion
Left to research/planning against the frost 3.0 and rust-bitcoin 0.32 APIs (grounded in the
SPEC + PITFALLS.md), unless a decision above constrains them:
- The exact mechanism for the **non-serializable nonce type** (SIGN-05) — e.g. a newtype
  around frost `SigningNonces` with no `Serialize` impl + a `trybuild` compile-fail test
  proving persistence won't compile.
- The **`Transport` trait contract** (sync/async, message/envelope model) — shaped to fit
  the later Nostr event model but concretely only needs the in-memory stub now.
- The **`ChainBackend` trait contract** (UTXO listing, fee estimation, broadcast, sighash
  helpers, watch-only descriptor import).
- **Liveness poll / 51-of-100 subset selection** logic and how the coordinator drives it
  over the in-memory transport.
- **Display-before-sign UX** specifics (what's rendered, `--yes` behavior) per SIGN-07.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design & spec (authoritative)
- `SPEC-frost-cli.md` — full design; §3 Cryptography (group key = internal key `P`, key-only
  P2TR `Q`, `aggregate_with_tweak(…, None)`), §6.5 Signing / nonce discipline, §9 Bitcoin
  integration & the byte-level bridge, §11 Security considerations (normative). **The crypto
  layer of this project *is* the `frost-secp256k1-tr` public API described here.**
- `.planning/research/PITFALLS.md` — implementation pitfalls for the FROST Taproot bridge;
  **highest-value read for Phase 1** (parity/even-Y, tweak application, sighash correctness).
- `implementations-resharing.md` — companion research on resharing/repair primitives and
  library selection rationale.

### Project planning
- `.planning/PROJECT.md` — locked crate stack + pins, `t=51`/`n=100`, DKG-only keygen,
  Key Decisions table (already-locked constraints — do not re-litigate).
- `.planning/REQUIREMENTS.md` — KEY-01…KEY-06, SIGN-01…SIGN-07, STOR-04 (the requirements
  this phase must satisfy).
- `.planning/ROADMAP.md` — Phase 1 success criteria. **Note:** Phase 3 fold-in (D-03/D-04)
  means the roadmap must be edited before it fully reflects this phase's scope.

### Crate stack (versions pinned in PROJECT.md / CLAUDE.md)
- `frost-secp256k1-tr` 3.0.0 (+ `frost-core`) — DKG (`keys::dkg::part1/2/3`),
  `round2::sign_with_tweak`, `aggregate_with_tweak`, `keys::Tweak` / `keys::EvenY`.
- `bitcoin` 0.32.101 — `SighashCache::taproot_key_spend_signature_hash`, `Address::p2tr`,
  `XOnlyPublicKey`.
- `bitcoincore-rpc` 0.19.0, `esplora-client` 0.13.0 — the two `ChainBackend` impls.
- `corepc-node` / `bitcoind` crate — auto-spawn regtest node in tests (D-05).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Greenfield** — no Rust source or `Cargo.toml` exists yet. Phase 1 establishes the crate
  skeleton (`bridge/`, crypto-core wrapper, `Transport` + `ChainBackend` traits, CLI).

### Established Patterns
- **Trait-seamed monolith** (from ROADMAP overview): the `Transport` trait with an in-memory
  stub is the seam that makes local-first development possible — every ceremony phase runs
  against it with no relay code. Establish this seam cleanly in Phase 1; do not leak
  transport concretes into ceremony/session logic.
- **Bridge is byte-level only** (PROJECT.md compat notes): FROST (`k256`) and rust-bitcoin
  (`secp256k1` C lib) are different curves' crates — never pass a `k256` point into
  rust-bitcoin; go 33-byte SEC1 → 32-byte x-only → `XOnlyPublicKey::from_slice`.

### Integration Points
- `Transport` trait ← in-memory stub (Phase 1) → real impls (Phase 7).
- `ChainBackend` trait ← Core RPC + Esplora (Phase 1) → reused by sweep/watch (Phase 5).
- Public-artifact file format (`PublicKeyPackage`) ← Phase 1 plaintext → Phase 2 encrypted
  secret store builds alongside it.

</code_context>

<specifics>
## Specific Ideas

- "Always run real where it counts": the n=100 end-to-end is the real proof; tiny sizes are
  only a TDD convenience, never the acceptance bar.
- The bridge KAT must be **auditable against the BIPs themselves**, not self-referential.
- The odd-Y vector is a deliberate adversarial case, not an edge nicety — it is *the* bug the
  bridge test exists to catch.

</specifics>

<deferred>
## Deferred Ideas

- **ROADMAP EDIT REQUIRED (roadmap action, not a new capability):** Fold former Phase 3 into
  Phase 1 — move KEY-06 (n=100 DKG correctness) and the O(n²) compute measurement into
  Phase 1; move the persist/reload-at-scale check into Phase 2; delete Phase 3 and renumber
  Phases 4→3, 5→4, 6→5, 7→6. Run **`/gsd-phase`** after this discussion to apply the edit and
  keep `ROADMAP.md` / `REQUIREMENTS.md` traceability in sync. (Captured here so the intent
  isn't lost; the discuss workflow does not edit the roadmap.)
- **Esplora-over-regtest (electrs) confirm path** — deferred; Core fronts the confirm in
  Phase 1 (D-07). Revisit only if a real need to exercise Esplora against confirmed spends
  emerges.

### Reviewed Todos (not folded)
None — no pending todos matched this phase.

</deferred>

---

*Phase: 1-crypto-bridge-in-process-signing*
*Context gathered: 2026-07-10*
