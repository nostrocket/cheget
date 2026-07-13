# tsig

`tsig` is a single-binary Rust command-line tool that lets a fixed **51-of-100**
group jointly control one Bitcoin Taproot address using FROST threshold Schnorr
signatures (RFC 9591, secp256k1, BIP340/341 key-path spend). On-chain, its spends
are indistinguishable from ordinary single-sig. The CLI is organized into three
personae — participant, coordinator, watcher — selected by subcommand.

> **Status:** early. Only the cryptographic core (Phase 1) is implemented today.
> It runs entirely **in-process on a single host** — there is no networking,
> no relay/transport layer, and no persistence yet. Everything under
> "Planned" below is designed but not built. See
> [`.planning/ROADMAP.md`](.planning/ROADMAP.md) and the draft design in
> [`SPEC-frost-cli.md`](SPEC-frost-cli.md) for the full end-state vision.

## Core value (the goal)

The project aims to let a group of 100 jointly control a single Bitcoin address
such that:

1. **any 51 can spend** — a `t = 51`, `n = 100` FROST threshold;
2. **no individual ever holds the key** — the group signing key exists only as a
   sharing, never reconstructed on any one machine;
3. **membership can rotate with zero on-chain footprint** — refresh/enroll/repair
   change who holds a share without changing the address or touching the chain;
   and
4. **past compromise is truly revocable** — by sweeping all funds to a
   pre-generated standby key.

The threshold is fixed: `t = 51`, `n = 100`, and never changes. Properties 3 and 4
are design goals for later phases; today only the crypto core that makes property
1 and 2 possible in-process is built.

## Status: what works today (Phase 1)

Phase 1 proves the whole cryptographic value in-process — DKG → BIP341 address →
two-round tweaked signing → a confirmed regtest key-spend — with zero transport and
no persistence. What is implemented and tested:

| Capability | Where | Pinned by |
|------------|-------|-----------|
| **frost↔rust-bitcoin key bridge** — FROST `VerifyingKey` → 33-byte SEC1 → x-only → `XOnlyPublicKey` → BIP341 P2TR address (merkle root `None`) and output key `Q` | `src/bridge/taproot.rs` | byte-level round-trip / known-answer test in `tests/bridge_roundtrip.rs` (against `tests/vectors/bip341_keyspend.json`) |
| **In-process DKG** — simulated participants, no transport, producing a group `PublicKeyPackage` whose verifying key is the Taproot internal key `P`, with client-side key confirmation | `src/crypto/` | `tests/dkg_small.rs`, `tests/dkg_100_correctness.rs` |
| **In-process two-round FROST signing** — Taproot tweak via `sign_with_tweak` / `aggregate_with_tweak(…, None)`, producing a 64-byte BIP340 signature that verifies against the output key `Q`, finalizing a PSBT and broadcasting a **confirmed** key-spend on regtest | `src/session/` | crown-jewel test `tests/inproc_sign_100.rs::inproc_sign_confirmed_regtest_key_spend_51_of_100` (`#[ignore]`, run on demand) |
| **`Transport` trait + in-memory/in-process stub** — the architectural seam later ceremony phases run against; no relay or Nostr code exists yet | `src/transport/` | `tests/transport_stub.rs` |
| **`ChainBackend` trait** — Bitcoin Core JSON-RPC and Esplora implementations plus a key-spend sighash helper | `src/chain/` | `tests/chain_backend_conformance.rs`, `tests/regtest_fixture.rs` |

All of the above runs on a **single host with no networking**. The DKG uses
simulated participants in one process; signing runs the coordinator and all
signers in-process against the in-memory transport stub.

**CLI status (honest):** the clap persona tree (participant / coordinator /
watcher) is scaffolded in `src/cli/`, but only `tsig watcher address` — which
prints the group's BIP341 P2TR address from a public-key-package file — is wired
end-to-end from the command line. The `keygen` and `sign` flows exist as the
in-process, test-driven paths (`tests/inproc_sign*.rs`, `tests/dkg_*.rs`); their
CLI handlers are not yet a polished multi-command UX.

**Structural security controls (present from Phase 1):**

- signing nonces use a **non-serializable type** — the code will not compile if you
  try to persist them (`src/crypto/nonce.rs`, proven by the compile-fail test
  `tests/compile_fail.rs` / `tests/ui/nonce_no_serialize.rs`);
- **display-before-sign**: the sighash is recomputed locally from the PSBT before
  round 2 (`src/session/display.rs`);
- the tweak/aggregate result is **verified against the output key `Q`** before a
  signature is accepted;
- FROST 3.0 cheater-detection culprits are surfaced on abort
  (`tests/sign_adversarial.rs`).

**Measured local timings (optional, informational):** on one developer machine the
full 51-of-100 regtest key-spend runs in ~9.9 s and the DKG group-key proof in
~4.4 s. These are local measurements, not benchmarks.

## Planned (not yet built)

The following are designed in the roadmap and spec but **not implemented**. In
particular, **no transport, relay, or Nostr code exists yet**, and there is **no
persistence layer yet** — all current state lives in memory for the duration of a
test run.

- **Phase 2 — Persistence & storage:** at-rest share encryption (age/scrypt) with
  in-memory zeroize, encrypted between-round ceremony checkpointing, and a
  coordinator SQLite store (roster, transcripts, logs, policy, churn ledger).
- **Phase 3 — DKG at scale (local):** scale the in-process DKG to the full n=100
  share set on one host and measure the O(n²) computation cost locally.
- **Phase 4 — Membership rotation:** refresh (removals + proactivize), enroll
  (repair/RTS + immediate refresh), and repair, over the in-memory stub, with the
  mandatory client-side same-key check and epoch discipline.
- **Phase 5 — Key lifecycle & revocation:** standby-key pre-generation, the sweep
  flow (all UTXOs → standby, rollover on confirmation), and a policy-driven watcher.
- **Phase 6 — Hardening & security-reviewable release:** reproducible builds,
  pinned/audited dependencies (`cargo audit` / `cargo deny` in CI), a
  locally-verifiable adversarial test suite, and external review.
- **Phase 7 — Transport & transport at scale:** the real `Transport`
  implementations behind the existing trait — offline `FileTransport` first, then a
  Nostr transport (NIP-44 v2 / NIP-42, multi-relay pool, roster pinning) — plus the
  gating n=100 DKG relay load test and transport-dependent adversarial tests. Nostr
  identity keys will be transport-only, independently generated, and never derived
  from or reused as FROST material.

See [`.planning/ROADMAP.md`](.planning/ROADMAP.md) for the full phase breakdown.

## Architecture

`tsig` is a layered, trait-seamed monolith built bottom-up (see `src/lib.rs`):

- **`bridge`** — pure, I/O-free: the single canonical frost→rust-bitcoin key seam
  (`VerifyingKey` → x-only → `XOnlyPublicKey` → BIP341 P2TR + output key `Q`).
- **`crypto`** — pure, I/O-free: the FROST wrapper (DKG, tweaked signing, the
  non-serializable nonce type).
- **`chain`** — the `ChainBackend` trait plus Core RPC / Esplora implementations.
- **`transport`** — the `Transport` trait plus its in-memory stub. This is the
  load-bearing seam: every later ceremony phase runs against this trait, so DKG,
  rotation, lifecycle, and hardening can all be proven locally with zero relay code,
  and Phase 7 swaps in real transport behind the same trait.
- **`session`** — the two-round signing session; owns the nonce lifetime (RAM only).
- **`cli`** — the clap persona tree (participant / coordinator / watcher).

`bridge` and `crypto` are kept I/O-free so the auditable, reproducible trusted
computing base stays small. For the full design rationale see
[`SPEC-frost-cli.md`](SPEC-frost-cli.md) and [`.planning/ROADMAP.md`](.planning/ROADMAP.md).

## Building & testing

```sh
# Build the release binary.
cargo build --release

# Run the standard test suite.
cargo test
```

The heavy full-scale tests — notably the crown-jewel confirmed-regtest key-spend
`tests/inproc_sign_100.rs::inproc_sign_confirmed_regtest_key_spend_51_of_100` — are
marked `#[ignore]` and run on demand (they spin up a throwaway regtest `bitcoind`
via `corepc-node`). Run an ignored test explicitly, for example:

```sh
cargo test --release -- --ignored inproc_sign_confirmed_regtest_key_spend_51_of_100
```

**MSRV is Rust 1.85**, and `Cargo.lock` is committed so builds are reproducible —
verifiability is a first-class goal (many people must be able to verify what they
run). There is no packaged release: build from source with the commands above.

## Security model (current)

Only properties the code enforces today are claimed:

- **Nonces are never persisted.** Signing nonces use a type that cannot be
  serialized; attempting to persist one is a compile error, enforced by a
  compile-fail test.
- **Display-before-sign.** Each signer recomputes the key-spend sighash locally
  from the PSBT before round 2 — no blind signing of a coordinator-supplied hash.
- **No individual holds the key.** The group signing key exists only as a FROST
  sharing produced by in-process DKG; it is never reconstructed on one machine.
- **Signatures are verified against `Q`.** The tweak/aggregate output is checked
  against the Taproot output key `Q` before acceptance, and FROST 3.0
  cheater-detection surfaces culprits on abort.

Security properties tied to later phases — at-rest encryption, epoch discipline,
same-key rotation checks, standby/sweep revocation, and Nostr↔FROST key separation
— are **not yet enforced** and are described above as planned.

## License

Licensed under either of MIT or Apache-2.0 at your option.
