# Pairing-Based Threshold Signatures (BLS and Relatives) and Bitcoin

> **Scope.** This document is a deep survey of pairing-based signatures — Boneh–Lynn–Shacham (BLS) and its threshold, multi-, and aggregate variants — and a rigorous treatment of *why they cannot be verified natively on Bitcoin* and what bridging proposals exist. BLS threshold signatures are the "cleanest" threshold signatures known: **non-interactive, one-round, deterministic, uniquely-defined, and aggregatable**. They are the workhorse of Ethereum consensus, drand, DFINITY, and Chainlink. Yet Bitcoin cannot check them, because `secp256k1` is not a pairing-friendly curve and Bitcoin Script has no pairing operation. This is the central contrast case for the rest of the threshold-signatures research corpus.

---

## 1. Executive Summary / Overview

BLS signatures derive their remarkable properties from a **bilinear pairing** `e : G1 × G2 → GT` on a pairing-friendly elliptic curve. A signature is a single group element `σ = x·H(m)`; verification is one pairing equation `e(σ, g2) = e(H(m), pk)`. Because the map is linear in the secret key, BLS composes *for free* with the two most powerful ideas in multi-party cryptography:

- **Shamir secret sharing → threshold BLS** (Boldyreva 2003): each shareholder signs with a single exponentiation, producing a *partial signature* verifiable on its own; any `t+1` partials **Lagrange-interpolate in the exponent** to the exact same signature a single signer would have produced. No interaction, no per-signing-session randomness, no coordination beyond collecting `t+1` shares.
- **Homomorphic aggregation → multi-signatures & aggregate signatures** (Boneh–Gentry–Lynn–Shacham 2003, Boneh–Drijvers–Neven 2018): many signatures on the same message (multi-sig) or on different messages (aggregate) collapse into one group element.

These are properties that ECDSA and Schnorr *do not* natively have. Threshold ECDSA needs heavy interactive multiplication protocols; threshold Schnorr (FROST) needs a commitment round and per-session nonces. BLS needs **one round and nothing else**.

The catch is the pairing. Verification *requires* computing `e`, which only exists efficiently on special curves (BN254, BLS12-381). Ethereum verifies these on-chain via precompiles (BN254 since Byzantium, BLS12-381 via EIP-2537 in the Pectra upgrade). **Bitcoin has no pairing operation, `secp256k1` is not pairing-friendly, and there is no realistic path to an `OP_PAIRING` in Script.** Therefore BLS threshold signatures are used *around* Bitcoin (off-chain in federations and bridges that then produce an ordinary ECDSA/Schnorr signature on-chain), never *by* Bitcoin's consensus. The only conceivable on-chain path is to verify a BLS aggregate *inside a SNARK* and check the SNARK on Bitcoin — which today means BitVM's optimistic, off-chain-execution model, not native validation.

---

## 2. Pairing Background (what makes BLS possible)

### 2.1 Bilinear pairings

A **bilinear pairing** is an efficiently computable map

```
e : G1 × G2 → GT
```

where `G1`, `G2` are additive groups of prime order `r` on an elliptic curve `E/Fq`, and `GT` is an order-`r` multiplicative subgroup of a finite-field extension `Fq^k`. It satisfies:

- **Bilinearity:** `e(aP, bQ) = e(P, Q)^{ab}` for all scalars `a, b` and points `P ∈ G1, Q ∈ G2`.
- **Non-degeneracy:** `e(g1, g2) ≠ 1` for generators `g1, g2`.
- **Computability:** `e` is efficient (Miller loop + final exponentiation).

The value `k` is the **embedding degree** — the smallest `k` such that `r | q^k − 1`, i.e. the smallest field extension into which the pairing lands. Pairing-friendly curves are engineered to have *small* `k` (so `GT` arithmetic is feasible) while `r` and `q^k` are large enough for security.

### 2.2 Pairing types (Galbraith–Paterson–Smart classification)

- **Type-1 (symmetric):** `G1 = G2`. Simplest to describe (co-CDH collapses to CDH) but requires supersingular curves; largely deprecated for efficiency/security.
- **Type-2:** `G1 ≠ G2` with an efficiently computable isomorphism `ψ : G2 → G1` but no efficient hash-to-`G2`.
- **Type-3 (asymmetric, the modern default):** `G1 ≠ G2`, **no** efficiently computable isomorphism between them, but efficient hashing into both groups. BN254 and BLS12-381 are Type-3. Type-3 gives the best size/speed and is what every deployed system uses. Security rests on the **co-CDH / co-GDH** assumptions (below).

### 2.3 The curves that matter

| Curve | Family | Embedding degree `k` | `G1` element | `G2` element | Security (post-2016 est.) | Where used |
|---|---|---|---|---|---|---|
| **BN254 / BN128 / alt_bn128** | Barreto–Naehrig | 12 | 64 B (uncompressed) | 128 B | ~100 bits (dropped from 128 after Kim–Barbulescu exTNFS) | Ethereum `ecPairing` precompile (BN254), Zcash Sprout, BitVM Groth16 verifier |
| **BLS12-381** | Barreto–Lynn–Scott | 12 | 48 B compressed | 96 B compressed | ~120–128 bits | Ethereum consensus, drand, Zcash Sapling, Chia, Filecoin, EIP-2537 |

The **Kim–Barbulescu (2016) extended tower number field sieve** attack reduced the estimated security of BN254 from ~128 to ~100 bits, which is precisely why Ethereum's *consensus* layer chose **BLS12-381** (designed by Sean Bowe at Zcash) rather than reusing BN254. BLS12-381 is a deliberate "sweet spot": `~255`-bit prime-order groups, 48-byte `G1` / 96-byte `G2` compressed encodings, and a fast pairing.

- **`minimal-signature-size`**: public keys in `G2` (96 B), signatures in `G1` (48 B). Preferred when signatures dominate.
- **`minimal-pubkey-size`**: public keys in `G1` (48 B), signatures in `G2` (96 B). Ethereum uses this (validator pubkeys are 48 B in `G1`).

### 2.4 Security assumptions

- **co-CDH (computational co-Diffie–Hellman):** given `g1, g2, x·g2` (and, in some formulations, `x·g1`) and a random `Y ∈ G1`, compute `x·Y`. BLS EUF-CMA security reduces to co-CDH in the **Random Oracle Model** (hashing to the curve modeled as a random oracle).
- **co-GDH (Gap co-Diffie–Hellman):** co-CDH is hard *even given* an oracle that solves the *decisional* problem. On pairing-friendly curves DDH is *easy* (the pairing itself decides it: `e(a·g1, b·g2) = e(g1, g2)^{ab}` can be checked), so these are naturally "gap groups" — CDH hard, DDH easy. This gap is exactly what Boneh–Lynn–Shacham exploited: the pairing is the DDH oracle that lets a *verifier* check a signature the *forger* still cannot produce.

---

## 3. Per-Scheme Deep Dive

### 3.1 BLS signatures — Boneh, Lynn, Shacham (2001 / 2004)

- **Paper:** "Short Signatures from the Weil Pairing." Dan Boneh, Ben Lynn, Hovav Shacham.
- **Venue/Year:** ASIACRYPT 2001 (LNCS 2248, pp. 514–532); journal version *Journal of Cryptology* 17(4), 2004.
- **Links:** https://www.iacr.org/archive/asiacrypt2001/22480516.pdf · abstract: https://crypto.stanford.edu/~dabo/pubs/abstracts/weilsigs.html

**Construction (Type-3, `minimal-signature-size` convention):**

- **Setup:** generators `g1 ∈ G1`, `g2 ∈ G2`; hash-to-curve `H : {0,1}* → G1`.
- **KeyGen:** secret `x ← Z_r`; public key `pk = x·g2 ∈ G2`.
- **Sign(x, m):** `σ = x·H(m) ∈ G1`. (A single scalar multiplication.)
- **Verify(pk, m, σ):** accept iff `e(σ, g2) = e(H(m), pk)`.
  - Correctness: `e(σ, g2) = e(x·H(m), g2) = e(H(m), g2)^x = e(H(m), x·g2) = e(H(m), pk)`.

**Defining properties (why cryptographers call BLS the "cleanest"):**

- **Deterministic:** no per-signature randomness. Same `(x, m)` → same `σ`, always. (Contrast ECDSA/Schnorr, which need a fresh nonce whose reuse or bias leaks the key.)
- **Unique:** exactly one valid signature per `(pk, m)` — signatures are *verifiably unique*, which is why BLS doubles as a **Verifiable Random Function (VRF)** and hence a randomness beacon.
- **Short:** 48 bytes (`G1`), the shortest signatures at 128-bit-ish security among widely deployed schemes.
- **Non-interactive & one-move:** signing is a local operation with no protocol.
- **Aggregatable & threshold-friendly:** linearity in both the key and the hash (see §3.3–3.4).

**Security:** EUF-CMA under co-CDH (co-GDH) in the ROM. Requires `H` to be a proper hash-to-curve (the IETF `hash_to_curve` standard; SSWU map for BLS12-381) and requires **subgroup membership checks** (see §5).

### 3.2 Threshold BLS — Boldyreva (2003), the canonical construction

- **Paper:** "Threshold Signatures, Multisignatures and Blind Signatures Based on the Gap-Diffie-Hellman-Group Signature Scheme." Alexandra Boldyreva.
- **Venue/Year:** PKC 2003 (LNCS 2567, pp. 31–46), Springer.
- **Links:** https://www.iacr.org/archive/pkc2003/25670031/25670031.pdf · IACR cryptodb: https://iacr.org/cryptodb/data/paper.php?pubkey=3368

This is *the* reference threshold BLS. It builds a **robust, proactive `(t, n)` threshold** scheme, a **multi-signature** scheme, and a **blind** signature scheme, all in any Gap-Diffie-Hellman group, all inheriting BLS's simplicity.

**`(t+1)`-of-`n` threshold BLS construction:**

1. **Key sharing.** The secret `x` is shared with a degree-`t` Shamir polynomial `f(z) = x + a_1 z + … + a_t z^t` over `Z_r`. Party `i` holds `x_i = f(i)`; the group public key is `pk = x·g2`, and each party has a verification key `pk_i = x_i·g2`.
   - In practice `f` is never dealt by a trusted party; it comes from a **Distributed Key Generation (DKG)** protocol (§3.6). `x` is *never reconstructed anywhere*.
2. **Partial signing (fully non-interactive, one message):** party `i` outputs `σ_i = x_i·H(m)`. That's it — no round of nonce commitments, no awareness of who else is signing.
3. **Partial-signature verification (robustness):** anyone can check a partial with `e(σ_i, g2) = e(H(m), pk_i)`. This is what makes threshold BLS **robust**: a malicious shareholder's bad share is *immediately detectable*, so honest parties can discard it and still finish — no abort, no need to identify a full quorum in advance beyond having `t+1` *good* partials.
4. **Combination via Lagrange in the exponent:** given any set `S` of `t+1` valid partials, compute Lagrange coefficients `λ_{i,S} = ∏_{j∈S, j≠i} j/(j−i) (mod r)` and output
   ```
   σ = Σ_{i∈S} λ_{i,S} · σ_i
     = Σ_{i∈S} λ_{i,S} · x_i · H(m)
     = ( Σ_{i∈S} λ_{i,S} x_i ) · H(m)
     = f(0) · H(m) = x · H(m).
   ```
   The Lagrange interpolation is done over `Z_r` for the coefficients and *in the group `G1`* for the points. The reconstructed `σ` is **identical, bit-for-bit, to the single-signer signature** — a verifier cannot tell a threshold signature from an ordinary one, and needs only the *group* public key `pk` to verify. This "signature indistinguishability / transparency" is a headline property.

**Why this is uniquely clean:**
- **One round, no coordination:** partial signatures are independent; a combiner needs no interaction from the signers beyond receiving their partial. Compare threshold ECDSA (multiple rounds of multiplicative-to-additive share conversion) or FROST (a preprocessing/commitment round plus a signing round).
- **Deterministic partials:** no nonces to leak, no session state, so it is safe to reuse the same shares indefinitely and to sign concurrently.
- **Proactive security:** Boldyreva's scheme supports **proactive refresh** — shares are periodically re-randomized (add a share of zero) so an attacker must corrupt `t+1` parties *within a single epoch*, not over the system's lifetime.

**Security:** unforgeability reduces to co-GDH in the ROM, with robustness from the pairing-based partial-verification check.

### 3.3 Aggregate signatures — Boneh, Gentry, Lynn, Shacham (2003)

- **Paper:** "Aggregate and Verifiably Encrypted Signatures from Bilinear Maps," EUROCRYPT 2003.
- **Idea:** `n` signatures `σ_i = x_i·H(m_i)` on **distinct** messages/keys aggregate by simple addition: `σ_agg = Σ σ_i`. Verify with `e(σ_agg, g2) = ∏ e(H(m_i), pk_i)` (`n+1` pairings — one per distinct message).
- **Constraint:** messages must be distinct (or keys distinct-and-PoP'd), else the **rogue-key attack** applies (§5.1).

### 3.4 BLS multi-signatures with public-key aggregation — Boneh, Drijvers, Neven (2018)

- **Paper:** "Compact Multi-Signatures for Smaller Blockchains." Dan Boneh, Manu Drijvers, Gregory Neven.
- **Venue/Year:** ASIACRYPT 2018 (Part II, pp. 435–464).
- **Links:** ePrint **2018/483** https://eprint.iacr.org/2018/483 · Stanford: https://crypto.stanford.edu/~dabo/pubs/papers/BLSmultisig.html

This is the scheme most cited in the *blockchain* context. A **multi-signature** is many parties signing the **same** message `m`; it compresses to one signature *and* one aggregate public key, so a verifier stores only `(m, σ, apk)`. Crucially it works in the **plain public-key model** — signers do **not** need a proof-of-possession — by using randomizing coefficients derived from the *set* of keys.

**Scheme (often called "MSP"):**

- Second hash `H1 : G^n → {0,1}^{128 · n}` (or into `Z_r`).
- **Aggregate the signatures:** coefficients `(t_1,…,t_n) = H1(pk_1,…,pk_n)`, then `σ = Σ_i t_i · σ_i`.
- **Aggregate the keys:** `apk = Σ_i t_i · pk_i`.
- **Verify:** `e(σ, g2) = e(H(m), apk)` — one message, so **two pairings regardless of `n`**.

**Rogue-key defense without PoP:** an attacker cannot register `pk' = β·g2 − pk_target` to cancel an honest key, because the per-key coefficient `t_i` is an unpredictable function of the *whole* key vector, computed *after* keys are fixed. This binds each key to a coefficient the attacker cannot control, defeating the linear-cancellation attack. Security reduces to co-CDH in the ROM.

**Relation to threshold:** this is the *`n`-of-`n`* (accountable) case. For `t`-of-`n` you use the Shamir/Lagrange construction of §3.2. Ethereum consensus uses the *aggregate-on-same-message* pattern of §3.3–3.4 to compress hundreds of thousands of validator attestations.

### 3.5 Rogue-key defenses: Proof-of-Possession vs. Message-augmentation vs. MSP

Three standardized defenses (all three appear as ciphersuites in the IETF draft, §4):

1. **Proof-of-Possession (PoP)** — Ristenpart–Yilek, "The Power of Proofs-of-Possession: Securing Multiparty Signatures against Rogue-Key Attacks," EUROCRYPT 2007. Each party publishes `π = x·H_pop(pk)`; a key is only accepted after PoP verifies. Cheapest verification (plain sum aggregation), used by Ethereum (`POP` ciphersuite).
2. **Message augmentation** — sign `H(pk ‖ m)` instead of `H(m)`, forcing distinct effective messages so the aggregate cannot cancel.
3. **MSP / key-aggregation coefficients** — Boneh–Drijvers–Neven above; no PoP needed but each verification recomputes coefficients.

### 3.6 Distributed Key Generation (DKG) for BLS

Threshold BLS needs the Shamir polynomial `f` to exist *without a trusted dealer*. This couples tightly to the DKG research doc; the essentials for BLS:

- **Pedersen DKG / Gennaro–Jarecki–Krawczyk–Rabin (GJKR, "Secure Distributed Key Generation for Discrete-Log Based Cryptosystems," EUROCRYPT 1999/J.Crypto 2007):** each of `n` parties runs a Feldman/Pedersen VSS as a dealer; the group secret is the sum of the honest dealers' constant terms. GJKR showed the naive Pedersen DKG has a *biasable* public key and fixed it. This is the classic BLS DKG (used by drand's setup).
- **Aggregatable DKG** — Gurkan, Jovanovic, Maller, Meiklejohn, Stern, Tomescu, "Aggregatable Distributed Key Generation," EUROCRYPT 2021. Uses a **publicly verifiable secret sharing (PVSS)** with *aggregatable, publicly verifiable transcripts*, cutting the dealer-verification from `O(n²)` to `O(n log n)` and using gossip instead of all-to-all. Enables very large committees (the paper reports aggregating a 130,000-of-260,000 threshold signature in seconds).
- **GLOW / DVRF** — Galindo, Liu, Ordean, Wong, "Fully Distributed Verifiable Random Functions and their Application to Decentralised Random Beacons," IEEE EuroS&P 2021 — threshold BLS as a distributed VRF with a security proof suited to beacons.
- **Proactive / dynamic-committee refresh:** shares are re-shared to a new committee (handoff) so membership can change without changing the group public key — essential for long-lived beacons and validator sets.

DKG remains the *interactive, expensive* part of an otherwise non-interactive scheme. This is a recurring theme: BLS pushes all interaction into a one-time (or periodic) setup, leaving signing itself interaction-free.

---

## 4. Threshold BLS in Practice (and why those chains, not Bitcoin, can verify pairings)

| System | Role of threshold/aggregate BLS | On-chain pairing? |
|---|---|---|
| **Ethereum consensus (beacon chain)** | Validators sign attestations/blocks with BLS12-381; committee signatures **aggregate** into one signature (the breakthrough that lets ~1M validators scale). | Yes — BLS12-381 verification in-protocol; **EIP-2537** adds BLS12-381 precompiles to the *execution* layer (shipped in the Pectra upgrade), complementing the older BN254 `ecPairing` precompile. |
| **drand** (League of Entropy) | `t`-of-`n` **threshold BLS** signature over a round counter = an unbiasable, publicly verifiable randomness beacon; Pedersen DKG for setup; BLS12-381. | Verified by any client via pairing; `bls-unchained-on-g1` scheme puts sigs in `G1` to halve size. |
| **DFINITY / Internet Computer** | Pioneered threshold BLS as a VRF for its random beacon and block-notarization. | Native. |
| **Chainlink VRF / OCR, Dela, Filecoin, Chia, Skale, Dashpay (DIP-6), Tezos, Celo** | Threshold/aggregate BLS for randomness, oracle reports, or consensus. | Native to those chains. |

**The pivotal observation:** every one of these systems *chose a pairing-friendly curve for its consensus/verification layer specifically so it could check BLS on-chain.* Ethereum added an entire precompile (EIP-2537) to do it. **Bitcoin made the opposite choice in 2009** — `secp256k1`, a Koblitz curve optimized for fast ECDSA, with **embedding degree so large that no efficient pairing exists on it** — and Bitcoin's consensus rules are deliberately ossified against adding heavy new cryptographic primitives. That single curve choice is the root of the entire "Bitcoin gap."

---

## 5. Known Issues and Attacks (be careful with BLS)

### 5.1 Rogue-key attack
As in §3.5: without PoP/augmentation/MSP, an attacker registers `pk' = β·g2 − Σ(honest keys)` and forges an aggregate that verifies against the honest parties' keys. This is a *real, deployed-systems* concern (e.g. the Harmony rogue-key discussion). Defense: one of the three §3.5 mechanisms — **mandatory**.

### 5.2 Splitting-zero attack (uniqueness break) — Quan (2021)
- **Paper:** Nguyen Thoi Minh Quan, "Attacks and weaknesses of BLS aggregate signatures," ePrint **2021/377** (https://eprint.iacr.org/2021/377).
- **What breaks:** *uniqueness of aggregate signatures*, **not** unforgeability. Quan showed the PoP aggregate scheme in **BLS RFC draft v4** could produce two distinct valid aggregate signatures for the same message set (via carefully "split" zero-contributions), and identified a variant attack that PoP does **not** prevent. Consequence: applications that rely on aggregate-signature *uniqueness* (e.g. as a VRF seed, or for de-duplication) can be fooled even though no forgery of an honest party's signature occurs.
- **Fix:** later IETF drafts (v5) tightened validation; systems must not assume aggregate uniqueness, and must enforce distinct messages / correct key validation.

### 5.3 Subgroup / small-subgroup attacks
`G1`/`G2` elements must be checked to be in the **correct prime-order subgroup**, not merely on the curve — BLS12-381's `G2` has a cofactor, and a maliciously crafted "public key" or "signature" in a small subgroup can break soundness. The IETF `KeyValidate` and signature-validation steps mandate **subgroup membership checks** and reject the identity element. Several early implementations shipped without these checks and were vulnerable.

### 5.4 Hash-to-curve pitfalls
Security proofs model hash-to-curve as a random oracle. Ad-hoc "try-and-increment" hashing can leak timing or fail to be a proper RO; the standard mandates the constant-time **SSWU / `hash_to_curve`** construction with domain-separation tags.

### 5.5 Standardization status
- **IETF CFRG `draft-irtf-cfrg-bls-signature`** (currently -05): defines `sign/verify/aggregate/FastAggregateVerify`, the two size conventions, and three ciphersuites (`BASIC`, `MESSAGE_AUGMENTATION`, `POP`) over BLS12-381 with SSWU hash-to-curve. Example suite: `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_`. Link: https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-bls-signature-05

---

## 6. THE BITCOIN APPLICABILITY GAP (detailed)

This is the crux of the contrast case. **BLS threshold signatures cannot be verified by Bitcoin consensus.** The reasons are structural, not incidental.

### 6.1 Why `secp256k1` cannot support a pairing
- A pairing needs a curve with **small embedding degree `k`** (BN/BLS curves have `k = 12`). `secp256k1` is an ordinary (non-supersingular, non-pairing-friendly) curve whose embedding degree is enormous (on the order of `r` itself). Computing a pairing would require arithmetic in a field extension `Fq^k` of astronomically large `k` — utterly infeasible. This is by design: a *large* embedding degree is exactly what makes ECDLP-based ECDSA safe against MOV/pairing-reduction attacks. `secp256k1` is pairing-*hostile*, the opposite of pairing-friendly.
- Consequence: even if you could run pairing code, `secp256k1` public keys and points **cannot be inputs to any pairing**. BLS lives on BLS12-381/BN254; Bitcoin keys live on `secp256k1`. They are different curves in different groups.

### 6.2 Why Bitcoin Script cannot check a pairing
- **No pairing opcode.** Bitcoin Script has `OP_CHECKSIG`/`OP_CHECKSIGVERIFY` (ECDSA, and post-Taproot Schnorr, both hardwired to `secp256k1`) and `OP_CHECKMULTISIG`. There is **no `OP_PAIRING`, no field-extension arithmetic, no elliptic-curve point opcodes** at all.
- **No arithmetic to build one.** Script's math opcodes are 32-bit-limited integer ops; `OP_MUL`, `OP_LSHIFT`, etc. were disabled in 2010. There is no bignum arithmetic, no loops, no way to implement a Miller loop over `Fq^12`. A pairing is millions of field multiplications; even the *building blocks* are absent.
- **Consensus ossification.** Adding `OP_PAIRING` or a BLS12-381 precompile would be a consensus-changing **soft/hard fork** requiring near-universal agreement. Bitcoin's culture strongly disfavors adding heavy, hard-to-audit cryptographic primitives to consensus (the BN254/BLS precompile route Ethereum took is politically and technically off the table for Bitcoin). Even Schnorr — a *tiny* change on the *existing* curve — took years (BIP-340, Taproot 2021).

### 6.3 The practical reality: Bitcoin custody uses threshold ECDSA/Schnorr, never BLS
Because Bitcoin can only verify `secp256k1` ECDSA (always) and `secp256k1` Schnorr (since Taproot), *every* trust-minimized system that must move real BTC uses a **threshold `secp256k1`** scheme so the final on-chain artifact is a signature Bitcoin already understands:
- **Threshold ECDSA** (GG18/GG20, CMP/CGGMP, Lindell, DKLs) — used by **tBTC v2** (51-of-100 wallets), custodians, and MPC wallet providers. Heavy interactive protocol, but produces a standard ECDSA sig.
- **FROST / ROAST threshold Schnorr** (RFC 9591, June 2024) — produces a standard 64-byte BIP-340 Schnorr signature under one aggregate key, **indistinguishable from a single-signer Taproot key-path spend**. tBTC's roadmap and many new designs prefer this because it is simpler than threshold ECDSA and native to Bitcoin post-Taproot.
- **Federated sidechains** — **Liquid** (functionary federation, `k`-of-`n` multisig peg), **Rootstock/RSK** (merged-mined, PowPeg), and similar: the peg wallet is a Bitcoin multisig/threshold-`secp256k1` construction. BLS may be used *internally* off-chain for the federation's own consensus messaging, but the Bitcoin-side custody is always `secp256k1`.

**In short: BLS is used *next to* Bitcoin, never *by* Bitcoin.** The threshold-BLS committee signs *its own* records; a separate `secp256k1` threshold signer authorizes the actual BTC movement.

---

## 7. Bridging Proposals (how one might get BLS "onto" Bitcoin)

### 7.1 Federation / sidechain pattern (deployed today)
Run threshold BLS entirely **off-chain** for the sidechain's/oracle's internal consensus, and have the *same* set of parties (or a bridge) hold a **threshold `secp256k1`** key that produces the actual Bitcoin peg-out signature. Bitcoin never sees BLS. This is Liquid, RSK, and every production bridge. **Trust model:** honest-majority (or `t`-of-`n`) federation; not verified by Bitcoin consensus.

### 7.2 Drivechains (BIP-300/301) — a soft-fork alternative to federations
Peter Todd / Paul Sztorc's BIP-300/301 would let **miners** custody sidechain funds via a "hashrate escrow" (a very-large-`M`-of-`N` withdrawal vote spread over thousands of blocks). This *avoids* any pairing — it does not verify BLS; it replaces the federation's threshold key with miner voting. Relevant here only as the *non-cryptographic* alternative to needing on-chain BLS: it moves trust to hashpower rather than to a threshold committee. Still unmerged; requires a soft fork.

### 7.3 SNARK/STARK-verified BLS (the only way BLS could be "checked" via Bitcoin)
The idea: verify the BLS aggregate/pairing **inside a succinct proof** off-chain, then have Bitcoin check the *proof* (a much smaller, pairing-free computation) instead of the pairing itself.
- **The problem:** verifying even a Groth16 SNARK requires a **pairing** (Groth16's verifier is itself a pairing check on BN254). So "verify the proof on Bitcoin" runs into the *same* wall — Bitcoin can't do the pairing in the SNARK verifier either. You've moved the pairing, not eliminated it. STARKs (hash-based, no pairing) sidestep this in principle but produce large proofs and still need substantial computation Bitcoin Script can't natively do.

### 7.4 BitVM / BitVM2 — optimistic off-chain verification (the frontier)
- **Refs:** BitVM2 (https://bitvm.org/bitvm2.html), SNARK verifier in Script (https://bitvm.org/snark.html), BitVM bridge paper (https://bitvm.org/bitvm_bridge.pdf).
- **Mechanism:** BitVM2 implements a **Groth16 SNARK verifier over BN254** as Bitcoin Script — but the full verifier is ~1 GB of Script, far exceeding the 4 MB block / 520-byte-stack-item / ~1000-stack-item limits. So it is **not** run on-chain in the happy path. Instead:
  - The verifier is **chunked** into sub-programs committed via **Lamport/Winternitz signatures** across Tapleaves.
  - Execution is **optimistic**: a prover *asserts* the result on-chain (`assertTx`); if wrong, any watcher runs the single offending chunk in a **`disproveTx`** to slash the prover. Only the disputed step ever touches the chain.
- **Implication for BLS:** because BitVM2 already verifies BN254 pairings *inside* the SNARK, one can in principle prove "this BLS aggregate is valid" off-chain and have BitVM's fraud-proof machinery enforce it. This is the **only** credible route to Bitcoin "recognizing" a BLS threshold signature — and it is **optimistic and off-chain, requires ≥1 honest watcher, needs a large collateralized challenge protocol, and depends on emulated covenants / presigned transactions (and ideally `OP_CAT`/`OP_CHECKTEMPLATEVERIFY` to be robust).** It does not make Bitcoin *natively* verify pairings; it makes Bitcoin *adjudicate a dispute* about an off-chain pairing computation. (Related: Alpen's SNARKnado, BitVMX, Bitlayer — round-efficient SNARK-verifier-on-Bitcoin efforts.)

### 7.5 Covenants / `OP_CAT` (BIP-347) — necessary plumbing, not a pairing
- `OP_CAT` (concatenation, disabled 2010, re-proposed as **BIP-347**) enables Merkle-proof checks on the stack and, via Schnorr-signature tricks, **covenant emulation** and stronger commitment schemes — all of which *improve* BitVM-style constructions and trust-minimized bridges.
- **But `OP_CAT` does not enable pairings.** It gives concatenation, not field-extension arithmetic or a Miller loop. Even with `OP_CAT`, implementing on-chain elliptic-curve ops is expensive and pairings remain infeasible: the fundamental blockers are the missing bignum/modular arithmetic and the sheer computation count, not the lack of concatenation. `OP_CAT` helps you *commit to and verify data*, and to *build the covenants BitVM needs* — it does not let Script compute `e(σ, g2)`.
- **Bottom line:** no realistic soft fork short of a *purpose-built BLS12-381 precompile* (which Bitcoin will not adopt for the reasons in §6.2) makes native on-chain pairing verification feasible. The community consensus is that Bitcoin will verify **proofs about** off-chain computation (optimistically, via BitVM, possibly aided by `OP_CAT`/`CTV`/`CSFS`), not pairings directly.

---

## 8. Comparison to `secp256k1` Threshold Schemes

| Property | **Threshold BLS** (Boldyreva) | **Threshold ECDSA** (GG20/CGGMP) | **Threshold Schnorr / FROST** (RFC 9591) |
|---|---|---|---|
| Curve | BLS12-381 / BN254 (pairing) | `secp256k1` | `secp256k1` |
| Signing rounds | **1 (non-interactive)** | Several (MtA / presigning) | 1 online (+ 1 preprocessing) |
| Per-signing randomness | **None (deterministic)** | Fresh nonce shares | Fresh nonce shares |
| Partial-sig public verifiability / robustness | **Yes (pairing check)** | Hard / costly | Possible (identifiable abort in ROAST) |
| Signature aggregation across signers/messages | **Yes (native, homomorphic)** | No | No (single aggregate key only) |
| Signature size | 48 B | ~64–72 B | 64 B |
| **Verifiable on Bitcoin?** | **NO** (no pairing) | **YES** (standard ECDSA) | **YES** (BIP-340 Schnorr, since Taproot) |
| Setup | DKG (interactive) | DKG (interactive) | DKG (interactive) |
| Main cost | Setup + pairing verify | Heavy signing protocol | Nonce management |

**The trade is stark:** BLS is the best threshold *scheme* but the worst threshold *fit for Bitcoin*; FROST/threshold-ECDSA are messier cryptographically but are the *only* options that yield a Bitcoin-verifiable signature. This is exactly the contrast the surrounding research corpus is built around.

---

## 9. Latest Research (2022–2026)

- **hinTS — "Threshold Signatures with Silent Setup"** (Garg, Jain, Mukherjee, Sinha, et al.), ePrint **2023/567**, **IEEE S&P 2024**. https://eprint.iacr.org/2023/567. A BLS-based threshold scheme with a **silent setup**: parties only publish local public keys plus "hints"; the aggregate key is a *deterministic function* of those local keys — **no interactive DKG**. Supports **dynamic thresholds/signers after setup**, **weighted thresholds with zero overhead**, and produces a **succinct SNARK-style proof** verified with a constant number of pairings. A major step toward practical, DKG-free threshold BLS for large/dynamic validator sets.
- **Threshold Anonymous Credentials with Silent Setup** (E. Garg, S. Garg, et al.), ePrint **2025/2042** — extends the silent-setup paradigm to credentials.
- **Aggregatable DKG** (Gurkan et al., EUROCRYPT 2021) and follow-ups on **PVSS-based, publicly verifiable, `O(n log n)`** DKG transcripts — enabling very large BLS committees.
- **Subset-optimized BLS multi-signature with key aggregation** (Mysten Labs et al.), ePrint **2023/498** — optimizes verification when a *subset* of a fixed large key set signs (relevant to Sui/consensus).
- **Formal verification of aggregate-signature protocols** (Basin et al., "One For All: Formally Verifying Protocols which use Aggregate Signatures," 2025) — Tamarin-style proofs catching the uniqueness pitfalls Quan flagged.
- **BitVM2 / SNARK-verifier-on-Bitcoin** (2024–2026): the Groth16-in-Script verifier, plus round-efficient variants (Alpen **SNARKnado**, **BitVMX**, Bitlayer, Fiamma) — the active frontier for *any* pairing-dependent verification touching Bitcoin.
- **EIP-2537** shipped BLS12-381 precompiles on Ethereum (Pectra), underscoring the divergence: Ethereum keeps *adding* native pairing support; Bitcoin does not.
- **NIST Multi-Party Threshold Cryptography (MPTC)** program (ongoing 2023–2026) is standardizing threshold schemes; threshold BLS/EdDSA/ECDSA are all in scope, but the Bitcoin-relevant standards remain the `secp256k1` ones.

---

## 10. Open Problems

1. **Native on-chain pairing verification on Bitcoin** — effectively closed as infeasible without a purpose-built precompile Bitcoin will not adopt. Research energy has moved to §7.4 (optimistic proof adjudication).
2. **Making SNARK-verified-BLS-on-Bitcoin practical** — shrinking the BitVM Groth16 verifier, reducing challenge-protocol collateral/rounds, minimizing honest-watcher assumptions, and doing so with the *minimal* soft fork (`OP_CAT`/`CTV`/`CSFS`).
3. **DKG-free / silent threshold BLS at scale** — hinTS is promising; open questions on proactive refresh, adaptive security, and weighted-threshold proof costs.
4. **Adaptive & proactive security** for threshold BLS with dynamic committees (validator churn) without expensive re-sharing.
5. **Aggregate-signature uniqueness** — robustly specifying and proving uniqueness (post-splitting-zero) for applications that depend on it (VRF seeds, dedup).
6. **Whether Bitcoin *should* ever gain pairing capability** — a governance/ossification question more than a cryptographic one; the prevailing answer is no.

---

## 11. References

**Core schemes**
- Boneh, Lynn, Shacham. *Short Signatures from the Weil Pairing.* ASIACRYPT 2001; J. Cryptology 2004. https://www.iacr.org/archive/asiacrypt2001/22480516.pdf · https://crypto.stanford.edu/~dabo/pubs/abstracts/weilsigs.html
- Boldyreva. *Threshold Signatures, Multisignatures and Blind Signatures Based on the Gap-Diffie-Hellman-Group Signature Scheme.* PKC 2003. https://www.iacr.org/archive/pkc2003/25670031/25670031.pdf · https://iacr.org/cryptodb/data/paper.php?pubkey=3368
- Boneh, Gentry, Lynn, Shacham. *Aggregate and Verifiably Encrypted Signatures from Bilinear Maps.* EUROCRYPT 2003.
- Boneh, Drijvers, Neven. *Compact Multi-Signatures for Smaller Blockchains.* ASIACRYPT 2018. ePrint 2018/483: https://eprint.iacr.org/2018/483 · https://crypto.stanford.edu/~dabo/pubs/papers/BLSmultisig.html
- Ristenpart, Yilek. *The Power of Proofs-of-Possession: Securing Multiparty Signatures against Rogue-Key Attacks.* EUROCRYPT 2007.

**DKG / VSS**
- Gennaro, Jarecki, Krawczyk, Rabin. *Secure Distributed Key Generation for Discrete-Log Based Cryptosystems.* EUROCRYPT 1999 / J. Cryptology 2007.
- Gurkan, Jovanovic, Maller, Meiklejohn, Stern, Tomescu. *Aggregatable Distributed Key Generation.* EUROCRYPT 2021. https://link.springer.com/chapter/10.1007/978-3-030-77870-5_6 · https://www.benthamsgaze.org/2021/03/24/aggregatable-distributed-key-generation/
- Galindo, Liu, Ordean, Wong. *Fully Distributed Verifiable Random Functions... (GLOW-DVRF).* IEEE EuroS&P 2021.

**Security / attacks / standards**
- Nguyen Thoi Minh Quan. *Attacks and Weaknesses of BLS Aggregate Signatures.* ePrint 2021/377. https://eprint.iacr.org/2021/377
- IETF CFRG. *BLS Signatures*, draft-irtf-cfrg-bls-signature-05. https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-bls-signature-05 · https://www.ietf.org/archive/id/draft-irtf-cfrg-bls-signature-05.html
- *Subset-optimized BLS Multi-signature with Key Aggregation.* ePrint 2023/498. https://eprint.iacr.org/2023/498.pdf
- Basin et al. *One For All: Formally Verifying Protocols which use Aggregate Signatures.* 2025. https://people.inf.ethz.ch/basin/pubs/aggsig25.pdf
- Seurin. *BLS Signatures* (Crypto Book). https://yannickseurin.github.io/crypto-book/cryptography/bls-signatures.html

**Latest (silent setup / threshold)**
- *hinTS: Threshold Signatures with Silent Setup.* ePrint 2023/567; IEEE S&P 2024. https://eprint.iacr.org/2023/567
- *Threshold Anonymous Credentials with Silent Setup.* ePrint 2025/2042. https://eprint.iacr.org/2025/2042.pdf

**Curves / pairings**
- eth2book §2.9 *BLS12-381* and §2.9.1 *BLS Signatures.* https://eth2book.info/latest/part2/building_blocks/bls12-381/ · https://eth2book.info/latest/part2/building_blocks/signatures/
- Ben Edgington. *BLS12-381 For The Rest Of Us.* https://hackmd.io/@benjaminion/bls12-381
- EIP-2537. *Precompile for BLS12-381 curve operations.* https://eips.ethereum.org/EIPS/eip-2537

**Deployments**
- drand cryptography docs. https://docs.drand.love/docs/cryptography/ · https://docs.drand.love/about/
- DFINITY / drand collaboration (Protocol Labs / DEDIS).

**Bitcoin gap / bridging**
- BitVM2. https://bitvm.org/bitvm2.html · SNARK verifier in Script: https://bitvm.org/snark.html · Bridge: https://bitvm.org/bitvm_bridge.pdf
- Alpen Labs. *SNARKnado.* https://www.alpenlabs.io/blog/snarknado-practical-round-efficient-snark-verifier-on-bitcoin
- BitVMX. https://bitvmx.org/knowledge/zero-knowledge-proof-verification-on-bitcoin
- OP_CAT / BIP-347. https://bitcoinmagazine.com/glossary/bitcoin-covenants-op_cat-bip-347
- Drivechain BIP-300/301. https://drivechain.info/ · Peter Todd analysis: https://petertodd.org/2023/drivechains
- FROST. RFC 9591 (June 2024); tBTC v2 docs: https://docs.threshold.network/tbtc-v2
- Bitcoin signature schemes compared (ECDSA/Schnorr/FROST/MuSig2): https://www.spark.money/tools/bitcoin-signature-schemes-comparison
- *Evolving Bitcoin Custody.* https://arxiv.org/pdf/2310.11911

---

*Prepared 2026-07-03. Central conclusion: BLS threshold signatures are the cleanest threshold-signature construction in existence, but are structurally un-verifiable by Bitcoin consensus; on Bitcoin they can only ever be used off-chain (federations/bridges that sign with `secp256k1`) or adjudicated optimistically via BitVM-style proof systems — never checked natively.*
