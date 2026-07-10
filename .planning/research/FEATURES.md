# Feature Research

**Domain:** Large-membership (501-of-1000) FROST threshold-Schnorr Bitcoin Taproot signing CLI (`tsig`)
**Researched:** 2026-07-10
**Confidence:** HIGH (feature set defined by SPEC-frost-cli.md v0.1; comparable-tooling grounding from source-verified `implementations-resharing.md`)

## How this was scoped

The intended feature set is already fixed by `SPEC-frost-cli.md` (§5 CLI surface, §6 ceremony flows, §10 policy engine, §11 security) and `PROJECT.md`. This document does not invent features — it **categorizes** the SPEC's features as table stakes / differentiators / anti-features for a signer of this class, notes expected behavior and complexity of each, and maps the inter-feature dependencies that drive build order. "Table stakes" and "differentiator" are judged against how comparable threshold/multisig tooling actually behaves (ZF frost, Chainflip, DFINITY chain-key, Frostsnap, tss-lib, and conventional Bitcoin multisig wallets such as Sparrow/Nunchuk/Liana).

External web search was disabled for this run; findings rest on the two curated in-repo sources (both HIGH confidence: the SPEC is the authoritative design contract; `implementations-resharing.md` is source-verified against actual repos) plus domain knowledge of Bitcoin multisig UX (MEDIUM-HIGH).

## Feature Landscape

### Table Stakes (a threshold signer is broken without these)

These are non-negotiable. A tool claiming to be a threshold Bitcoin signer that lacks any of these is not a signer — it is a demo. Users give no credit for having them; the product is simply broken without them.

| Feature | Why Expected (broken without it) | Complexity | Notes |
|---------|----------------------------------|------------|-------|
| **Keygen — group key generation** (`ceremony keygen`, `keygen join`; dealer + DKG modes) | Without a shared group key there is nothing to sign with. Every threshold tool must produce a `PublicKeyPackage` (the verifying key) + per-seat `KeyPackage`. | HIGH | Dealer mode = one `generate_with_dealer` call (recommended default at n=1000, documented trust event). DKG mode = `dkg::part1/2/3`, O(n²) traffic (~10⁶ envelopes). Bridge: verifying key must equal Taproot internal key `P`. |
| **Address derivation** (`address`) — frost→rust-bitcoin bridge | A signer users cannot receive to is useless. Must turn the group key into a spendable P2TR address. | MEDIUM | 33-byte SEC1 → x-only 32B → `XOnlyPublicKey` → `Address::p2tr(secp, internal, None, network)`, merkle root `None` (BIP86-style). **The classic integration bug** — pin with a byte-level round-trip test. Address constant across all epochs. |
| **Two-round FROST signing** (`session sign`, `sign join`) with taproot tweak | The core function: produce a valid signature. Broken if it can't emit a standard 64-byte BIP340 sig over the key-spend sighash. | HIGH | `round1::commit` → `SigningPackage` → `round2::sign_with_tweak` → `aggregate_with_tweak(…, None)`. Coordinator verifies sig against output key `Q` before finalizing PSBT. On-chain indistinguishable from single-sig. |
| **PSBT parse + sighash computation** | Signing needs a canonical thing-to-sign. Without correct BIP341 sighash the signature is worthless. | MEDIUM | `SighashCache::taproot_key_spend_signature_hash`, default sighash type, one per input. Feeds display-before-sign. |
| **At-rest share storage + memory hygiene** | A key share written in plaintext, or left in memory, is a compromise. Every serious signer encrypts key material at rest. | MEDIUM | age/scrypt passphrase encryption of `KeyPackage`/`PublicKeyPackage`; `zeroize` in memory after use. Per-key-per-epoch files tagged `(key_id, epoch, identifier)`. |
| **Nonce discipline** — signing nonces in memory only, never persisted | Persisting-then-reusing a FROST nonce is a **key-extraction** bug class. A signer that can leak nonces is not merely incomplete — it is dangerous. | HIGH | Highest-severity implementation rule in the SPEC. Any session restart generates fresh nonces; never reuse commitments across sessions. Prevented *structurally* (nonces are the one ceremony secret never checkpointed). |
| **Share status** (`share status`) | A participant must be able to see which shares they hold and their state, or they can't operate. | LOW | Lists `key_id, epoch, state` for held shares (ACTIVE/STANDBY). |
| **Coordinator state** (roster, transcripts, session logs) | Sequencing 1000 participants across ceremonies is impossible without durable roster + transcript state. | MEDIUM | SQLite: roster (identifier ↔ npub ↔ status ↔ join/leave epochs), ceremony transcripts (event ids), session logs, policy, churn ledger. |
| **Bitcoin chain integration** (UTXO listing, broadcast, fee estimate) | A signer that can't see UTXOs or broadcast can't actually spend. | MEDIUM | `bitcoincore-rpc` against operator node, watch-only `tr(<internal-key>)` descriptor import; Esplora alternative behind the same trait. |
| **Resumable / idempotent ceremonies** (checkpoint per round; dedup per `(ceremony_id, round, seat)`) | At n=1000 a ceremony *will* be interrupted. Non-resumable = never completes. This is table stakes **at this scale** even though small-n tools skip it. | HIGH | Checkpoint encrypted round state between rounds (nonces excepted). Event-id + tags give replay protection and idempotent resumption. |
| **Repair** (`ceremony repair`, `repair help`) — recover a lost share | At n=1000, members *will* lose shares. A large-membership signer with no recovery path bleeds quorum until it's dead. | MEDIUM | `repairable::repair_share_part1/2/3` targeting an existing seat's identifier; ≥501 helpers; result verified against group `PublicKeyPackage`. Shares the RTS machinery with enroll. |

### Differentiators (this design's distinctive bets)

These set `tsig` apart from both conventional Bitcoin multisig and from other FROST tooling. They align directly with the Core Value in PROJECT.md: rotate membership with zero on-chain cost, and make past compromise truly revocable. Most comparable open-source FROST libraries ship *refresh only* or are embedded in a protocol (not a general CLI) — see Competitor Analysis.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Membership rotation, same address** (`ceremony refresh --remove`, `enroll`) | Members join/leave with **zero on-chain footprint** and no address change. Conventional Bitcoin multisig requires an on-chain move (new script → new address) to change signers; `tsig` does not. This is the headline bet. | HIGH | Refresh = ZF `refresh_dkg_part1/part2/refresh_dkg_shares`, drops any identifier not included (removals) and proactivizes (new randomness). O(n²) via relay, same transport as keygen. |
| **Same-key postcondition check** (mandatory, client-side, after every refresh) | Guarantees the address never silently changed under a hostile coordinator. The property that makes "same address" *trustworthy*, not just intended. | LOW–MEDIUM | Every participant checks new `PublicKeyPackage` verifying key `==` old; mismatch → abort + discard. Never trust the coordinator's word. Epoch increments only when coordinator has all confirmations. |
| **Sweep-as-revocation** (`sweep --to standby`) | The **real** revocation mechanism. Rotation only defends against external gradual compromise; retained-share insiders (501 from any one past epoch reconstruct forever) are mitigated *only* by moving funds. Distinctive because the design is explicit that share erasure is NOT the security boundary. | HIGH | One consolidation tx: all ACTIVE UTXOs → single STANDBY output (RBF, feerate arg/estimate) → run signing against ACTIVE → on ≥6 confs: ACTIVE→RETIRED, STANDBY→ACTIVE rollover. |
| **Standby key lifecycle** (`standby new`; STANDBY state kept refreshed) | Makes a sweep a routine **signing session**, not an emergency ceremony. The next key is pre-generated and rotation-maintained so revocation is fast when needed. | HIGH | Full keygen ceremony for the next key; kept refreshed on the same cadence (its epoch-1 holders are a future dangerous coalition too). At steady state every participant holds exactly two shares (ACTIVE + STANDBY). |
| **Policy-driven sweep triggers** (`policy show/set`: `value_cap`, `churn_budget`, `max_epochs`, `standby_max_age`) | Turns "should we revoke?" from a judgment call into a mechanical, auditable rule. Value bounds the prize; churn bounds the coalition pool — both trigger, either alone insufficient. | MEDIUM | Coordinator + watcher config. Rationale is normative (from the verified-erasure analysis). Defaults: churn 50, max_epochs 24, standby_max_age 90d; value_cap operator-set/required. |
| **Watcher** (`watch --node`) — cron/CI-friendly policy evaluator | Automated, unattended detection that a sweep is due. Read-only, no secrets — safe to run anywhere. | LOW–MEDIUM | Exit 0 ok / exit 2 sweep-due + JSON report on stdout when balance > value_cap OR distinct former holders since last DKG > churn_budget OR epochs since DKG > max_epochs. Consumes coordinator churn ledger. |
| **Nostr transport** (signed events, multi-relay, NIP-44 v2, roster pinning) | Replaces a bespoke relay server: the Nostr event model *is* the message-board / envelope-signature / delivery semantics ceremonies need, and multi-relay = redundant liveness by construction. Distinctive; most FROST tooling assumes a bespoke coordinator socket. | HIGH | Custom event kinds per message class; ceremony binding via tags; event BIP340 signature *is* the envelope signature; roster npubs pinned (hash committed in every ceremony-open), off-roster events discarded client-side. Confidential payloads (round-2 shares, dealer export, enroll/repair deltas) NIP-44 v2 to recipient npub; optional NIP-59 gift-wrap. **Key separation mandatory**: Nostr identity keys independently generated, never reused as / derived from FROST material. |
| **Offline file fallback** (`--in <dir> --out <dir>` on join/help) | First-class air-gapped path: same signed-event JSON carried by hand on removable media. Lets the highest-value participants stay fully offline; relays are default, not required. | MEDIUM | Identical envelope format to the online path, just transported manually. |
| **Display-before-sign** (participants recompute sighash from PSBT, ack outputs/amounts/fee) | A compromised coordinator must not be able to get a quorum to blind-sign an arbitrary sighash. At 501 signers, independent verification by each *is the point*. Distinctive vs. protocol-embedded signers that auto-sign. | MEDIUM | Participants recompute the sighash from the PSBT (don't trust coordinator's hash); human ack required unless `--yes`. |
| **Epoch discipline / mixed-epoch rejection** | Makes rotation's security property (epoch-mixing: shares from different epochs don't combine) *enforced*, and fails fast with a clear error instead of producing garbage signatures. | MEDIUM | Shares tagged `(key_id, epoch, identifier)`; signing sessions bind `(key_id, epoch)` and reject mixed-epoch shares early. |
| **Enroll** (`ceremony enroll`, `enroll help`) — add a member at same threshold | Adds a new seat without a full re-DKG, via repair/RTS against a fresh identifier. Partial-rotation capability that most libraries lack (they do refresh of the *same* set only). | HIGH | ≥501 helpers run `repair_share_part1/2`; new member runs `repair_share_part3`, verifies against group key. **Always immediately followed by a refresh in the same ceremony window** (proactivizes away helpers' delta knowledge; keeps the epoch boundary clean). Batch: enroll k, then one refresh. |
| **Verifiability posture** (reproducible builds, pinned/audited deps, `cargo audit`/`cargo deny` in CI) | 1000 people must be able to verify what they run. Makes trust in the binary a differentiator, not an assumption. | MEDIUM | Reproducible participant binary; ZF `frost-secp256k1-tr` is the audited (NCC) crypto path; external review of nonce discipline + bridge is first-class. |

### Anti-Features (SPEC non-goals — deliberately excluded)

Each is restated with the reason it is out of scope. Documenting these prevents scope creep and clarifies the address model.

| Anti-Feature | Why it seems desirable | Why excluded (SPEC reason) | What to do instead |
|--------------|------------------------|-----------------------------|--------------------|
| **Change the threshold `t` in place** | "Just adjust 501→601 without re-keying." | ZF frost cannot change `t` without a new DKG → new key → new address. In-place threshold change is cryptographically a re-key. | Supported **only as a sweep**: new DKG at the new threshold → new STANDBY → sweep funds to it. Never in place. |
| **Verifiable erasure / remote attestation of share deletion** | "Prove members actually deleted old shares so old epochs are safe." | Assumed **impossible** on untrusted member hardware. No security claim in the design rests on any local deletion. | The **sweep** is the true revocation. Old share material is deleted best-effort (hygiene, audit), but security comes from moving funds, not from deletion. |
| **Script-path spends** (Taproot script tree) | "Add timelocks / backup script paths." | Not needed for the single key-path address model; would change the address construction (merkle root ≠ None) and break single-sig indistinguishability. | Key-path spend only; merkle root `None` (BIP86-style). All policy lives in the FROST quorum, not on-chain script. |
| **Multi-address wallet management** | "Manage many addresses / HD accounts like a normal wallet." | The design controls exactly one key-path address per key. Multi-address adds coin management complexity with no benefit to the single-vault model. | One address per key (ACTIVE + STANDBY). Receive to the single P2TR; consolidate on sweep. |
| **Coin selection beyond consolidation** | "Smart coin selection, change outputs, privacy coin control." | Out of scope; the only spend patterns are quorum key-spends of specified PSBTs and full consolidation on sweep. | Sweep consolidates **all** UTXOs to one output. Ordinary spends are operator-supplied PSBTs. |
| **ECDSA (any variant)** | "Bitcoin has lots of ECDSA tooling / legacy addresses." | Wrong signature type for Taproot key-spend, and threshold ECDSA at n=1000 is infeasible (heavy O(n²)+ MPC, presigning, aborts). | FROST threshold **Schnorr** (BIP340) only — cheap, non-interactive-ish two rounds, native Taproot. |
| **Alt crypto libraries** (`secp256kfun`/`schnorr_fun`, tss-lib / any ECDSA stack, luxfi/threshold) | "More options / familiar libs." | `schnorr_fun` is primitives-level (would rebuild DKG/refresh/repair by hand); tss-lib is ECDSA + n=1000 infeasible; luxfi/threshold is stub code (per research §3.6). | ZF `frost-secp256k1-tr` ≥3.0 is the single audited, packaged path providing every primitive incl. the BIP341 tweak. |

## Feature Dependencies

```
[Keygen: PublicKeyPackage/KeyPackage]
    ├──requires──> [Resumable/idempotent ceremony infra]   (n=1000 won't finish otherwise)
    ├──requires──> [At-rest share storage + zeroize]
    └──enables───> [Address derivation (frost→rust-bitcoin bridge)]
                        └──enables──> [Two-round signing]
                                          ├──requires──> [PSBT parse + BIP341 sighash]
                                          ├──requires──> [Nonce discipline]
                                          ├──requires──> [Display-before-sign]  (recompute sighash)
                                          ├──requires──> [Bitcoin chain integration]  (broadcast/fees)
                                          └──requires──> [Epoch discipline]  (bind key_id,epoch)

[Nostr transport]  (init identity keys + roster pinning)
    ├──carries───> [Keygen], [Refresh], [Enroll], [Repair], [Signing]  (all ceremony traffic)
    └──alternative-of──> [Offline file fallback (--in/--out)]   (same envelope, manual carry)

[Refresh]
    ├──requires──> [Same-key postcondition check]  (mandatory, client-side)
    └──requires──> [Epoch bookkeeping]  (increment on all-confirmations)

[Enroll] ──requires immediate──> [Refresh]  (same ceremony window; proactivizes deltas)
[Enroll] ──shares machinery with──> [Repair]  (RTS: repair_share_part1/2/3)

[Standby key (standby new)]
    └──prerequisite-of──> [Sweep]           (sweep target must pre-exist)

[Sweep]
    ├──requires──> [Signing] (run §6.5 against ACTIVE) + [Chain integration] (build/broadcast/confirm)
    └──on-confirm──> [STANDBY→ACTIVE rollover] + [ACTIVE→RETIRED]
                          └──triggers──> [watch nags until new STANDBY exists]

[Watcher (watch)]
    ├──requires──> [Policy engine (value_cap/churn_budget/max_epochs/standby_max_age)]
    ├──requires──> [Chain integration]  (read balance/UTXOs)
    └──consumes──> [Coordinator churn ledger]  (distinct former holders)
        └──output signals──> [Sweep]  (exit 2 = sweep-due)
```

### Dependency Notes (build-order implications)

- **Address derivation requires keygen:** the P2TR address is derived from the `PublicKeyPackage` verifying key. Bridge cannot be exercised end-to-end until a key exists — but the byte-level round-trip test can (and should) be built against a simulated key first (PROJECT "crypto bridge proven early").
- **Signing requires keygen + address + chain integration + PSBT/sighash:** it is the convergence point of the whole M1 happy path.
- **Enroll requires an immediate refresh** (same ceremony window) — they are effectively one deliverable; planning enroll without the follow-on refresh leaves helper delta-knowledge un-proactivized and the epoch boundary dirty. Build refresh before/with enroll.
- **Sweep requires a pre-generated standby:** `standby new` must land before `sweep` is meaningful; sweep is otherwise an emergency ceremony, which the whole standby bet exists to avoid.
- **Watch feeds the sweep decision:** the policy engine + churn ledger + chain read are prerequisites for `watch`; `watch` exit code 2 is the signal that drives an operator `sweep`.
- **Resumable/idempotent infra underpins every ceremony at n=1000:** it is a cross-cutting prerequisite, not a feature added later — refresh/enroll/keygen all depend on it.
- **Nostr transport and offline file mode are alternatives over the same envelope format:** build the signed-event JSON schema once; relays vs. removable-media is a transport choice, not two schemas.
- **Epoch discipline is cross-cutting:** it gates both rotation (refresh/enroll increment epoch) and signing (reject mixed-epoch shares). Establish the `(key_id, epoch, identifier)` tagging before refresh exists.

## MVP Definition

Maps directly to the SPEC's M1–M5 milestones (PROJECT decision: roadmap covers all of M1–M5).

### Launch With (M1 — proves the concept)

- [ ] Dealer keygen → `PublicKeyPackage` (crypto bridge validated) — everything hangs off this
- [ ] frost→rust-bitcoin address bridge + byte-level round-trip test — the classic integration bug, de-risked first
- [ ] Two-round signing with taproot tweak → BIP340 key-spend sig, 501 simulated participants in-process on regtest
- [ ] PSBT parse + BIP341 sighash
- [ ] Nonce discipline (in-memory only) — structural, from day one

### Add After Validation (M2–M3)

- [ ] Nostr transport + DKG at n=1000 over self-hosted strfry; resumable/idempotent ceremonies (M2) — trigger: bridge proven, ready for real transport
- [ ] At-rest share encryption + coordinator SQLite state (M2)
- [ ] Refresh (removals + proactivize) + same-key postcondition + epoch bookkeeping (M3)
- [ ] Enroll (+ immediate refresh) and Repair (M3)

### Future Consideration (M4–M5)

- [ ] Standby lifecycle, sweep flow, watch + policy engine (M4) — trigger: rotation working, real funds in play
- [ ] Offline file mode, reproducible builds, adversarial test suite, external review of nonce discipline + bridge (M5)

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Keygen (dealer + DKG) | HIGH | HIGH | P1 |
| Address bridge + round-trip test | HIGH | MEDIUM | P1 |
| Two-round signing + taproot tweak | HIGH | HIGH | P1 |
| Nonce discipline | HIGH | HIGH | P1 |
| PSBT/sighash + chain integration | HIGH | MEDIUM | P1 |
| Nostr transport + resumable ceremonies | HIGH | HIGH | P1 (for real use) |
| At-rest storage + coordinator state | HIGH | MEDIUM | P1 |
| Refresh + same-key check + epochs | HIGH | HIGH | P2 |
| Enroll + repair | HIGH | HIGH | P2 |
| Display-before-sign | HIGH | MEDIUM | P2 |
| Standby + sweep + rollover | HIGH | HIGH | P2 |
| Policy engine + watch | MEDIUM | LOW–MEDIUM | P2 |
| Offline file fallback | MEDIUM | MEDIUM | P3 |
| Reproducible builds + adversarial tests | HIGH (trust) | MEDIUM | P3 |

**Priority key:** P1 = M1/M2 core; P2 = rotation/lifecycle (M3–M4); P3 = hardening (M5).

## Competitor Feature Analysis

Grounded in the source-verified `implementations-resharing.md`. The recurring gap across the field: most libraries ship **proactive refresh (same members)** or are **embedded in a protocol**, not a general-purpose large-membership CLI with sweep-based revocation.

| Feature | ZF frost (`-secp256k1-tr`) | Chainflip / DFINITY chain-key | Frostsnap | Conventional BTC multisig (Sparrow/Nunchuk/Liana) | `tsig` approach |
|---------|---------------------------|-------------------------------|-----------|----------------------------------------------------|-----------------|
| Scheme | FROST Schnorr BIP340/341 | FROST/IDKG Schnorr+ECDSA | FROST Schnorr | Native `multi()`/`tr()` script | FROST Schnorr, key-path only (uses ZF frost as its entire crypto layer) |
| Membership rotation, same address | refresh + add/remove + repair/enroll (no `t` change) | full validator-set handover every epoch (protocol-embedded) | add/remove signer, no on-chain move | requires on-chain move (new script → new address) | refresh + enroll/repair, zero on-chain footprint, `t` fixed |
| Threshold change | ❌ (needs new DKG) | varies | ❌ | on-chain re-setup | ❌ by design → only via sweep |
| Revocation of past holders | not addressed (library) | new epoch key | not addressed | move funds to new script | **sweep to pre-generated standby** (the distinctive bet) |
| Scale | library, no scale opinion | ~validator-set sized | small consumer groups | typically ≤15 keys | **n=1000** — the defining constraint |
| Transport | none (library) | protocol P2P | dedicated devices/coordinator | PSBT files / coordinator apps | **Nostr multi-relay** + offline file fallback |
| Delivery form | crate | embedded in a chain | hardware + app | GUI wallet | single-binary CLI, three personae |

Takeaways for `tsig`: it is essentially "ZF frost's rotation/repair primitives, operationalized at n=1000 with a Nostr transport and a sweep-based revocation lifecycle." No existing open-source tool combines large-membership CLI + zero-on-chain rotation + sweep revocation + policy-driven triggers — that combination is the product.

## Sources

- `SPEC-frost-cli.md` v0.1 (§§1–13) — authoritative design contract for the feature set. **HIGH.**
- `.planning/PROJECT.md` — Core Value, requirements, key decisions, security model. **HIGH.**
- `implementations-resharing.md` — source-verified survey of open-source FROST/threshold-ECDSA rotation tooling (ZF frost, Chainflip, DFINITY, Frostsnap, tss-lib, etc.). **HIGH** (claims checked against actual repo code/specs).
- Domain knowledge of conventional Bitcoin multisig UX (Sparrow/Nunchuk/Liana on-chain-move-to-rotate model). **MEDIUM** (not re-verified this run; external search disabled).

---
*Feature research for: 501-of-1000 FROST Taproot signing CLI (`tsig`)*
*Researched: 2026-07-10*
