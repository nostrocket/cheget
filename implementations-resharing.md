# Open-Source Threshold-ECDSA & FROST Implementations with Signer-Set Rotation (Same Address)

> **Goal:** find every **open-source** implementation of **threshold ECDSA** or **FROST (threshold
> Schnorr)** on **secp256k1** that can **rotate the set of signers and/or refresh shares while keeping
> the same aggregated public key** — i.e. the Bitcoin receiving address never changes.
>
> Compiled from deep source-level research (mid-2026). Every "✅ verified" claim below was checked
> against an actual function, module path, test, or spec quote in the repository — not assumed.
> See the companion landscape docs in [`schemes/`](schemes/00-overview.md).

---

## How to read this document

**Two different capabilities** are tracked, because they are often conflated and only one is *true
signer-set rotation*:

| Capability | What changes | Same address? | Rotates the *set* of signers? |
|---|---|---|---|
| **(a) Proactive refresh** | New random shares for the **same** committee & threshold | ✅ yes | ❌ no (same members) |
| **(b) Committee reshare** | **New set** of parties and/or **new threshold** t/n | ✅ yes | ✅ **yes** — this is what you asked for |
| *(repair / enroll)* | Recover a lost share / add one new member | ✅ yes | partial (add/recover, not full re-choose) |

Your requirement — *rotate the set of signers, same address* — is **capability (b)**. Repair/enroll
is a useful partial (add or recover a member). Refresh (a) keeps the same members and is listed for
completeness because it is what many libraries actually ship.

**License filter.** You required *open source*. Several widely-cited MPC libraries are **source-available
but NOT OSI open source** (non-commercial licenses, "all rights reserved", restrictive NOASSERTION).
Those are pulled out into their own clearly-flagged section (§5) so they are not mistaken for open source.

**Verification levels:** ✅ verified in code · 📄 verified via official docs/spec · ⚠️ claimed but
unverified / low-trust.

---

## 1. TL;DR — the shortlist

### Genuine committee reshare (change signer set / threshold, same address), truly open source

| Library | Scheme | Lang | License | Reshare evidence | Prod |
|---|---|---|---|---|---|
| **bnb-chain/tss-lib** | ECDSA GG18/GG20 | Go | MIT | `ecdsa/resharing/` — changes **both n and t** ✅ | Binance + ~all Go TSS |
| **near/threshold-signatures** | ECDSA (triples/robust) | Rust | MIT | `do_reshare()`, pubkey-equality enforced ✅ | NEAR MPC mainnet |
| **dfinity/ic** (chain-key) | ECDSA **and** Schnorr BIP340 | Rust | Apache-2.0 | `ReshareOfUnmasked`, cross-subnet ✅📄 | ICP mainnet (native BTC) |
| **Chainflip** | FROST Schnorr BIP340 | Rust | Apache-2.0 | `initiate_key_handover` Lagrange reshare ✅ | Chainflip mainnet BTC vault |
| **entropyxyz/synedrion** | ECDSA CGGMP'24 | Rust | AGPL-3.0 | `key_resharing.rs` (new set + new t) ✅ | Entropy network |
| **ZenGo-X/fs-dkr** | ECDSA GG20 | Rust | GPL-3.0 | add/remove party, `reconstruct(old)==reconstruct(new)` ✅ | reference (stale) |
| **vultisig/mobile-tss-lib** | ECDSA GG18 (tss-lib) | Go | Apache-2.0 | `ExecuteKeyResharing`, add/remove devices ✅ | Vultisig wallet |
| **sygmaprotocol/sygma-relayer** | ECDSA GG18 (tss-lib) | Go | LGPL-3.0 | `tss/ecdsa/resharing/`, key-rotation ceremony ✅ | Sygma bridge |

### Best-in-class by use case

- **Bitcoin Taproot (Schnorr), audited, maintained library:** **ZcashFoundation/frost** (`frost-secp256k1-tr`) — refresh + add/remove + repair/enroll, **but cannot change the threshold t** without a new DKG.
- **Production committee rotation on a live Bitcoin vault:** **Chainflip** (`key_handover`) and **DFINITY** chain-key (IDKG) — both rotate the whole validator/node set every epoch keeping the same key. Neither is a drop-in library (embedded in a protocol).
- **Consumer Bitcoin hardware, add/remove signer, no on-chain move:** **Frostsnap**.
- **Cleanest permissive t-of-n ECDSA reshare library:** **near/threshold-signatures** (MIT) or the canonical **bnb-chain/tss-lib** (MIT).
- **Proactive refresh only (same members):** taurusgroup/multi-party-sig, coinbase/cb-mpc, fireblocks/mpc-lib, LFDT-Lockness/cggmp21.

> ⚠️ The single most common limitation: many libraries do **refresh** (same members) or **repair/enroll**
> (add/recover one member) but **cannot arbitrarily re-choose the committee AND change the threshold** on
> a fixed key. Full (b) is confirmed in tss-lib, near/threshold-signatures, synedrion, dfinity/ic,
> Chainflip, and fs-dkr (party count only).

---

## 2. Threshold ECDSA — committee reshare (capability b)

### 2.1 bnb-chain/tss-lib ✅ — the canonical reference
- **Repo:** https://github.com/bnb-chain/tss-lib · **Lang:** Go · **Org:** BNB Chain / Binance · **License:** MIT
- **Scheme:** GG18 (with GG20 signing improvements) + EdDSA · **secp256k1**
- **Capability:** **(b) committee reshare** — package `ecdsa/resharing/` (rounds `round_1_old_step_1.go` … `round_5_new_step_3.go`), driven by `tss.ReSharingParameters`. Constructor takes **separate old and new committees**: `NewReSharingParameters(ec, ctx, newCtx, partyID, partyCount, threshold, newPartyCount, newThreshold)`.
- **Same pubkey:** ✅ README: *"Dynamic Groups to change the group of participants while keeping the secret ('resharing')."*
- **Change t / n:** ✅ **both** — old `(threshold, partyCount)` vs new `(newThreshold, newPartyCount)`.
- **Status:** actively maintained (v2), ~1k+ stars; the ancestor of nearly every Go TSS deployment.
- **Summary:** The de-facto standard and the only *canonical* resharing round protocol in Go. Rotates the signer set and changes both party count and threshold while preserving the address. Use a current version — the 2021 Trail of Bits findings and 2023 TSSHOCK/BitForge (CVE-2023-33241) touched the DLN/Paillier proof paths.

#### tss-lib forks (all inherit `ecdsa/resharing`, capability (b), same pubkey)
| Fork | URL | License | Note |
|---|---|---|---|
| SwingbyProtocol/tss-lib | https://github.com/SwingbyProtocol/tss-lib | MIT | Skybridge; submitted resharing security patches (2021) |
| THORChain go-tss + tss-lib | https://gitlab.com/thorchain/tss/go-tss | MIT | Vault-churn resharing; go-tss now **archived** |
| ModChain/tss-lib (v2) | https://github.com/ModChain/tss-lib | MIT | Currently-maintained drop-in fork |
| iofinnet/tss-lib | (module ref) | MIT | Hardened fork; **public repo 404'd at check** (may be private) |

### 2.2 near/threshold-signatures ✅ — strongest permissive match
- **Repo:** https://github.com/near/threshold-signatures · **Lang:** Rust · **Author:** Lúcás Meier (cronokirby) / NEAR · **License:** MIT
- **Scheme:** two secp256k1 threshold-ECDSA schemes (OT/triple-based cait-sith lineage + a robust secret-sharing ECDSA); DKG/resharing is FROST-based · secp256k1 via `k256`/`frost-secp256k1`
- **Capability:** **(a)+(b)** — `src/dkg.rs` → `do_reshare(chan, participants, me, threshold, old_signing_key, old_public_key, old_participants, rng)`. README: *"Key Resharing / Key Refresh: allows parties to reshare their keys, add new members, or remove existing ones."*
- **Same pubkey:** ✅ **enforced in code** — *"check if the old public key is the same as the new one"*, errors if different.
- **Change t / n:** ✅ both (`test_reshare` uses two thresholds).
- **Status:** actively developed; **production — NEAR MPC / chain-signatures mainnet**.
- **Summary:** The cleanest permissively-licensed, code-verified dynamic-membership + threshold-change reshare on secp256k1. Not Bitcoin-specific, but directly usable for a secp256k1 key.

### 2.3 dfinity/ic — chain-key threshold ECDSA (IDKG) ✅📄
- **Repo:** https://github.com/dfinity/ic · **Lang:** Rust · **Org:** DFINITY · **License:** Apache-2.0
- **Scheme:** Groth–Shoup distributed ECDSA (IACR ePrint 2022/506), Pedersen/Feldman VSS · `AlgorithmId::ThresholdEcdsaSecp256k1` (**also** threshold Schnorr BIP340 — see §3.7)
- **Capability:** **(b), incl. cross-subnet** — `IDkgTranscriptOperation::ReshareOfUnmasked(...)` (*"Reshares the public key. Needed, e.g., after subnet topology changes."*), separate `dealers`/`receivers` sets, `verify_initial_dealings(...)` for XNet.
- **Same pubkey:** ✅ reshares the same key; XNet copies the same key to a new subnet.
- **Change t / n:** ✅ independent dealer/receiver sets; threshold from the new receiver set.
- **Status:** ~1.8k stars, release-tagged 2026-07; **production — ICP mainnet native Bitcoin/Ethereum**.
- **Summary:** Production, permissive committee reshare across changing node sets, address preserved — but deeply embedded in the IC replica, not a drop-in library.

### 2.4 entropyxyz/synedrion ✅
- **Repo:** https://github.com/entropyxyz/synedrion · **Lang:** Rust · **Org:** Entropy · **License:** **AGPL-3.0** (strong copyleft — commercial flag)
- **Scheme:** CGGMP'24 (on the `manul` framework) · secp256k1 (hardcoded)
- **Capability:** **(a)+(b)** — `src/protocols/key_refresh.rs` (proactive) **and** `src/protocols/key_resharing.rs`; `KeyResharingProtocol` is *"A protocol for modifying the set of owners of a shared secret key"*; `KeyResharing::new(old_holder, new_holder, new_holders, new_threshold, OldHolder{verifying_key})`.
- **Same pubkey:** ✅ the `verifying_key` is carried through resharing.
- **Change t / n:** ✅ distinct old/new holder sets + explicit `new_threshold`.
- **Status:** ~85 stars, active (2025); **unaudited** ("work in progress").
- **Summary:** Cleanest open CGGMP dynamic-membership + threshold-change reshare. Blockers for a commercial product: AGPL-3.0 and no audit.

### 2.5 ZenGo-X/fs-dkr ✅ — "one-round distributed key rotation"
- **Repo:** https://github.com/ZenGo-X/fs-dkr · **Lang:** Rust · **Org:** ZenGo-X · **License:** GPL-3.0
- **Scheme:** GG20 (on `ZenGo-X/multi-party-ecdsa` `LocalKey`) · secp256k1
- **Capability:** **(a)+(b)** — `refresh_message.rs` (carries `public_key: local_key.y_sum_s`), `add_party_message.rs` (`JoinMessage`), `remove_party_indices`. Tests: `test_sign_rotate_sign`, `test_remove_sign_rotate_sign`, `test_add_party`.
- **Same pubkey:** ✅ **verified in tests** — asserts `vss.reconstruct(old) == vss.reconstruct(new)` while shares differ, then signs.
- **Change t / n:** party count **n** yes (add/remove); reconstruction **threshold t preserved** (re-randomized same-degree); changing t not demonstrated.
- **Status:** ~34 stars, last push **2023 — effectively unmaintained**; dated `curv-kzen 0.7`.
- **Summary:** The canonical GG20 one-round rotation reference with verified same-address preservation and add/remove — but stale and GPL.

### 2.6 getamis/alice (AMIS) ✅ — refresh + add-member
- **Repo:** https://github.com/getamis/alice · **Lang:** Go · **Org:** AMIS · **License:** Apache-2.0
- **Scheme:** CGGMP21 (recommended), GG18 (deprecated); FROST for EdDSA · secp256k1
- **Capability:** **(a)** `crypto/tss/ecdsa/cggmp/refresh/` (CGGMP aux-info/key refresh) **+ partial (b)** `crypto/tss/ecdsa/addshare/` — *"Adding share creates a new share for new participant without changing the original public key."*
- **Same pubkey:** ✅ (both refresh and addshare).
- **Change t / n:** partial — `addshare` **adds** a member; no single primitive to remove parties or change t like tss-lib.
- **Status:** maintained, clean-room CGGMP.
- **Summary:** Proactive refresh + enroll-a-new-participant, address preserved. Not arbitrary t-of-n → t'-of-n' reshare.

### 2.7 vultisig/mobile-tss-lib ✅ — production "Vault Reshare"
- **Repo:** https://github.com/vultisig/mobile-tss-lib · **Lang:** Go · **Org:** Vultisig · **License:** Apache-2.0
- **Scheme:** GG18 ECDSA + EdDSA (wraps bnb-chain/tss-lib), gomobile bindings · secp256k1
- **Capability:** **(b)** — `coordinator.ExecuteKeyResharing`; `ReshareInput` carries `OldParties`, `NewParties`, `PubKey`.
- **Same pubkey:** ✅ docs: *"Resharing modifies the number of devices in a vault without changing wallet addresses or moving funds."*
- **Change t / n:** ✅ add/remove devices, add a Vultiserver co-signer.
- **Status:** active (2024–2026), consumer product.
- **Summary:** Practical mobile wrapper exposing tss-lib committee reshare as a first-class feature — a good reference for driving tss-lib's reshare in an app.

### 2.8 sygmaprotocol/sygma-relayer ✅
- **Repo:** https://github.com/sygmaprotocol/sygma-relayer · **Lang:** Go · **Org:** ChainSafe / Sygma · **License:** LGPL-3.0
- **Scheme:** GG18 via tss-lib · secp256k1
- **Capability:** **(b)** — `tss/ecdsa/resharing/`; docs: *"Key Reshare … rotate key shares with new participants without changing the underlying public key."*
- **Same pubkey / change t,n:** ✅ / ✅ (inherits tss-lib semantics).
- **Status:** actively maintained (production bridge infra); LGPL-3.0 (copyleft).
- **Summary:** Production wiring of tss-lib resharing into an operational key-rotation ceremony.

### 2.9 tangle-network/cggmp-threshold-ecdsa ⚠️
- **Repo:** https://github.com/tangle-network/cggmp-threshold-ecdsa · **Lang:** Rust · **License:** GPL-3.0 · **Scheme:** CGGMP
- **Capability:** **(a)+partial (b)** via a fork of `fs-dkr` (refresh + add-party). Committee-change specifics **unverified in code**. Research-grade, unaudited.

### 2.10 cait-sith (cronokirby) — reference root of NEAR's reshare
- **Repo:** https://github.com/cronokirby/cait-sith (LIT fork **archived**) · **Lang:** Rust · **License:** MIT
- **Scheme:** threshold ECDSA via triples · secp256k1 (`k256`)
- **Capability:** README claims **refresh + resharing**; exact reshare fn **unverified in cait-sith itself** — the maintained, code-verified version is **near/threshold-signatures** (§2.2, same author). Use that.

---

## 3. FROST (threshold Schnorr, BIP340) — rotation, same address

### 3.1 ZcashFoundation/frost ✅ — the audited reference (best all-round library)
- **Repo:** https://github.com/ZcashFoundation/frost · **Lang:** Rust · **Org:** Zcash Foundation · **License:** MIT OR Apache-2.0
- **Curves:** `frost-secp256k1`, and **`frost-secp256k1-tr` = Bitcoin Taproot BIP340/341** (v3.0.0, `aggregate_with_tweak()`); also ed25519/ristretto/P-256 · **FROST variant:** RFC 9591; `-tr` is Taproot-tweaked BIP340
- **Capability (verified in `frost-core/src/keys/`):**
  - **Refresh** — `refresh::compute_refreshing_shares()` / `refresh_share()` (dealer) and `refresh_dkg_part1/part2/refresh_dkg_shares()` (no dealer). *"if not all identifiers are passed, the refresh procedure will effectively remove the missing participants"* → **add/remove supported**.
  - **Repair** — `repairable::repair_share_part1/2/3()` recovers a lost share via a threshold of helpers.
  - **Enroll** — evaluate the joint polynomial at a new identifier (repair/RTS technique) to issue a share to a new member, same threshold.
- **Same pubkey:** ✅ refresh/repair keep the `PublicKeyPackage` verifying key identical.
- **Change t / n:** party set ✅ (add/remove/repair/enroll); **threshold t ❌ — validated & blocked** (*"the refresh process can't reduce the threshold"*); changing t needs a fresh DKG (new key).
- **Status:** very active, v3.0.0 (2026-04), partially NCC-audited; widely forked (e.g. Lightspark/Spark).
- **Summary:** The audited baseline for Bitcoin Taproot FROST. Refresh + add/remove + repair/enroll with a constant group key; the one hard limit is you cannot change the threshold without re-keying.

### 3.2 Chainflip (chainflip-backend) ✅ — production committee reshare on a Bitcoin vault
- **Repo:** https://github.com/chainflip-io/chainflip-backend · **Lang:** Rust · **Org:** Chainflip Labs · **License:** Apache-2.0
- **Curves:** **secp256k1 / Bitcoin BIP340 Taproot** (`engine/multisig/src/crypto/bitcoin.rs`, `BtcCryptoScheme`, `XOnlyPublicKey`) + EVM-Schnorr, ed25519, Polkadot · **FROST variant:** FROST, BIP340 x-only for BTC
- **Capability (verified):** **(b) committee reshare / "key handover"** — `MultisigClientApi::initiate_key_handover(...)` → `ResharingContext` → `start_keygen_with_resharing_context(...)`. Outgoing signers contribute their existing share Lagrange-scaled (`get_lagrange_coeff(...) * key_share.x_i`) into `generate_secret_and_shares(..., existing_secret)`.
- **Same pubkey:** ✅ Lagrange-scaled shares keep the aggregate point unchanged; the only handover variant is `BtcKeyHandoverRequest` — precisely because a Taproot vault address can't be re-pointed.
- **Change t / n:** ✅ distinct `sharing_participants` (outgoing subset) and `receiving_participants` (incoming set); new `ThresholdParameters` each epoch.
- **Status:** very active (2026-07); **production — Chainflip mainnet rotates its validator set every epoch keeping the same Bitcoin vault key.**
- **Summary:** The strongest real-world code evidence for same-address committee rotation on Bitcoin. Protocol code, not a packaged library, but genuine FROST secp256k1.

### 3.3 Frostsnap ✅ — consumer Bitcoin hardware, add/remove signer, no on-chain move
- **Repo:** https://github.com/frostsnap/frostsnap · **Lang:** Rust + Dart · **Org:** Frostsnap (Fournier/Farrow) · **License:** MIT
- **Curves:** **secp256k1 / BIP340 Taproot** (via `schnorr_fun`/`secp256kfun`) · **FROST variant:** FROST + ChillDKG
- **Capability (verified):** repair/enroll/restoration — `coordinator/restoration.rs` & `device/restoration.rs` (`start_restoring_key`, `add_recovery_share_to_restoration`, `check_recover_share_compatible_with_restoration`, `consolidate_physical_backup`), validated against the existing `access_structure_ref`/`shared_key`; primitives `SecretShare::recover_secret`, `SharedKey::from_share_images`.
- **Same pubkey:** ✅ restoration is compatibility-checked against the existing `shared_key`.
- **Change t / n:** add/remove/replace signers "without on-chain transactions keeping the key the same" is a headline feature; **repair/enroll is code-verified**, full threshold-change reshare is advertised but not isolated to one verified function (⚠️).
- **Status:** very active; **production — shipping Bitcoin hardware wallet.**
- **Summary:** The standout consumer product: Taproot FROST with a real restoration subsystem that repairs a lost device's share or enrolls a new device against the same group key.

### 3.4 secp256kfun / schnorr_fun (LLFourn) ✅ (primitives)
- **Repo:** https://github.com/LLFourn/secp256kfun · **Lang:** Rust · **Author:** Lloyd Fournier · **License:** 0BSD
- **Curves:** **secp256k1 only (BIP340)** · **FROST variant:** FROST + ChillDKG
- **Capability:** repair/enroll primitives (`SecretShare::recover_secret`, `SharedKey::from_share_images`); Nick Farrow's [gist](https://gist.github.com/nickfarrow/64c2e65191cde6a1a47bbd4572bf8cf8) demonstrates recover + enroll + proactive threshold reduction, **no MPC**. No single stable `reshare()` (⚠️ partial); productized in Frostsnap.
- **Same pubkey:** ✅ by design. **Change t/n:** add signers + reduce threshold demonstrated; increasing t noted as "harder than re-keygen".
- **Summary:** The canonical low-level Rust secp256k1 FROST toolkit; reshare/enroll/repair as primitives rather than a packaged API.

### 3.5 taurusgroup/multi-party-sig ✅ — Go FROST refresh (same set only)
- **Repo:** https://github.com/taurusgroup/multi-party-sig · **Lang:** Go · **Org:** Taurus SA · **License:** Apache-2.0
- **Curves:** **secp256k1** incl. **BIP340 Taproot** (`KeygenTaproot`) · **FROST variant:** FROST (ePrint 2020/852) + Taproot; also CMP ECDSA
- **Capability:** **(a) only** — `frost.Refresh(config, participants)` / `RefreshTaproot(...)` re-runs keygen with `refresh:=true`, carrying the existing `config.PublicKey`. (ECDSA side: `cmp.Refresh()`, also proactive-only.)
- **Same pubkey:** ✅. **Change t / n:** ❌ — no `Reshare`/`Repair`; participant list & threshold immutable.
- **Status:** ~386 stars, active (2025); maintained by Taurus.
- **Summary:** Cleanest, best-maintained Go FROST-secp256k1 with Taproot and a working proactive refresh. Cannot enroll/remove or change threshold. (Note: the Go **taurushq-io/multi-party-sig** is the org's newer home; same refresh-only story.)

### 3.6 luxfi/threshold ⚠️ — claims full reshape; LOW TRUST
- **Repo:** https://github.com/luxfi/threshold · **Lang:** Go · **Org:** Lux Network · **License:** Apache-2.0 · **~2 stars**
- **Scheme:** CMP (ECDSA) + FROST + an "LSS" dynamic-resharing layer; secp256k1 + Taproot per docs
- **Capability (partly verified):** `protocols/lss/lss.go` → `Reshare(cfg, newParticipants, newThreshold, pool)`; `reshare/round3.go` **enforces** `errors.New("public key changed during reshare")`. But the **FROST-specific and CMP reshare entrypoints are stub/unverified** — the Go agent found `dynamic.go` contains simulated shares (*"In real implementation … simulate with random shares"*), `SignWithBlinding` returns *"implementation in progress"*.
- **Verdict:** **Do not rely on it.** On paper the most feature-complete (change set + threshold, FROST + Taproot), reshare rounds + pubkey check are present, but core logic is placeholder, adoption is ~nil, and there is no audit. **Independent review required before any use.**

### 3.7 dfinity/ic — chain-key threshold Schnorr BIP340 📄
- **Repo:** https://github.com/dfinity/ic · **Lang:** Rust · **License:** Apache-2.0
- **Scheme:** **IDKG** threshold Schnorr **BIP340 over secp256k1** (distinct from RFC-9591 FROST) — see also §2.3 for its ECDSA side
- **Capability:** **(b)** — subnet membership changes trigger XNet/DKG **resharing**; docs: *"the subnet's public key stays the same, but the underlying shares change."* Production BIP340 keys reshared on subnets `2fq7c`/`fuqsr`.
- **Same pubkey / change set:** ✅ / ✅ across rotating node sets.
- **Status:** live on ICP mainnet.
- **Summary:** Real-world same-address resharing of threshold BIP340 secp256k1 keys across rotating committees — but via IDKG, not FROST proper. Relevant if you want the *capability* on secp256k1 Schnorr; not a drop-in FROST library.

### 3.8 Academic / paper-stage (no usable reshare code yet)
- **D-FROST (Dynamic FROST)** — Banca d'Italia, IACR ePrint 2024/896. FROST + CHURP → first Schnorr threshold scheme claiming **both committee and threshold change with no trusted party**, on secp256k1. **Reshare code unreleased.** The related open repo **bancaditalia/secp256k1-frost** (C, MIT, ~17 stars, "testing only") is **DKG + signing only — no reshare.**
- **HARTS** — IACR ePrint 2024/280 (high-threshold/robust/adaptive threshold Schnorr). **No public code.**

---

## 4. Proactive refresh only (same committee) — ECDSA, truly open source

These keep the same address but do **not** rotate the signer set. Listed for completeness.

| Library | URL | Lang | License | Scheme | Evidence | Note |
|---|---|---|---|---|---|---|
| **coinbase/cb-mpc** | https://github.com/coinbase/cb-mpc | C++ (+Go bindings) | MIT | Coinbase ECDSA-MPC (OT/DKLs-style) | `ecdsa_mp.cpp::refresh()`; hard-checks `new_key.Q = current_key.Q` ✅ | active, production-derived; **no** party/threshold change |
| **fireblocks/mpc-lib** | https://github.com/fireblocks/mpc-lib | C++ | GPL-3.0 | MPC-CMP (2020/492) | `cmp_offline_refresh_service` returns existing `public_key` ✅ | active; no party/threshold change |
| **LFDT-Lockness/cggmp21** (ex dfns/cggmp21) | https://github.com/LFDT-Lockness/cggmp21 | Rust | MIT OR Apache-2.0 | CGGMP21 | `key_refresh.rs` (`aux_only`, `non_threshold`) ✅ | audited (Kudelski); **refresh only for n-of-n; NO threshold-key refresh, NO reshare** |

> The much-asked "does **dfns/cggmp21** support reshare?" → **No.** It has refresh, but only for non-threshold
> keys; threshold-key refresh is an open issue and there is no committee-reshare protocol.

---

## 5. Source-available but NOT OSI open source — excluded on license

These have the capability but their licenses are **not** open source (fail your hard requirement).
Flagged so they are not mistaken for OSS.

| Library | URL | Scheme / capability | License problem |
|---|---|---|---|
| **silence-laboratories/dkls23** | https://github.com/silence-laboratories/dkls23 | DKLs23 ECDSA; `key_refresh.rs` + `quorum_change.rs` (add/remove) + `migration.rs` — full (a)+(b) | **Silence Laboratories Non-Commercial License (SLL)** — commercial use requires a paid license. (`quorum_change`/`migration` also **outside** the Trail of Bits audit scope.) |
| **silence-laboratories/ecdsa-tss-js** | https://github.com/silence-laboratories/ecdsa-tss-js | 2-of-2 ECDSA; `getInstanceForKeyRefresh()` (refresh only) | Same non-commercial SLL |
| **Safeheron/multi-party-sig-cpp** | https://github.com/Safeheron/multi-party-sig-cpp | GG18/GG20/CMP; `key_refresh/` + `aux_info_key_refresh/` (refresh only, same pubkey) | **Restrictive NOASSERTION** — forbids distribute/modify/reverse-engineer except contributing back; not OSI |
| **Web3Auth / MetaMask tkey** (`@tkey/tss`, `mpc-core-kit`) | https://github.com/MetaMask/tkey-mpc · https://github.com/Web3Auth/mpc-core-kit | DKLS19 + Torus RSS; `addFactorPub`/`deleteFactorPub` (add/remove factor keys, refresh), same pubkey ✅ | **Ambiguous/proprietary** — `mpc-core-kit`/`tss-client` are ISC, but **`@tkey/tss` ships an "All rights reserved" (Torus) LICENSE** despite declaring ISC, and `MetaMask/tkey-mpc` is NOASSERTION. Also **fixed 2-party threshold** (t not reconfigurable). The only verified JS/TS option, but license is not clean OSS. |
| **Sodot** | https://sodot.dev (demos MIT: https://github.com/sodot-rs) | FROST **BIP340 secp256k1** + DKLs23; "Key Refresh & Resharing", change t/n same address (e.g. 2-of-3→3-of-4) 📄 | **Core is closed-source** proprietary SaaS (gated WASM SDK); only demos are MIT. Acquired by MoonPay 2026. |
| **Lit Protocol** (Node) | (public repo now 404s) | DKLS/cait-sith with per-epoch operator rotation | **Production signing code is in a private repo** — not open source / not verifiable. Its open cait-sith fork is archived (§2.10). |

---

## 6. Excluded — no resharing, wrong curve, or not the target scheme

**Open source but no reshare/refresh at all:**

| Project | Lang | Reason |
|---|---|---|
| ZenGo-X/multi-party-ecdsa | Rust | Only low-level `refresh_private_key` primitives, **no orchestrated reshare**; **abandoned** (multiple CVEs). fs-dkr (§2.5) is the real reshare layer on top. |
| axelarnetwork/tofn & tofnd | Rust | grep for `reshare\|refresh\|proactive\|rotate` empty on both branches; GG20 removed from `main`. |
| ing-bank/threshold-signatures | Rust | GG18 keygen+sign only; no refresh/reshare; archived. |
| coinbase/kryptology | Go | GG20/DKLs + FROST DKG, **no reshare**; **archived** (not used by Coinbase). |
| keep-network/keep-core (tBTC) | Go | GG18-family; rotation = **new DKG → new wallet + on-chain MovingFunds** (funds sent to a *different* key), **not** same-address reshare. |
| bytemare/frost | Go | RFC 9591 signing only; no reshare. |
| topos-protocol/ice-frost | Rust | Explicitly the "static version"; ristretto/arkworks, not secp256k1; no reshare. |
| @cmdcode/frost, StackOverflowExcept1on/frost-secp256k1-evm, safe-research/safe-frost | TS/Rust | secp256k1 FROST but signing-only, no reshare. |
| BlockstreamResearch **ChillDKG** (bip-frost-dkg) | Python | **DKG only; resharing explicitly out of scope** — a new key needs a new DKG (which changes the threshold public key). |
| BIP-445 frost-signing (siv2r) | Python | Signing-only draft; keygen out of scope. |
| secp256k1-zkp FROST (PRs #138/#278) | C | Not in mainline; trusted-dealer signing only; no DKG/reshare. |
| johnoliverdriscoll/py-ggmpc, NillionNetwork/tinysig, h4sh3d PoC | Python | keygen+sign only / educational; no reshare. |
| BitGo/BitGoJS | TS | MPCv2 ECDSA; reshare paths login-gated, **unverified**. |

**Wrong scheme / wrong curve for a Bitcoin FROST/ECDSA requirement:**
- ristretto/ed25519-only FROST (frost-dalek, substrate-system/frost) — not secp256k1.
- commonware-cryptography — threshold **BLS12-381**, not secp256k1 (and Bitcoin can't verify BLS anyway; see [`schemes/06-pairing-based-bls.md`](schemes/06-pairing-based-bls.md)).
- Vultisig iOS/Android apps expose "Reshare", but the crypto is Go `mobile-tss-lib` (§2.7) — counted there.

---

## 7. Cross-cutting caveats before you build on any of these

1. **Refresh ≠ rotation.** Confirm you need capability (b). If you only need proactive security with a
   fixed committee, the refresh-only libraries (§4) are simpler and better audited.
2. **The threshold-change gap.** Even strong FROST libraries (ZF frost) let you change the *members* but
   **not the threshold t** without a new DKG (new key, new address). Only tss-lib, near/threshold-signatures,
   synedrion, dfinity/ic, and Chainflip demonstrably change **both** set and threshold on a fixed key.
3. **Erasure is load-bearing.** Same-address rotation defends against a *mobile* adversary only if old
   shares are securely deleted after resharing. A quorum of old members who kept their shares from one
   epoch can still reconstruct.
4. **Audit status varies widely.** tss-lib and its lineage carry a real vulnerability history
   (Alpha-Rays 2021/1621, TSSHOCK/BitForge CVE-2023-33241/33242) — all implementation ZK-proof bugs, fixed
   in current versions. synedrion, tangle, luxfi are **unaudited**; luxfi is **stub code**. cb-mpc,
   cggmp21, dkls23 (refresh path), Safeheron GG18 are audited.
5. **Library vs protocol.** The two best *production* committee-reshare proofs (Chainflip, dfinity/ic)
   are embedded in large protocol codebases, not packaged libraries — expect integration work.
6. **Bitcoin footprint.** All of these produce a single standard signature; the rotation happens
   **off-chain** with the address unchanged — the whole point. Contrast with native script multisig, where
   rotating a signer forces a new address and an on-chain move ([`schemes/07-native-script-multisig.md`](schemes/07-native-script-multisig.md)).

---

## 8. All source links

**ECDSA (committee reshare):** [bnb-chain/tss-lib](https://github.com/bnb-chain/tss-lib) ·
[near/threshold-signatures](https://github.com/near/threshold-signatures) ·
[dfinity/ic](https://github.com/dfinity/ic) · [entropyxyz/synedrion](https://github.com/entropyxyz/synedrion) ·
[ZenGo-X/fs-dkr](https://github.com/ZenGo-X/fs-dkr) · [getamis/alice](https://github.com/getamis/alice) ·
[vultisig/mobile-tss-lib](https://github.com/vultisig/mobile-tss-lib) ·
[sygmaprotocol/sygma-relayer](https://github.com/sygmaprotocol/sygma-relayer) ·
[tangle-network/cggmp-threshold-ecdsa](https://github.com/tangle-network/cggmp-threshold-ecdsa) ·
[SwingbyProtocol/tss-lib](https://github.com/SwingbyProtocol/tss-lib) ·
[ModChain/tss-lib](https://github.com/ModChain/tss-lib) · [cronokirby/cait-sith](https://github.com/cronokirby/cait-sith)

**ECDSA (refresh only):** [coinbase/cb-mpc](https://github.com/coinbase/cb-mpc) ·
[fireblocks/mpc-lib](https://github.com/fireblocks/mpc-lib) ·
[LFDT-Lockness/cggmp21](https://github.com/LFDT-Lockness/cggmp21)

**FROST / threshold Schnorr:** [ZcashFoundation/frost](https://github.com/ZcashFoundation/frost)
([refresh module](https://github.com/ZcashFoundation/frost/blob/main/frost-core/src/keys/refresh.rs),
[frost-secp256k1-tr docs](https://docs.rs/frost-secp256k1-tr)) ·
[chainflip-io/chainflip-backend](https://github.com/chainflip-io/chainflip-backend) ·
[frostsnap/frostsnap](https://github.com/frostsnap/frostsnap) ·
[LLFourn/secp256kfun](https://github.com/LLFourn/secp256kfun)
([FROST-modification gist](https://gist.github.com/nickfarrow/64c2e65191cde6a1a47bbd4572bf8cf8)) ·
[taurusgroup/multi-party-sig](https://github.com/taurusgroup/multi-party-sig) ·
[luxfi/threshold](https://github.com/luxfi/threshold) ⚠️ ·
[bancaditalia/secp256k1-frost](https://github.com/bancaditalia/secp256k1-frost) (no reshare)

**Not OSS (license-excluded):** [silence-laboratories/dkls23](https://github.com/silence-laboratories/dkls23) ·
[Safeheron/multi-party-sig-cpp](https://github.com/Safeheron/multi-party-sig-cpp) ·
[Web3Auth/mpc-core-kit](https://github.com/Web3Auth/mpc-core-kit) · [Sodot](https://sodot.dev)

*Research method: three parallel source-level surveys (Go ECDSA, non-Go ECDSA, all-language FROST) plus a
vendor-license verification pass. Function/module names, licenses, and secp256k1 support were checked against
the actual repositories; unverifiable claims are marked ⚠️ and nothing was fabricated.*
