# Multi-Signature Schemes for Bitcoin: Schnorr Key-Aggregation, ECDSA, and the Road to Taproot

> Deep research note. Scope: n-of-n key-aggregation multisignatures applicable to Bitcoin, with emphasis on Schnorr/BIP340 (MuSig, MuSig2, BIP327), the ECDSA and pre-Schnorr history, the security-model subtleties (plain public-key model, concurrency, the ROS problem), signature-aggregation proposals, and the state of the art to 2026.
> Last updated: 2026-07-03.

---

## 1. Overview and scope

A **multisignature** scheme lets a fixed group of *n* signers jointly produce a single signature on a single message *m*, such that the joint signature convinces any verifier that **all n** signers participated. The modern (post-2018) Bitcoin-relevant flavor adds **key aggregation**: the *n* individual public keys are compressed into one *aggregate public key*, and the joint signature verifies against that single aggregate key with the *ordinary* single-signer verification equation. On Bitcoin+Taproot this means an n-of-n arrangement is represented on-chain by a single 32-byte x-only key and a single 64-byte signature — **indistinguishable** from a single-signer spend.

Multisignatures are the **n-of-n boundary case** of threshold signing. They are, by a wide margin, the most-deployed form of "threshold-like" signing on Bitcoin after the Taproot soft fork (BIPs 340/341/342, activated November 2021), because BIP327 (MuSig2) standardized them and they map cleanly onto Taproot key-path spends.

### 1.1 How a multisignature differs from a t-of-n threshold signature

This distinction is the single most important framing for the whole document.

| Property | Multisignature (n-of-n) | Threshold signature (t-of-n) |
|---|---|---|
| Who must sign | **All** n signers | **Any** t of n signers |
| Key material | Each signer holds an **independent** key it generated itself | A single secret is **Shamir-shared**; each signer holds a *share* of one key |
| Setup | No shared-secret setup; just exchange public keys (interactive **key aggregation**, but no DKG) | Requires **Distributed Key Generation (DKG)** or a trusted dealer to create shares |
| Fault tolerance | **None** — losing any one signer/key loses the funds | Tolerates loss/unavailability of up to n−t signers |
| Accountability | Every signer provably participated (all-or-nothing) | The signer set is a subset; who signed is not inherently revealed unless the scheme is *accountable* |
| Canonical Bitcoin scheme | **MuSig / MuSig2** (BIP327) | **FROST** (and its Bitcoin roadmap), GG18/GG20 for ECDSA |
| On-chain footprint | 1 aggregate key + 1 Schnorr sig | 1 aggregate key + 1 Schnorr sig (same!) |

Both families produce a *single ordinary signature* under a *single aggregate key*, so on-chain they look identical to each other and to a single-sig. The difference is entirely in the **key setup and who can sign**. MuSig2 is n-of-n; you cannot directly express "2-of-3" with it (you can approximate policies by putting several MuSig2 aggregate keys into a Taproot script tree / MAST). FROST is the true t-of-n cousin and is treated in the threshold-signature note; the ROS discussion below is shared between them.

A subtle intermediate is the **accountable-subgroup multisignature (ASM)** of Micali–Ohta–Reyzin: a *subgroup* S of a fixed group G signs, and the signature *reveals exactly which members of S signed*. ASM is not a threshold scheme (there is no secret sharing and no t-out-of-n unforgeability guarantee against a coalition), but it relaxes the "all-n" requirement while preserving accountability.

### 1.2 Why n-of-n multisig matters on Bitcoin

- **2-of-2 channels / Lightning:** funding and commitment transactions are the archetypal n-of-n case; MuSig2 makes the funding output look like single-sig and cuts on-chain bytes.
- **Custody / vaults:** enterprise cold storage as k independent HSMs where all must sign a given transaction (often layered with Taproot script paths for recovery).
- **Privacy / fungibility:** replacing `OP_CHECKMULTISIG`/`OP_CHECKSIGADD` scripts (which reveal "this is a 2-of-3") with a single aggregate key hides the policy entirely.
- **Fee savings:** one 64-byte signature and one 32-byte key regardless of n.

---

## 2. Bitcoin / Taproot applicability primer

The relevant standards:

- **BIP340** — Schnorr signatures over secp256k1 with **x-only** 32-byte public keys, and a specific challenge hash `e = H_tag("BIP0340/challenge", R.x ‖ P.x ‖ m)`. Verification: `s·G = R + e·P`, with R having implicitly even Y. Everything below is built to output exactly a BIP340 signature.
- **BIP341** — Taproot output: a program is `Q = P + t·G` where `t = H_tag("TapTweak", P.x ‖ merkle_root)`. The output key `Q` is x-only. **Key-path** spends sign with the tweaked key; **script-path** spends reveal a leaf. MuSig2's *tweaking* support is precisely what lets an aggregate key be used as the internal key `P` and then Taproot-tweaked to `Q`.
- **BIP342** — Tapscript; `OP_CHECKSIGADD` gives an explicit (batch-verifiable) k-of-n *in script*, which is the non-aggregated, non-private alternative to MuSig2.

**x-only and even-Y handling.** BIP340 keys and nonces are x-only, so their Y-coordinate parity is fixed to even by convention. Any aggregation or tweaking that lands on an odd-Y point must be *negated*. This is the source of the "sign flip" bookkeeping (`gacc`, negation factors `g`) that pervades BIP327: when the aggregate key `Q` has odd Y, signers negate their secret keys; when the final nonce `R` has odd Y, signers negate their nonce scalars. Getting this bookkeeping right, especially under chained plain + x-only tweaks, is most of BIP327's engineering complexity.

**Companion BIPs for wallets:** BIP328 (MuSig2 deterministic key derivation from an aggregate xpub), BIP373 (PSBT fields for MuSig2), BIP390 (`musig()` descriptor). These make MuSig2 usable in the standard wallet stack (descriptors + PSBT + HW signers).

---

## 3. Per-scheme deep dive

Schemes are grouped chronologically/thematically. For each: authors, venue/year, ePrint, construction, rounds, security model, key-aggregation method, and known issues.

### 3.1 Itakura–Nakamura (1983) — the origin

- **Authors / venue:** K. Itakura, K. Nakamura. *"A public-key cryptosystem suitable for digital multisignatures."* NEC Journal of Research and Development, no. 71, Oct. 1983.
- **Construction:** An **RSA-based sequential (serial) multisignature**. Signers sign in a predefined order, each "re-signing" the running signature; a reblocking step handles modulus mismatches between successive signers.
- **Significance:** First multisignature scheme; established the core selling point that the multisignature's **size is independent of the number of signers** (unlike naive concatenation).
- **Known issues:** Keys must be generated consistent with the signing order; RSA reblocking is awkward (later "dynamic reblocking" variants addressed this). No formal security model — this predates provable-security definitions for multisig by ~18 years. Not directly relevant to Bitcoin, but it is the historical root.

### 3.2 Micali–Ohta–Reyzin (2001) — Accountable-Subgroup Multisignatures (ASM)

- **Authors / venue:** Silvio Micali (MIT), Kazuo Ohta (UEC Tokyo), Leonid Reyzin (Boston U). *"Accountable-Subgroup Multisignatures."* ACM CCS 2001, pp. 245–254. (Page: cs.bu.edu/~reyzin/multisig.html.)
- **Contribution:** The **first formal security model** for multisignatures that *includes key generation without a trusted third party*. Introduces **ASM**: any subgroup S ⊆ G can jointly sign so that the signature *provably reveals the identities of exactly the signers in S*.
- **Construction:** A Schnorr-based protocol requiring **three rounds** of communication per signature.
- **Why it matters:** MOR articulated *why* formal models matter for multisig — unlike threshold signatures, multisig had lacked precise definitions and several prior proposals had been broken. ASM's accountability idea reappears in modern accountable/"forensic" multisig and in some BLS aggregate constructions.

### 3.3 Bellare–Neven (2006) — the plain public-key model (BN)

- **Authors / venue:** Mihir Bellare, Gregory Neven. *"Multi-Signatures in the Plain Public-Key Model and a General Forking Lemma."* ACM CCS 2006, pp. 390–399. (cseweb.ucsd.edu/~mihir/papers/multisignatures.pdf)
- **Central idea — the plain public-key model:** Earlier DL-based multisig required a **key-setup assumption** (a trusted KOSK/"knowledge of secret key" registration, or proofs of possession) to stop rogue-key attacks. BN eliminated this: security holds even if the adversary registers arbitrary public keys with **no** proof of knowledge and no honest key-setup. This is the model MuSig later inherited and it is the *right* model for a permissionless system like Bitcoin.
- **Construction (BN multisig):** Each signer i broadcasts a commitment `t_i = H(R_i)` to its nonce `R_i = g^{r_i}`; after collecting all commitments, signers reveal `R_i`; the combined nonce is `R = ∏ R_i`; the challenge is *per-signer key-prefixed*: `c_i = H(⟨all keys⟩, R, m)` — critically, each signer's public key is hashed into the challenge, which is what defeats rogue-key attacks **without** key aggregation. Each `s_i = r_i + c_i·x_i`; the signature is `(R, s = Σ s_i)`.
- **Rounds:** Three (commit / reveal / sign).
- **Security:** EUF-CMA in the plain public-key model under **discrete log** in the ROM, proved via a *generalized forking lemma* (a reusable tool the paper also contributes).
- **Cost / footprint:** **No key aggregation** — verification is O(n) exponentiations and the verifier needs all n public keys. This is the key difference from MuSig: BN is secure and simple but the "one aggregate key that looks like single-sig" property is missing. MuSig's contribution is precisely to add key aggregation to the BN template.

### 3.4 Bagherzandi–Cheon–Jarecki (2008) — BCJ, two rounds via homomorphic commitments

- **Authors / venue:** Ali Bagherzandi, Jung Hee Cheon, Stanisław Jarecki. *"Multisignatures Secure under the Discrete Logarithm Assumption and a Generalized Forking Lemma."* ACM CCS 2008, pp. 449–458.
- **Contribution:** A **two-round** O(n)-verification multisig secure under DL in the plain model (and an O(1)-verification variant in a key-verification model), using a **multiplicatively homomorphic equivocable commitment** to compress BN's commit/reveal into fewer rounds, plus a generalized forking lemma enabling simultaneous extraction from many NIZK instances.
- **Why it matters here:** BCJ is the direct ancestor of the *insecure* two-round DL multisig line that Drijvers et al. later broke, and the **mBCJ** repair (below) is the concrete "fix BCJ" scheme.

### 3.5 MuSig (2018/2019) — Schnorr multisig with key aggregation

- **Authors / venue:** Gregory Maxwell, Andrew Poelstra, Yannick Seurin, Pieter Wuille. *"Simple Schnorr Multi-Signatures with Applications to Bitcoin."* ePrint **2018/068**; journal version in *Designs, Codes and Cryptography* 87, 2139–2164 (2019), DOI 10.1007/s10623-019-00608-x (received May 2018, revised Jan 2019).
- **The headline feature — key aggregation (KOA):** Define the list `L = {P_1,…,P_n}`. Each signer gets a **delinearization coefficient** `a_i = H_agg(L, P_i)`. The **aggregate public key** is
  `X̃ = Σ a_i · P_i` (multiplicatively: `∏ P_i^{a_i}`).
  The joint signature verifies as an ordinary Schnorr signature under `X̃`. This is what BN lacks and what makes MuSig Bitcoin-relevant: one aggregate key, single-sig verification.
- **Rogue-key attack and defense:** Naively summing keys (`X̃ = Σ P_i`) lets a malicious last signer pick `P_1 = g^{x_1} − Σ_{i≥2} P_i`, yielding an aggregate key it alone controls — it can then sign for the whole group. MuSig defeats this with the **hash-of-all-keys delinearization**: because `a_i = H(L, P_i)` depends on the *entire* key list (and the attacker cannot predict `H` outputs), the attacker cannot cancel the honest keys. This is a strict generalization of BN's key-prefixing, adapted so that aggregation still works.
- **Protocol (three rounds):**
  1. **Commit:** each signer sends `t_i = H_com(R_i)`, where `R_i = g^{r_i}`.
  2. **Reveal:** after receiving all `t_j`, each signer reveals `R_i`; others check `t_j = H_com(R_j)`.
  3. **Sign:** `R = Σ R_i`, `c = H_sig(X̃, R, m)`, `s_i = r_i + c·a_i·x_i`. Final signature `(R, s=Σ s_i)`; verify `s·G = R + c·X̃`.
- **Security model:** EUF-CMA in the **plain public-key model** under DL in the ROM.
- **THE PROOF FLAW (important history):** The *first* preprint of MuSig proposed a **two-round** variant (dropping the commitment round) and claimed a proof under the One-More Discrete Log (OMDL) assumption. **Drijvers et al. (S&P 2019)** discovered the proof was **flawed** and, via a *meta-reduction*, showed the two-round scheme **cannot** be proven secure by any algebraic black-box reduction to (OM)DL — and gave a concrete concurrent attack. The authors **withdrew the two-round claim** and MuSig was corrected to the **three-round** commitment-based protocol above, which *is* provably secure. Lesson that framed the entire field: **removing the nonce-commitment round in a DL multisig is dangerous**, because an adversary who chooses its nonce *after seeing the honest nonce* can steer the combined nonce R and defeat the ROM/forking simulation. Getting back to two rounds *securely* required a genuinely new idea — MuSig2.
- **Known issues:** Three rounds (a latency and statefulness burden); the original two-round misstep; interactive commitment round adds a message and requires state.

### 3.6 On the Security of Two-Round Multi-Signatures (Drijvers et al., 2019) — the attack paper

- **Authors / venue:** Manu Drijvers, Kasra Edalatnejad, Bryan Ford, Eike Kiltz, Julian Loss, Gregory Neven, Igors Stepanovs. *"On the Security of Two-Round Multi-Signatures."* IEEE S&P 2019. (PDF: bford.info/pub/sec/two-round-multisig.pdf)
- **Result:** **All** then-proposed two-round DL multisig schemes without pairings (BCJ, MWLD, CoSI, the two-round MuSig, etc.) are **insecure under concurrent signing sessions**. The attack is a *k-sum / Wagner-style* attack: an adversary opens ℓ−1 concurrent sessions, and by choosing its contributions adaptively across sessions can produce a forgery in roughly `O(ℓ · 2^{lg q / (1+lg ℓ)})` time — e.g., for a 256-bit group and just 15 concurrent sessions, ~2^62 work. This is the concrete precursor of the ROS framing.
- **Constructive side:** They also give **mBCJ**, a *secure* two-round Schnorr multisig obtained by patching BCJ with a properly instantiated homomorphic (equivocal) commitment. mBCJ is provably secure but has a larger signature and does not give the clean BIP340 single-signature output, so it is not the Bitcoin choice.

### 3.7 The ROS problem (Benhamouda et al., 2021) — why naive two rounds die

- **Authors / venue:** Fabrice Benhamouda, Tancrède Lepoint, Julian Loss, Michele Orrù, Mariana Raykova. *"On the (in)Security of ROS."* EUROCRYPT 2021; journal version *J. Cryptology* 35, 25 (2022).
- **ROS = "Random inhomogeneities in an Overdetermined, Solvable system of linear equations."** Solving ROS in dimension ℓ means finding coefficients that make a chosen linear combination of random-oracle outputs hit a target — exactly the algebraic freedom an adversary gets from opening ℓ concurrent sessions and choosing challenges/aggregation coefficients.
- **Result:** ROS mod p is solvable in **polynomial time** once ℓ > log₂ p (⇒ ~256 concurrent sessions for a 256-bit group breaks the scheme *completely and cheaply*), and sub-exponentially for smaller ℓ (subsuming Wagner). This yields **practical** forgeries against: Schnorr and Okamoto–Schnorr **blind signatures**, **CoSI**, the **two-round MuSig**, threshold schemes like **GJKR** and the **original FROST** presentation, and Abe–Okamoto partially-blind signatures.
- **Consequence for multisig design:** A secure two-round DL multisig must **structurally prevent** the adversary from expressing its forgery as a solvable ROS instance. MuSig2's answer is to have each signer commit to **two (or more) nonces** and combine them with a random-oracle coefficient `b = H(...)`, so the final nonce is a *non-linear* function `R = R_{·,1} + b·R_{·,2}` the adversary cannot control linearly across sessions. DWMS independently uses the same "delinearize multiple pre-nonces" trick.

### 3.8 MuSig-DN (2020) — verifiably deterministic nonces

- **Authors / venue:** Jonas Nick, Tim Ruffing, Yannick Seurin, Pieter Wuille. *"MuSig-DN: Schnorr Multi-Signatures with Verifiably Deterministic Nonces."* ACM CCS 2020; ePrint **2020/1057**.
- **Problem it solves:** In multisig you *cannot* naively derandomize nonces (RFC 6979 style). If a signer's nonce is a deterministic function only of (key, message), a malicious cosigner can trick the victim into signing the *same* (message, its own nonce) against **two different** sets of cosigner contributions, producing two signatures with the same nonce ⇒ the secret key leaks. So determinism must be *bound to the whole session*, and the signer must **prove** it derived the nonce correctly.
- **Construction:** Nonce is a PRF of the message and *all* signers' public keys; the signer attaches a **non-interactive zero-knowledge proof** (a purpose-built Bulletproof-style NIZK over a PRF circuit) that the nonce was computed correctly. This gives the first Schnorr multisig with **stateless, deterministic** signing (no RNG at signing time, no secret state to persist between rounds).
- **Rounds:** Two, but the NIZK is heavy.
- **Known issues:** The ZK proof is **large and slow to generate** (hundreds of ms; proof size on the order of kilobytes). Regarded as an important stepping stone rather than the production answer — MuSig2 got two-round-ness without the NIZK cost, at the price of needing good randomness (with defense-in-depth options).

### 3.9 MuSig2 (2020/2021) — the two-round standard

- **Authors / venue:** Jonas Nick, Tim Ruffing, Yannick Seurin. *"MuSig2: Simple Two-Round Schnorr Multi-Signatures."* CRYPTO 2021; ePrint **2020/1261** (major revision Oct 2023). Standardized as **BIP327**.
- **The core trick (defeating ROS while staying two-round):** Each signer publishes **ν ≥ 2** nonces `R_{i,1}, …, R_{i,ν}` in round 1 (BIP327 uses **ν = 2**). After aggregation, all signers compute a coefficient `b = H_non(aggregate nonces, X̃, m)` and set the **effective nonce** `R = Σ_j R_{·,1} + b·(Σ_j R_{·,2}) + …`. Because `b` is a random oracle applied *to the nonces themselves*, an adversary choosing its nonces after seeing others cannot linearize the resulting `R` across concurrent sessions — the ROS/Wagner attack no longer applies. **Two nonces is the minimum**; with a single nonce the scheme reduces to the broken two-round MuSig. (More nonces buy a tighter/different proof but ν=2 suffices for the AGM proof.)
- **Rounds & preprocessing:** **Two** rounds. Round 1 (nonce exchange) is **message-independent** and can be **preprocessed** — nonces can be generated and shared before the message or even the signer set is finalized, so at signing time only one round remains. This is the killer feature for Lightning and interactive protocols.
- **Key aggregation:** Same KOA as MuSig, with the **MuSig2\*** optimization in BIP327 — the "second distinct key" is assigned coefficient `a = 1` (saving one scalar-mult in key aggregation) while remaining provably secure (proof in the paper's appendix).
- **Security model:** EUF-CMA under the **Algebraic One-More Discrete Logarithm (AOMDL)** assumption in the **Algebraic Group Model (AGM) + ROM**. AOMDL is a falsifiable ("algebraic") variant of OMDL. The paper also gives a proof for ν=4 under plain OMDL in the ROM (no AGM), so there is a spectrum: fewer nonces + AGM, or more nonces + weaker model.
- **Concurrency:** Provably secure under **arbitrarily many concurrent sessions** — the property that ROS/Drijvers denied to naive two-round schemes.
- **Nonce reuse = catastrophic:** As in all Schnorr, reusing a secnonce across two `Sign` calls **leaks the secret key** (two equations, same r, solve for x). BIP327 states the rule bluntly: *"The Sign algorithm must not be executed twice with the same secnonce."* Signers are therefore **stateful**: generate secnonce, persist it, use exactly once. `NonceGen` must draw from a high-quality RNG; optional inputs (sk, aggpk, m, extra_in with a counter/session-id) are folded in as **defense-in-depth** so that even partial RNG failure does not immediately repeat a nonce. BIP327 also specifies **`DeterministicSign`**, letting the *last* signer be stateless by deriving its nonce from all other signers' nonces (safe because at that point the session is fully determined).
- **Tweaking (Taproot):** BIP327 supports **plain tweaks** (`Q' = Q + t·G`, for BIP32 child derivation) and **x-only tweaks** (`Q' = with_even_y(Q) + t·G`, for BIP341 taptweak), and they can be **chained**. Accumulators `gacc` (sign flips) and `tacc` (tweak sum) track the bookkeeping; `PartialSigAgg` folds `e·g·tacc` into the final `s`. **Open caveat:** BIP327 explicitly notes it is *an open question whether allowing fully adversary-chosen tweaks affects unforgeability*, and recommends tweaks be derived per other specs (e.g., a real Taproot merkle root) rather than accepted from an adversary.
- **Infinity edge cases:** If nonce aggregation yields the point at infinity, `R` is set to the generator `G`; likewise KeyAgg is defined so the aggregate is never the identity. These guard against degenerate/forced-abort inputs.
- **Untrusted aggregator:** An optional aggregator can sum nonces and partial sigs (turning O(n²) broadcast into O(n)); a malicious aggregator can force an **abort** but **cannot forge**. With authenticated contributions the protocol supports **identifiable aborts** via `PartialSigVerify`.
- **Known issues / caveats:** Requires good randomness (unlike MuSig-DN); statefulness between rounds is a real operational hazard for HW wallets and backups; adversarial-tweak question open; no threshold (n-of-n only).

### 3.10 DWMS (2020/2021) — parallel work to MuSig2

- **Authors / venue:** Handan Kılınç Alper, Jeffrey Burdges. *"Two-Round Trip Schnorr Multi-Signatures via Delinearized Witnesses."* CRYPTO 2021; ePrint **2020/1245**.
- **Construction:** Independently and contemporaneously with MuSig2, DWMS achieves a secure **two-round** Schnorr multisig by the *same underlying idea*: each signer sends **two pre-nonces (pre-witnesses)**, and the combined nonce is formed by **delinearizing** them with a random oracle. Proven secure in **AGM + ROM** under OMDL and a "2-entwined sum" assumption.
- **Relationship to MuSig2:** DWMS and MuSig2 are the two independent discoveries of the multi-nonce delinearization technique. MuSig2 additionally provides key aggregation optimizations and became the standardized/deployed one; DWMS is the important parallel citation confirming the design principle.

### 3.11 HBMS (Bellare–Dai, 2021) — better security via chain reductions

- **Authors / venue:** Mihir Bellare, Wei Dai. *"Chain Reductions for Multi-Signatures and the HBMS Scheme."* ASIACRYPT 2021; ePrint **2021/404**.
- **Contribution:** A two-round DL multisig (HBMS) with a **modular "chain reduction"** framework giving improved / more transparent concrete security, and communication complexity independent of n. Part of the post-2020 wave rigorously nailing down the concrete (tight-ish) security of two-round multisig in the DL setting.

### 3.12 The tight-security line: Chopsticks → Toothpicks → T-Spoon → Earpicks (2023–2026)

A concerted effort (largely Jiaxin Pan & Benedikt Wagner, later with Renas Bacho) to get **tight** security (reduction loss independent of the number of signers/queries, avoiding the lossy forking lemma) for two-round multisig **without pairings**:

- **Chopsticks** — Jiaxin Pan, Benedikt Wagner. *"Chopsticks: Fork-Free Two-Round Multi-Signatures from Non-Interactive Assumptions."* EUROCRYPT 2023; ePrint **2023/198**. First **fork-free, tightly secure** two-round multisig (DDH / lossy-identification style), but with large signatures and no key aggregation.
- **Toothpicks** — Jiaxin Pan, Benedikt Wagner. *"Toothpicks: More Efficient Fork-Free Two-Round Multi-Signatures."* EUROCRYPT 2024; ePrint **2023/1613**. Tightly secure under DDH, pairing-free; ~3× smaller signatures and ~2× less communication than Chopsticks. Still no key aggregation.
- **T-Spoon** — Renas Bacho, Benedikt Wagner. *"T-Spoon: Tightly Secure Two-Round Multi-Signatures with Key Aggregation."* 2025; ePrint **2025/840**. **First** pairing-free two-round multisig to achieve **tight security AND key aggregation simultaneously** (DDH-based; truly compact aggregate key and signatures). Practical parameters cited: over P-384 (≥321-bit order needed for 128-bit security), ~1152-bit signature and ~1535-bit per-signer communication. This closes the open problem that the earlier tight schemes left (their approach precluded a single short aggregate key).
- **Earpicks** — *"Earpicks: Tightly Secure Two-Round Multi- and Threshold Signatures."* ePrint **2026/572**. Extends the tight, fork-free approach to cover **both** multi- and **threshold** signatures.
- **Putting Multi into Multi-Signatures** — Anja Lehmann, Cavit Özbay. *"Putting Multi into Multi-Signatures: Tight Security for Multiple Signers."* 2025; ePrint **2025/2198**. Revisits tight security specifically as the *number of signers* grows.

None of these has (yet) displaced MuSig2 in Bitcoin practice — MuSig2's AGM proof is considered adequate and its signatures are exactly BIP340 (32+32 bytes on P-256k1), which the tight schemes over P-384 cannot match. But they are the frontier on the *provable-security* axis.

### 3.13 ECDSA multisignatures and history context

Bitcoin used **ECDSA** exclusively before Taproot, and ECDSA is **non-linear** in the secret and nonce (`s = k^{-1}(H(m) + r·x)`), which makes clean key aggregation and additive multisig far harder than for Schnorr. Consequently, "multi-party ECDSA" is almost always realized as **threshold ECDSA MPC** (Lindell; Gennaro–Goldfeld GG18/GG20; Doerner–Kondi–Lee–shelat; CGGMP) rather than as a native aggregating multisignature, and it lives in the threshold-signature note. Native ECDSA *multisignatures* with key aggregation do exist in the literature (e.g., "Multi-Signatures for ECDSA and Its Applications in Blockchain," 2022) but are heavier and were never standardized for Bitcoin. **Practically, on pre-Taproot Bitcoin, "multisig" meant `OP_CHECKMULTISIG` — n independent ECDSA signatures verified in-script, with no aggregation, no privacy, and linear on-chain cost.** Schnorr's linearity is the entire reason MuSig-style aggregation became possible; this is a primary motivation Taproot cites for switching the signature scheme.

---

## 4. Comparison table

| Scheme | Year / venue | ePrint | Rounds | Key aggregation | Security model / assumption | Concurrency-safe | Bitcoin/BIP340 output | Status |
|---|---|---|---|---|---|---|---|---|
| Itakura–Nakamura | 1983 / NEC J.R.D. | — | sequential | no (size-const) | none (pre-formal) | n/a | no (RSA) | historical |
| MOR (ASM) | 2001 / CCS | — | 3 | accountable subgroup | first formal model, DL/ROM | — | no | foundational |
| Bellare–Neven (BN) | 2006 / CCS | — | 3 | **no** (key-prefixing) | plain-PK, DL, ROM | yes | Schnorr-shaped, O(n) verify | foundational |
| BCJ | 2008 / CCS | — | 2 | no | DL, ROM (homomorphic commit) | **broken concurrently** (Drijvers) | no | broken/patched |
| MuSig (v1) | 2018/2019 / DCC | 2018/068 | **3** | **yes** (`a_i=H(L,P_i)`) | plain-PK, DL, ROM | yes | yes (Schnorr) | superseded by MuSig2 |
| mBCJ | 2019 / S&P | 2018/417 | 2 | no | DL, ROM (equivocal commit) | yes | no (larger) | niche |
| MuSig-DN | 2020 / CCS | 2020/1057 | 2 | yes | AGM/ROM + NIZK; deterministic | yes | yes | research (heavy NIZK) |
| DWMS | 2020/2021 / CRYPTO | 2020/1245 | 2 | (limited) | AGM+ROM, OMDL + 2-entwined-sum | yes | yes | research (parallel to MuSig2) |
| **MuSig2** | 2020/2021 / CRYPTO | **2020/1261** | **2** (preprocessable) | **yes** (MuSig2\*) | **AGM+ROM, AOMDL** (ν=2); OMDL/ROM (ν=4) | **yes** | **yes — BIP327** | **deployed standard** |
| HBMS | 2021 / ASIACRYPT | 2021/404 | 2 | no | DL, chain reductions | yes | no | research |
| Chopsticks | 2023 / EUROCRYPT | 2023/198 | 2 | no | **tight**, DDH, fork-free | yes | no (large) | research |
| Toothpicks | 2024 / EUROCRYPT | 2023/1613 | 2 | no | tight, DDH | yes | no | research |
| T-Spoon | 2025 | 2025/840 | 2 | **yes** | **tight + KOA**, DDH | yes | P-384, not 340 | research (frontier) |
| Earpicks | 2026 | 2026/572 | 2 | (multi + threshold) | tight, fork-free | yes | — | research (frontier) |
| DualMS / Squirrel / Chipmunk | 2022–2023 | 2023/263 etc. | 2 / sync | varies | **lattice (post-quantum)** | yes | no | PQ research |

---

## 5. The ROS problem and concurrency — consolidated

The recurring villain across two-round multisig, blind signatures, and threshold Schnorr is the **linearization attack**, formalized as **ROS**:

1. **Setup for the attack.** Each signing session gives the adversary an equation `s = r + c·x` (Schnorr form). Across ℓ concurrent sessions the adversary holds ℓ equations with adversarially-influenceable nonces `r` and challenges `c`.
2. **Wagner / k-sum (Drijvers 2019).** If the adversary can make the *combined nonce* a **linear** function of its own choices (true whenever it picks its single nonce after seeing the honest nonce), it can solve a generalized birthday / k-sum problem to craft a forgeable target challenge. Cost ≈ `2^{n/(1+lg ℓ)}` — sub-exponential, and *practical* for modest ℓ.
3. **ROS (Benhamouda 2021).** Recasting this as the ROS linear-algebra problem shows it is **polynomial-time** for ℓ > log₂ p. So the two-round-DL-multisig insecurity is not merely a loose proof — there are **cheap real forgeries** once enough sessions run concurrently.
4. **The defenses.**
   - **Add a commitment round** (BN, MuSig v1): binding nonces before reveal kills the "choose nonce after seeing others" freedom → 3 rounds.
   - **Use ≥2 nonces per signer with a random-oracle combiner** (MuSig2, DWMS): the effective nonce `R = R_1 + b·R_2` with `b = H(nonces,…)` is **non-linear** in the adversary's choices, so no solvable ROS instance exists → 2 rounds, still secure. **This is why MuSig2 needs two nonces, not one.**
   - **Fork-free tight constructions** (Chopsticks/Toothpicks/T-Spoon): sidestep the lossy forking lemma entirely with lossy-identification/DDH techniques.

**Concurrency** is the crux: security under *sequential* signing is easy; security when a signer runs many overlapping sessions (unavoidable for a busy Lightning node or custody signer) is what these schemes must and do provide. MuSig2's headline theorem is exactly EUF-CMA under arbitrarily many concurrent sessions.

---

## 6. Signature aggregation for Bitcoin

Distinct from multisignatures (many keys, **one** message), **signature aggregation** combines **many independent signatures** (many keys, possibly many messages) into one shorter object. Two threads matter for Bitcoin:

### 6.1 Half-aggregation of BIP340 signatures
- **Idea:** Given n Schnorr signatures `(R_i, s_i)`, keep all the `R_i` but combine the `s_i` into a *single* scalar `s = Σ z_i·s_i` with random-oracle weights `z_i`. Result ≈ **half** the naive size (you save the n scalars but keep the n nonces).
- **History:** Proposed for **block-wide** aggregation by Tadge Dryja (bitcoin-dev, 2017); an initial security gap was spotted and fixed by **Russell O'Connor and Andrew Poelstra**. A ROM security proof reducing half-aggregation security to Schnorr unforgeability was given by **Chalkias, Garillot, Kondi, Nikolaenko**, *"Non-Interactive Half-Aggregation of EdDSA and Variants of Schnorr Signatures,"* CT-RSA 2021, ePrint **2021/350** (with tighter-reduction follow-ups in 2022).
- **Non-interactive & incremental:** A third party can aggregate *without* signer involvement, and **incremental aggregation** lets new signatures be folded into an existing half-aggregate.

### 6.2 Cross-Input Signature Aggregation (CISA)
- **Idea:** Within a single transaction, aggregate the signatures across *all inputs* into one signature — big fee savings for multi-input spends (CoinJoins, consolidations) and a **fungibility win** (CoinJoins become cheaper than non-joins, flipping today's incentive).
- **Status:** Active research/spec effort by Blockstream Research / Elements Project (repos: `BlockstreamResearch/cross-input-aggregation`, `ElementsProject/cross-input-aggregation`), analyzed by Bitcoin Optech. Two flavors: **interactive full aggregation** (one signature per tx, needs a signing protocol among inputs' owners) vs **non-interactive half-aggregation** (any relayer can compress). **Not yet a deployed BIP / consensus change** as of 2026; it would require a soft fork and interacts delicately with adaptor signatures and the txsighash structure.

### 6.3 Interactive vs non-interactive, and how it differs from MuSig2
- **MuSig2** = *interactive*, *before* signing, *many keys → one message → one key+sig* (a multisignature).
- **Half-aggregation / CISA** = *after* signing (can be non-interactive), *many independent sigs → one shorter object*; keys are **not** aggregated and the object is **not** a plain BIP340 signature (verifier still needs all keys/nonces). They are complementary, not substitutes.

---

## 7. Latest research, 2022–2026

- **Tight, fork-free two-round multisig without pairings:** Chopsticks (EUROCRYPT 2023, 2023/198) → Toothpicks (EUROCRYPT 2024, 2023/1613) → **T-Spoon** (2025/840, first to add **key aggregation** to tight security) → **Earpicks** (2026/572, unifying multi- and threshold). Plus Lehmann–Özbay "Putting Multi into Multi-Signatures" (2025/2198) and explicit **high-moment forking lemma** work tightening BN/BLS concrete security (CiC). These attack MuSig2's one soft spot — its reduction is loose (forking) and AGM-reliant.
- **Post-quantum (lattice) multisig:** **DOTT** (Damgård–Orlandi–Takahashi–Tibouchi, PKC 2021) two-round from Dilithium-G; **Squirrel** (Fleischhacker–Simkin–Zhang, CCS 2022) and **Chipmunk** (Fleischhacker–Herold–Simkin–Zhang, CCS 2023) efficient *synchronized* multisig from lattices; **DualMS** (Yanbo Chen, CRYPTO 2023, ePrint 2023/263) two-round lattice multisig with **trapdoor-free simulation**. These aim to replace the DL-based schemes if/when a CRQC threatens secp256k1 — relevant to Bitcoin's long-horizon PQ migration, not to today's consensus.
- **MuSig2 productionization and the wallet stack (2022–2026):** MuSig2 moved from `libsecp256k1-zkp` into upstream **Bitcoin Core's `libsecp256k1`** (`secp256k1` PR #1479 by Jonas Nick; released as libsecp256k1 v0.6.0, non-experimental, Oct 2024). Companion BIPs **328 / 373 / 390** (key derivation, PSBT fields, `musig()` descriptor) landed to make it wallet-usable. **Ledger** shipped MuSig2 in Bitcoin app **v2.4.0** (Apr 2025). **Lightning Labs** uses MuSig2 for Taproot channels and migrated **Loop** submarine swaps to MuSig2 to cut on-chain cost. Blockstream's stack (Green, Greenlight, statechains à la Mercury) and various Taproot-multisig wallets adopt it for the single-key privacy/fee profile. BitBox and others document MuSig2/FROST support.
- **Clarified security caveats in BIP327 itself:** the standard now explicitly flags the **open adversarial-tweak question** and the **strict single-use secnonce / no-deterministic-nonce** requirement, plus provides `DeterministicSign` for a stateless last signer.

---

## 8. Open problems

1. **Adversarial tweaks:** Does allowing an adversary to choose Taproot/BIP32 tweaks affect MuSig2 unforgeability? BIP327 leaves this open; deployments must derive tweaks honestly.
2. **Statefulness vs determinism:** MuSig2 needs good randomness and one-time secnonce state; MuSig-DN removed randomness at heavy NIZK cost. A *cheap*, verifiable deterministic-nonce multisig remains desirable, especially for HW wallets and backup-and-restore where state can be rolled back.
3. **Tight security with BIP340-native output:** T-Spoon gets tightness + key aggregation but only over larger curves (P-384). A tightly secure scheme that outputs an *exact* secp256k1/BIP340 signature is still open.
4. **Threshold on Bitcoin via the same primitives:** True t-of-n (FROST) standardization for Bitcoin (DKG, robustness, identifiable aborts) is in progress but not yet a BIP with Core support; the multisig/threshold boundary (e.g., FROST3, ROAST for robustness) is active.
5. **Cross-input aggregation deployment:** CISA needs a soft fork and careful interaction with adaptor signatures, sighash, and half-aggregation; consensus design is unsettled.
6. **Post-quantum migration:** No lattice multisig yet matches ECC compactness/latency; synchronized-vs-general and key/signature sizes are open engineering problems for any future PQ Bitcoin.

---

## 9. References

Primary papers (with links):

- Itakura, Nakamura. "A public-key cryptosystem suitable for digital multisignatures." *NEC J. Res. Dev.* 71 (1983).
- Micali, Ohta, Reyzin. "Accountable-Subgroup Multisignatures." ACM CCS 2001. https://www.cs.bu.edu/~reyzin/multisig.html · https://dl.acm.org/doi/10.1145/501983.502017
- Bellare, Neven. "Multi-Signatures in the Plain Public-Key Model and a General Forking Lemma." ACM CCS 2006. https://cseweb.ucsd.edu/~mihir/papers/multisignatures.pdf · https://dl.acm.org/doi/10.1145/1180405.1180453
- Bagherzandi, Cheon, Jarecki. "Multisignatures Secure under the Discrete Logarithm Assumption and a Generalized Forking Lemma." ACM CCS 2008. https://dl.acm.org/doi/abs/10.1145/1455770.1455827
- Maxwell, Poelstra, Seurin, Wuille. "Simple Schnorr Multi-Signatures with Applications to Bitcoin." ePrint 2018/068; *Designs, Codes and Cryptography* 87 (2019). https://eprint.iacr.org/2018/068 · https://dl.acm.org/doi/abs/10.1007/s10623-019-00608-x
- Drijvers, Edalatnejad, Ford, Kiltz, Loss, Neven, Stepanovs. "On the Security of Two-Round Multi-Signatures." IEEE S&P 2019. https://bford.info/pub/sec/two-round-multisig.pdf · https://eprint.iacr.org/2018/417
- Benhamouda, Lepoint, Loss, Orrù, Raykova. "On the (in)Security of ROS." EUROCRYPT 2021; *J. Cryptology* 35 (2022). https://link.springer.com/chapter/10.1007/978-3-030-77870-5_2 · https://link.springer.com/article/10.1007/s00145-022-09436-0
- Nick, Ruffing, Seurin, Wuille. "MuSig-DN: Schnorr Multi-Signatures with Verifiably Deterministic Nonces." ACM CCS 2020; ePrint 2020/1057. https://eprint.iacr.org/2020/1057
- Nick, Ruffing, Seurin. "MuSig2: Simple Two-Round Schnorr Multi-Signatures." CRYPTO 2021; ePrint 2020/1261 (rev. Oct 2023). https://eprint.iacr.org/2020/1261 · https://link.springer.com/chapter/10.1007/978-3-030-84242-0_8
- Alper, Burdges. "Two-Round Trip Schnorr Multi-Signatures via Delinearized Witnesses (DWMS)." CRYPTO 2021; ePrint 2020/1245. https://eprint.iacr.org/2020/1245
- Bellare, Dai. "Chain Reductions for Multi-Signatures and the HBMS Scheme." ASIACRYPT 2021; ePrint 2021/404. https://eprint.iacr.org/2021/404
- Pan, Wagner. "Chopsticks: Fork-Free Two-Round Multi-Signatures from Non-Interactive Assumptions." EUROCRYPT 2023; ePrint 2023/198. https://eprint.iacr.org/2023/198
- Pan, Wagner. "Toothpicks: More Efficient Fork-Free Two-Round Multi-Signatures." EUROCRYPT 2024; ePrint 2023/1613. https://eprint.iacr.org/2023/1613
- Bacho, Wagner. "T-Spoon: Tightly Secure Two-Round Multi-Signatures with Key Aggregation." 2025; ePrint 2025/840. https://eprint.iacr.org/2025/840
- "Earpicks: Tightly Secure Two-Round Multi- and Threshold Signatures." 2026; ePrint 2026/572. https://eprint.iacr.org/2026/572
- Lehmann, Özbay. "Putting Multi into Multi-Signatures: Tight Security for Multiple Signers." 2025; ePrint 2025/2198. https://eprint.iacr.org/2025/2198
- Chalkias, Garillot, Kondi, Nikolaenko. "Non-Interactive Half-Aggregation of EdDSA and Variants of Schnorr Signatures." CT-RSA 2021; ePrint 2021/350. https://eprint.iacr.org/2021/350
- Chen (Yanbo). "DualMS: Efficient Lattice-Based Two-Round Multi-Signature with Trapdoor-Free Simulation." CRYPTO 2023; ePrint 2023/263. https://eprint.iacr.org/2023/263
- Fleischhacker, Simkin, Zhang. "Squirrel: Efficient Synchronized Multi-Signatures from Lattices." ACM CCS 2022.
- Fleischhacker, Herold, Simkin, Zhang. "Chipmunk: Better Synchronized Multi-Signatures from Lattices." ACM CCS 2023.
- Damgård, Orlandi, Takahashi, Tibouchi. "Two-Round n-out-of-n and Multi-Signatures and Trapdoor Commitment from Lattices (DOTT)." PKC 2021.

Standards / specifications:

- BIP327 — MuSig2 for BIP340-compatible Multi-Signatures. https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki · https://bips.dev/327/
- BIP340 — Schnorr Signatures for secp256k1. https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki
- BIP341 — Taproot: SegWit v1 spending rules. https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki
- BIP342 — Validation of Taproot Scripts (Tapscript, `OP_CHECKSIGADD`). https://github.com/bitcoin/bips/blob/master/bip-0342.mediawiki
- BIPs 328 / 373 / 390 — MuSig2 key derivation, PSBT fields, `musig()` descriptor.

Implementations / deployment:

- `bitcoin-core/secp256k1` PR #1479 (musig module). https://github.com/bitcoin-core/secp256k1/pull/1479
- `BlockstreamResearch/secp256k1-zkp` (musig module, header `secp256k1_musig.h`). https://github.com/BlockstreamResearch/secp256k1-zkp
- Cross-input aggregation research. https://github.com/BlockstreamResearch/cross-input-aggregation · https://github.com/ElementsProject/cross-input-aggregation
- Half-aggregation of BIP340 sigs (Blockstream). https://blog.blockstream.com/half-aggregation-of-bip-340-signatures/
- Cross-input signature aggregation & MuSig topics (Bitcoin Optech). https://bitcoinops.org/en/topics/cross-input-signature-aggregation/ · https://bitcoinops.org/en/topics/musig/
- Ledger Bitcoin app v2.4.0 MuSig2. https://www.ledger.com/blog-musig2-ledger-bitcoin-app
- Lightning Labs — Taproot + MuSig2, Loop MuSig2. https://lightning.engineering/posts/2023-04-19-taproot-musig2-recap/ · https://lightning.engineering/posts/2025-02-13-loop-musig2/
- MuSig2 vs FROST (Blockchain Commons / BitBox). https://learningfrost.blockchaincommons.com/01_3_FROST_vs_MuSig/ · https://blog.bitbox.swiss/en/musig2-and-frost-explaining-multisignature-schemes-on-taproot/
