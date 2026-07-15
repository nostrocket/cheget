# cheget

`cheget` is a single-binary Rust command-line tool that lets a fixed **51-of-100**
group jointly control one Bitcoin Taproot address using FROST threshold Schnorr
signatures (RFC 9591, secp256k1, BIP340/341 key-path spend). On-chain, its spends
are indistinguishable from ordinary single-sig. The CLI is organized into three
personae — participant, coordinator, watcher — selected by subcommand.

> **Status:** early. The cryptographic core (Phase 1) and the persistence &
> storage **layer** (Phase 2) are implemented today: an age/scrypt participant
> store, encrypted between-round ceremony checkpointing, a coordinator SQLite
> store, and two read-only inspector commands (`share-status`, `roster`). It
> still runs entirely **in-process on a single host** — there is no networking
> and no relay/transport layer — and **no CLI flow yet persists or consumes a
> secret share**, so the persisted-key ceremony/signing flow that would wire the
> storage layer into fund custody is later-phase work. Everything under
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
are design goals for later phases; today the crypto core that makes properties 1
and 2 possible in-process is built, together with the Phase 2 persistence &
storage layer (not yet wired into a fund-custody signing flow).

## Status: what works today (Phases 1–2)

Phase 1 proves the whole cryptographic value in-process — DKG → BIP341 address →
two-round tweaked signing → a confirmed regtest key-spend — with zero transport.
Phase 2 adds the persistence & storage **layer** beneath it. The Phase 1 crypto
core is implemented and tested as follows:

| Capability | Where | Pinned by |
|------------|-------|-----------|
| **frost↔rust-bitcoin key bridge** — FROST `VerifyingKey` → 33-byte SEC1 → x-only → `XOnlyPublicKey` → BIP341 P2TR address (merkle root `None`) and output key `Q` | `src/bridge/taproot.rs` | byte-level round-trip / known-answer test in `tests/bridge_roundtrip.rs` (against `tests/vectors/bip341_keyspend.json`) |
| **In-process DKG** — simulated participants, no transport, producing a group `PublicKeyPackage` whose verifying key is the Taproot internal key `P`, with client-side key confirmation | `src/crypto/` | `tests/dkg_small.rs`, `tests/dkg_100_correctness.rs` |
| **In-process two-round FROST signing** — Taproot tweak via `sign_with_tweak` / `aggregate_with_tweak(…, None)`, producing a 64-byte BIP340 signature that verifies against the output key `Q`, finalizing a PSBT and broadcasting a **confirmed** key-spend on regtest | `src/session/` | crown-jewel test `tests/inproc_sign_100.rs::inproc_sign_confirmed_regtest_key_spend_51_of_100` |
| **`Transport` trait + in-memory/in-process stub** — the architectural seam later ceremony phases run against; no relay or Nostr code exists yet | `src/transport/` | `tests/transport_stub.rs` |
| **`ChainBackend` trait** — Bitcoin Core JSON-RPC and Esplora implementations plus a key-spend sighash helper | `src/chain/` | `tests/chain_backend_conformance.rs`, `tests/regtest_fixture.rs` |

All of the above runs on a **single host with no networking**. The DKG uses
simulated participants in one process ("simulate-all-seats"); signing runs the
coordinator and all signers in-process against the in-memory transport stub.

**Phase 2 — persistence & storage layer (shipped, not yet wired into custody).**
The storage layer is implemented and tested: an **age/scrypt participant store**
for at-rest share encryption (with in-memory `zeroize`), **encrypted
between-round ceremony checkpointing**, and a **coordinator SQLite store**
(roster and related public state). Two read-only inspector commands expose it
from the CLI: `participant share-status` (lists held shares by reading the
plaintext manifest — no decryption, no passphrase prompt) and
`coordinator roster` (lists the roster from the public SQLite store). This is the
storage **layer only**: `keygen` still writes only the public `PublicKeyPackage`
envelope, `sign` still runs an in-process simulate-all-seats DKG, and **no CLI
command persists or consumes a secret share yet** — so the manifest
`share-status` reads is not populated by the current `keygen`/`sign` flows. The
persisted-key ceremony/signing flow that wires this layer into fund custody is
later-phase work.

### CLI surface today (honest)

The clap persona tree (participant / coordinator / watcher) dispatches to real
handlers. Seven subcommands run end-to-end from the command line — participant
`keygen`/`sign`/`share-status`, coordinator `keygen`/`sign`/`roster`, and watcher
`address` (participant and coordinator `keygen`/`sign` share the same in-process
handler). The two ceremony commands (`keygen`, `sign`) still run an **in-process
simulate-all-seats DKG** — there is no transport, no multi-party rounds over a
network, and no persisted secret share. `share-status` and `roster` are the two
read-only inspectors added in Phase 2.

```text
cheget
├── participant
│   ├── keygen        in-process DKG; writes ONLY the public key package
│   ├── sign          in-process DKG + two-round signing over a supplied PSBT
│   └── share-status  read-only; lists held shares from the plaintext manifest;
│                     never unlocks or prompts for a passphrase
├── coordinator
│   ├── keygen        (same in-process handler as participant keygen)
│   ├── sign          (same in-process handler as participant sign)
│   └── roster        read-only; lists the roster from the coordinator's public
│                     SQLite store
└── watcher
    └── address       derive the group's BIP341 P2TR address (offline, no network)
```

- **`watcher address`** — fully usable offline. Reads a public-artifact file (a
  serialized `PublicKeyPackage` envelope: `key_id`, `epoch`, `pubkey_package_hex`)
  and prints the group's BIP341 P2TR address. No secret material is read, and no
  network call is made.

  ```text
  cheget watcher address --pubkey <FILE> [--network <NETWORK>]
  ```

  `--network` accepts exactly `bitcoin` (mainnet, `bc1p…`), `testnet`
  (`tb1p…`), `signet` (`tb1p…`), or `regtest` (`bcrt1p…`). The default is
  `bitcoin`.

- **`participant keygen` / `coordinator keygen`** — run an in-process DKG and
  write **only** the public `PublicKeyPackage` envelope to `--out`; the secret
  shares live in the process for the run and are never serialized. Without
  `--full` a fast **3-of-5** ceremony runs; `--full` runs the real **51-of-100**
  acceptance target; explicit `--seats` / `--threshold` override both.

  ```text
  cheget coordinator keygen --key-id <ID> --out <FILE> [--full] \
                            [--seats <N>] [--threshold <T>] [--ceremony <NAME>]
  ```

  The written file is exactly what `watcher address --pubkey <FILE>` consumes.

- **`participant sign` / `coordinator sign`** — a **self-contained demonstration**
  of the two-round signing pipeline. Because no secret share is persisted, the
  command runs its own in-process DKG, derives the group address, and signs the
  supplied `--psbt` against it. The PSBT must therefore spend the address this run
  derives — it is not signing of an externally-funded, persisted-key wallet
  (that persisted-key custody flow is later-phase work; Phase 2 shipped only the
  storage layer, not a signing flow that consumes a persisted share). The default
  network is `regtest`. Signing is gated by
  a display-before-sign acknowledgement; `--yes` skips only the interactive human
  ack (for automation/regtest), never the local sighash recompute.

  ```text
  cheget coordinator sign --psbt <FILE> [--network <NETWORK>] [--yes] \
                          [--full] [--seats <N>] [--threshold <T>] [--session <NAME>]
  ```

- **`participant share-status`** — a read-only inspector introduced in Phase 2.
  It reads the plaintext share manifest from the store root and lists the held
  shares. It performs **no decryption** and **never prompts for a passphrase**
  (D-05) — it does not unlock any encrypted share and reads no `--pubkey` file.
  Note that no CLI flow yet persists a secret share, so on a normal
  `keygen`/`sign` install this manifest is empty; the command reports what the
  storage layer holds, which current ceremony flows do not populate.

  ```text
  cheget participant share-status [--home <PATH>]
  ```

  `--home` overrides the store root (otherwise `CHEGET_HOME` or `~/.cheget`).

- **`coordinator roster`** — a read-only inspector introduced in Phase 2. It
  lists the roster from the coordinator's **public SQLite store** (STOR-03). It
  reads only public roster data — no secret material and no network call.

  ```text
  cheget coordinator roster [--key-id <ID>] [--home <PATH>]
  ```

  `--key-id` selects the group-key label (`active` | `standby`, default
  `active`); `--home` overrides the store root (otherwise `CHEGET_HOME` or
  `~/.cheget`).

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

## How to use on regtest

Everything in this section works today. There is **no standing `cheget` daemon**
that talks to a live regtest node — live signing all the way to a confirmed
key-spend is exercised through the integration test harness, which spins up a
throwaway node for you.

**1. End-to-end confirmed key-spend (the crown jewel).** The full pipeline — DKG →
address → fund → sign → aggregate-with-tweak → verify against `Q` → finalize →
broadcast → **confirm** — runs as an integration test that auto-spawns a throwaway
regtest `bitcoind` via the `corepc-node` dev-dependency. No manual node setup is
needed:

```sh
cargo test --release --test inproc_sign_100 -- --nocapture
```

Release is strongly recommended: on one developer machine the confirmed 51-of-100
key-spend runs in roughly ~9 s under `--release` versus ~90 s in a debug build
(local measurements, not benchmarks). The scale is overridable via the
`CHEGET_SIGN_T` / `CHEGET_SIGN_N` environment variables (default 51 / 100) to
capture faster intermediate data points.

**2. Derive a regtest address (offline).** From any public-key-package file:

```sh
cheget watcher address --pubkey pk.json --network regtest
# → bcrt1p...
```

**3. Run the in-process signing pipeline locally.** The `sign` command defaults to
the `regtest` network and drives the real two-round signing flow over the
in-memory transport stub. It is self-contained: it runs its own in-process DKG and
the supplied PSBT must spend the address it prints.

```sh
cheget coordinator sign --psbt spend.psbt --network regtest --yes
```

## How to use on mainnet

Only **one** mainnet action is possible today, and it is offline and safe: deriving
the group's mainnet P2TR address from a public `PublicKeyPackage`. It makes no
network calls and touches no secret material.

```sh
# 1. Produce a public key package (in-process DKG; writes only public data).
cheget coordinator keygen --key-id active --out pk.json

# 2. Derive the mainnet address from that public package.
cheget watcher address --pubkey pk.json --network bitcoin
# → bc1p...
```

> **Safety / status — read this before doing anything with real funds.**
> This is **early, pre-custody** software and is **not production-audited**. The
> full mainnet signing and custody ceremony — multi-party signing rounds over a
> transport, signing of an externally-funded PSBT, broadcast, watching, and the
> sweep to a standby key — is **NOT yet wired** and arrives in later phases (see
> Planned, below). The Phase 2 storage layer exists but is **not yet wired into a
> fund-custody signing flow**: the in-process `keygen`/`sign` commands simulate
> all seats in one process and **no CLI command persists or consumes a secret
> share**, so they cannot custody funds for a real 100-member group. **Do not
> entrust real funds to flows that do not exist yet.** The only mainnet-safe
> action today is offline address derivation.

## Planned (not yet built)

The following are designed in the roadmap and spec but **not implemented**. In
particular, **no transport, relay, or Nostr code exists yet**, and the Phase 2
storage layer is **not yet wired into any fund-custody signing flow** — no CLI
command persists or consumes a secret share.

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

`cheget` is a layered, trait-seamed monolith built bottom-up (see `src/lib.rs`):

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

There is no packaged release — the crate is not published to any registry. Build
from source:

```sh
# Build (debug).
cargo build

# Build the optimized release binary.
cargo build --release

# Run the standard test suite.
cargo test
```

Two full-scale tests exercise the real 51-of-100 acceptance target and now **run by
default** (no `#[ignore]` — an earlier version of this README wrongly claimed they
had to be run with `--ignored`). Because both are O(n²), the release profile is
strongly recommended:

```sh
# Crown-jewel: confirmed regtest key-spend at t=51, n=100 (auto-spawns bitcoind).
cargo test --release --test inproc_sign_100 -- --nocapture

# Full n=100 in-process DKG correctness + O(n²) timing/memory instrumentation.
cargo test --release --test dkg_100_correctness -- --nocapture
```

Both accept environment overrides so you can capture faster, smaller data points:
the signing test honors `CHEGET_SIGN_T` / `CHEGET_SIGN_N` and the DKG test honors
`CHEGET_DKG_T` / `CHEGET_DKG_N` (each defaults to 51 / 100). On one developer
machine the confirmed regtest key-spend runs in ~9 s under `--release` versus ~90 s
in debug; these are local measurements, not benchmarks.

**MSRV is Rust 1.85**, and `Cargo.lock` is committed so builds are reproducible —
verifiability is a first-class goal (many people must be able to verify what they
run).

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

The Phase 2 storage layer provides **at-rest share encryption** (age/scrypt with
in-memory `zeroize`), but no CLI signing flow yet persists or consumes a secret
share, so this protection is not yet exercised in a fund-custody path. Other
security properties tied to later phases — epoch discipline, same-key rotation
checks, standby/sweep revocation, and Nostr↔FROST key separation — are **not yet
enforced** and are described above as planned.

## License

Licensed under either of MIT or Apache-2.0 at your option.
