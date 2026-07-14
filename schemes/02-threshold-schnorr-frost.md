# Threshold Schnorr Signatures for Bitcoin (FROST and the Broader Family)

> Scope: threshold Schnorr signature schemes that produce a single, standard
> **BIP340** Schnorr signature on **secp256k1**, usable directly in Taproot
> (BIP340/341/342). A *t*-of-*n* set of signers jointly output one 64-byte
> Schnorr signature that is **indistinguishable from an ordinary single-signer
> signature** and carries **zero on-chain overhead** beyond a normal key-path
> spend.
>
> Last updated: 2026-07. Research surveyed through ePrint 2026/431.

---

## 1. Family overview

### 1.1 Why Schnorr threshold-izes so cleanly

The Schnorr signature is *linear* in the secret key and the nonce, which is the
single fact that makes distributed signing tractable:

- **Signature form (BIP340).** For key pair `(d, P = d·G)`, message `m`, nonce
  `k` with `R = k·G`, the signature is `(R, s)` where
  `s = k + e·d  (mod n)`, `e = H(R ∥ P ∥ m)`. Verification checks
  `s·G = R + e·P`.
- **Linearity → additive sharing.** If the secret is additively shared,
  `d = Σ dᵢ`, and the nonce is additively shared, `k = Σ kᵢ`, then
  `s = Σ (kᵢ + e·dᵢ) = Σ sᵢ`. Each party can compute a *partial signature* `sᵢ`
  from only its own shares, and the coordinator simply **sums** them. No
  interaction is needed to combine — this is the property BLS/Schnorr share and
  ECDSA lacks (ECDSA needs a multiplicative inverse of a shared nonce, which is
  why threshold ECDSA — GG18/GG20, DKLs, Lindell17 — is far heavier).
- **Shamir + Lagrange.** For genuine *t*-of-*n* (not just *n*-of-*n*), the key
  is Shamir-shared over a degree-`t−1` polynomial. Any signing set `S` of size
  `≥ t` reconstructs the secret *in the exponent* using **Lagrange
  coefficients** `λᵢ,S`: `d = Σ_{i∈S} λᵢ,S · dᵢ`. Each signer scales its share
  by its own `λᵢ,S` locally, so the combined signature verifies under the fixed
  group key `P` regardless of which quorum signs.

### 1.2 The central danger: nonce generation and the ROS/Drijvers attack

The hard part of threshold (and multi-)Schnorr is **distributed nonce
generation** under **concurrency**. Naive two-round schemes let each signer
publish `Rᵢ = kᵢ·G`, set `R = Σ Rᵢ`, and sign `sᵢ = kᵢ + e·λᵢ dᵢ`. This is
broken in the concurrent setting:

- **Drijvers et al. (S&P 2019)** showed a sub-exponential *k*-sum / **ROS**
  (Random inhomogeneities in an Overdetermined Solvable system) attack: an
  adversary opens many parallel sessions, waits for honest nonce commitments,
  and using Wagner's generalized birthday algorithm chooses its own
  contributions so that a linear combination of session challenges forges a
  signature on a fresh message. For a 256-bit group, ~128 open sessions bring
  the cost to ≈ 2³⁹.
- **Benhamouda, Lepoint, Loss, Orrù, Raykova — "On the (in)security of ROS"
  (EUROCRYPT 2021, ePrint 2020/945)** turned this into a **polynomial-time**
  attack once the adversary can open `> log₂ p` concurrent sessions. This
  killed naive multi-round Schnorr, CoSi-style, and the original Gennaro et al.
  concurrent constructions, and forced *every* modern scheme to **bind** the
  nonces.

Two defenses dominate:

1. **Commit-then-reveal (three-round).** Round 1 broadcasts a hash commitment
   to `Rᵢ`; only after all commitments are fixed are the `Rᵢ` opened. The
   adversary cannot adaptively choose its nonce as a function of others' — this
   is the Stinson–Strobl / Lindell'22 / Sparkle approach.
2. **Binding factors (two-round, FROST).** Each signer pre-commits *two* nonces
   `(Dᵢ = dᵢ·G, Eᵢ = eᵢ·G)` and the effective nonce is
   `Rᵢ = Dᵢ + ρᵢ·Eᵢ` where the **binding factor**
   `ρᵢ = H(i, m, B)` is a hash of the signer index, message, and the *entire*
   commitment list `B`. Because `ρᵢ` depends on the whole set of commitments and
   the message, the linear system the ROS attacker needs to solve becomes
   non-linear/binding, defeating Wagner's algorithm. This is FROST's key
   innovation and lets it be **two-round**.

### 1.3 Round taxonomy

- **Three-round (commit / reveal / respond):** Stinson–Strobl, Lindell'22,
  Sparkle. Conceptually simple, provably concurrently secure, but three network
  round-trips per signature and no offline preprocessing.
- **Two-round / "semi-interactive" (FROST family):** one *message-independent*
  preprocessing round (nonce commitments) + one *online* signing round. The
  preprocessing round can be **batched/precomputed** offline, so the online
  latency is effectively one round. This is the dominant design for Bitcoin.
- **One-round online / non-interactive online:** with precomputed nonce
  commitments distributed ahead of time, the online phase is a single
  non-interactive message from each signer (FROST used this way; SPRINT pushes
  batching to extremes).

### 1.4 Robustness vs. identifiable abort

- **Identifiable abort:** if a signer sends a malformed share, the others can
  *identify* the culprit (FROST supports this — each `sᵢ` is individually
  checkable against the signer's public share). But a single faulty signer
  still aborts the session.
- **Robustness (guaranteed output delivery):** honest quorum *always* produces a
  signature despite malicious participants. FROST alone is **not** robust; a
  disruptor can force restart. **ROAST** wraps FROST to add robustness;
  **SPRINT/HARTS/Glacius** build robustness in.

### 1.5 Security-model landscape

- **Game-based (EUF-CMA-style)** vs. **UC (universally composable / simulation
  with ideal functionality).** Lindell'22 is fully simulatable/UC-flavoured;
  FROST's canonical proofs are game-based.
- **Static vs. adaptive corruption.** *Static*: adversary fixes the corrupted
  set before key generation. *Adaptive*: adversary corrupts parties on the fly,
  seeing state, during signing. Adaptive is strictly stronger and much harder;
  achieving it without the Algebraic Group Model (AGM) or exponential tightness
  loss was open until 2023–2025 (Sparkle+, Twinkle, Glacius, Dazzle, Sparkle
  revisited).
- **TS-UF-0…4 hierarchy (Bellare–Crites–Komlo–Maller–Tessaro–Zhu, CRYPTO
  2022).** A unified syntax and *increasing* unforgeability levels for
  non-interactive threshold signatures, capturing how much power the adversary
  has over *which* signing sessions/partial signatures count toward the forgery
  bound. Roughly: **TS-UF-0** (weakest — adversary must "use" complete honest
  sessions) up through **TS-UF-1** (the practically meaningful level: forgery on
  a message for which fewer than the required honest partial signatures were
  requested) to stronger TS-SUF-2/3/4 variants distinguishing strong
  unforgeability and finer accounting of leaked partial signatures. The paper
  proved **FROST1 and FROST2 reach TS-UF-1** — *stronger* than originally
  advertised.

---

## 2. Bitcoin / Taproot applicability and BIP340 specifics

A BIP340 Schnorr signature is `(R, s)` serialized as 64 bytes: `bytes(R.x) ∥
bytes(s)` — **R is x-only** (32 bytes) and **s is 32 bytes**. Public keys are
**x-only** (32 bytes). This creates three compatibility wrinkles a threshold
scheme must handle, none of which are addressed by the generic RFC 9591:

1. **Even-Y / x-only public key.** BIP340 fixes the convention that the point
   with the x-coordinate has **even Y**. A threshold group key `P = Σ λᵢ dᵢ·G`
   may land on an odd-Y point. The protocol must **conditionally negate** the
   effective secret (all signers flip the sign of their share contributions)
   so the signature verifies under the even-Y x-only key. BIP 445 tracks this
   with an accumulated sign-flip parameter (`gacc`).

2. **Nonce parity.** Likewise the aggregate nonce `R` must have even Y; if
   `R = Σ Rᵢ` is odd-Y, signers negate their nonce contributions (`k → n − k`)
   before computing `sᵢ`. All signers must agree on the parity flip
   deterministically from the aggregate commitment.

3. **Tagged hashes and key-prefixing.** The challenge is
   `e = H_tagged("BIP0340/challenge", R.x ∥ P.x ∥ m)` — the tag-prefixed SHA-256
   of BIP340, and it is **key-prefixed** (includes `P.x`). A BIP340-compatible
   threshold scheme must use exactly this challenge, unlike RFC 9591's
   ciphersuite hash. RFC 9591's `FROST(secp256k1, SHA-256)` therefore produces
   signatures that are **not** BIP340-verifiable out of the box.

4. **Taproot tweaking (BIP341).** Real spends require **x-only tweaking**:
   `Q = P + t·G` where `t = H_tagged("TapTweak", P.x ∥ merkle_root)`, so the
   output key commits to a script tree. The threshold protocol must apply the
   tweak inside signing (adjusting the aggregate key and the sign-flip
   bookkeeping) so the produced signature verifies under the tweaked output key.
   BIP32/BIP341 tweaks are supported by BIP 445's `ApplyTweak`.

**Payoff.** Because the output is a single ordinary Schnorr signature, an *n*-of-
*n* or *t*-of-*n* Taproot key-path spend is **byte-for-byte indistinguishable**
from a single-sig spend: no `OP_CHECKMULTISIG`, no script revealing the policy,
minimal fees, maximal privacy. This is the decisive advantage over Bitcoin's
legacy on-chain multisig and even over `OP_CHECKSIGADD` (Tapscript *k*-of-*n*),
which reveals the multisig on spend.

---

## 3. Per-scheme deep dive

### 3.1 Schnorr's signature (foundation)

- **Author / venue:** Claus-Peter Schnorr, "Efficient Signature Generation by
  Smart Cards," *J. Cryptology* 1991 (CRYPTO'89). The linear structure exploited
  by everything below. Standardized for Bitcoin as **BIP340** (Wuille, Nick,
  Ruffing, 2020).

### 3.2 Gennaro–Jarecki–Krawczyk–Rabin (GJKR) — DKG foundations

- **Paper:** "Secure Distributed Key Generation for Discrete-Log Based
  Cryptosystems." EUROCRYPT 1999; journal version *J. Cryptology* 20(1), 2007.
  <https://link.springer.com/article/10.1007/s00145-006-0347-3>
- **Contribution:** Identified a *bias flaw* in **Pedersen's DKG** (a rushing
  adversary can skew the distribution of the public key) and fixed it with a
  commitment/complaint round yielding a **uniformly distributed** discrete-log
  key. Also the robust threshold DSS line (companion work) that establishes the
  Shamir + Feldman/Pedersen VSS + Lagrange-in-the-exponent template every
  threshold Schnorr scheme reuses.
- **Relevance:** Not a Schnorr signing scheme per se, but the DKG bedrock.
  Modern schemes often accept the biased **PedPoP/SimplPedPop** DKG because the
  bias is provably harmless for Schnorr unforgeability (see Olaf, §3.9).

### 3.3 Stinson–Strobl (2001) — first provably secure distributed Schnorr

- **Paper:** D. Stinson, R. Strobl, "Provably Secure Distributed Schnorr
  Signatures and a (t,n) Threshold Scheme for Implicit Certificates," **ACISP
  2001**, LNCS 2119. DOI 10.1007/3-540-47719-5_33.
- **Construction:** Robust *t*-of-*n* Schnorr via GJKR-style DKG for the key,
  and a **fresh distributed key generation of the nonce** `k` for every
  signature (i.e., a full DKG per signature — three broadcast rounds with
  Pedersen VSS), then Lagrange-combined partial signatures.
- **Rounds:** ~3+ (a joint-nonce DKG each time), robust.
- **Security:** EUF-CMA in the ROM, as secure as single Schnorr — but only in
  the **sequential/synchronous** model. **Not concurrently secure** (vulnerable
  to the later ROS/Drijvers attacks if run in parallel).
- **DKG:** requires a robust GJKR DKG; heavy communication.

### 3.4 Gennaro et al. concurrent attempts (context / cautionary)

Early attempts to make threshold/multi-Schnorr concurrent by re-using
preprocessed nonces without binding were later shown insecure by the ROS attack.
This is precisely the gap FROST's binding factors close.

### 3.5 Lindell (2022) — three-round, fully simulatable, dishonest majority

- **Paper:** Yehuda Lindell, "Simple Three-Round Multiparty Schnorr Signing with
  Full Simulatability." ePrint **2022/374**; published in *Communications in
  Cryptology* (CiC) 1(1), 2024. <https://eprint.iacr.org/2022/374>
- **Construction:** Commit–reveal–respond. Round 1: each party commits (via a
  UC commitment) to `Rᵢ` and proves knowledge (ZK) of `kᵢ`; Round 2: open;
  Round 3: partial signatures. Simulator can extract and equivocate → **full
  simulatability**.
- **Threshold:** designed for the *n*-of-*n* / **dishonest-majority** multiparty
  setting (any number corrupted), extensible to *t*-of-*n* with Shamir.
- **Rounds:** 3.
- **Security model:** **UC-style full simulatability**, secure under
  **concurrent composition**; standard model or ROM depending on commitment/ZK
  instantiation. **Static** corruption.
- **Cost:** heavier per-round (commitments + ZK proofs) than FROST, but the
  gold standard for provable composability. Used as a robust, conservative
  option in some MPC-wallet stacks.
- **Note:** subsequently shown (Meier 2025, §3.16) *not* provably **fully
  adaptively** secure without modification — like FROST and Sparkle.

### 3.6 FROST (Komlo–Goldberg, SAC 2020) — the workhorse

- **Paper:** Chelsea Komlo, Ian Goldberg, "FROST: Flexible Round-Optimized
  Schnorr Threshold Signatures." **SAC 2020**; ePrint **2020/852**.
  <https://eprint.iacr.org/2020/852>
- **Construction:** Two-nonce binding design (§1.2). Preprocessing: each signer
  commits `(Dᵢ, Eᵢ)`. Signing: coordinator sends message + commitment list `B`;
  each signer computes binding factor `ρᵢ = H(i, m, B)`, effective nonce
  `Rᵢ = Dᵢ + ρᵢ Eᵢ`, group nonce `R = Σ Rᵢ`, challenge `e = H(R,P,m)`, and
  partial signature `sᵢ = dᵢ_nonce + ρᵢ eᵢ_nonce + λᵢ·e·dᵢ`. Coordinator sums:
  `s = Σ sᵢ`.
- **Threshold:** *t*-of-*n*, Shamir-shared key + Lagrange at signing.
- **Rounds:** **2** (one preprocessable). "Round-optimized": online phase is
  effectively one round; FROST can even precompute batches of commitments.
- **DKG:** Pedersen DKG with proof-of-possession ("**PedPoP**"), or trusted
  dealer, or ChillDKG (§5).
- **Security:** original proof EUF-CMA in ROM under DL, **static**, concurrent —
  but under a somewhat idealized argument. The binding factor defeats
  ROS/Drijvers. **Not robust** (one disruptor aborts), but supports
  **identifiable abort**. Note: original FROST used a slightly different
  aggregation ("FROST1"); see FROST2.
- **Trade-off vs. three-round:** trades a round for a stronger reliance on the
  binding-factor heuristic; enormously popular because of the offline
  preprocessing.

### 3.7 FROST2 / FROST-Interpolate and the security-proof line

- **"How to Prove Schnorr Assuming Schnorr" — Crites, Komlo, Maller.** ePrint
  **2021/1375**. <https://eprint.iacr.org/2021/1375>. Introduces the
  optimization **FROST2**, which folds the Lagrange coefficients so signing
  needs a *single* scalar multiplication instead of one per signer (FROST1 was
  linear in `t`). Proves FROST2+PedPoP is **TS-UF-0** under a Schnorr-KoE
  (knowledge-of-exponent) assumption + OMDL in the ROM.
- **"Better than Advertised Security for Non-Interactive Threshold Signatures"
  — Bellare, Crites, Komlo, Maller, Tessaro, Zhu. CRYPTO 2022**, ePrint under
  IACR cryptodb pubkey 32268.
  <https://link.springer.com/chapter/10.1007/978-3-031-15985-5_18>. Defines the
  **TS-UF-0…4 hierarchy** and proves **FROST1 → TS-UF-1** and **FROST2 →
  TS-UF-1** (and BLS results). This is the reference for FROST's *actual*
  strength: forgery-hard even when the adversary partially participates in
  sessions.
- **Naming caution:** "FROST-Interpolate" and FROST2/FROST3 nomenclature vary
  across papers; FROST3 (used by Olaf/BIP 445) is a further-optimized variant
  aggregating nonces before the binding hash.

### 3.8 ROAST (CCS 2022) — robustness wrapper for Bitcoin

- **Paper:** Tim Ruffing, Viktoria Ronge, Elliott Jin, Jonas Schneider-Bensch,
  Dominique Schröder, "ROAST: Robust Asynchronous Schnorr Threshold Signatures."
  **ACM CCS 2022**; ePrint **2022/550**. <https://eprint.iacr.org/2022/550>.
  (Blockstream / FAU / Monash.)
- **Contribution:** A **wrapper**, not a new signing core. Given any
  **semi-interactive** (1 preprocessing + 1 signing round) threshold scheme with
  **identifiable abort** and **concurrent unforgeability** — i.e. **FROST** — it
  produces a scheme with **robust, asynchronous** signing: `t` honest signers
  *always* obtain a valid signature even with malicious disruptors and no
  synchrony assumption. It does so by having a coordinator launch enough
  overlapping signing sessions and pruning identified misbehavers.
- **Relevance:** The practical answer to "FROST isn't robust." Directly targeted
  at Bitcoin custody / federations. Presented at Bitcoin TABConf 2022.

### 3.9 Olaf / FROST3 without the AGM (CRYPTO 2023)

- **Paper:** Hien Chu, Paul Gerhart, Tim Ruffing, Dominique Schröder,
  "Practical Schnorr Threshold Signatures Without the Algebraic Group Model."
  **CRYPTO 2023**; ePrint **2023/899**. <https://eprint.iacr.org/2023/899>.
- **Contribution:** **Olaf** = **FROST3** (most efficient FROST variant) +
  **PedPoP DKG**, proven **unforgeable together** (signing *and* DKG in one
  proof) **without the AGM** — only **AOMDL** (algebraic one-more DL, a
  falsifiable weakening of OMDL) + ROM. Crucially it tolerates the **biased**
  Pedersen-DKG key, closing the gap between what's proven and what's deployed.
- **Rounds/DKG/security:** 2-round signing; PedPoP DKG; static, concurrent;
  strongest "real deployment" proof for the FROST family. This proof underpins
  the confidence behind ChillDKG and BIP 445.

### 3.10 The many faces of Schnorr (2023/2024) — modular toolkit

- **Paper:** "The many faces of Schnorr: a toolkit for the modular design of
  threshold Schnorr signatures." ePrint **2023/1019**; *Communications in
  Cryptology* 2024. <https://eprint.iacr.org/2023/1019>.
- **Contribution:** Abstracts FROST-style techniques into composable building
  blocks and reduces threshold security to "enhanced" (single-signer) Schnorr
  attack modes, letting designers plug in DKG/VSS subprotocols. A unifying lens
  over FROST1/2/3, Sparkle, etc.

### 3.11 Sparkle (CRYPTO 2023) — first adaptive pairing-free threshold Schnorr

- **Paper:** Elizabeth Crites, Chelsea Komlo, Mary Maller, "Fully Adaptive
  Schnorr Threshold Signatures." **CRYPTO 2023**; ePrint **2023/445**.
  <https://eprint.iacr.org/2023/445>.
- **Construction:** A **three-round** commit–reveal–respond threshold Schnorr
  (no binding-factor trick; the commitment round provides the binding). Clean,
  minimal.
- **Security (the headline):**
  - **Sparkle** is **statically** secure under plain **DL + ROM** (minimal
    assumptions).
  - **Sparkle+** claimed **full adaptive** security (up to `t` adaptive
    corruptions) under **AOMDL in the AGM + ROM** — the *first* threshold
    Schnorr adaptive-security result without exponential tightness loss.
  - **Without the AGM**, Sparkle's adaptive proof only tolerates **t/2**
    corruptions — a real limitation that motivated Twinkle/Glacius/Dazzle.
- **Rounds:** 3. **DKG:** trusted dealer or standard DKG. **Robustness:** no.
- **Significance:** reopened the adaptive-security research program for
  pairing-free threshold Schnorr. (See §3.16 and §3.18 for the subsequent
  correction and repair of the adaptive claim.)

### 3.12 SPRINT (EUROCRYPT 2024) — high-throughput robust batch signing

- **Paper:** Fabrice Benhamouda, Shai Halevi, Hugo Krawczyk, Yiping Ma, Tal
  Rabin, "SPRINT: High-Throughput Robust Distributed Schnorr Signatures."
  **EUROCRYPT 2024**; ePrint **2023/427**. <https://eprint.iacr.org/2023/427>.
- **Construction:** Single message-independent randomness-generation (DKG-like)
  step feeds a **non-interactive multi-message** signing phase; uses packed
  secret sharing / super-invertible matrices to amortize. **Guaranteed output
  delivery** (robust).
- **Threshold/scale:** designed for **large committees** (hundreds of parties),
  thousands of signatures/minute; honest-majority style thresholds. Aimed at
  proof-of-stake / committee settings more than 2-of-3 Bitcoin custody, but
  BIP340-instantiable.
- **Security:** static; robust; concurrent.

### 3.13 Twinkle (EUROCRYPT 2024) — adaptive from DDH, no AGM

- **Paper:** Renas Bacho, Julian Loss, Stefano Tessaro, Benedikt Wagner,
  Chenzhi Zhu, "Twinkle: Threshold Signatures from DDH with Full Adaptive
  Security." **EUROCRYPT 2024**; ePrint **2023/1482**.
  <https://eprint.iacr.org/2023/1482>.
- **Contribution:** First pairing-free threshold scheme with **full adaptive
  security (up to t corruptions)** from a **well-studied non-interactive
  assumption (DDH)** **without the AGM** — removing Sparkle's t/2 restriction.
  Built from a generic linear-function framework whose one-more assumption
  reduces to DDH.
- **Trade-off:** produces a slightly different (still Schnorr-type) signature;
  larger than a bare BIP340 signature in its base form, so less "drop-in" for
  Taproot than FROST, but a landmark for the security theory.

### 3.14 HARTS (ASIACRYPT 2024) — high-threshold, adaptive, robust

- **Paper:** Renas Bacho, Julian Loss, Gilad Stern, Benedikt Wagner, "HARTS:
  High-Threshold, Adaptively Secure, and Robust Threshold Schnorr Signatures."
  **ASIACRYPT 2024**; ePrint **2024/280**. <https://eprint.iacr.org/2024/280>.
- **Contribution:** First to combine **high threshold + adaptive security +
  robustness + subcubic communication** simultaneously, via a new adaptively
  secure high-threshold **asynchronous VSS (AVSS)**. Targets asynchronous
  distributed-systems deployments.

### 3.15 Arctic (PKC 2025) — deterministic, stateless, two-round

- **Paper:** Chelsea Komlo, Ian Goldberg, "Arctic: Lightweight and Stateless
  Threshold Schnorr Signatures." **PKC 2025**; ePrint **2024/466**.
  <https://eprint.iacr.org/2024/466>.
- **Motivation:** Randomized two-round schemes (FROST) require signers to keep
  **secret nonce state** between rounds; if that state is reused/reset,
  **nonce-reuse → catastrophic key recovery**. Prior deterministic threshold
  Schnorr schemes were expensive (needed ZK proofs of correct PRF evaluation).
- **Contribution:** A **deterministic**, **stateless**, **two-round** threshold
  Schnorr that is cheap — proven **statically secure under DL in the ROM**. Very
  attractive for hardware wallets / embedded signers where secure statefulness
  is hard.

### 3.16 A Plausible Attack on the Adaptive Security of Threshold Schnorr (CRYPTO 2025)

- **Paper:** Lúcás Meier (cronokirby), "A Plausible Attack on the Adaptive
  Security of Threshold Schnorr Signatures." **CRYPTO 2025**; ePrint
  **2025/1001**. <https://eprint.iacr.org/2025/1001>.
- **Contribution (important correction):** Shows that **all variants of FROST,
  Sparkle, and Lindell'22** — any scheme with public key shares
  `pkᵢ = g^{skᵢ}` where the `skᵢ` lie on a sharing polynomial — **cannot be
  proven fully adaptively secure** without modifications or without assuming a
  new (plausibly hard) search problem the paper defines. Also generalizes below
  `t−1` corruptions. This is the theoretical statement of the widespread
  intuition "**FROST is secure, but not *adaptively* secure**" and it flagged a
  **gap in Sparkle+'s adaptive proof**.

### 3.17 Glacius & Dazzle (EUROCRYPT/PKC 2025) — adaptive without AGM, refined

- **Glacius — Bacho, Das, Loss, Ren. EUROCRYPT 2025**; ePrint **2024/1628**.
  <https://eprint.iacr.org/2024/1628>. First threshold Schnorr with **full
  adaptive security from DDH in the ROM** supporting **full threshold t<n**,
  with **constant-size signing keys** and **identifiable abort** — improving on
  Twinkle's efficiency/robustness profile.
- **Dazzle / Dazzle-T — Yanbo Chen. PKC 2025**; ePrint **2025/264**.
  <https://eprint.iacr.org/2025/264>. Adaptively secure from **DDH without the
  AGM**, improving on Twinkle in **signature size, round complexity, and/or
  tightness**.
- **Tightly Secure Threshold Signatures over Pairing-Free Groups — Bacho et
  al.**, *Communications in Cryptology* 2024; ePrint **2024/1557** — tight
  (loss-free) adaptive proofs in the pairing-free setting.

### 3.18 Latest repairs & new constructions (2025–2026)

- **Round-Efficient Adaptively Secure Threshold Signatures with Rewinding** —
  ePrint **2025/638**. Uses rewinding to shrink rounds for adaptive security.
- **Fully-Adaptive Two-Round Threshold Schnorr Signatures from DDH** — Paul
  Gerhart, Davide Li Calsi, Luigi Russo, Dominique Schröder. **EUROCRYPT 2026**;
  ePrint **2025/1478**. <https://eprint.iacr.org/2025/1478>. **Round-optimal
  (two-round)** with **full adaptive security from DDH in ROM**, via a new
  *equivocal deterministic nonce derivation* technique; requires periodic
  refresh of part of each signer's public key. (Has a reference implementation.)
- **Adaptively Secure Three-Round Threshold Schnorr Signatures from DDH** —
  CRYPTO 2025 (Springer 978-3-032-01887-8_13). Adaptive, DDH, three rounds.
- **Adaptively Secure Partially Non-Interactive Threshold Schnorr Signatures in
  the AGM** — ePrint **2025/1953**. <https://eprint.iacr.org/2025/1953>.
- **Revisiting the Security of Sparkle** — ePrint **2026/431** (Mar 2026).
  <https://eprint.iacr.org/2026/431>. **The current state of the art on
  Sparkle:** gives the **first proof of static security** for plain Sparkle
  (which had lacked one), and a **tight full-adaptive-security proof in the pure
  ROM (no AGM)** based on a new **Vandermonde circular discrete-log (VCDL)**
  assumption — repairing the gap identified by Meier (§3.16). Also yields a tight
  adaptive multi-user proof for *basic* Schnorr.
- **UC4Free! Existing Threshold Signatures are UC Secure** — 2025 result arguing
  existing game-based-secure threshold signatures also achieve UC security.

---

## 4. In-family comparison table

| Scheme | Authors / Venue / Year | ePrint | Rounds | Robust? | DKG | Security model & level | Assumptions | Adaptive? | BIP340-ready |
|---|---|---|---|---|---|---|---|---|---|
| Stinson–Strobl | Stinson, Strobl / ACISP 2001 | — (LNCS 2119) | ~3 (nonce DKG each sig) | Yes | GJKR DKG | EUF-CMA, ROM, **synchronous only** | DL, ROM | No | Adaptable, not concurrent |
| Lindell'22 | Lindell / CiC 2024 | 2022/374 | 3 | No | Shamir/DKG | **UC / full simulatability**, concurrent, dishonest majority | DL + commit/ZK, (RO/std) | Static | Yes (with parity handling) |
| **FROST(1)** | Komlo, Goldberg / SAC 2020 | 2020/852 | **2** (1 offline) | No (ident. abort) | PedPoP / dealer | EUF-CMA, ROM, concurrent; **TS-UF-1** (BCKMTZ'22) | DL / OMDL, ROM | Static | Yes (via BIP 445) |
| FROST2 | Crites–Komlo–Maller; BCKMTZ / CRYPTO 2022 | 2021/1375; 32268 | 2 | No | PedPoP | **TS-UF-1** | Schnorr-KoE + OMDL, ROM | Static | Yes |
| Olaf / **FROST3** | Chu, Gerhart, Ruffing, Schröder / CRYPTO 2023 | 2023/899 | 2 | No | PedPoP (biased ok) | EUF-CMA, concurrent, **no AGM** | **AOMDL**, ROM | Static | **Yes** (basis of BIP 445) |
| ROAST (wrapper) | Ruffing et al. / CCS 2022 | 2022/550 | wraps FROST | **Yes** (async) | inherits | inherits FROST + robustness | inherits | Static | Yes |
| Sparkle / Sparkle+ | Crites, Komlo, Maller / CRYPTO 2023 | 2023/445 | 3 | No | dealer/DKG | static: DL+ROM; **adaptive: AGM+AOMDL** (t/2 w/o AGM) | DL / AOMDL, (AGM) ROM | **Sparkle+ yes*** | Yes |
| SPRINT | Benhamouda, Halevi, Krawczyk, Ma, Rabin / EUROCRYPT 2024 | 2023/427 | 1 DKG + non-interactive batch | **Yes (GOD)** | built-in | static, robust, high-throughput | DL, ROM | No | Instantiable |
| Twinkle | Bacho, Loss, Tessaro, Wagner, Zhu / EUROCRYPT 2024 | 2023/1482 | 3 | — | DKG | **full adaptive, no AGM** | **DDH**, ROM | **Yes** | Schnorr-type (heavier) |
| HARTS | Bacho, Loss, Stern, Wagner / ASIACRYPT 2024 | 2024/280 | async | **Yes** | AVSS | **adaptive + high-threshold + robust**, subcubic | DL/AVSS, ROM | **Yes** | Instantiable |
| Arctic | Komlo, Goldberg / PKC 2025 | 2024/466 | 2 | No | DKG | **deterministic, stateless**, static | DL, ROM | Static | Yes |
| Glacius | Bacho, Das, Loss, Ren / EUROCRYPT 2025 | 2024/1628 | — | ident. abort | DKG | **full adaptive, full threshold, no AGM** | **DDH**, ROM | **Yes** | Schnorr-type |
| Dazzle | Y. Chen / PKC 2025 | 2025/264 | reduced | — | DKG | adaptive, improved size/tightness | **DDH**, ROM | **Yes** | Schnorr-type |
| Fully-Adaptive 2-Round | Gerhart, Li Calsi, Russo, Schröder / EUROCRYPT 2026 | 2025/1478 | **2** | — | DKG + key refresh | **full adaptive, round-optimal** | **DDH**, ROM | **Yes** | Schnorr-type |
| Sparkle (revisited) | / ePrint 2026 | 2026/431 | 3 | No | dealer/DKG | **static proof + tight adaptive, pure ROM** | **VCDL** (new), ROM | **Yes** | Yes |

*Sparkle+'s original AGM adaptive proof had a gap (Meier 2025); repaired by
ePrint 2026/431 under a new assumption.

---

## 5. DKG requirements (couples to the DKG document)

- **Trusted dealer.** Simplest: one party Shamir-shares the key and distributes.
  Acceptable when a dealer is temporarily trusted (RFC 9591 gives this as an
  appendix option). No dealer trust needed thereafter.
- **Pedersen DKG / PedPoP / SimplPedPop.** Each party runs a Feldman/Pedersen
  VSS as dealer; sum of sub-shares is the key share. **PedPoP** adds a
  proof-of-possession to bind each contribution. Pedersen DKG has the GJKR bias,
  but Olaf (§3.9) proves the bias is harmless for FROST unforgeability.
- **ChillDKG (Bitcoin BIP draft).** Tim Ruffing, Jonas Nick, Sivaram
  Dhakshinamoorthy. A **self-contained** DKG for FROST over **secp256k1** that
  needs **no external secure/broadcast channel** — it internally achieves
  agreement, has an *investigation phase* for identifying faulty participants,
  and integrated backup/recovery. Built on **SimplPedPop / EncPedPop** derived
  from Olaf's analysis. (See the C2SP "COCKTAIL-DKG" discussion and
  BlockstreamResearch/bip-frost-dkg.) → *Detailed in the DKG document.*
- **AVSS (asynchronous).** HARTS/Glacius-style schemes use asynchronous VSS for
  robustness in partially/ fully asynchronous networks.

---

## 6. Adaptive-security discussion (the crux of 2023–2026 research)

The dominant deployed schemes (FROST1/2/3, and Lindell'22, Sparkle) are proven
under **static** corruption. Adaptive corruption — where the adversary chooses
whom to corrupt during the protocol, based on observed transcripts and internal
state — is the realistic threat model for long-lived custody keys.

Timeline of the adaptive story:

1. **Sparkle (2023/445)** claimed the first adaptive proof (Sparkle+) but only
   full-`t` under the **AGM**; without AGM, restricted to **t/2**.
2. **Twinkle (2023/1482)** achieved full adaptive from **DDH without AGM** — at
   the cost of a heavier, non-BIP340-native signature and more rounds.
3. **HARTS, Glacius, Dazzle (2024–2025)** pushed adaptive+robust, adaptive+full-
   threshold, and better efficiency/tightness, all from DDH without AGM.
4. **Meier's "Plausible Attack" (2025/1001, CRYPTO 2025)** proved a **barrier**:
   FROST/Sparkle/Lindell'22 — any scheme with linear-in-the-exponent key shares
   — **cannot** be proven fully adaptive without new assumptions or design
   changes, and exposed a gap in Sparkle+'s AGM proof.
5. **Gerhart–Li Calsi–Russo–Schröder (2025/1478, EUROCRYPT 2026)** delivered a
   **round-optimal (two-round) fully adaptive** scheme from DDH (with periodic
   public-key refresh).
6. **"Revisiting the Security of Sparkle" (2026/431)** closed the Sparkle loop:
   first **static** proof for plain Sparkle and a **tight full-adaptive proof in
   the pure ROM** under a new **VCDL** assumption.

**Practical takeaway for Bitcoin.** FROST/Olaf remain the right engineering
choice today (BIP340-native, efficient, extensively analyzed for *static*
concurrent security, TS-UF-1). Their lack of a clean *adaptive* proof is a
theoretical caveat, not a known attack; the DDH-based adaptive family
(Twinkle/Glacius/Dazzle/2025-1478) is the research frontier but produces
non-BIP340-native signatures or needs key refresh, so it is not yet a drop-in
Taproot replacement.

---

## 7. Known attacks & caveats

- **ROS / Drijvers / Wagner *k*-sum** (2020/945, S&P 2019): breaks naive
  concurrent multi-round Schnorr; the reason FROST uses **binding factors** and
  three-round schemes use **commit-then-reveal**. Do not deploy an unbound
  two-round Schnorr threshold scheme.
- **Nonce state reuse / reset:** in randomized two-round FROST, resetting a
  signer to reuse a preprocessed nonce across two different messages leaks the
  secret share (two equations, one nonce). Mitigations: strict state deletion,
  or use **Arctic** (deterministic, stateless).
- **DKG bias (Pedersen):** benign for FROST (Olaf), but must not be assumed
  benign for arbitrary protocols.
- **Adaptive corruption:** no proof for FROST/Sparkle/Lindell'22 (Meier barrier)
  — a modeling caveat.
- **Coordinator trust:** the coordinator sees no secrets and cannot forge, but a
  malicious coordinator can withhold/reorder (→ use ROAST for robustness) or
  perform a **rogue-session / Wagner** style attack only if binding is absent.
- **Parity/tweak bugs:** BIP340 even-Y and Taproot tweak sign-flip bookkeeping
  (`gacc`) is a notorious implementation footgun — get it from BIP 445 /
  secp256k1-zkp, don't hand-roll.

---

## 8. Implementations & production deployments

- **ZF FROST (`frost-secp256k1`, `frost-core`)** — ZcashFoundation/frost, Rust,
  implements RFC 9591's five ciphersuites incl. secp256k1; partially audited by
  NCC Group. <https://github.com/ZcashFoundation/frost>,
  <https://frost.zfnd.org/>. Note: RFC 9591 secp256k1 is **not** BIP340-
  compatible without the BIP 445 x-only/tweak layer.
- **secp256k1-zkp / libsecp256k1 modules (Blockstream)** — FROST + MuSig2
  modules on the production Bitcoin curve library;
  **BlockstreamResearch/bip-frost-dkg** (ChillDKG). Blockstream drives ROAST,
  Olaf, ChillDKG, and the BIP work.
- **BIP 445 — `bip-frost-signing`** (Sivaram Dhakshinamoorthy, Draft assigned
  2026-01-30): FROST3-based, **BIP340-compatible** signing with x-only keys,
  even-Y sign-flips, and Taproot/BIP32 tweaking.
  <https://github.com/siv2r/bip-frost-signing>.
- **ChillDKG BIP** (Ruffing, Nick, Dhakshinamoorthy) — companion DKG.
- **Banca d'Italia `secp256k1-frost`** — C implementation of FROST on secp256k1.
  <https://github.com/bancaditalia/secp256k1-frost>.
- **Chainflip** — first production blockchain to use FROST at scale; **100-of-
  150** threshold across all supported chains, ~1 signature/second target.
  <https://docs.chainflip.io/protocol/frost-signature-scheme>.
- **Zcash** — FROST for shielded-transaction multisig (in progress).
- **Contrast with threshold *ECDSA*** (DKLs, GG18/GG20, Lindell17; Coinbase/
  Fireblocks/Ledger custody) — needed while pre-Taproot addresses dominate;
  Schnorr/FROST is strictly simpler and is the Taproot-native path forward.

---

## 9. Open problems

1. **Adaptive security for a BIP340-native scheme.** FROST/Olaf have no clean
   adaptive proof (Meier barrier); DDH-adaptive schemes aren't BIP340-drop-in.
   A round-optimal, adaptively secure, *bare-Schnorr-output* scheme without key
   refresh is open.
2. **Robustness without a wrapper, at Bitcoin scale.** ROAST works but is a
   meta-protocol; native robust *and* BIP340-native *and* two-round is unsettled.
3. **Assumption hygiene.** Adaptive proofs lean on new/interactive assumptions
   (AOMDL, VCDL, one-more variants); reducing to plain DL/DDH tightly remains a
   goal.
4. **Standardization gap.** RFC 9591 is not BIP340-compatible; BIP 445 +
   ChillDKG are still **Draft**. Interop and audited reference code are maturing.
5. **Deterministic + adaptive + two-round** simultaneously (Arctic is
   deterministic+static; 2025/1478 is adaptive+two-round-randomized).
6. **Post-quantum threshold Schnorr** — none of these survive a quantum
   adversary; lattice analogues (e.g. TALUS/Threshold ML-DSA) are nascent and
   off the secp256k1/BIP340 path.

---

## 10. References

**Foundations**
- C.-P. Schnorr, "Efficient Signature Generation by Smart Cards," *J. Cryptology*
  4(3), 1991.
- R. Gennaro, S. Jarecki, H. Krawczyk, T. Rabin, "Secure Distributed Key
  Generation for Discrete-Log Based Cryptosystems," EUROCRYPT 1999 / *J.
  Cryptology* 2007. <https://link.springer.com/article/10.1007/s00145-006-0347-3>
- D. Stinson, R. Strobl, "Provably Secure Distributed Schnorr Signatures and a
  (t,n) Threshold Scheme for Implicit Certificates," ACISP 2001, LNCS 2119.
  <https://link.springer.com/chapter/10.1007/3-540-47719-5_33>

**FROST line**
- C. Komlo, I. Goldberg, "FROST: Flexible Round-Optimized Schnorr Threshold
  Signatures," SAC 2020. ePrint 2020/852. <https://eprint.iacr.org/2020/852>
- E. Crites, C. Komlo, M. Maller, "How to Prove Schnorr Assuming Schnorr:
  Security of Multi- and Threshold Signatures," ePrint 2021/1375 (FROST2).
  <https://eprint.iacr.org/2021/1375>
- M. Bellare, E. Crites, C. Komlo, M. Maller, S. Tessaro, C. Zhu, "Better than
  Advertised Security for Non-Interactive Threshold Signatures," CRYPTO 2022
  (TS-UF hierarchy). <https://link.springer.com/chapter/10.1007/978-3-031-15985-5_18>
- H. Chu, P. Gerhart, T. Ruffing, D. Schröder, "Practical Schnorr Threshold
  Signatures Without the Algebraic Group Model" (Olaf / FROST3), CRYPTO 2023.
  ePrint 2023/899. <https://eprint.iacr.org/2023/899>
- "The many faces of Schnorr: a toolkit for the modular design of threshold
  Schnorr signatures," ePrint 2023/1019. <https://eprint.iacr.org/2023/1019>
- RFC 9591, "The Flexible Round-Optimized Schnorr Threshold (FROST) Protocol,"
  IRTF/CFRG, 2024. <https://www.rfc-editor.org/rfc/rfc9591.html>

**Robustness / throughput**
- T. Ruffing, V. Ronge, E. Jin, J. Schneider-Bensch, D. Schröder, "ROAST: Robust
  Asynchronous Schnorr Threshold Signatures," CCS 2022. ePrint 2022/550.
  <https://eprint.iacr.org/2022/550>
- F. Benhamouda, S. Halevi, H. Krawczyk, Y. Ma, T. Rabin, "SPRINT: High-
  Throughput Robust Distributed Schnorr Signatures," EUROCRYPT 2024. ePrint
  2023/427. <https://eprint.iacr.org/2023/427>

**Simulatability / UC**
- Y. Lindell, "Simple Three-Round Multiparty Schnorr Signing with Full
  Simulatability," ePrint 2022/374 / CiC 2024. <https://eprint.iacr.org/2022/374>

**Adaptive security**
- E. Crites, C. Komlo, M. Maller, "Fully Adaptive Schnorr Threshold Signatures"
  (Sparkle / Sparkle+), CRYPTO 2023. ePrint 2023/445.
  <https://eprint.iacr.org/2023/445>
- R. Bacho, J. Loss, S. Tessaro, B. Wagner, C. Zhu, "Twinkle: Threshold
  Signatures from DDH with Full Adaptive Security," EUROCRYPT 2024. ePrint
  2023/1482. <https://eprint.iacr.org/2023/1482>
- R. Bacho, J. Loss, G. Stern, B. Wagner, "HARTS: High-Threshold, Adaptively
  Secure, and Robust Threshold Schnorr Signatures," ASIACRYPT 2024. ePrint
  2024/280. <https://eprint.iacr.org/2024/280>
- R. Bacho, S. Das, J. Loss, L. Ren, "Glacius: Threshold Schnorr Signatures from
  DDH with Full Adaptive Security," EUROCRYPT 2025. ePrint 2024/1628.
  <https://eprint.iacr.org/2024/1628>
- Y. Chen, "Dazzle: Improved Adaptive Threshold Signatures from DDH," PKC 2025.
  ePrint 2025/264. <https://eprint.iacr.org/2025/264>
- R. Bacho et al., "Tightly Secure Threshold Signatures over Pairing-Free
  Groups," CiC 2024. ePrint 2024/1557. <https://eprint.iacr.org/2024/1557>
- L. Meier, "A Plausible Attack on the Adaptive Security of Threshold Schnorr
  Signatures," CRYPTO 2025. ePrint 2025/1001. <https://eprint.iacr.org/2025/1001>
- "Round-Efficient Adaptively Secure Threshold Signatures with Rewinding,"
  ePrint 2025/638. <https://eprint.iacr.org/2025/638>
- P. Gerhart, D. Li Calsi, L. Russo, D. Schröder, "Fully-Adaptive Two-Round
  Threshold Schnorr Signatures from DDH," EUROCRYPT 2026. ePrint 2025/1478.
  <https://eprint.iacr.org/2025/1478>
- "Adaptively Secure Partially Non-Interactive Threshold Schnorr Signatures in
  the AGM," ePrint 2025/1953. <https://eprint.iacr.org/2025/1953>
- "Revisiting the Security of Sparkle," ePrint 2026/431 (2026).
  <https://eprint.iacr.org/2026/431>

**Stateless / deterministic**
- C. Komlo, I. Goldberg, "Arctic: Lightweight and Stateless Threshold Schnorr
  Signatures," PKC 2025. ePrint 2024/466. <https://eprint.iacr.org/2024/466>

**Attacks**
- M. Drijvers et al., "On the Security of Two-Round Multi-Signatures," IEEE S&P
  2019.
- F. Benhamouda, T. Lepoint, J. Loss, M. Orrù, M. Raykova, "On the (in)security
  of ROS," EUROCRYPT 2021. ePrint 2020/945. <https://eprint.iacr.org/2020/945>

**Bitcoin / standardization / implementations**
- BIP340, "Schnorr Signatures for secp256k1" (Wuille, Nick, Ruffing).
  <https://bips.xyz/340>
- BIP 445 / `bip-frost-signing` (S. Dhakshinamoorthy), FROST3 BIP340 signing.
  <https://github.com/siv2r/bip-frost-signing>
- ChillDKG BIP / `bip-frost-dkg` (Ruffing, Nick, Dhakshinamoorthy).
  <https://github.com/BlockstreamResearch/bip-frost-dkg>
- ZcashFoundation/frost (`frost-secp256k1`). <https://github.com/ZcashFoundation/frost>
- bancaditalia/secp256k1-frost. <https://github.com/bancaditalia/secp256k1-frost>
- Chainflip FROST signature scheme.
  <https://docs.chainflip.io/protocol/frost-signature-scheme>
