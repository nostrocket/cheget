# Requirements: tsig

**Defined:** 2026-07-10
**Core Value:** A group of 1000 can jointly control one Bitcoin address (any 501 can spend, no individual holds the key), rotate membership with zero on-chain cost, and truly revoke past compromise by sweeping to a standby key.

## v1 Requirements

Requirements for the initial release covering SPEC milestones M1–M5. Each maps to a roadmap phase.

### Crypto Bridge & Keygen (KEY)

- [x] **KEY-01**: Operator can generate a group key via DKG (`dkg::part1/2/3`) producing a `KeyPackage` + `PublicKeyPackage` whose verifying key is the Taproot internal key `P` (DKG is the only keygen path)
- [x] **KEY-02**: The same DKG routines run in-process on a single host with all participants simulated and no transport, for fast local testing and to prove the bridge + signing end-to-end (M1 keygen path; no dealer mode)
- [x] **KEY-03**: A byte-level round-trip test pins the frost→rust-bitcoin key bridge (33-byte SEC1 → 32-byte x-only → `XOnlyPublicKey` → `Address::p2tr(secp, internal, None, network)`), asserting x-only parity and internal-vs-output-key correctness
- [x] **KEY-04**: `tsig address [--key active|standby]` prints the BIP341 P2TR address (`Q = P + H_taproot(P)·G`, merkle root `None`), constant across all refresh epochs
- [x] **KEY-05**: Each participant confirms the group verifying key to the coordinator after keygen; any mismatch aborts the ceremony
- [x] **KEY-06**: DKG generates the full n=1000 share set in-process on a single host with no transport, producing 1000 `KeyPackage`s that all verify to one group `PublicKeyPackage`; validates the O(n²) computation scales locally (distinct from the transport-layer load test)

### Signing (SIGN)

- [ ] **SIGN-01**: Coordinator can run a signing session from a PSBT (`tsig session sign --psbt <file> [--key active]`), computing the BIP341 key-spend sighash per input (`SighashCache::taproot_key_spend_signature_hash`, default sighash type)
- [ ] **SIGN-02**: Coordinator selects 501 live participants via liveness poll and runs FROST round 1 (`round1::commit`) collecting `SigningCommitments`
- [ ] **SIGN-03**: Participants run round 2 (`round2::sign_with_tweak`); coordinator aggregates with the taproot tweak (`aggregate_with_tweak(…, merkle_root: None)`) into a 64-byte BIP340 signature
- [ ] **SIGN-04**: Coordinator verifies the aggregated BIP340 signature against the output key `Q`, finalizes the PSBT, and prints the raw tx (broadcast is operator-driven or `--broadcast` via the configured node)
- [x] **SIGN-05**: Signing nonces live in memory only and are represented by a type that cannot be serialized/persisted; any session restart generates fresh nonces (structural prevention of nonce-reuse key extraction)
- [ ] **SIGN-06**: Aggregation surfaces the 3.0 cheater-detection culprits list; a participant timeout aborts the session and a new session runs with a replacement subset (commitments are never reused across sessions)
- [ ] **SIGN-07**: Before signing, each participant recomputes the sighash from the PSBT and is shown human-readable tx outputs/amounts/fee, requiring an explicit ack (unless `--yes`); no blind signing of a coordinator-supplied hash

### Transport — Nostr (TRAN)

- [ ] **TRAN-01**: `tsig init` generates a dedicated Nostr identity keypair (independent of, and never derived from, FROST material) and prints the npub for out-of-band roster registration
- [ ] **TRAN-02**: Protocol messages are published as signed Nostr events, one custom kind per message class (`ceremony-open`, `round1-package`, `round2-bundle`, `commitments`, `signature-share`, `confirmation`, `session-control`), tagged for ceremony/session/round/seat binding
- [ ] **TRAN-03**: Confidential payloads (DKG round-2 shares, enroll/repair deltas) are encrypted with NIP-44 v2 to the recipient npub inside the signed event
- [ ] **TRAN-04**: Every event is published to all configured relays (≥3 self-hosted); readers merge and dedup by event id; events from npubs outside the pinned roster are discarded client-side (relays never trusted to filter)
- [ ] **TRAN-05**: The roster is a pinned set of npubs whose hash is committed in every ceremony-open event and verified by all clients
- [ ] **TRAN-06**: Ceremonies are resumable and idempotent per `(ceremony_id, round, seat)` via event-id dedup; round-2 events are published in paced batches
- [ ] **TRAN-07**: Every `join`/`help` command supports offline file mode (`--in <dir> --out <dir>`) carrying the same signed-event JSON on removable media, behind the same transport interface as Nostr
- [ ] **TRAN-08**: A containerized n=1000 DKG load test validates relay rate-limit/retention tuning and paced batching against self-hosted strfry (~10⁶ events / ~1 GB per ceremony)

### Rotation (ROT)

- [ ] **ROT-01**: Coordinator can run a refresh (`tsig ceremony refresh --remove <ids> [--key active|standby]`) using refresh-DKG (`refresh_dkg_part1/part2/refresh_dkg_shares`), removing any identifier not included, with all remaining holders participating
- [ ] **ROT-02**: Every participant verifies, client-side, that the new `PublicKeyPackage` verifying key equals the old one after refresh; mismatch aborts and discards the new share (never trust the coordinator's word)
- [ ] **ROT-03**: Coordinator can enroll a new member (`tsig ceremony enroll --seat <id> --new-member <pubkey>`) via repair/RTS (`repairable::repair_share_part1/2/3`) against a fresh identifier, immediately followed by a refresh in the same ceremony window (proactivizing helper knowledge)
- [ ] **ROT-04**: Coordinator can repair a lost share for an existing seat (`tsig ceremony repair --seat <id>`) with ≥501 helpers, and the recovering member verifies against the group `PublicKeyPackage`
- [ ] **ROT-05**: Every completed refresh/enroll increments `epoch`; share files are tagged `(key_id, epoch, identifier)`; signing sessions bind `(key_id, epoch)` and reject mixed-epoch shares early with a clear error
- [ ] **ROT-06**: `tsig share status` lists held shares (key_id, epoch, state); at steady state each participant holds exactly two — one ACTIVE, one STANDBY

### Lifecycle & Revocation (LIFE)

- [ ] **LIFE-01**: Coordinator can pre-generate the next (STANDBY) key via a full ceremony (`tsig standby new`), kept refreshed on the same cadence as the active key
- [ ] **LIFE-02**: `tsig sweep [--to standby] [--feerate <sat/vb>]` builds one RBF-enabled consolidation tx spending ALL active UTXOs to the standby address and signs it via the signing session flow against ACTIVE
- [ ] **LIFE-03**: On sweep confirmation (depth ≥ 6): ACTIVE→RETIRED, STANDBY→ACTIVE rollover; retired share material may be kept briefly for audit then deleted best-effort (no security claim rests on the deletion)
- [ ] **LIFE-04**: After rollover, `tsig watch` nags until a new STANDBY exists

### Policy & Watch (POL)

- [ ] **POL-01**: `tsig policy show|set` manages `value_cap`, `churn_budget` (default 50), `max_epochs` (default 24), and `standby_max_age` (default 90d)
- [ ] **POL-02**: `tsig watch --node <rpc-url>` evaluates policy and is cron/CI-friendly: exit 0 ok / exit 2 sweep-due, emitting a JSON report on stdout when balance > value_cap OR distinct former holders since last DKG > churn_budget OR epochs since DKG > max_epochs
- [ ] **POL-03**: STANDBY older than `standby_max_age` forces regeneration

### Storage & Chain (STOR)

- [ ] **STOR-01**: Participant storage (`~/.tsig/`) holds the identity keypair and per-key-per-epoch `KeyPackage`+`PublicKeyPackage` encrypted at rest (age/scrypt) and zeroized in memory after use
- [ ] **STOR-02**: Ceremony round secrets (DKG parts) are checkpointed encrypted between rounds of the same ceremony; signing nonces are never persisted (the sole exception)
- [ ] **STOR-03**: Coordinator state is SQLite (rusqlite): roster (identifier ↔ npub ↔ status ↔ join/leave epochs), ceremony transcripts, session logs, policy config, churn ledger
- [ ] **STOR-04**: Chain access is behind a trait with a Bitcoin Core JSON-RPC backend (`bitcoincore-rpc`, watch-only `tr(<internal-key>)` descriptor import) and an Esplora (`esplora-client`) alternative for UTXO listing, broadcast, and fee estimation

### Verifiability & Hardening (SEC)

- [ ] **SEC-01**: Library versions are pinned (`Cargo.lock` committed); `cargo audit` and `cargo deny` run in CI with documented allow-lists (duplicate secp256k1, age label)
- [ ] **SEC-02**: The participant binary has a reproducible build so members can verify what they run
- [ ] **SEC-03**: Locally-verifiable adversarial tests: mixed-epoch shares rejected early, and a nonce-reuse attempt that fails to compile / is rejected before any partial signature is emitted (no transport required)
- [ ] **SEC-04**: External review targets the nonce discipline (§6.5) and the bridge code (§9) specifically
- [ ] **SEC-05**: Transport-dependent adversarial tests: malicious-relay DoS (any one honest relay suffices) and replayed-envelope rejection via event-id/`(ceremony_id, round, seat)` dedup (validated once real transport exists)

## v2 Requirements

Acknowledged but deferred; not in the current roadmap.

### Privacy & Robustness

- **PRIV-01**: Optional NIP-59 gift-wrapping of directed events for roster/metadata privacy from relay observers (not required for key security)
- **ROBU-01**: ROAST-style robust signing wrapper to tolerate participant dropout at 501-of-1000 without full session restart (quantify dropout impact via an M2 spike first)
- **CHAIN-01**: Maintained replacement/fork for the stale `bitcoincore-rpc` client if it falls behind rust-bitcoin releases

## Out of Scope

Explicitly excluded, per SPEC non-goals. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Dealer-mode keygen (`generate_with_dealer`) | Dropped — DKG is the only keygen path (no single-party trust event); local in-process DKG on one host serves fast testing |
| Changing the threshold `t` in place | Requires new DKG → new key → sweep; supported only *as* a sweep, never in place |
| Verifiable erasure / remote attestation of share deletion | Assumed impossible on member hardware; no security claim rests on local deletion — the sweep is the revocation |
| Script-path (tapscript) spends | Design is key-only P2TR (merkle root `None`); breaks single-sig indistinguishability |
| Multi-address wallet management / coin selection beyond consolidation | Out of the single-address model; only sweep consolidation is needed |
| ECDSA of any kind | Wrong signature type for Taproot key-spend; infeasible at n=1000 |
| `secp256kfun`/`schnorr_fun`, tss-lib, luxfi/threshold | Primitives-level, wrong-scheme, or stub code — `frost-secp256k1-tr` is the audited packaged path |

## Traceability

Every v1 requirement maps to exactly one phase. The ordering proves the entire system LOCALLY first — Phase 1 introduces the `Transport` trait + an in-memory stub so every ceremony phase (3–6) runs with zero relay code — then Phase 7 (FINAL) swaps in the real `FileTransport`/`NostrTransport` impls behind the same trait and re-runs the flows at scale over relays. The local DKG-at-scale compute proof (KEY-06, Phase 3) is deliberately separated from the transport-layer relay load test (TRAN-08, Phase 7). Locally-verifiable adversarial tests (SEC-03) stay in hardening (Phase 6); transport-dependent adversarial tests (SEC-05) move to the final transport phase (Phase 7).

| Requirement | Phase | Status |
|-------------|-------|--------|
| KEY-01 | Phase 1 | Complete |
| KEY-02 | Phase 1 | Complete |
| KEY-03 | Phase 1 | Complete |
| KEY-04 | Phase 1 | Complete |
| KEY-05 | Phase 1 | Complete |
| SIGN-01 | Phase 1 | Pending |
| SIGN-02 | Phase 1 | Pending |
| SIGN-03 | Phase 1 | Pending |
| SIGN-04 | Phase 1 | Pending |
| SIGN-05 | Phase 1 | Complete |
| SIGN-06 | Phase 1 | Pending |
| SIGN-07 | Phase 1 | Pending |
| STOR-04 | Phase 1 | Pending |
| STOR-01 | Phase 2 | Pending |
| STOR-02 | Phase 2 | Pending |
| STOR-03 | Phase 2 | Pending |
| KEY-06 | Phase 3 | Complete |
| ROT-01 | Phase 4 | Pending |
| ROT-02 | Phase 4 | Pending |
| ROT-03 | Phase 4 | Pending |
| ROT-04 | Phase 4 | Pending |
| ROT-05 | Phase 4 | Pending |
| ROT-06 | Phase 4 | Pending |
| LIFE-01 | Phase 5 | Pending |
| LIFE-02 | Phase 5 | Pending |
| LIFE-03 | Phase 5 | Pending |
| LIFE-04 | Phase 5 | Pending |
| POL-01 | Phase 5 | Pending |
| POL-02 | Phase 5 | Pending |
| POL-03 | Phase 5 | Pending |
| SEC-01 | Phase 6 | Pending |
| SEC-02 | Phase 6 | Pending |
| SEC-03 | Phase 6 | Pending |
| SEC-04 | Phase 6 | Pending |
| TRAN-01 | Phase 7 | Pending |
| TRAN-02 | Phase 7 | Pending |
| TRAN-03 | Phase 7 | Pending |
| TRAN-04 | Phase 7 | Pending |
| TRAN-05 | Phase 7 | Pending |
| TRAN-06 | Phase 7 | Pending |
| TRAN-07 | Phase 7 | Pending |
| TRAN-08 | Phase 7 | Pending |
| SEC-05 | Phase 7 | Pending |

**Coverage:**

- v1 requirements: 43 total (was 41; added KEY-06 local DKG-at-scale compute proof and SEC-05 transport-dependent adversarial tests)
- Mapped to phases: 43 ✓
- Unmapped: 0

---
*Requirements defined: 2026-07-10*
*Last updated: 2026-07-10 after roadmap revision (major reorder to prove the system locally first, real transport last; 7 phases; added KEY-06 and SEC-05; 43 v1 requirements mapped)*
