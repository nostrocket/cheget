# SPEC — `tsig`: a 51-of-100 FROST Taproot signing CLI with rotation and sweep

Status: draft v0.1 · 2026-07-09
Companion research: [implementations-resharing.md](implementations-resharing.md)

---

## 1. Purpose

`tsig` is a command-line tool that lets a fixed-threshold, large-membership group control a
Bitcoin Taproot address:

- **Scheme:** FROST threshold Schnorr (RFC 9591), secp256k1, BIP340/341 Taproot key-path spend.
- **Parameters:** `t = 51`, `n = 100`. The threshold **never changes** (this is what makes
  ZcashFoundation/frost sufficient — its one limitation, no threshold change without re-key,
  is out of scope by design).
- **Membership rotation, same address:** members join/leave via enroll + refresh ceremonies
  with zero on-chain footprint.
- **True revocation = sweep:** the design explicitly does **not** claim share erasure is
  verifiable. Rotation defends against *external gradual* compromise only. Revoking past
  epochs is done by an on-chain sweep of all UTXOs to a **pre-generated standby key** owned
  by the current membership. Sweeps are triggered by policy (value cap, churn budget, or
  compromise signal).

### Non-goals

- Changing the threshold `t` (requires new DKG → new key → sweep; supported only *as* a sweep).
- Verifiable erasure / remote attestation of deletion. Assumed impossible on member hardware.
- Script-path spends, multi-address wallet management, coin selection beyond consolidation.
- ECDSA anything.

---

## 2. System model

| Role | Count | Trust |
|---|---|---|
| **Participant** | 100 | Holds one share. Untrusted individually; security assumption is <51 of any single epoch's holders collude/leak. |
| **Coordinator** | 1 | Sequences ceremonies/sessions. Just another Nostr identity publishing control events. **Untrusted for key security**; can stall or censor, cannot forge messages, learn shares, or alter the address. |
| **Nostr relays** | ≥3, self-hosted | Dumb signed-event stores (strfry or similar). See only signed public protocol messages and NIP-44-encrypted blobs. Trusted for liveness only, and redundantly — any one honest reachable relay suffices. |
| **Dealer (optional)** | 0 or 1 | Only in `keygen dealer` mode. Sees the full secret momentarily — a deliberate, documented trust trade to avoid the 100-party DKG. Air-gapped ceremony recommended. |
| **Watcher** | any | Read-only chain monitor evaluating sweep policy. No secrets. |

Adversary model: mobile adversary compromising devices over time (refresh resets its
progress, per the epoch-mixing property: shares from different epochs do not combine);
plus retained-share insiders (mitigated **only** by sweep); plus malicious relay (DoS only).

---

## 3. Cryptography

- **Group key:** FROST `PublicKeyPackage` verifying key = Taproot *internal key* `P`.
- **Address:** BIP341 key-only output: `Q = P + H_taproot(P)·G`, `Address = P2TR(Q)`
  (merkle root `None`, BIP86-style). Derived once per key; constant across all refresh epochs.
- **Signing:** FROST two-round; final aggregation applies the taproot tweak via
  `frost_secp256k1_tr::aggregate_with_tweak(…, merkle_root: None)` so the output is a
  standard 64-byte BIP340 signature over the key-spend sighash. On-chain it is
  indistinguishable from single-sig.
- **Identifiers:** `frost::Identifier` fixed per member seat, `1..=100` (u16). A seat's
  identifier survives refresh; an enrolled replacement receives the vacated or a fresh
  identifier (identifier space may exceed 100 historical values; live set is always ≤100).
- **Epochs:** every completed refresh/enroll ceremony increments `epoch`. Share files are
  tagged `(key_id, epoch, identifier)`. Signing sessions bind to `(key_id, epoch)` and reject
  mixed-epoch shares (they would produce garbage — the library will fail, but we fail early
  with a clear error).

---

## 4. Key lifecycle

```
            DKG/dealer ceremony                 sweep tx confirmed
  (none) ───────────────────────► STANDBY ────────────────────────► ACTIVE ──► RETIRED
                                     ▲                                 │
                                     └── regenerate on churn budget ───┘
```

- **ACTIVE** — the key whose address currently holds funds. Refresh/enroll ceremonies run
  against it on every membership change.
- **STANDBY** — the *next* key, fully generated in advance so a sweep is a signing session,
  not an emergency ceremony. Kept refreshed on the same cadence as the active key (its
  epoch-1 holders are a future dangerous coalition too). Policy may force regeneration.
- **RETIRED** — post-sweep. Share material may be kept briefly for audit, then deleted
  (best-effort; no security claim rests on this deletion).

At steady state every participant holds exactly two shares: one for ACTIVE, one for STANDBY.

---

## 5. CLI surface

Single binary, three personae selected by subcommand. All ceremony commands are resumable
(state checkpointed after every round) and idempotent per `(ceremony_id, round)`.

### Participant commands

```
tsig init                                # generate Nostr identity keypair, print npub for
                                         #   out-of-band roster registration
                                         # (all commands read relay URLs from config or --relays)
tsig keygen join      --ceremony <id>    # DKG part1/2/3 or receive dealer share
tsig refresh join     --ceremony <id>    # refresh-DKG rounds for current epoch+1
tsig enroll help      --ceremony <id>    # act as helper issuing a share to a new seat
tsig repair help      --ceremony <id>    # help an existing seat recover a lost share
tsig sign  join       --session  <id>    # round1 commit + round2 sign (prompts to display
                                         #   tx summary: outputs, amounts, fee — human ack
                                         #   required unless --yes)
tsig share status                        # list held shares: key_id, epoch, state
```

### Coordinator commands

```
tsig ceremony keygen  --mode dkg|dealer --seats 100 --threshold 51
tsig ceremony refresh --remove <ids> [--key active|standby]
tsig ceremony enroll  --seat <id> --new-member <identity-pubkey>
tsig ceremony repair  --seat <id>
tsig session sign     --psbt <file> [--key active]  # select 51 online, run 2 rounds,
                                                    #  aggregate, emit finalized tx
tsig sweep            [--to standby] [--feerate <sat/vb>]  # build+sign consolidation tx
                                                    #  spending ALL utxos to standby addr;
                                                    #  on confirm: standby→active rollover
tsig standby new                                    # pre-generate next key (full ceremony)
```

### Watcher / policy commands

```
tsig address          [--key active|standby]     # print P2TR address
tsig watch            --node <rpc-url>           # evaluate policy; exit 0 ok / 2 sweep-due
tsig policy show|set  --value-cap <sats> --churn-budget <count> --max-epochs <count>
```

`tsig watch` is cron/CI-friendly: nonzero exit + JSON report on stdout when
`balance > value_cap` **or** `distinct former holders since last DKG > churn_budget`
**or** `epochs since DKG > max_epochs`.

---

## 6. Ceremony flows (mapping to library calls)

### 6.1 Keygen — DKG mode
1. Coordinator opens ceremony; each participant runs `dkg::part1` → broadcasts round-1
   package (small; 100 broadcasts total).
2. `dkg::part2` → each participant produces **99 per-recipient packages**, uploaded as one
   batched, E2E-encrypted bundle (~10⁴ envelopes total across the group — relay sizing, §8).
3. `dkg::part3` → `(KeyPackage, PublicKeyPackage)`. Coordinator collects verifying-key
   confirmations from all 100; mismatch aborts ceremony.

### 6.2 Keygen — dealer mode (recommended default at n=100)
`frost::keys::generate_with_dealer(100, 51, IdentifierList, rng)` on an air-gapped
machine; shares exported encrypted per recipient identity key; dealer state destroyed
(best-effort, documented as a trust event in the ceremony transcript).

### 6.3 Refresh (rotate membership / proactivize)
- Removals: run refresh omitting the leavers' identifiers — ZF frost's refresh-DKG
  (`refresh_dkg_part1/part2/refresh_dkg_shares`) removes any identifier not included.
- All remaining holders participate (refresh at this scale is again O(n²) via the relay;
  same transport path as keygen).
- Postcondition checked by every participant: new `PublicKeyPackage` verifying key
  `==` old one, else abort and discard. Epoch increments only on coordinator receiving
  all confirmations. Old `KeyPackage` deleted locally (hygiene, not a security claim).

### 6.4 Enroll (add member, same threshold)
Repair/RTS technique against a fresh identifier: ≥51 helpers run
`repairable::repair_share_part1/2` targeting the new seat's identifier; deltas are
E2E-encrypted to the new member, who runs `repair_share_part3` and verifies against the
group `PublicKeyPackage`. **Every enroll is immediately followed by a refresh** in the same
ceremony window, so the helpers' knowledge of delta contributions is proactivized away and
the epoch boundary stays clean. (Batch: enroll k new members, then one refresh.)

### 6.5 Signing (`session sign`)
1. Coordinator parses PSBT, computes the BIP341 key-spend sighash per input
   (`SighashCache::taproot_key_spend_signature_hash`, default sighash type).
2. Selects 51 live participants (liveness poll); round 1: each runs `round1::commit`,
   returns `SigningCommitments`. **Nonces live in memory only, never written to disk; any
   session restart generates fresh nonces** (crash-then-resign with a persisted nonce is a
   key-extraction bug class; we prevent it structurally).
3. Coordinator builds `SigningPackage` (one per input); round 2: participants display the
   tx summary, ack, run `round2::sign_with_tweak`.
4. Coordinator runs `aggregate_with_tweak(…, None)`, verifies the BIP340 signature against
   the output key `Q`, finalizes the PSBT, prints the raw tx (broadcast is the operator's
   step, or `--broadcast` via the configured node).
5. Timeout of any participant → session aborted, **new session** with a replacement subset
   (never reuse commitments across sessions).

### 6.6 Sweep
`tsig sweep` = build one consolidation tx: all UTXOs of ACTIVE → single output to STANDBY
address (RBF-enabled, feerate from arg or node estimate) → run §6.5 against ACTIVE →
on confirmation depth ≥ 6: ACTIVE→RETIRED, STANDBY→ACTIVE, and `tsig watch` starts nagging
until a new STANDBY exists (`standby new`).

---

## 7. Transport & message format — Nostr

The transport is **Nostr**: every protocol message is a signed Nostr event published to a
configured set of relays. This replaces any bespoke relay server — the message-board,
signature-envelope, and delivery semantics the ceremonies need are exactly the Nostr event
model, and multi-relay publication makes the liveness story redundant by construction.

- **Identity:** each participant and the coordinator has a dedicated Nostr keypair
  (secp256k1/BIP340), generated at `tsig init`. **Key separation is mandatory:** the Nostr
  identity key is generated independently and MUST NOT be derived from, or shared with, any
  FROST share or group-key material. The roster is a pinned set of npubs (hash committed in
  every ceremony-open event), registered with the coordinator out-of-band.
- **Events:** application-specific event kinds in the addressable/regular custom range, one
  kind per message class (`ceremony-open`, `round1-package`, `round2-bundle`,
  `commitments`, `signature-share`, `confirmation`, `session-control`). Ceremony/session
  binding via tags: `["d"|"cer", <ceremony_id>]`, `["round", <n>]`, `["seat", <identifier>]`,
  and `["p", <recipient npub>]` for directed messages. Event `id` + tags give replay
  protection and idempotent resumption (`(ceremony_id, round, seat)` dedup).
- **Authenticity:** the Nostr event signature (BIP340 over the event) *is* the envelope
  signature; events from npubs outside the pinned roster are discarded client-side. Relays
  are never trusted to filter.
- **Confidential payloads** (DKG round-2 shares, dealer share export, enroll/repair
  deltas): encrypted with **NIP-44 v2** to the recipient's npub, inside the signed event.
  Optionally gift-wrapped (NIP-59) if roster/metadata privacy from relay observers is
  wanted; not required for key security. Everything else (round-1 packages, nonce
  commitments, signature shares) is public-by-design and goes in plaintext event content.
- **Relays:** ceremonies at n=100 generate ~10⁴ events (§8) — far beyond public-relay rate
  limits and etiquette. Operators MUST run ≥3 dedicated relays (e.g. strfry /
  nostr-rs-relay), optionally restricted to roster npubs via AUTH (NIP-42). Every event is
  published to all configured relays; readers merge and dedup by event id. Public relays
  MAY be added as extra redundancy for low-volume messages (session control, sweeps), never
  relied on for ceremonies.
- **Serialization:** ZF frost types via their `serialization` feature, base64 in event
  content; NIP-44 padding hides share-payload sizes.
- **Offline fallback:** every `join`/`help` command accepts `--in <dir> --out <dir>` to run
  air-gapped via files carried on removable media (same signed-event JSON, transported by
  hand); relays are the default path, not a requirement.

## 8. Storage

- **Participant** (`~/.tsig/`): identity keypair; per-key-per-epoch `KeyPackage` +
  `PublicKeyPackage`, encrypted at rest with a passphrase (age/scrypt), zeroized in memory
  after use (`zeroize`). Checkpointed ceremony state (round secrets for DKG parts are
  written encrypted **only** between rounds of the same ceremony — signing nonces are the
  exception and are never persisted, §6.5).
- **Coordinator:** SQLite — roster (identifier ↔ npub ↔ status ↔ join/leave epochs),
  ceremony transcripts (event ids), session logs, policy config, churn ledger (feeds
  `watch`).
- **Relay sizing:** DKG/refresh worst case ≈ 10⁴ events × ~1 KB ≈ 10 MB per ceremony,
  ~99 directed events per sender in round 2. strfry handles this comfortably; relay
  configs must raise default rate limits and retention for ceremony kinds, and clients
  publish round-2 events in paced batches. Noted so nobody points a ceremony at a public
  relay and gets banned mid-round.

## 9. Bitcoin integration

- Address/tx/PSBT/sighash: `rust-bitcoin`. Chain access for `watch`/`sweep`/UTXO listing:
  Bitcoin Core JSON-RPC (`bitcoincore-rpc`) against an operator-run node; descriptor
  watch-only import (`tr(<internal-key>)`) so Core tracks the address. Esplora
  (`esplora-client`) as a light alternative behind the same trait.
- Key bridging: frost verifying key (33-byte SEC1) → x-only 32 bytes →
  `bitcoin::XOnlyPublicKey` as internal key; `Address::p2tr(secp, internal, None, network)`.
  A round-trip unit test pins this byte-level bridge (the classic integration bug).

## 10. Policy engine

Config (coordinator + watcher):

| Knob | Meaning | Default |
|---|---|---|
| `value_cap` | max sats at ACTIVE before sweep is due | operator-set, required |
| `churn_budget` | max distinct former share-holders since last DKG | 50 |
| `max_epochs` | max refreshes before sweep regardless | 24 |
| `standby_max_age` | max age of STANDBY before regeneration | 90 d |

Rationale (from the verified erasure analysis): risk between sweeps = feasibility of a
51-coalition **within any single past epoch** since the last DKG. Value bounds the prize;
churn bounds the coalition pool. Both trigger sweeps; either alone is insufficient.

## 11. Security considerations (normative)

1. **Epoch quorum retention is the residual risk.** 51 shares from one epoch reconstruct
   the key forever; mixed epochs and sub-threshold sets are useless (verified by
   simulation). No deletion claim is load-bearing anywhere in this design; the sweep is.
2. **Nonce discipline** (§6.5) is the highest-severity implementation rule in this spec.
3. **Same-key check after every refresh** is mandatory and client-side (never trust the
   coordinator's word for it).
4. **Roster pinning:** all ceremony messages verify against the pinned roster hash;
   membership changes happen only via enroll/refresh ceremonies, never by relay fiat.
5. **Dealer mode is a documented trust event.** Transcript records who, where, what
   hardware; DKG mode exists for when that's unacceptable.
6. **Coordinator can censor, not steal — and relays even less.** Liveness comes from
   publishing every event to multiple independent self-hosted relays plus the offline file
   mode; these are operational mitigations, not cryptographic ones. A hostile relay set can
   stall a ceremony but never alter its outcome (roster-pinned signatures, client-side
   same-key checks).
6a. **Nostr key separation.** Nostr identity keys are transport-layer only: independently
   generated, never reused as, or derived from, FROST material — both live on secp256k1,
   which makes accidental reuse easy and forbidden.
7. **Display-before-sign:** participants ack human-readable tx outputs/amounts/fee;
   a compromised coordinator must not be able to get a quorum to blind-sign an arbitrary
   sighash. (At 51 signers full verification by each is the point — they each recompute
   the sighash from the PSBT, not trust the coordinator's hash.)
8. Library versions pinned; `cargo audit`/`cargo deny` in CI; reproducible builds for the
   participant binary (100 people must be able to verify what they run).

## 12. Libraries

| Concern | Crate | Why |
|---|---|---|
| FROST core, Taproot | **`frost-secp256k1-tr` ≥ 3.0** (+ `frost-core`, features `serialization`) | The audited reference (NCC-reviewed, MIT/Apache); provides DKG, dealer keygen, refresh-DKG, repair/enroll primitives, `sign_with_tweak`/`aggregate_with_tweak` for BIP341. The entire crypto layer of this spec is its public API. |
| Bitcoin types, PSBT, sighash, address | **`bitcoin`** (rust-bitcoin, latest stable) | Canonical; BIP341 sighash + P2TR address construction. |
| Node RPC | **`bitcoincore-rpc`**; alt: **`esplora-client`** | UTXO set, broadcast, fee estimation, watch-only descriptors. |
| Nostr transport | **`nostr-sdk`** (rust-nostr, latest; features: NIP-44, NIP-42, NIP-59) | Event build/sign/publish/subscribe, multi-relay pool with dedup, NIP-44 v2 payload encryption. Replaces any bespoke relay server, HTTP stack, and separate envelope-signature scheme. |
| Relay software (ops, not a dependency) | **strfry** (or nostr-rs-relay) | Self-hosted ceremony relays; NIP-42 AUTH to restrict to roster npubs. |
| At-rest share encryption | **`age`** (scrypt passphrase) + **`zeroize`** | Share files; memory hygiene. |
| CLI / config / serde | **`clap` 4**, **`serde`/`serde_json`**, **`toml`**, **`rusqlite`** (coordinator state) | Standard. |

Explicitly rejected: `secp256kfun`/`schnorr_fun` (excellent but primitives-level; ZF frost
is the audited packaged path), tss-lib/any ECDSA stack (n=100 infeasible, wrong signature
type for Taproot key-spend), luxfi/threshold (stub code per research doc §3.6).

## 13. Milestones

1. **M1 — single-machine happy path:** dealer keygen → address → sign a regtest key-spend
   with 51 simulated participants in-process. Proves the frost↔rust-bitcoin bridge.
2. **M2 — real transport:** Nostr event schema + participant binary against self-hosted
   strfry relays; DKG at n=100 (containerized load test, relay rate-limit tuning);
   resumable ceremonies via event-id dedup.
3. **M3 — rotation:** refresh with removals, enroll+refresh, epoch bookkeeping,
   same-key postcondition tests.
4. **M4 — lifecycle:** standby key, sweep flow on regtest/signet, watch + policy engine.
5. **M5 — hardening:** offline file mode, reproducible builds, adversarial tests
   (malicious relay, mixed-epoch shares, replayed envelopes, nonce-reuse attempts),
   external review of §6.5 and the bridge code.
