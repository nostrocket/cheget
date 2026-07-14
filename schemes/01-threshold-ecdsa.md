# Threshold ECDSA Signature Schemes

*A deep survey of t-of-n threshold ECDSA over secp256k1, with emphasis on Bitcoin applicability.*

Last updated: 2026-07-03. All IACR ePrint identifiers below were verified against the live ePrint abstract pages during research; the few citations with no public ePrint (or with uncertain metadata) are explicitly flagged.

---

## 1. Family Overview

### 1.1 What threshold ECDSA is

A **threshold ECDSA** signature scheme lets a set of `n` parties hold *shares* of a single ECDSA private key `d` such that any authorized subset (a `t`-of-`n` or `(t+1)`-of-`n` quorum) can jointly produce a **standard ECDSA signature** on a message, while any unauthorized subset (below threshold) learns nothing about `d` and cannot sign. Crucially the reconstructed signature `(r, s)` is a *plain* ECDSA signature: it verifies under the ordinary ECDSA verification equation against a single public key `Q = d·G`, and is bit-for-bit indistinguishable from a signature produced by a single signer holding `d`.

### 1.2 The core cryptographic problem

An ECDSA signature on message `m` under private key `d` (public key `Q = d·G`, on a curve of prime order `q` with generator `G`) is the pair `(r, s)`:

- Sample a uniformly random nonce `k ∈ Z_q*`; compute `R = k·G`, set `r = R.x mod q` (x-coordinate reduced mod `q`).
- Compute **`s = k⁻¹ · (H(m) + r·d) mod q`**.

Distributing this computation is *much* harder than distributing Schnorr/EdDSA, and the reason is structural:

1. **The signing equation is non-linear in the secrets.** Schnorr's equation `s = k + H·d` is *linear* in the secret, so additive secret-sharing of `k` and `d` composes for free — this is why threshold Schnorr/FROST is comparatively simple. ECDSA instead requires (a) **inverting** the secret shared nonce `k`, and (b) **multiplying** two shared secrets (`k⁻¹` and `d`). If `k = Σ kᵢ` and `d = Σ dᵢ` are additively shared, then `k·d = Σᵢ,ⱼ kᵢ·dⱼ`; the diagonal terms `kᵢ·dᵢ` are local, but the **cross terms** `kᵢ·dⱼ` (i ≠ j) each mix one secret from each of two parties and cannot be computed locally.

2. **Converting a product-of-shares into a fresh additive sharing without revealing either operand is the multiplicative-to-additive (MtA) problem** — the technical crux that every threshold ECDSA scheme must solve, and the dominant cost and main axis of differentiation across the whole family. Formally: party A holds `a`, party B holds `b`; they wish to obtain `α` (to A) and `β` (to B) with `α + β = a·b mod q`, with neither learning the other's input.

3. **Inverting a shared nonce.** Computing `k⁻¹` on a *shared* `k` is expensive. Two standard workarounds:
   - **Bar-Ilan–Beaver "inversion trick"** (used by GGN/GG18/GG20/CGGMP): jointly sample a shared random mask `γ` (or `ρ`), compute and *open* `δ = k·γ`, then `k⁻¹ = γ·δ⁻¹` where `δ⁻¹` is a public field inversion. This turns a distributed inversion into one distributed multiplication plus a local inversion. `R = δ⁻¹ · (Σ γᵢ)·G = k⁻¹·G` is recovered without anyone learning `k`.
   - **Multiplicative sharing of the nonce** (used by MacKenzie–Reiter, Lindell 2-party): set `k = k₁·k₂`, making inversion "free" since `k⁻¹ = k₁⁻¹·k₂⁻¹`, each factor inverted locally.

### 1.3 The three MtA building-block families (and their trade-offs)

Schemes are best organized by *how they realize the MtA / multiplication step*:

| MtA family | Mechanism | Bandwidth | Computation | Setup / assumption cost | Range proofs? |
|---|---|---|---|---|---|
| **Paillier-based** (GGN, Lindell, GG18, GG20, CGGMP) | Additively-homomorphic Paillier: `Enc(a)^b · Enc(β') = Enc(a·b + β')` | Low–medium (compact ciphertexts) | High (2048–3072-bit modular exponentiation) | High: each party generates a Paillier keypair; needs biprime/no-small-factor + ring-Pedersen proofs | **Yes — mandatory, expensive.** Plaintext space `Z_N ≫ q`, so must prove inputs are in-range to prevent wraparound. Historically the #1 source of key-extraction bugs. |
| **OT-based** (DKLs18/19/23) | Gilboa OT multiplication run over OT extension (KOS15 / SoftSpokenOT) → VOLE | High (OT-extension transcripts are large) | Very low (symmetric-key hashing only, no modexp) | None number-theoretic; relies only on ECDSA + CDH in ROM | **No.** Correctness comes from the OT structure, not from proving ranges. |
| **Class-group / CL-based** (CCL+, Deng, Braun–Damgård–Orlandi, Wong, Trout, Lyu) | CL linearly-homomorphic encryption with plaintext space *exactly* `Z_q` | Very low (up to ~10× less than Paillier) | High-ish (class-group ideal arithmetic slower than modexp) | Transparent: only public discriminant params, no per-party key, no trusted setup | **No.** Plaintext space is exactly `Z/qZ`, so there is no larger ring to overflow into — range proofs are unnecessary by construction. |

One-line summary of the fundamental trade-off: **OT-based MtA = fast compute, no number-theoretic setup, high bandwidth; Paillier MtA = low bandwidth, heavy compute + heavy setup + fragile mandatory range proofs; class-group MtA = low bandwidth + transparent setup + no range proofs, at the cost of slower group arithmetic.**

### 1.4 Dimensions that distinguish schemes

- **Threshold structure**: 2-of-2, 2-of-n, general t-of-n / (t+1)-of-n; *threshold-optimal* means security against exactly `t` corruptions for a `(t+1)` signing quorum.
- **Rounds and presigning**: total signing rounds; whether there is an **offline/preprocessing (presign)** phase that is message-independent, enabling **non-interactive (single-round) online signing** once `m` is known.
- **DKG / setup**: trusted dealer vs. dealerless DKG; Paillier key generation and its proofs; ring-Pedersen auxiliary parameters; class-group discriminant.
- **Security model**: game-based vs. simulation-based vs. **UC** (universal composability); **static** vs. **adaptive** corruption; **honest-majority** (t < n/2) vs. **dishonest-majority** (up to t−1 or n−1 corruptions).
- **Identifiable abort (IA)**: on failure, can honest parties pinpoint the cheater (accountability) rather than merely abort?
- **Proactive security / key refresh**: periodic re-randomization of shares so a mobile adversary must corrupt a quorum within a single epoch.
- **Robustness / guaranteed output delivery (GOD)**: can the honest quorum complete a signature despite misbehaving or dropped parties, without restart?
- **Concurrency**: safety under parallel/concurrent signing sessions (UC gives this for free; game-based schemes often only prove sequential or partial concurrency).

---

## 2. Bitcoin Applicability

Bitcoin's legacy (P2PKH), P2SH, and SegWit v0 (P2WPKH / P2WSH) spending paths verify **ECDSA signatures over secp256k1** (`OP_CHECKSIG` / `OP_CHECKSIGVERIFY`). Therefore threshold ECDSA is directly applicable to *all pre-Taproot* Bitcoin outputs:

- **Zero on-chain footprint.** A threshold ECDSA signature is a normal 64–72-byte DER/compact ECDSA signature. An observer cannot distinguish a t-of-n threshold-signed transaction from a single-key transaction. This contrasts sharply with on-chain multisig (`OP_CHECKMULTISIG` / P2SH multisig), where the number of signers, the public keys, and the policy are all revealed on-chain and pay proportionally more in fees and witness size.
- **Fee and privacy parity with single-sig.** Because the witness is a single signature + single pubkey, fees and the UTXO/witness size match single-sig, and the wallet's policy (2-of-3 custody, MPC across HSMs, etc.) is private.
- **Custody use case.** This is why threshold ECDSA is the workhorse of institutional custody and MPC wallets: the private key never exists in one place — not at key generation, not at signing — eliminating the single point of compromise while remaining a standard Bitcoin address.
- **Taproot caveat.** Taproot (BIP340) key-path spends use **Schnorr**, not ECDSA. For Taproot key-path, threshold Schnorr (FROST and relatives) is the natural tool; threshold ECDSA remains the tool for legacy/SegWit-v0 outputs and for the very large installed base of ECDSA-only chains (most EVM chains, etc.). Many production custodians run both: threshold ECDSA (secp256k1) for Bitcoin/EVM and threshold Schnorr/EdDSA for Taproot/Ed25519 chains.
- **BIP32 / additive key derivation interaction.** Hierarchical-deterministic derivation (BIP32) composes with threshold ECDSA, but combining **additive key derivation with presignatures** has a subtle security pitfall (Groth–Shoup, EUROCRYPT 2022 — see §6.3): naively it admits a cube-root attack. Bitcoin-facing deployments that presign must use *re-randomized presignatures* / *homogeneous key derivation* to stay at plain-ECDSA security.

---

## 3. Per-Scheme Deep Dives

Grouped by MtA family. Each entry: authors / paper / venue / year / ePrint + URL; threshold; rounds; DKG/setup; security model & assumptions; costs; robustness; known attacks.

### 3.A Early and Two-Party Schemes

#### 3.A.1 MacKenzie–Reiter (2001 / 2004) — the ancestor

- **Authors / paper**: Philip D. MacKenzie, Michael K. Reiter, *"Two-Party Generation of DSA Signatures."*
- **Venue / year**: CRYPTO 2001 (LNCS 2139, pp. 137–154); extended journal version in *International Journal of Information Security* 2(3–4):218–239, 2004.
- **ePrint**: **None found** — predates routine ePrint posting; no IACR ePrint entry located. Authoritative free copy: https://users.ece.cmu.edu/~reiter/papers/2004/IJIS.pdf ; CRYPTO DOI 10.1007/3-540-44647-8_8.
- **Threshold**: strictly **2-of-2**.
- **MtA / shares**: **multiplicative sharing** of both the key (`d = d₁·d₂`) and the nonce (`k = k₁·k₂`). Share conversion uses an additively-homomorphic encryption scheme (concretely **Paillier**) plus a battery of special-purpose ZK proofs forcing consistent, in-range values. This multiplicative rewriting of DSA is the template for the entire two-party literature.
- **Setup / assumptions**: security under the **Decision Composite Residuosity Assumption (DCRA)** in the **random oracle model**.
- **Security model**: simulation-based, secure under **concurrent composition** (strong for its era), static malicious adversary, 2-party (no honest majority).
- **Rounds / cost**: multi-round; several Paillier operations + ZK proofs per signature — heavier than modern schemes but conceptually foundational.
- **Output**: standard DSA/ECDSA `(r, s)`.

#### 3.A.2 Gennaro–Goldfeder–Narayanan (GGN, ACNS 2016)

- **Authors / paper**: Rosario Gennaro, Steven Goldfeder, Arvind Narayanan, *"Threshold-Optimal DSA/ECDSA Signatures and an Application to Bitcoin Wallet Security."*
- **Venue / year**: ACNS 2016 (LNCS 9696, pp. 156–174). DOI 10.1007/978-3-319-39555-5_9.
- **ePrint**: **2016/013** — https://eprint.iacr.org/2016/013
- **Threshold**: general **t-of-n**, and **threshold-optimal** (a `(t+1)` quorum signs; secure against `t` corruptions); **first general threshold DSA without an honest-majority requirement**.
- **MtA**: **Paillier-based MtA** with **ZK range proofs**; nonce inversion via the Bar-Ilan–Beaver masking trick. DKG produces a Shamir/additive sharing of `d` plus a shared/threshold Paillier key.
- **Setup / assumptions**: Paillier (DCRA) + range proofs; the shared/threshold Paillier key generation is the practical bottleneck (hard for >2 parties — the open problem later addressed by Lindell–Nof and by GG18's per-party keys).
- **Security model**: simulation-based, static malicious, dishonest majority; assumes reliable broadcast.
- **Rounds**: ~6-round interactive signing.
- **Output**: standard ECDSA/DSA. Explicitly motivated by Bitcoin wallet security.

#### 3.A.3 Boneh–Gennaro–Goldfeder (BGG, LATINCRYPT 2017)

- **Authors / paper**: Dan Boneh, Rosario Gennaro, Steven Goldfeder, *"Using Level-1 Homomorphic Encryption to Improve Threshold DSA Signatures for Bitcoin Wallet Security."*
- **Venue / year**: LATINCRYPT 2017 (proceedings LNCS 11368, pp. 352–377). DOI 10.1007/978-3-030-25283-0_19.
- **ePrint**: **None verified** — no corresponding IACR ePrint located; cite via the Springer DOI. (Some web summaries wrongly attribute 2017/552 to this paper; that ID is Lindell 2017.)
- **Contribution**: improves GGN by using a **"level-1" (one-multiplication) homomorphic encryption** in place of pure Paillier, reducing signing to **~4 rounds**. Same threshold-optimal t-of-n, dishonest-majority, Paillier-family assumptions and ZK machinery. Direct predecessor of GG18.

#### 3.A.4 Lindell (CRYPTO 2017) — fast two-party ECDSA

- **Authors / paper**: Yehuda Lindell, *"Fast Secure Two-Party ECDSA Signing."*
- **Venue / year**: CRYPTO 2017 (LNCS 10402, pp. 613–644); full version *Journal of Cryptology* 34:44 (2021), DOI 10.1007/s00145-021-09409-9.
- **ePrint**: **2017/552** — https://eprint.iacr.org/2017/552
- **Threshold**: **2-of-2** (dishonest majority).
- **MtA / core trick**: **multiplicative sharing** of `d` (`d = d₁·d₂`, `Q = d₁·(d₂·G)`). P1 publishes `c_key = Enc_{pk1}(d₁)` (Paillier) at key-gen with a ZK proof that the ciphertext encrypts the discrete log behind its curve point and that `N` is well-formed. To sign, P2 uses Paillier's additive homomorphism to compute, in a **single homomorphic evaluation**, an encryption of `k₂⁻¹·H(m) + k₂⁻¹·r·d₁·d₂` (masked); P1 decrypts and multiplies by `k₁⁻¹` to get `s`. This one-shot homomorphic evaluation replaces GGN's interactive range-proof MtA — the source of its ~100× speedup.
- **Setup / assumptions**: DKG with Schnorr-style ZK proofs of contributions; P1's Paillier keypair.
- **Security model**: **game-based** proof under standard assumptions for **sequential** composition; *additionally* a simulation proof under a **non-standard "Paillier-EC" interactive assumption**. Partial concurrency ("abort-all-if-one-aborts"). Static malicious.
- **Cost / rounds**: extremely light — ~2 rounds of signing, **≈37 ms** for a P-256 signature on standard VMs.
- **Output**: standard ECDSA.
- **Known attacks**: (1) **chosen-N small-primes attack** — a malicious party sets its Paillier `N` to a product of small primes, decrypts the honest share modulo each factor, and CRT-combines to recover the key; mitigated by biprime/no-small-factor proofs and by strict abort-on-failure. (2) **CVE-2023-33242 (Fireblocks "BitForge", Lindell17 abort vulnerability)** — implementations that continue after a failed signature (contrary to the paper's mandate) leak one key bit per signing via a maliciously crafted ciphertext, giving full recovery in ~200–256 signatures. See §6.

#### 3.A.5 Lindell–Nof (CCS 2018) — practical multiparty with fast DKG

- **Authors / paper**: Yehuda Lindell, Ariel Nof, *"Fast Secure Multiparty ECDSA with Practical Distributed Key Generation and Applications to Cryptocurrency Custody."* (Extended ePrint versions additionally credit Iftach Haitner and Samuel Ranellucci.)
- **Venue / year**: ACM CCS 2018 (pp. 1837–1854). DOI 10.1145/3243734.3243788.
- **ePrint**: **2018/987** — https://eprint.iacr.org/2018/987
- **Threshold**: general **t-of-n** — **first practical full-threshold (>2 party) ECDSA with fast DKG** (avoids distributed Paillier key generation, which was the prior obstacle).
- **MtA / core trick — "compute-then-check"**: rather than proving each multiplication correct with ZK range proofs, they run a secure multiplication guaranteeing only *privacy* (not correctness), then **verify the resulting ECDSA signature** (cheap and public) — a wrong signature just fails verification and is discarded. This lets them skip expensive per-multiplication range proofs. Multiplication is instantiated with **either Paillier or OT**; ElGamal-in-the-exponent commitments enable the practical multiparty DKG.
- **Security model**: simulation-based, static malicious, dishonest majority. **Security depends on aborting on verification failure** (the check is load-bearing).
- **Rounds / cost**: multi-round; competitive with GG18 (concurrent independent work).
- **Output**: standard ECDSA. Explicitly targeted at cryptocurrency custody.

---

### 3.B Paillier-Based Multiparty (the production mainstream: GG18, GG20, CGGMP)

**How Paillier MtA works.** Paillier is additively homomorphic: `Enc(a)·Enc(b)=Enc(a+b)`, `Enc(a)^b=Enc(a·b)`. To convert `a·b`: (1) Alice sends `c_A = Enc_A(a)`; (2) Bob picks random `β' ← Z_N`, computes `c_B = c_A^b · Enc_A(β') = Enc_A(a·b+β')`, returns it; (3) Alice decrypts `α = a·b+β' mod N`, Bob sets `β = −β' mod q`, so `α+β = a·b mod q`. The **MtAwc** ("with check") variant adds a ZK proof, used when a multiplicand equals a committed key share `W_j = w_j·G`, so Bob cannot inject a wrong key. Because Paillier plaintexts live in `Z_N` (`N≈2048–3072 bits`) while inputs are ~256 bits, **MtA is unsound without ZK range proofs** bounding the inputs — omitting/weakening them is exactly the origin of the 2021–2023 key-extraction attacks (§6).

#### 3.B.1 GG18 — Gennaro–Goldfeder (CCS 2018)

- **Authors / paper**: Rosario Gennaro, Steven Goldfeder, *"Fast Multiparty Threshold ECDSA with Fast Trustless Setup."*
- **Venue / year**: ACM CCS 2018 (pp. 1179–1194). DOI 10.1145/3243734.3243859.
- **ePrint**: **2019/114** — https://eprint.iacr.org/2019/114 (last revised 2021-12-17, note "Fixes some issues in the protocol and security proof").
- **Threshold**: full **(t+1)-of-n**, dishonest majority, arbitrary `t ≤ n`.
- **DKG / setup**: first scheme with **efficient dealerless (trustless) DKG**; each party generates and publishes **its own Paillier keypair** (originally with only a weak well-formedness proof — later a documented weakness).
- **MtA**: Paillier MtA/MtAwc with **range proofs**; nonce inversion via Bar-Ilan–Beaver mask `γ` (open `δ = kγ`, invert publicly).
- **Rounds**: paper describes **6 phases**; deployments expand this (e.g. Binance `tss-lib` runs a **9-round** signing protocol).
- **Security model**: **simulation/game-based** (not UC), static malicious, dishonest majority. Assumptions: Paillier semantic security, DDH, Strong-RSA (for range proofs), ECDSA unforgeability.
- **Identifiable abort**: **No.** **Proactive**: No.
- **Cost**: `O(n)` MtA instances per signer (one per co-signer), each a few Paillier exponentiations over a 2048–3072-bit modulus + range proofs; `O(n)` comm per party, a few–tens of KB per party per signature.
- **Output**: standard ECDSA.
- **Known attacks**: Alpha-Rays (2021/1621), TSSHOCK (2023), BitForge/CVE-2023-33241 — all exploit missing/weak Paillier-key or range proofs. See §6.

#### 3.B.2 GG20 — Gennaro–Goldfeder (2020): one-round online + identifiable abort

- **Authors / paper**: Rosario Gennaro, Steven Goldfeder, *"One Round Threshold ECDSA with Identifiable Abort."*
- **Venue / year**: 2020 (associated with ACM CCS 2020).
- **ePrint**: **2020/540** — https://eprint.iacr.org/2020/540 (last revised 2021-12-17: "Second Revision fixes issues with the multiplicative-to-additive share conversion protocol").
- **Threshold**: (t+1)-of-n, dishonest majority.
- **Key advances**: (1) **offline/online split with a non-interactive (one-round) online phase** — all message-independent work (MtA, `R`, presignature) is precomputed; once `m` is known each signer broadcasts a *single* message and signers need not be online simultaneously. (2) A **tailored identifiable-abort mechanism** with minimal overhead — on failure the misbehaving party is pinpointed via ZK consistency checks (rather than expensive generic MPC-with-IA).
- **DKG / setup / MtA**: same trustless DKG and per-party Paillier keys and Paillier MtA/MtAwc + range proofs as GG18 (hardened in the 2021 revision).
- **Security model**: game/simulation (not UC), static, dishonest majority. **Identifiable abort: Yes. Proactive: No.**
- **Output**: standard ECDSA.
- **Known attacks**: same Paillier-key/range-proof attack class as GG18 (Alpha-Rays, TSSHOCK, CVE-2023-33241).

#### 3.B.3 CMP / CGGMP20 / CGGMP21 — the UC, proactive, adaptive baseline

- **Authors / paper**: Ran Canetti, Rosario Gennaro, Steven Goldfeder, Nikolaos Makriyannis, Udi Peled, *"UC Non-Interactive, Proactive, Threshold ECDSA with Identifiable Aborts."* (CCS'20 abstract = "CMP"; the extended five-author ePrint = "CGGMP21".)
- **Venue / year**: ACM CCS 2020 (pp. 1769–1787). DOI 10.1145/3372297.3423367.
- **ePrint**: **2021/060** — https://eprint.iacr.org/2021/060 (last revised 2024-10-21).
- **Threshold**: (t+1)-of-n, dishonest majority.
- **Rounds**: two main variants —
  - **4-round protocol**: **3-round message-independent presign + 1 non-interactive online round** (the flagship cold-wallet mode).
  - **7-round protocol**: heavier variant with reduced assumptions / stronger IA.
  Only the last round needs the message.
- **DKG / setup — the most elaborate in the family**: (1) distributed ECDSA key generation with committed (Feldman) shares; (2) **auxiliary-info & key-refresh**: each party generates a Paillier modulus `N_i` and **ring-Pedersen parameters `(N̂_i, s_i, t_i)`** and must publish ZK proofs of well-formedness — **Πmod** (Paillier–Blum: `N` is a Blum biprime, no small factors), **Πprm** (ring-Pedersen `s = t^λ mod N̂`), and a **no-small-factor** proof used inside signing range proofs. *These proofs are exactly what GG18/GG20 lacked and what closes the 2023 attack surface.*
- **MtA**: Paillier MtA wrapped in a **full ZK suite** (Π_enc, Π_aff-g, Π_log, …) tied to the ring-Pedersen parameters, making share conversion soundly range-bounded.
- **Security model — the strongest of the classical family**: **UC-secure** in the global ROM; realizes a threshold-signature functionality guaranteeing unforgeability, **proactive security**, and **identifiable abort**; withstands **adaptive corruption** (GG18/GG20 are static-only). Assumptions: Strong-RSA, DDH, Paillier semantic security, enhanced ECDSA unforgeability.
- **Identifiable abort: Yes. Proactive/refresh: Yes. Adaptive: Yes. Concurrency: UC ⇒ arbitrary composition.**
- **Cost**: higher per-signature compute than GG18/GG20 (extra ZK proofs), but the presign/online split makes the *online* cost minimal (one round, no Paillier ops online). `O(n)` comm per party per presignature.
- **Output**: standard ECDSA.
- **Implementations**: Fireblocks MPC-CMP, DFNS (Rust), Entropy `synedrion`, LF Decentralized Trust `cggmp21`, Bolt Labs `tss-ecdsa`, Taurus `multi-party-sig`.
- **Known attacks**: not vulnerable to the "missing-proof" class *by design*, **but** TSSHOCK's α-shuffle broke a specific CGGMP21 implementation (Taurus) via a Fiat-Shamir encoding flaw — CGGMP implementations are only as safe as their proof encodings and iteration counts.

#### 3.B.4 Companion / production notes

- **Fireblocks MPC-CMP** popularized the "3-round presign + 1-round non-interactive online-sign" framing and the cold-wallet motivation.
- **Makriyannis & Peled, "A Note on the Security of GG18"** (Fireblocks whitepaper) documents GG18's Paillier-key/range-proof soundness gap motivating CGGMP.

---

### 3.C OT-Based (the DKLs family)

**How OT MtA works.** Use **Gilboa's OT-based two-party multiplication**: B's input `b` is fed (bit/digit-wise) as the chooser across a batch of oblivious transfers whose sender messages derive from A's input `a`; the outputs sum to an additive sharing of `a·b`. Batched cheaply over **OT extension** (KOS15 or SoftSpokenOT), this needs **only symmetric-key hashing** — no modexp, no Paillier, no range proofs; security from CDH/ECDSA alone. Cost profile: fast compute, large transcripts (high bandwidth).

#### 3.C.1 DKLs18 — two-party from ECDSA assumptions

- **Authors / paper**: Jack Doerner, Yashvanth Kondi, Eysa Lee, abhi shelat, *"Secure Two-party Threshold ECDSA from ECDSA Assumptions."*
- **Venue / year**: IEEE S&P 2018. DOI via IEEE Xplore 8418649.
- **ePrint**: **2018/499** — https://eprint.iacr.org/2018/499
- **Threshold**: 2-of-n (2-party signing core).
- **MtA / setup**: hardened **Gilboa OT multiplication** over **KOS15 actively-secure OT extension**; dealerless DKG; one-time base OTs then extension. **No Paillier, no range proofs.**
- **Security**: simulation-based, malicious, static, dishonest majority, in the **random oracle model** under **CDH + ECDSA assumptions only**.
- **Cost**: multiplication ≈ a few thousand hashes (single-digit ms); low compute, higher bandwidth. Considered **subsumed by DKLs23**.

#### 3.C.2 DKLs19 — the multiparty case

- **Authors / paper**: Doerner, Kondi, Lee, shelat, *"Threshold ECDSA from ECDSA Assumptions: The Multiparty Case."*
- **Venue / year**: IEEE S&P 2019. IEEE Xplore 8835354.
- **ePrint**: **2019/523** — https://eprint.iacr.org/2019/523
- **Threshold**: full **t-of-n**.
- **MtA / setup**: composes **pairwise Gilboa OT multiplications** among the `t` signers over OT extension; fully distributed DKG; Lagrange conversion of shares at signing.
- **Security**: malicious, static, dishonest majority (up to `t−1`) under **CDH** in the **global ROM** (designed for concurrent composition).
- **Cost**: low compute, bandwidth grows with threshold (pairwise MtA).

#### 3.C.3 DKLs23 (DKLs24) — threshold ECDSA in three rounds

- **Authors / paper**: Jack Doerner, Yashvanth Kondi, Eysa Lee, abhi shelat, *"Threshold ECDSA in Three Rounds."*
- **Venue / year**: **IEEE S&P 2024** (pp. 3053–3071); paper dated 2023 on ePrint (hence "DKLs23"/"DKLs24"). Project: http://dkls.org/
- **ePrint**: **2023/765** — https://eprint.iacr.org/2023/765
- **Threshold**: full **t-of-n**, malicious, dishonest majority.
- **Rounds**: **3** (state-of-the-art for dishonest majority prior to 2025); with a key-independent presign round precomputed, online signing is 2 rounds.
- **MtA / core**: a new **2-round OT-based vectorized multiplication (VOLE)** that outperforms prior OT MtAs; uses the Abram et al. (EUROCRYPT'22) intermediate signature representation + a statistical consistency check. DKG via simple commit-release-and-complain, **no proofs of knowledge**.
- **Security model**: **information-theoretically UC-realizes** a standard threshold-signing functionality **assuming only ideal commitment + ideal two-party multiplication** — so the ECDSA-specific portion is unconditional and all computational assumptions enter only through the (OT/ECDSA-based) instantiation of those two primitives. Overall security rests **solely on ECDSA hardness**.
- **Cost**: fast compute, `O(n)` outgoing communication. **Identifiable abort: not a headline claim. Proactive: not the focus.**
- **Output**: standard ECDSA.
- **Adoption**: the industry-dominant modern protocol — Coinbase, Silence Laboratories, Web3Auth/MetaMask Embedded Wallets, Vultisig, Utila, etc.
- **Follow-up hardening**: **Asharov (ePrint 2026/976, "Revisiting DKLs…")** identifies **security-parameter issues in DKLs's OT-based VOLE** (original parameters don't reach the intended security level) and fixes them, analyzes three VOLE variants, and gives an optimized two-party signing with a **~0.2 KB/party online phase** not vulnerable to known presignature attacks.

#### 3.C.4 OT building blocks

- **KOS15** — Keller, Orsini, Scholl, *"Actively Secure OT Extension with Optimal Overhead,"* CRYPTO 2015, **ePrint 2015/546**. Actively-secure OT extension in ROM. **Caveat**: Roy (CRYPTO 2022) found KOS15's Lemma 1 flawed, so the protocol as stated lacks a complete proof — motivating SoftSpokenOT.
- **SoftSpokenOT** — Lawrence Roy, CRYPTO 2022, **ePrint 2022/192**. First OT extension to beat IKNP communication in the Minicrypt model, with an explicit **communication–computation tradeoff** — lets DKLs implementations trade spare compute for lower bandwidth, and repairs KOS's proof gap.
- **PCG/OT with constant overhead** — Boyle, Couteau, Gilboa, Ishai, Kohl, Resch, Scholl, EUROCRYPT 2024, **ePrint 2023/817** — underpins "cheap OT/VOLE." (Note a cross-venue number collision: NDSS paper #2023-817 is instead Wong et al. "Real Threshold ECDSA.")

---

### 3.D Class-Group / CL-Encryption Based

**Foundation — Castagnos–Laguillaumie (CT-RSA 2015, ePrint 2015/047), "Linearly Homomorphic Encryption from DDH."** Builds a linearly-homomorphic encryption whose **plaintext space is exactly `Z/qZ`** for the curve order `q`, instantiated in the **class group of an imaginary quadratic order** — a group of *unknown order* containing a subgroup `F` of known order `q` with easy discrete log. Security rests on the **HSM (Hard Subgroup Membership)** assumption (a DDH-type assumption over class groups), **not on factoring**. Because the order is unknown, there is **no factorization trapdoor and no secret setup** (unlike Paillier's `p,q`) — parameters are public-coin/transparent. **Why this matters for MtA**: with plaintext space exactly `Z/qZ` there is *no larger ring to overflow into*, so the homomorphic product is correct mod `q` by construction and **the expensive Paillier range proofs vanish** — only cheap well-formedness/knowledge proofs remain.

#### 3.D.1 CCL+ two-party (CRYPTO 2019)

- **Authors / paper**: Castagnos, Catalano, Laguillaumie, Savasta, Tucker, *"Two-Party ECDSA from Hash Proof Systems and Efficient Instantiations."*
- **Venue / year**: CRYPTO 2019 (LNCS 11694).
- **ePrint**: **2019/503** — https://eprint.iacr.org/2019/503
- **Threshold**: 2-of-2.
- **Contribution**: recasts Lindell 2017 through **hash proof systems**, yielding a simulation proof **without Lindell's non-standard interactive "Paillier-EC" assumption**, and instantiates via **CL class groups** so the Paillier plaintext-mismatch disappears (operations are naturally mod `q`).

#### 3.D.2 CCL+ Bandwidth-Efficient Threshold EC-DSA (PKC 2020)

- **Authors / paper**: Castagnos, Catalano, Laguillaumie, Savasta, Tucker, *"Bandwidth-Efficient Threshold EC-DSA."*
- **Venue / year**: PKC 2020 (LNCS 12111).
- **ePrint**: **2020/084** — https://eprint.iacr.org/2020/084
- **Threshold**: full **t-of-n**, malicious, dishonest majority.
- **Design**: a **CL-based variant of GG18** — swaps Paillier MtA for **CL-encryption MtA**, **eliminating all range proofs** (only ciphertext well-formedness proofs remain). No per-party Paillier key, no trusted setup — only public discriminant params.
- **Result**: reduces signing bandwidth by **~4.4×–9×** vs. best prior secure protocols.
- **Security**: simulation-based, static, malicious dishonest majority.
- **Trade-off**: class-group arithmetic is slower per op than Paillier modexp, traded for far smaller messages + eliminated range proofs + trustless setup.

#### 3.D.3 CCL+ "…Revisited" (2021 ePrint / TCS 2023)

- **Authors / paper**: Castagnos, Catalano, Laguillaumie, Savasta, Tucker, *"Bandwidth-efficient threshold EC-DSA revisited: Online/Offline Extensions, Identifiable Aborts, Proactivity and Adaptive Security."*
- **Venue / year**: *Theoretical Computer Science* (Elsevier) 2023; ePrint posted 2021.
- **ePrint**: **2021/291** — https://eprint.iacr.org/2021/291
- **New features** layered on the PKC 2020 CL scheme: **online/offline (presign) with non-interactive online phase**; **identifiable abort**; **proactive security** (share refresh); **adaptive security** (for the n-of-n case); concurrency-safe. Claims **up to ~10×** bandwidth improvement over Paillier-based schemes achieving the same goals.

#### 3.D.4 Deng et al. — Promise Σ-protocol (ASIACRYPT 2021)

- **Authors / paper**: Yi Deng, Shunli Ma, Xinxuan Zhang, Hailong Wang, Xuyang Song, Xiang Xie, *"Promise Σ-protocol: How to Construct Efficient Threshold ECDSA from Encryptions Based on Class Groups."*
- **Venue / year**: ASIACRYPT 2021 (LNCS 13093).
- **ePrint**: **2022/297** — https://eprint.iacr.org/2022/297 (later full version).
- **Contribution**: removes the **non-standard low-order assumption** and the **parallel-repetition** overhead of earlier CL ZK proofs, via a "promise Σ-protocol" with weaker (but sufficient) "promise extractability" — the efficiency baseline for subsequent CL threshold ECDSA.

#### 3.D.5 Braun–Damgård–Orlandi — threshold CL encryption (CRYPTO 2023)

- **Authors / paper**: Lennart Braun, Ivan Damgård, Claudio Orlandi, *"Secure Multiparty Computation from Threshold Encryption Based on Class Groups."*
- **Venue / year**: CRYPTO 2023 (LNCS 14081).
- **ePrint**: **2022/1437** — https://eprint.iacr.org/2022/1437
- **Contribution**: first **actively-secure *threshold* CL cryptosystem** (threshold-distributed CL decryption key) + **constant-communication ZK proofs of multiplicative relations** on CL ciphertexts, enabling UC MPC with only transparent setup. Not ECDSA-specific, but it enables the "threshold-CL-encryption ECDSA" line that **avoids pairwise MtA entirely** and reaches constant communication. (Refined by "An Improved Threshold Homomorphic Cryptosystem Based on Class Groups," SCN 2024.)

---

## 4. In-Family Comparison Table

| Scheme | Threshold | Rounds (signing) | MtA method | Assumption / setup | Security model | Ident. abort | Proactive | Year / ePrint |
|---|---|---|---|---|---|---|---|---|
| MacKenzie–Reiter | 2-of-2 | multi-round | Paillier + special ZK | DCRA, ROM | sim, concurrent, static | No | No | 2001/2004 (no ePrint) |
| GGN | t-of-n (thr-opt) | ~6 | Paillier MtA + range proofs | DCRA + Strong-RSA | sim, static, dishonest-maj | No | No | 2016 / 2016/013 |
| BGG | t-of-n | ~4 | Level-1 HE | Paillier-family | sim, static | No | No | 2017 (no ePrint) |
| Lindell | 2-of-2 | ~2 | Paillier (1 homomorphic eval) | Paillier + non-std "Paillier-EC" | game-based (seq.) + sim | No | No | 2017 / 2017/552 |
| Lindell–Nof | t-of-n | multi-round | Paillier **or** OT, compute-then-check | standard (abort-on-fail) | sim, static, dishonest-maj | No | No | 2018 / 2018/987 |
| **GG18** | (t+1)-of-n | 6 phases (≈9 impl.) | Paillier MtA/MtAwc + range proofs | Paillier, DDH, Strong-RSA | sim/game, static, dishonest-maj | No | No | 2018 / **2019/114** |
| **GG20** | (t+1)-of-n | offline + **1 online** | Paillier MtA + range proofs | Paillier, DDH, Strong-RSA | sim/game, static | **Yes** | No | 2020 / **2020/540** |
| **CGGMP (CMP)** | (t+1)-of-n | 3 presign + **1 online** (or 7) | Paillier MtA + full ZK (ring-Pedersen) | Strong-RSA, DDH, Paillier | **UC, adaptive** | **Yes** | **Yes** | 2020 / **2021/060** |
| **DKLs18** | 2-of-n | multi-round | OT (Gilboa) over KOS15 | **CDH + ECDSA**, ROM | sim, static, dishonest-maj | No | No | 2018 / 2018/499 |
| **DKLs19** | t-of-n | multi-round | pairwise OT mult. | **CDH**, global ROM | sim, static, dishonest-maj | No | No | 2019 / 2019/523 |
| **DKLs23** | t-of-n | **3** (2 online + presign) | OT/VOLE mult. | **ECDSA only** (IT-UC given ideal commit + 2P-mult) | UC, dishonest-maj | (no) | (no) | 2023 / **2023/765** (S&P'24) |
| CCL+ 2P | 2-of-2 | ~2 | CL enc. (no range proofs) | HSM (class groups) | sim, static | No | No | 2019 / 2019/503 |
| **CCL+ PKC20** | t-of-n | multi-round | CL enc. (no range proofs) | HSM, transparent setup | sim, static, dishonest-maj | No | No | 2020 / **2020/084** |
| CCL+ Revisited | t-of-n | offline + online | CL enc. | HSM | sim, adaptive (n-of-n) | **Yes** | **Yes** | 2021/2023 / 2021/291 |
| Deng et al. | t-of-n | multi-round | CL enc. (promise Σ) | HSM (no low-order) | sim, static, dishonest-maj | No | No | 2021 / 2022/297 |
| Xue et al. (2P) | 2-of-n | offline + light online | 1 offline MtA | HE/OT | sim | No | No | 2021 / 2022/318 |
| 2PC-MPC | t-of-n | broadcast | threshold AHE (Tiresias) | DCR | UC | **Yes** | – | 2024 / 2024/253 |
| Katz–Urban | t-of-n (honest-maj) | non-int. online | honest-maj MPC | honest majority | sim, active, honest-maj | – | – | 2024 / 2024/2011 |
| Groth–Shoup service | t-of-n, f<n/3 async | non-int. online | (AVSS-based) | async BFT | UC, **robust (GOD)** | – | – | 2022 / 2022/506 |
| TX25 | t-of-n | **3** | MtA (EC online) | – | sim | – (robust) | – | 2025 / 2025/910 (S&P'25) |
| RompSig-Q/L | t-of-n | **3** | MtA / threshold CL | HSM | sim, **robust** | – | – | 2025 / 2025/828 |
| Trout | arbitrary t-of-n | **2** | class-group LHE | HSM, no trusted setup | UC | **Yes** | – | 2025 / 2025/1666 (CCS'25) |
| ECDSA in Two Rounds | thr-optimal | **2** | NIM (class groups) | HSM, 2 CL generators | sim | – | – | 2025 / 2025/1696 (CCS'25) |

*(Notes: "rounds" figures reflect each paper's own framing; concrete counts vary with presigning and parameterization. DKLs23 does not headline IA/proactivity but its UC functionality can be extended.)*

---

## 5. Latest Research (2023–2026)

The field's two structural takeaways for this period: **(1)** signing converged on **two rounds** in late 2025, achieved largely via **class groups / Non-Interactive Multiplication (NIM)** rather than the OT/VOLE or Paillier routes; and **(2)** the **Groth–Shoup presignature security loss** became the defining safety concern, now provably avoided for the first time.

### 5.1 The presignature-security pivot (Groth–Shoup, 2022) — the bar everyone is measured against

- **Groth & Shoup, "On the Security of ECDSA with Additive Key Derivation and Presignatures,"** EUROCRYPT 2022, **ePrint 2021/1330** — https://eprint.iacr.org/2021/1330. First rigorous (generic-group-model, with an EC-specific GGM capturing ECDSA's conversion function) analysis of BIP32 additive key derivation and of presignatures. **Key finding**: combining additive key derivation with presignatures yields a **cube-root attack** (vs. the √-attack on plain ECDSA), with security loss **growing in the number of pre-released, unused presignatures**. Mitigations: **re-randomized presignatures** and **homogeneous key derivation** (lightweight, restoring near-plain-ECDSA security). *Every subsequent presigning scheme is measured against this.*
- **Groth & Shoup, "Design and analysis of a distributed ECDSA signing service,"** **ePrint 2022/506** — https://eprint.iacr.org/2022/506. First distributed ECDSA with **guaranteed output delivery over an asynchronous network** (n parties, f < n/3 Byzantine), non-interactive online signing, BIP32 support. Deployed as chain-key ECDSA on the **Internet Computer** (holds native BTC). Introduces a simple AVSS and a multi-recipient encryption scheme.

### 5.2 SPDZ-style / generic-MPC threshold ECDSA

- **Smart & Talibi Alaoui, "Distributing any Elliptic Curve Based Protocol,"** IMACC 2019, **ePrint 2019/768** — runs full-threshold actively-secure SPDZ MPC over `F_p` and locally maps `F_p` Beaver triples into EC triples. Conceptual root of the "generic MPC gives threshold EC signatures" line.
- **Dalskov, Orlandi, Keller, Shrishak, Shulman, "Securing DNSSEC Keys via Threshold ECDSA from Generic MPC,"** ESORICS 2020 (no confirmed standalone ePrint) — shows off-the-shelf **MP-SPDZ** computes threshold ECDSA at efficiency comparable to bespoke schemes, spanning semi-honest/malicious and honest/dishonest-majority.
- **Damgård, Jakobsen, Nielsen, Pagter, Østergård, "Fast Threshold ECDSA with Honest Majority,"** SCN 2020 / J. Computer Security 2022, **ePrint 2020/501** — honest-majority (t < n/2) Shamir-based scheme using Beaver multiplication instead of heavy public-key MtA; fast but needs the stronger honest-majority trust.
- **Survey**: Aumasson, Hamelink, Shlomovits, "A Survey of ECDSA Threshold Signing," **ePrint 2020/1390** — unifies schemes under an extended arithmetic-black-box formalism.

### 5.3 Online-friendly two-party & efficient MtA (HE branch)

- **Xue, Au, Xie, Yuen, Cui, "Efficient Online-friendly Two-Party ECDSA Signature,"** ACM CCS 2021, **ePrint 2022/318** — offline/online split with a **single offline MtA** and a lightweight online phase; 2–9× better comm+compute than prior OT/HE two-party schemes. Originates the "online-friendly" terminology.
- **Xue et al., "Efficient Multiplicative-to-Additive Function from Joye–Libert Cryptosystem…,"** ACM CCS 2023, **ePrint 2023/1312** — new MtA from the **Joye–Libert** cryptosystem + JL commitments + standard-soundness ZK, attacking the range-proof bottleneck.
- **Tang, Han, Lin, Wei, Yan, "Batch Range Proof: How to Make Threshold ECDSA More Efficient,"** ACM CCS 2024, **ePrint 2024/1677** — a **Multi-Dimension Forking Lemma** yields batch range proofs/MtA improving Paillier MtA ~2× and JL-MtA ~3× (amortized), and improving CGGMP20 bandwidth ~2.1–2.4× / compute ~1.5–1.7×.

### 5.4 Massively-multiparty / emulated-2PC networks

- **Tiresias — "Large Scale, UC-Secure Threshold Paillier,"** ASIACRYPT 2024, **ePrint 2023/998** — trustless DKG + robust proofs for threshold Paillier under **DCR alone**, scaling to 1000 parties. The threshold-decryption workhorse for large networks.
- **"2PC-MPC: Emulating Two-Party ECDSA in Large-Scale MPC,"** **ePrint 2024/253** — t-of-n, UC, **publicly verifiable, identifiable abort**; emulates the "second party" of a 2-party ECDSA with a network of `n` parties. Reduces message complexity **O(n²)→O(n)** and per-party compute **O(n)→~O(1)**, needs only a **broadcast channel**. Signs in 1.23 s (256 parties) / 12.7 s (1024 parties). Powers dWallet / the Ika Network.

### 5.5 DKLs consolidation (OT/VOLE)

- **DKLs23** (§3.C.3), S&P 2024, **ePrint 2023/765** — the 3-round dishonest-majority baseline; industry-dominant.
- **Asharov, "Revisiting DKLs Threshold ECDSA…,"** **ePrint 2026/976** — fixes DKLs's OT-VOLE security-parameter issues, analyzes three VOLE variants, and gives a two-party protocol with a **~0.2 KB/party (~600×-reduced)** online phase resistant to presignature attacks. A standardization-oriented hardening paper.

### 5.6 Honest-majority non-interactive presigning

- **Katz & Urban, "Honest-Majority Threshold ECDSA with Batch Generation of Key-Independent Presignatures,"** **ePrint 2024/2011** (also IACR CiC 2:1:8) — honest-majority, actively secure, **batch key-independent presignatures** enabling truly non-interactive online signing at **~1.3 ms/presignature** — a property the authors note is unavailable in dishonest-majority protocols.

### 5.7 Robustness / class-group wave (2025–2026)

- **Wong, Ma, Yin, Chow, "Real Threshold ECDSA,"** NDSS 2023 — first threshold ECDSA **robust without full restart**, but 7 rounds. (No public ePrint; NDSS paper #2023-817 — do not conflate with ePrint 2023/817.)
- **Wong, Ma, Chow, "Secure MPC of Threshold Signatures Made More Efficient,"** NDSS 2024 — **threshold CL encryption**; first **constant outgoing communication per party**, 2-round robust DKG in dishonest majority; cost ≥4 rounds + expensive HE online phase.
- **Tang & Xue (TX25), "Robust Threshold ECDSA with Online-Friendly Design in Three Rounds,"** **ePrint 2025/910** (S&P'25) — first **3-round** scheme that is both robust and online-friendly (online phase uses only EC group ops, 2–3 orders of magnitude cheaper than LHE-based online).
- **Lyu, Li, Zhou, Xue, Wang, Wang, Liu, "Bandwidth-Efficient Robust Threshold ECDSA in Three Rounds" (RompSig-Q/L),** **ePrint 2025/828** — two 3-round robust schemes; RompSig-L uses **threshold CL encryption** for scalability + dynamic participation; ~1.0t+1.6 KiB / 3.0 KiB per signer.
- **Jiang, Tang, Xue, "Three-Round (Robust) Threshold ECDSA from Threshold CL Encryption,"** ACISP 2025, **ePrint 2026/190** — **3 rounds with constant outgoing communication** (vs. Wong's ≥4).
- **Tang, Qiu, Jiang, Xue, Hao, Yang, Deng, "ARES / ARES⁺,"** **ePrint 2026/130** — constant per-party offline sending (2.22 KB); ARES⁺ uses **packed secret sharing** for **linear amortized compute + constant online communication**.
- **Ko, Lee, Eom, Jo, "ART-ECDSA: Hardware-Friendly Robust Threshold ECDSA in an Asymmetric Model,"** **ePrint 2026/094** — asymmetric model where one party is a resource-constrained device; full robustness + cheater ID, UC; **CL encryption drops Paillier and range proofs**; ARM Cortex-M7 does ~50 ms presign, ≤10 s sign, ~300 B / 3 KB transmitted (fits BLE/NFC cold storage).

### 5.8 The two-round frontier (late 2025) — and resolving Groth–Shoup

- **Lyu, Li, Zhou, Deng, "Threshold ECDSA in Two Rounds,"** ACM CCS 2025, **ePrint 2025/1696** — first **two-round threshold-optimal** protocol (one fewer round than DKLs24). Crucially it **evades the Groth–Shoup security loss** that grows in the number of unused presignatures — the first threshold-optimal scheme to do so. Built on **Non-Interactive Multiplication (NIM)** (Boyle et al., PKC'25) with the Abram et al. (EUROCRYPT'24) class-group construction; minimal transparent setup (two class-group generators); 1.9 KiB messages at 128-bit security.
- **Dahari-Garbian, Nof, Parker, "Trout: Two-Round Threshold ECDSA from Class Groups,"** ACM CCS 2025, **ePrint 2025/1666** — first **two-round** signing for **arbitrary thresholds** with **identifiable abort and no trusted setup**; constant upload per party. Built on class-group LHE + commitments + an exponent-VRF (Boneh et al., EUROCRYPT 2025) + a novel "scaled decryption." 6.5 KB/party; Rust impl to 100 parties, **first constant-time class-group threshold-ECDSA** variant.

### 5.9 NIST standardization status (MPTC, mid-2026)

- Project: **NIST Multi-Party Threshold Cryptography (MPTC)** — https://csrc.nist.gov/projects/threshold-cryptography
- **NIST IR 8214C, "First Call for Multi-Party Threshold Schemes," finalized 2026-01-20** (after ipd Jan 2023 and 2pd Mar 2025). **Threshold ECDSA is explicitly in scope** (Sign category covers EdDSA, ECDSA, RSA). 26 preview writeups posted 2026-01-22; MPTS 2026 preview talks Jan 26–29; a Phase-2 preview of 10 additional submission plans scheduled **July 7–8, 2026**. Net status: threshold ECDSA is an active standardization target, no standard finalized yet.

---

## 6. Known Vulnerabilities & Attacks (critical)

**The recurring root cause:** almost every real-world break targets the **Paillier MtA machinery** — specifically implementations that **dropped, weakened, or mis-encoded the mandatory ZK proofs** (range proofs, Paillier biprime/no-small-factor proofs, correct Fiat-Shamir encoding, sufficient soundness iterations) for performance. The academic protocols warned that these proofs are load-bearing; implementers treated them as overhead. OT-based (DKLs) and class-group (CCL+) schemes structurally avoid this entire attack surface (no Paillier, no range proofs).

### 6.1 Alpha-Rays (ZenGo, Dec 2021) — GG18/GG20

- **Authors / source**: Dmytro Tymokhanov, Omer Shlomovits (ZenGo-X), *"Alpha-Rays: Key Extraction Attacks on Threshold ECDSA Implementations,"* **ePrint 2021/1621**; writeup https://blog.ledger.com/alpha-rays/.
- **Target**: Binance/BNB-Chain **tss-lib** (Go) GG18/GG20 + 10+ other wallets/libraries.
- **Root cause**: use of a **small Paillier encryption key** combined with a **missing range proof** (a key-size check was never specified in the protocol, so absent from most implementations). Disproved a GG-authors' conjecture that a "heavy" range proof was unnecessary at a step.
- **Impact**: **full key extraction by one malicious party in ~8 signatures.** Fireblocks reported patching within 96 hours.

### 6.2 BitForge (Fireblocks, Aug 2023) — GG18/GG20 and Lindell17

Umbrella disclosure affecting **15+ wallet providers**, two CVEs:

- **CVE-2023-33241 — GG18/GG20 Paillier key vulnerability.** Root cause: signatories' Paillier public keys are **not checked for small factors / biprimality**; an attacker submits a malformed modulus `N` during MtA, and the honest party's share leaks modulo small factors, reconstructed via CRT. **Impact/effort**: full extraction in **~16 signatures** (β~q⁵, no factor checks) up to ~200k or ~10⁹ signatures for harder variants; Apache Milagro's variant leaks straight from the transcript. **Affected**: BNB-Chain tss-lib, Safeheron `multi-party-ecdsa-cpp` (patched), ZenGo-X `multi-party-ecdsa` (unmaintained/unpatched), Apache Milagro MPC (patched), BitGoJS, and 10+ orgs. **Fix**: ZK proof at key-gen that `N` is a well-formed biprime with no small factors. NVD: https://nvd.nist.gov/vuln/detail/cve-2023-33241; report: https://www.fireblocks.com/blog/gg18-and-gg20-paillier-key-vulnerability-technical-report.
- **CVE-2023-33242 — Lindell17 abort vulnerability.** Root cause: implementations **deviate from the paper's mandate to terminate on a failed signature** and keep operating. The attacker crafts a message where signature validity depends on one bit of the counterparty's secret share; each signing leaks one bit (set `k1 = 2^i`, ciphertext valid only if target bit = 0). **Impact**: full key recovery in **~200–256 attempts**. **Affected**: Coinbase WaaS SDK React Native (<1.0.0), ZenGo gotham-city and multi-party-ecdsa (<1.0.0), other retail WaaS wallets. **Fix**: halt permanently on signature failure / distinguish failure aborts / add a ZK proof on the final message. NVD: https://nvd.nist.gov/vuln/detail/CVE-2023-33242; report: https://www.fireblocks.com/blog/lindell17-abort-vulnerability-technical-report.

### 6.3 TSSHOCK (Verichains, Black Hat USA 2023) — GG18/GG20/CGGMP21

- **Source**: Verichains Research Team, Black Hat USA 2023 (Aug 10, 2023). https://verichains.io/tsshock/. Related CVE-2022-47931 (io.finnet/tss-lib fork). Credits Kudelski Security (2019) as first reporting the ambiguous-encoding issue (then rated low severity).
- **Three attacks**:
  - **α-Shuffle** — an **ambiguous Fiat-Shamir encoding** (values concatenated with a `$` delimiter) lets different tuples hash identically; chained α-values are re-interpreted after the challenge is revealed, breaking soundness. Affected: BNB-Chain tss-lib, **Taurus multi-party-sig (a CGGMP-21 implementation)**, Multichain/Anyswap, THORChain, Threshold Network (tBTC), Swingby.
  - **c-split** — an optimization treating the composite order φ(N) as prime; when the challenge is divisible by a factor, the discrete log becomes computable; combined with lattice attacks across sessions. Affected: Axelar `tofn`, ING Bank threshold-signatures, ZenGo-X multi-party-ecdsa.
  - **c-guess** — reduced dlnproof iterations (e.g. Multichain cut iterations 128→1) make challenge-bit guessing feasible.
- **Impact**: full private-key extraction by **one** malicious party in **1–2 signatures, with no abort** (leaves no trace). **Key lesson**: even a UC-secure protocol (CGGMP21) is only as safe as its Fiat-Shamir encoding and proof iteration counts.

### 6.4 Academic cryptanalysis and surveys

- **Aumasson & Shlomovits, "Attacking Threshold Wallets,"** **ePrint 2020/1052** — early systematic look at TSS implementation flaws.
- **Makriyannis, Yomtov, Galansky, "Practical Key-Extraction Attacks in Leading MPC Wallets,"** **ePrint 2023/1234**, ACM CCS 2024 — the peer-reviewed academic version of BitForge: **four** key-extraction attacks (requiring ~10⁶, 256, 16, and even **one** signature), naming Coinbase, Binance, ZenGo, BitGo, ING Bank.
- **"Attacks on Implementations of Lindell 17 and Its Variants,"** Springer 2025 — further analysis of the Lindell17 abort class.
- *(Note: the prompt's "Makarov" appears to be a misremembering of **Makriyannis**; no distinct "Makarov" GG18 cryptanalysis was found. A far-future arXiv side-channel preprint could not be verified and is omitted.)*

### 6.5 General lessons

1. **Never optimize away the ZK proofs** (range, biprime/no-small-factor, correct encoding, full iteration counts) — they are security, not overhead.
2. **Abort strictly on failure** (Lindell17 / compute-then-check schemes) and never continue signing after an invalid signature.
3. **Verify counterparties' public parameters** (Paillier modulus well-formedness, ring-Pedersen) at setup — CGGMP's Πmod/Πprm/no-small-factor proofs are precisely this fix.
4. **Domain-separate and unambiguously encode Fiat-Shamir inputs** (the TSSHOCK α-shuffle lesson) — this bit even UC-secure CGGMP implementations.
5. **OT-based (DKLs) and class-group (CCL+) schemes avoid the Paillier attack surface entirely** — a strong argument for the newer families in adversarial deployments. Generic ECDSA nonce-bias / side-channel (Hidden Number Problem) risks still apply to the *output* regardless of the TSS protocol.

---

## 7. Production Implementations

| Implementation | Scheme | Language | Status / notes |
|---|---|---|---|
| ZenGo-X `multi-party-ecdsa` | GG18/GG20 | Rust | Hit by Alpha-Rays, CVE-2023-33241, TSSHOCK c-split. GG modules unmaintained ("won't fix"); gotham-city (Lindell17) patched (≥1.0.0). |
| Binance / BNB-Chain `tss-lib` | GG18/GG20 | Go | Most-forked TSS library; hit by all three attack waves; patched each time. |
| Coinbase `cb-mpc` (ex-kryptology) | ECDSA-2PC, ECDSA-MPC, Schnorr/BIP340 | C++17 (+Go) | Open-sourced 2025; production engine; HackerOne bounty; constant-time tests. Earlier WaaS (Lindell17) hit by CVE-2023-33242, patched. |
| Fireblocks MPC-CMP / MPC | CMP / CGGMP | Proprietary | UC-secure; markets non-exposure to the GG18/GG20 attack class; also the discloser of Alpha-Rays-adjacent work and BitForge. |
| Taurus `multi-party-sig` | CGGMP-21 | Go | Broken by TSSHOCK α-shuffle (Fiat-Shamir encoding). |
| DFNS | CGGMP21 | Rust | https://www.dfns.co/article/cggmp21-in-rust-at-last |
| Silence Laboratories `dkls23` (Silent Shard) | **DKLs23** (OT-based) | Rust | Audited by **Trail of Bits** (2024); one of the first production DKLs23 impls. No Paillier/range-proof attack class. |
| Web3Auth (tKey / MPC Core Kit) → MetaMask Embedded Wallets | **DKLS** (secp256k1), FROST (Ed25519) | TS/Rust | Avoids GG18/GG20 attack class. |
| THORChain | GG20 (tss-lib fork) | Go | TSSHOCK α-shuffle (fixed). A reported 2026 exploit could not be verified from a primary advisory. |
| Axelar `tofn` | GG20 | Rust | TSSHOCK c-split; reported not fixed at disclosure. |
| Keep / tBTC (Threshold Network) | GG18/GG20 (tss-lib fork) | Go | TSSHOCK α-shuffle (fixed). |
| Entropy `synedrion` | CGGMP21 | Rust | Reference CGGMP21 implementation. |
| Safeheron `multi-party-ecdsa-cpp` | GG18/GG20 | C++ | CVE-2023-33241 (patched). |
| Apache Milagro MPC | GG-family | C | CVE-2023-33241 transcript-recoverable variant (patched). |
| BitGo (BitGoJS TSS) | GG18/GG20 → newer | TypeScript | CVE-2023-33241 (tracked). |
| Internet Computer (chain-key ECDSA) | Groth–Shoup async service | Rust | Robust/GOD async; holds native BTC. |
| dWallet / Ika Network | 2PC-MPC | Rust | Large-scale emulated-2PC. |
| Turnkey | **Not threshold ECDSA** — TEE/secure-enclave based | — | Commonly mis-grouped with MPC; architecture is enclave-based per its own docs. |

*(Sepior/Blockdaemon (DKLs lineage, acquired 2022) and Lit Protocol scheme/audit specifics could not be fully verified from primary sources.)*

---

## 8. Open Problems

1. **Fewer rounds with dishonest-majority + robustness simultaneously.** Two-round signing now exists (Trout, ECDSA-in-Two-Rounds, both CCS'25) but via class groups; combining minimal rounds, dishonest majority, robustness (GOD), identifiable abort, and adaptive security in *one* practical scheme remains open.
2. **Presignature security formalization and deployment discipline.** Groth–Shoup (2021/1330) showed the danger; 2025/1696 is the first threshold-optimal scheme provably avoiding the unused-presignature loss. Making re-randomized/homogeneous presigning the default across implementations (especially with BIP32) is unfinished.
3. **Standardizing and hardening OT/VOLE parameters.** Asharov (2026/976) showed DKLs's VOLE parameters didn't meet their target security level — a reminder that concrete parameterization of OT-extension-based MtA needs careful, standardized treatment (feeding NIST MPTC).
4. **Adaptive security without heavy machinery.** Most efficient schemes prove only static security; CGGMP and CCL+-Revisited achieve adaptivity but at cost. Cheap adaptive (and proactive) security for the low-round class-group/OT schemes is open.
5. **Bandwidth vs. compute frontier.** OT (fast/heavy-bandwidth), Paillier (compact/heavy-compute + fragile proofs), class groups (compact/slow arithmetic): whether a single scheme can be simultaneously bandwidth- and compute-optimal without exotic setup is unresolved; packed secret sharing (ARES⁺) and NIM are recent partial answers.
6. **Constant-time / side-channel-hardened implementations.** Trout provided the first constant-time class-group threshold ECDSA; constant-time, formally-verified implementations across all families remain scarce.
7. **Verified, ceremony-free trustless setup.** Class-group schemes offer transparent setup but rely on class-group assumptions still less battle-tested than factoring/DL; broader cryptanalytic confidence in class-group hardness is an ongoing need.
8. **Eliminating the implementation-vs-paper gap.** The entire vulnerability history is implementations diverging from proven protocols. Machine-checkable protocol specifications and reference test vectors (a NIST MPTC goal) are still immature.

---

## 9. References

**Verified ePrint identifiers unless otherwise noted.**

### Early / two-party
- MacKenzie, Reiter. *Two-Party Generation of DSA Signatures.* CRYPTO 2001 / IJIS 2004. No ePrint. https://users.ece.cmu.edu/~reiter/papers/2004/IJIS.pdf
- Gennaro, Goldfeder, Narayanan. *Threshold-Optimal DSA/ECDSA… Bitcoin Wallet Security.* ACNS 2016. ePrint 2016/013. https://eprint.iacr.org/2016/013
- Boneh, Gennaro, Goldfeder. *Using Level-1 HE to Improve Threshold DSA… Bitcoin Wallet Security.* LATINCRYPT 2017. No verified ePrint. DOI 10.1007/978-3-030-25283-0_19
- Lindell. *Fast Secure Two-Party ECDSA Signing.* CRYPTO 2017 / J. Cryptology 2021. ePrint 2017/552. https://eprint.iacr.org/2017/552
- Lindell, Nof. *Fast Secure Multiparty ECDSA with Practical DKG…* CCS 2018. ePrint 2018/987. https://eprint.iacr.org/2018/987

### Paillier-based multiparty
- Gennaro, Goldfeder. *Fast Multiparty Threshold ECDSA with Fast Trustless Setup (GG18).* CCS 2018. ePrint 2019/114. https://eprint.iacr.org/2019/114
- Gennaro, Goldfeder. *One Round Threshold ECDSA with Identifiable Abort (GG20).* 2020. ePrint 2020/540. https://eprint.iacr.org/2020/540
- Canetti, Gennaro, Goldfeder, Makriyannis, Peled. *UC Non-Interactive, Proactive, Threshold ECDSA with Identifiable Aborts (CMP/CGGMP).* CCS 2020. ePrint 2021/060. https://eprint.iacr.org/2021/060

### OT-based (DKLs)
- Doerner, Kondi, Lee, shelat. *Secure Two-party Threshold ECDSA from ECDSA Assumptions (DKLs18).* S&P 2018. ePrint 2018/499. https://eprint.iacr.org/2018/499
- Doerner, Kondi, Lee, shelat. *Threshold ECDSA from ECDSA Assumptions: The Multiparty Case (DKLs19).* S&P 2019. ePrint 2019/523. https://eprint.iacr.org/2019/523
- Doerner, Kondi, Lee, shelat. *Threshold ECDSA in Three Rounds (DKLs23).* S&P 2024. ePrint 2023/765. https://eprint.iacr.org/2023/765
- Keller, Orsini, Scholl. *Actively Secure OT Extension with Optimal Overhead (KOS15).* CRYPTO 2015. ePrint 2015/546. https://eprint.iacr.org/2015/546
- Roy. *SoftSpokenOT.* CRYPTO 2022. ePrint 2022/192. https://eprint.iacr.org/2022/192
- Boyle, Couteau, Gilboa, Ishai, Kohl, Resch, Scholl. *OT with Constant Computational Overhead.* EUROCRYPT 2024. ePrint 2023/817. https://eprint.iacr.org/2023/817
- Asharov. *Revisiting DKLs Threshold ECDSA: Enhanced OT-based VOLE and Two-Party Signing.* ePrint 2026/976. https://eprint.iacr.org/2026/976

### Class-group / CL
- Castagnos, Laguillaumie. *Linearly Homomorphic Encryption from DDH (CL framework).* CT-RSA 2015. ePrint 2015/047. https://eprint.iacr.org/2015/047
- Castagnos, Catalano, Laguillaumie, Savasta, Tucker. *Two-Party ECDSA from Hash Proof Systems…* CRYPTO 2019. ePrint 2019/503. https://eprint.iacr.org/2019/503
- Castagnos, Catalano, Laguillaumie, Savasta, Tucker. *Bandwidth-Efficient Threshold EC-DSA.* PKC 2020. ePrint 2020/084. https://eprint.iacr.org/2020/084
- Castagnos, Catalano, Laguillaumie, Savasta, Tucker. *Bandwidth-efficient threshold EC-DSA revisited…* TCS 2023. ePrint 2021/291. https://eprint.iacr.org/2021/291
- Deng, Ma, Zhang, Wang, Song, Xie. *Promise Σ-protocol: Efficient Threshold ECDSA from Class Groups.* ASIACRYPT 2021. ePrint 2022/297. https://eprint.iacr.org/2022/297
- Braun, Damgård, Orlandi. *MPC from Threshold Encryption Based on Class Groups.* CRYPTO 2023. ePrint 2022/1437. https://eprint.iacr.org/2022/1437

### Presignatures, SPDZ, latest (2023–2026)
- Groth, Shoup. *On the Security of ECDSA with Additive Key Derivation and Presignatures.* EUROCRYPT 2022. ePrint 2021/1330. https://eprint.iacr.org/2021/1330
- Groth, Shoup. *Design and analysis of a distributed ECDSA signing service.* ePrint 2022/506. https://eprint.iacr.org/2022/506
- Smart, Talibi Alaoui. *Distributing any Elliptic Curve Based Protocol.* IMACC 2019. ePrint 2019/768. https://eprint.iacr.org/2019/768
- Dalskov, Orlandi, Keller, Shrishak, Shulman. *Securing DNSSEC Keys via Threshold ECDSA from Generic MPC.* ESORICS 2020. (no confirmed ePrint) DOI 10.1007/978-3-030-59013-0_32
- Damgård, Jakobsen, Nielsen, Pagter, Østergård. *Fast Threshold ECDSA with Honest Majority.* SCN 2020. ePrint 2020/501. https://eprint.iacr.org/2020/501
- Aumasson, Hamelink, Shlomovits. *A Survey of ECDSA Threshold Signing.* ePrint 2020/1390. https://eprint.iacr.org/2020/1390
- Xue, Au, Xie, Yuen, Cui. *Efficient Online-friendly Two-Party ECDSA Signature.* CCS 2021. ePrint 2022/318. https://eprint.iacr.org/2022/318
- Xue et al. *Efficient MtA from Joye–Libert… Threshold ECDSA.* CCS 2023. ePrint 2023/1312. https://eprint.iacr.org/2023/1312
- Tang, Han, Lin, Wei, Yan. *Batch Range Proof: Make Threshold ECDSA More Efficient.* CCS 2024. ePrint 2024/1677. https://eprint.iacr.org/2024/1677
- Friedman et al. *Tiresias: Large Scale, UC-Secure Threshold Paillier.* ASIACRYPT 2024. ePrint 2023/998. https://eprint.iacr.org/2023/998
- Friedman, Marmor, Mutzari, Sadika, Scaly, Spiizer, Yanai. *2PC-MPC: Emulating Two-Party ECDSA in Large-Scale MPC.* ePrint 2024/253. https://eprint.iacr.org/2024/253
- Katz, Urban. *Honest-Majority Threshold ECDSA with Batch Generation of Key-Independent Presignatures.* IACR CiC 2025. ePrint 2024/2011. https://eprint.iacr.org/2024/2011
- Tang, Xue. *Robust Threshold ECDSA with Online-Friendly Design in Three Rounds (TX25).* S&P 2025. ePrint 2025/910. https://eprint.iacr.org/2025/910
- Lyu, Li, Zhou, Xue, Wang, Wang, Liu. *Bandwidth-Efficient Robust Threshold ECDSA in Three Rounds (RompSig).* ePrint 2025/828. https://eprint.iacr.org/2025/828
- Jiang, Tang, Xue. *Three-Round (Robust) Threshold ECDSA from Threshold CL Encryption.* ACISP 2025. ePrint 2026/190. https://eprint.iacr.org/2026/190
- Tang, Qiu, Jiang, Xue, Hao, Yang, Deng. *ARES / ARES⁺: Online-Friendly Robust Threshold ECDSA with Amortized Costs.* ePrint 2026/130. https://eprint.iacr.org/2026/130
- Ko, Lee, Eom, Jo. *ART-ECDSA: Hardware-Friendly Robust Threshold ECDSA in an Asymmetric Model.* ePrint 2026/094. https://eprint.iacr.org/2026/094
- Lyu, Li, Zhou, Deng. *Threshold ECDSA in Two Rounds.* CCS 2025. ePrint 2025/1696. https://eprint.iacr.org/2025/1696
- Dahari-Garbian, Nof, Parker. *Trout: Two-Round Threshold ECDSA from Class Groups.* CCS 2025. ePrint 2025/1666. https://eprint.iacr.org/2025/1666
- Wong, Ma, Yin, Chow. *Real Threshold ECDSA.* NDSS 2023. (NDSS #2023-817; no ePrint)
- Wong, Ma, Chow. *Secure MPC of Threshold Signatures Made More Efficient.* NDSS 2024. (NDSS #2024-601; no ePrint)

### Attacks / vulnerabilities
- Tymokhanov, Shlomovits. *Alpha-Rays: Key Extraction Attacks on Threshold ECDSA Implementations.* ePrint 2021/1621. https://eprint.iacr.org/2021/1621 ; https://blog.ledger.com/alpha-rays/
- Makriyannis, Yomtov, Galansky. *Practical Key-Extraction Attacks in Leading MPC Wallets.* CCS 2024. ePrint 2023/1234. https://eprint.iacr.org/2023/1234
- Aumasson, Shlomovits. *Attacking Threshold Wallets.* ePrint 2020/1052. https://eprint.iacr.org/2020/1052
- Verichains. *TSSHOCK.* Black Hat USA 2023. https://verichains.io/tsshock/
- Fireblocks. *BitForge.* 2023. CVE-2023-33241 (GG18/GG20): https://www.fireblocks.com/blog/gg18-and-gg20-paillier-key-vulnerability-technical-report ; CVE-2023-33242 (Lindell17): https://www.fireblocks.com/blog/lindell17-abort-vulnerability-technical-report
- NVD: https://nvd.nist.gov/vuln/detail/cve-2023-33241 ; https://nvd.nist.gov/vuln/detail/CVE-2023-33242
- Makriyannis, Peled. *A Note on the Security of GG18.* Fireblocks whitepaper.

### Standardization
- NIST Multi-Party Threshold Cryptography (MPTC): https://csrc.nist.gov/projects/threshold-cryptography
- NIST IR 8214C, *First Call for Multi-Party Threshold Schemes* (finalized 2026-01-20).

---

*Uncertainty flags: MacKenzie–Reiter and Boneh–Gennaro–Goldfeder have no verified public ePrint (cite via journal/DOI). Dalskov et al. (ESORICS 2020) and the two Wong et al. NDSS papers have no located standalone ePrint. The exact author byline of ePrint 2023/765 is cited here as Doerner–Kondi–Lee–shelat (per the ePrint listing). Beware the cross-venue collision between ePrint 2023/817 (OT with constant overhead) and NDSS paper #2023-817 (Wong et al. Real Threshold ECDSA).*
