# Distributed Key Generation (DKG), Verifiable Secret Sharing (VSS), and Proactive Refresh

**Scope.** This document is a deep technical reference on the primitives that let a set of parties *create* and *maintain* a shared secret key with no trusted dealer — the mandatory setup and lifecycle machinery beneath every threshold signature scheme used for Bitcoin (threshold ECDSA; FROST/Schnorr for BIP340). It covers secret sharing, verifiable secret sharing (VSS), publicly-verifiable secret sharing (PVSS), synchronous and asynchronous DKG, and proactive / dynamic-committee refresh, with an emphasis on discrete-log constructions over **secp256k1** (no pairings) and their Bitcoin applicability.

**Convention used throughout.** We work in a cyclic group `G` of prime order `q` with generator `g` where the discrete-log problem is hard (instantiated as secp256k1). The shared secret is `x ∈ Z_q`; the public key is `X = xG` (additive) or `y = g^x` (multiplicative — both notations appear in the literature and are used interchangeably here). A `t`-of-`n` scheme means any `t` parties can sign/reconstruct and any `t-1` learn nothing. Papers differ on whether `t` is the *reconstruction* threshold or the *corruption* bound; each entry states its convention.

---

## 1. Overview and role as a building block

A threshold signature is only as trustworthy as the way its key was born. Two setup models exist:

- **Trusted dealer.** One party samples `x`, computes a Shamir sharing, hands share `x_i` to party `i`, publishes `X = xG`, and erases `x`. Simple and few rounds, but the dealer transiently holds the whole secret — a single point of compromise at setup.
- **Distributed Key Generation (DKG).** The parties *jointly* produce `(X, x_1, …, x_n)` such that `X = xG`, each honest party holds a Shamir share `x_i = f(i)` of an implicitly-defined `f` with `f(0) = x`, and **no party (or coalition below threshold) ever learns `x`**. There is no moment at which the full key exists in one place.

A DKG must satisfy three properties:

- **Correctness.** All honest parties agree on the same public key `X`; the shares held by honest parties are a consistent `t`-of-`n` Shamir sharing of the `x` with `X = xG`; any `t` honest shares reconstruct `x`.
- **Secrecy.** An adversary controlling up to the corruption bound learns nothing about `x` beyond `X` — formally, the protocol view is simulatable given only `X` (so `x` is as good as uniformly random even to the adversary).
- **Robustness (optional).** The protocol completes and outputs a usable key even if up to the corruption bound of parties deviate arbitrarily. Non-robust ("with abort") DKGs instead detect misbehavior and halt, optionally identifying a culprit (identifiable abort).

DKG is the foundational layer; VSS is the sub-primitive that makes it possible (each party deals its contribution *verifiably* so cheaters are caught), and proactive refresh is the lifecycle layer that keeps the key alive against an adversary that compromises parties gradually over time.

---

## 2. Security and network models (read this first)

These axes determine which scheme is appropriate and appear in every entry below.

- **Adversary corruption model.**
  - *Static:* the adversary fixes its corrupted set before the protocol starts.
  - *Adaptive:* the adversary corrupts parties during execution, based on the transcript it has seen. Much harder to prove secure because the simulator must later "explain" a newly-corrupted party's internal state consistently with everything already broadcast.
  - *Mobile / proactive:* the adversary moves between parties over time, corrupting a different subset each epoch (§7). The defense is periodic share refresh.
- **Rushing adversary.** One that speaks *last* in a round, after seeing honest parties' messages. Rushing is what enables the GJKR key-biasing attack (§4.2).
- **Network model.**
  - *Synchronous:* known bound on message delay; a party can time out a silent peer and disqualify it. Enables honest-majority thresholds `t < n/2` (`n ≥ 2t+1`).
  - *Asynchronous:* no delay bound; "slow" and "faulty" are indistinguishable, so a party can only ever wait for `n − t` messages. Forces `n ≥ 3t+1` (`t < n/3`) because the agreement layer (reliable broadcast, Byzantine agreement) requires it (§6).
  - *Partially synchronous / network-agnostic:* somewhere in between; some 2023 schemes tolerate both with a graceful threshold degradation.
- **Communication assumptions.** Broadcast channel vs. authenticated point-to-point vs. confidential (encrypted) point-to-point vs. a public bulletin board / ledger. A recurring theme in modern DKG is *reducing* the assumption from "reliable broadcast" to "authenticated channel + an in-band equality/agreement check" (ChillDKG, §8).
- **Setup.** PKI (each party has a known public key), a common reference / structured reference string (trusted powers-of-tau, needed by KZG/pairing schemes), or nothing beyond the agreed participant set.

---

## 3. Verifiable secret sharing primitives

### 3.1 Shamir Secret Sharing (1979)

**Adi Shamir, "How to Share a Secret," Communications of the ACM 22(11):612–613, 1979.** DOI [10.1145/359168.359176](https://dl.acm.org/doi/10.1145/359168.359176). (Predates the ePrint archive.)

A dealer with secret `s ∈ Z_q` samples a random degree-`(t−1)` polynomial `f(x) = s + a_1 x + … + a_{t−1} x^{t−1}` and gives party `i` the share `s_i = f(i)`. Any `t` parties reconstruct via Lagrange interpolation at `x=0`: `s = Σ_{i∈S} λ_i s_i` with `λ_i = Π_{j∈S,j≠i} j/(j−i)`. Any `t−1` shares reveal *information-theoretically nothing* — every candidate secret remains equally likely. This gives perfect secrecy and a clean threshold, but assumes an **honest dealer** and honest holders: a recipient cannot tell whether the share it received is consistent with everyone else's. Fixing that gap is exactly what VSS adds.

### 3.2 Feldman VSS (1987)

**Paul Feldman, "A Practical Scheme for Non-interactive Verifiable Secret Sharing," FOCS 1987, pp. 427–438.** DOI [10.1109/SFCS.1987.4](https://dl.acm.org/doi/10.1109/SFCS.1987.4). PDF: [cs.umd.edu](https://www.cs.umd.edu/~gasarch/TOPICS/secretsharing/feldmanVSS.pdf).

Feldman layers verifiability onto Shamir at essentially no extra interaction. The dealer broadcasts commitments to each polynomial coefficient: `C_k = g^{a_k}` for `k = 0…t−1`, so `C_0 = g^s`. Party `i` checks its share by testing

```
g^{s_i} =?= Π_{k=0}^{t−1} C_k^{(i^k)}
```

If the check fails the party broadcasts a **complaint**. The commitment is **computationally binding** under discrete-log but only **computationally hiding**, and — crucially — it *exposes* `C_0 = g^s`. For a DKG this leakage is a feature: `g^s` is exactly the public key you want. Feldman VSS is therefore the workhorse for DL-based threshold public keys.

### 3.3 Pedersen VSS (1991)

**Torben Pryds Pedersen, "Non-Interactive and Information-Theoretic Secure Verifiable Secret Sharing," CRYPTO '91, LNCS 576, pp. 129–140.** [Springer](https://link.springer.com/chapter/10.1007/3-540-46766-1_9) · [PDF](https://www.cs.cornell.edu/courses/cs754/2001fa/129.PDF).

Pedersen uses two generators `g, h` with `log_g h` unknown to everyone, and commits to each coefficient with a **Pedersen commitment** `C_k = g^{a_k} h^{b_k}`, where a second random polynomial `f'` supplies the blinding coefficients `b_k`. Each party gets two shares `s_i = f(i)`, `s'_i = f'(i)` and verifies `g^{s_i} h^{s'_i} = Π_k C_k^{(i^k)}`. Because `g^a h^b` is a *perfectly hiding* commitment, Pedersen VSS is **unconditionally (information-theoretically) hiding** and only **computationally binding** — the exact dual of Feldman. The trade-off: Feldman reveals `g^s` (needed for a public key) but hides `s` only computationally; Pedersen hides `s` unconditionally but by itself never exposes `g^s`. This complementarity is precisely the lever the GJKR fix pulls (§4.3).

### 3.4 Polynomial commitments: KZG and eVSS (pairing-based, not secp256k1)

**Aniket Kate, Gregory M. Zaverucha, Ian Goldberg, "Constant-Size Commitments to Polynomials and Their Applications," ASIACRYPT 2010, LNCS 6477, pp. 177–194.** [Springer](https://link.springer.com/chapter/10.1007/978-3-642-17373-8_11) · full version [CACR 2010-10](https://cacr.uwaterloo.ca/techreports/2010/cacr2010-10.pdf).

KZG commits to a degree-`t` polynomial `φ` as a **single group element** `C = g^{φ(τ)}`, where `τ` is a secret trapdoor from a structured reference string (a trusted powers-of-tau setup). An evaluation `φ(i)=y` is proven with a *single* group-element witness `w = g^{ψ(τ)}`, `ψ(X)=(φ(X)−y)/(X−i)`, checked by a pairing equation. Commitment and proof are **constant size** regardless of degree, under the `t`-Strong Diffie–Hellman assumption. The flagship application **eVSS** collapses Feldman/Pedersen's `O(t)` per-party broadcast/verification to `O(1)`. **Caveat: KZG/eVSS require a pairing and a trusted SRS; secp256k1 has no efficient pairing, so these are not directly usable for Bitcoin keys** — but the design (commit once, constant-size openings) reappears in CHURP and pairing-based DKGs. Bivariate-polynomial commitments in this style underpin high-threshold AVSS (§6) and CHURP (§7.4).

---

## 4. DKG protocols — synchronous, discrete-log

### 4.1 Pedersen DKG (1991)

**Torben Pryds Pedersen, "A Threshold Cryptosystem without a Trusted Party," EUROCRYPT '91, LNCS 547, pp. 522–526.** [Springer](https://link.springer.com/chapter/10.1007/3-540-46416-6_47).

The canonical dealerless template: every party `i` acts as a dealer of its own random secret `z_i` via **Feldman VSS**, publishing `g^{z_i}` and coefficient commitments and sending private shares `s_{ij}` to each `j`. Misbehaving dealers are disqualified via a complaint round, leaving a **qualified set QUAL**. The group secret is `x = Σ_{i∈QUAL} z_i`, each party's final share is `x_j = Σ_i s_{ij}`, and the public key is `y = g^x = Π_i g^{z_i}`. This "`n` parallel Feldman VSS, then sum" structure is the ancestor of nearly every DL-based DKG (including FROST's). Its flaw, found eight years later, is that the parallel-Feldman-plus-complaints structure lets a rushing adversary *bias* `x`.

### 4.2 GJKR: the key-biasing attack and the secure DKG (1999 / 2007)

**Rosario Gennaro, Stanisław Jarecki, Hugo Krawczyk, Tal Rabin, "Secure Distributed Key Generation for Discrete-Log Based Cryptosystems."** EUROCRYPT '99, LNCS 1592, pp. 295–310 ([Springer](https://link.springer.com/chapter/10.1007/3-540-48910-X_21)); journal version *Journal of Cryptology* 20(1):51–83, 2007, DOI [10.1007/s00145-006-0347-3](https://link.springer.com/article/10.1007/s00145-006-0347-3) ([PDF](https://link.springer.com/content/pdf/10.1007/s00145-006-0347-3.pdf)). (No ePrint number; cite the DOI.)

**The attack.** The public key is `y = Π_{i∈QUAL} g^{z_i}`. A **rushing adversary** — one that speaks last, after seeing honest parties' broadcasts — can **bias the distribution of `y` away from uniform**. The mechanism: the adversary controls whether each of its corrupted parties ends up in `QUAL`. It sends a *valid* share to some honest parties and an *invalid* share to others, deliberately triggering complaints it can choose to answer or ignore. Having already observed the honest dealers' contributions (hence the emerging value of `y`), it then decides which of its own dealings to complete or sacrifice, steering `y` toward values with a chosen property (e.g. fixing bits of `y`). The generated key is *not* uniformly random, which breaks the simulation-based security proofs of the threshold schemes built on top.

**The fix — "New-DKG" (commit-then-reveal).** Run the sharing in **two phases** so the secret is committed *before* any information about `g^x` leaks:

1. **Phase 1 — Pedersen-VSS commit.** Every party shares `z_i` using **Pedersen VSS** (`g^{a_k} h^{b_k}`), which is *unconditionally hiding*, so nothing about `g^{z_i}` leaks. Parties verify, complain, and disqualify. This *locks in* `QUAL` and the value `x = Σ z_i` while the adversary has **zero information about `g^x`** — removing its ability to correlate its choices with `y`.
2. **Phase 2 — Feldman expose.** Each `QUAL` party broadcasts the Feldman commitments `g^{a_k}` to its already-committed polynomial. These are checked against the Phase-1 Pedersen commitments; a dealer failing the check is reconstructed in the open. The public key `y = g^x` is then computed from the exposed `g^{z_i}`.

Because `QUAL` and `x` are fixed during the unconditionally-hiding phase, Phase-2 choices cannot shift the distribution: `x` is uniformly random. **Model:** static adversary corrupting up to `t`, **synchronous** with broadcast, honest majority `t < n/2` (`n ≥ 2t+1`), reconstruction threshold `t+1`. This is the reference *secure, robust* synchronous DKG.

### 4.3 When is the bias tolerable? (Revisiting GJKR, 2003)

**Gennaro, Jarecki, Krawczyk, Rabin, "Revisiting the Distributed Key Generation for Discrete-Log Based Cryptosystems," CT-RSA 2003.** [Springer](https://link.springer.com/chapter/10.1007/3-540-36563-X_26).

The same authors later showed that for many DL applications — notably **threshold Schnorr** and certain DSS/ElGamal variants — the *cheaper, single-phase* Pedersen/Feldman DKG **remains secure despite the bias**: the security reduction still goes through because the adversary's limited biasing power does not help it forge. This is why modern threshold-Schnorr / FROST deployments accept the simpler (biasable) DKG. The provably-*uniform* GJKR New-DKG is reserved for applications whose proofs need a uniform key. (The same "biasing the DKG doesn't help forgery" observation is the crux of the FROST/Olaf security analysis, §8.2.)

### 4.4 Adaptive security: CGJKR (1999)

**Ran Canetti, Rosario Gennaro, Stanisław Jarecki, Hugo Krawczyk, Tal Rabin, "Adaptive Security for Threshold Cryptosystems," CRYPTO '99, LNCS 1666, pp. 98–116.** [Springer](https://link.springer.com/chapter/10.1007/3-540-48405-1_7) · [PDF](https://link.springer.com/content/pdf/10.1007/3-540-48405-1_7.pdf).

GJKR proves security only against a *static* adversary. CGJKR strengthens the model to an **adaptive** adversary that may corrupt parties at any point during execution, based on the transcript — the realistic threat for long-lived keys. The obstacle is simulation: on an adaptive corruption the simulator must "explain" the corrupted party's shares and randomness consistently with everything already broadcast. Their solution combines the two-phase DKG with additional commitments and a **"single inconsistent player" / rewinding-free simulation** technique so the simulator can equivocate on exactly one party's state. Same network assumptions (synchronous, broadcast) and honest-majority threshold `t < n/2`, but the adversary is adaptive. Adaptive security remains an active research frontier (see §10 — Twinkle, Sparkle, GRandLine, Any-Trust DKG).

---

## 5. Publicly Verifiable Secret Sharing (PVSS) and PVSS-based DKG

**Idea.** In VSS only the *recipient* of a share can verify it (over a private channel), which forces complaint/dispute rounds when a share is bad. In **PVSS** the dealer posts *encrypted* shares plus non-interactive proofs of correctness to a public transcript, so **anyone** can verify consistency from public data alone. A malformed contribution is simply rejected by inspection — there is no dispute to adjudicate, no interactive defense, no consensus round on who misbehaved. This makes PVSS the natural engine for DKG over a bulletin board / blockchain and for handing secret state between committees (§7).

### 5.1 Stadler (1996) — the concept

**Markus Stadler, "Publicly Verifiable Secret Sharing," EUROCRYPT 1996, LNCS 1070, pp. 190–199.** [Springer](https://link.springer.com/chapter/10.1007/3-540-68339-9_17). Coined PVSS; ElGamal-based, general access structures, introduced double-discrete-log and root-of-DL proof techniques. Discrete-log based.

### 5.2 Schoenmakers (1999) — the workhorse

**Berry Schoenmakers, "A Simple Publicly Verifiable Secret Sharing Scheme and Its Application to Electronic Voting," CRYPTO 1999, LNCS 1666, pp. 148–164.** [Springer](https://link.springer.com/chapter/10.1007/3-540-48405-1_10).

The dealer picks `φ` of degree `t`, publishes Feldman commitments `C_j = g^{a_j}`, encrypts share `i` under party `i`'s public key `y_i` as `Y_i = y_i^{φ(i)}`, and attaches a **Chaum–Pedersen DLEQ proof** that `log_g(C-derived value) = log_{y_i}(Y_i)` — i.e. that the encrypted share matches the committed polynomial. Any observer verifies all proofs; reconstruction reveals `g^{φ(0)}` via decryption + DLEQ proofs and Lagrange-in-the-exponent. Runs in `O(nk)` under DDH. **Discrete-log based — directly relevant to secp256k1.** Its headline use is universally-verifiable e-voting.

### 5.3 SCRAPE (2017) — cheap verification via coding theory

**Ignacio Cascudo, Bernardo David, "SCRAPE: Scalable Randomness Attested by Public Entities," ACNS 2017. ePrint [2017/216](https://eprint.iacr.org/2017/216).**

Prior PVSS verification cost `O(n·t)` exponentiations per check. SCRAPE observes that a vector of shares lies on a Reed–Solomon codeword iff it is orthogonal to the dual code, so a verifier samples a random dual codeword `c⊥` and checks a *single* aggregate relation `Π (commitment_i)^{c⊥_i} = 1` — **`O(n)` verification** via a linear-algebra test instead of per-share proofs. Comes in a **DLog (Schoenmakers-style, secp256k1-relevant)** variant and a pairing variant. It builds a scalable randomness beacon with guaranteed output under honest majority. The dual-code check is the key trick reused by Aggregatable DKG.

### 5.4 ALBATROSS (2020) — batched randomness

**Ignacio Cascudo, Bernardo David, "ALBATROSS: publicly AttestabLe BATched Randomness based On Secret Sharing," ASIACRYPT 2020 (not CRYPTO). ePrint [2020/644](https://eprint.iacr.org/2020/644).** Amortizes SCRAPE to produce *many* random values per run using **packed Shamir** (many secrets per polynomial) plus FFT-based linear randomness extraction, trading corruption tolerance for amortized cost. DLog and pairing variants; DLog is secp256k1-relevant.

### 5.5 YOSO and YOLO YOSO — evolving committees

**Craig Gentry, Shai Halevi, Hugo Krawczyk, Bernardo Magri, Jesper Buus Nielsen, Tal Rabin, Sophia Yakoubov, "YOSO: You Only Speak Once — Secure MPC with Stateless Ephemeral Roles," CRYPTO 2021. ePrint [2021/210](https://eprint.iacr.org/2021/210).** Formalizes MPC where each *role* is executed by an anonymous, randomly-selected, single-message committee that then erases its state, so an adaptive adversary cannot target a party before it speaks. Separates **role assignment** from **execution**; PVSS is the natural tool for handing secret state to the next committee.

**Ignacio Cascudo, Bernardo David, Lydia Garms, Anders Konring, "YOLO YOSO: Fast and Simple Encryption and Secret Sharing in the YOSO Model," ASIACRYPT 2022. ePrint [2022/242](https://eprint.iacr.org/2022/242).** Provides the concretely-efficient PVSS (re-)sharing and encryption-toward-a-future-anonymous-committee machinery YOSO needs. Discrete-log based. A class-group follow-up is *PVSS over Class Groups and Applications to DKG and YOSO*, ePrint [2023/1651](https://eprint.iacr.org/2023/1651).

### 5.6 Aggregatable DKG (2021)

**Kobi Gurkan, Philipp Jovanovic, Mary Maller, Sarah Meiklejohn, Gilad Stern, Alin Tomescu, "Aggregatable Distributed Key Generation," EUROCRYPT 2021, LNCS 12696, pp. 147–176. ePrint [2021/005](https://eprint.iacr.org/2021/005).**

Each party is a PVSS dealer publishing a **publicly verifiable transcript** built from parallel SCRAPE instances (pairing-based; SCRAPE dual-code validity check). The decisive property is **aggregatability**: two valid transcripts can be *summed* into one still-valid transcript, so parties **gossip and merge** contributions rather than doing all-to-all broadcast, and public verifiability means there are **no complaint/dispute rounds**. Final transcript size and verification drop from `O(n²)` to `O(n log n)`. **Important caveat:** because aggregation is homomorphic *in the exponent*, the reconstructed secret key is a **group element `g^s`, not a field element `s`** — incompatible with plain BLS/Schnorr, so the authors build a custom group-element VUF/threshold signature instead. **Pairing-based (not directly secp256k1)**, but the pattern — publicly-verifiable + aggregatable transcripts to eliminate complaints — is what a discrete-log Bitcoin DKG emulates.

### 5.7 Groth NIDKG and resharing (2021)

**Jens Groth, "Non-interactive distributed key generation and key resharing," ePrint [2021/339](https://eprint.iacr.org/2021/339).**

The DKG deployed on the Internet Computer / DFINITY for **threshold BLS**. A dealer builds a *non-interactive* PVSS of a field element and distributes shares confidentially yet verifiably, using a new **pairing-based CCA-secure encryption scheme with forward secrecy** plus NIZKs of correct sharing. Because dealings are publicly verifiable and posted (no interactive complaint round), it composes cleanly over a consensus/asynchronous layer. It adds **non-interactive resharing**: a changed set of holders re-creates a sharing of the *same* secret while preserving the public key — proactive security against a mobile adversary. Pairing-specialized (BLS), not curve-agnostic.

---

## 6. Asynchronous DKG (ADKG)

### 6.1 Why ADKG is hard

Classic DKG assumes a synchronous network so it can time out and disqualify silent dealers. In the **asynchronous** model there is no delay bound, so "slow" and "faulty" are indistinguishable and a party can never safely wait for more than `n − t` contributions. Two consequences:

- **Agreement on QUAL is mandatory.** Honest parties each see different `n − t` subsets, so they must run a **Byzantine agreement** (Asynchronous Common Subset / MVBA) to converge on a common qualified set — otherwise they derive inconsistent public keys.
- **`n ≥ 3t+1` (`t < n/3`) is forced.** Asynchronous reliable broadcast, Byzantine agreement, and verifiable secret sharing are all impossible below `3t+1`; ADKG inherits this optimal-resilience bound.

ADKG composes three asynchronous primitives: **AVSS** (asynchronous VSS, so a dealt secret is recoverable even under asynchrony), **ACS/MVBA** (agree on QUAL), and secret **aggregation**. Reconstruction thresholds above `t+1` additionally need **high-threshold AVSS**, so a party who missed a dealing can still recover its share with help from `f+1` honest peers.

### 6.2 First ADKG — Kokoris-Kogias, Malkhi, Spiegelman (CCS 2020)

**"Asynchronous Distributed Key Generation for Computationally-Secure Randomness, Consensus, and Threshold Signatures," ACM CCS 2020. ePrint [2019/1015](https://eprint.iacr.org/2019/1015)** (note: 2019/1015, *not* 2021/1015). The first fully asynchronous DKG and the first to produce a **dual `(f, 2f+1)`-threshold** key. Introduces a **High-threshold AVSS** using asymmetric bivariate polynomials (reconstruction threshold `f+1 ≤ k ≤ 2f+1`) and an *Eventually Perfect Common Coin* to drive agreement. Optimal resilience `n ≥ 3f+1`, cost `O(n⁴)` words, `O(f)` expected rounds.

### 6.3 Practical ADKG — Das et al. (IEEE S&P 2022)

**Sourav Das, Thomas Yurek, Zhuolun Xiang, Andrew Miller, Lefteris Kokoris-Kogias, Ling Ren, "Practical Asynchronous Distributed Key Generation," IEEE S&P 2022. ePrint [2021/1591](https://eprint.iacr.org/2021/1591).** The concretely-efficient reference ADKG, at optimal resilience `t < n/3`, whose secret is a **field element** — directly compatible with off-the-shelf threshold Schnorr / ECDSA / BLS over discrete-log groups (secp256k1 included). Each party shares a random secret via a PVSS/`hbACSS`-based AVSS; agreement **reduces to a single ACS instance** to fix QUAL; agreed contributions are aggregated. Expected communication `O(κn³)` — a cubic improvement over the earlier `O(n⁴)`.

### 6.4 Reaching consensus for ADKG — Abraham et al. (PODC 2021)

**Ittai Abraham, Philipp Jovanovic, Mary Maller, Sarah Meiklejohn, Gilad Stern, Alin Tomescu, "Reaching Consensus for Asynchronous Distributed Key Generation," ACM PODC 2021. ePrint [2021/1015](https://eprint.iacr.org/2021/1015).** Optimally-resilient (`f < n/3`), **high-threshold**, **constant expected rounds** with `Õ(n³)` expected communication, assuming only a PKI. Core new primitive is a **Proposal Election** (built on a *gather* primitive) that lets parties retrospectively agree on a valid proposal; composed with an **aggregatable PVSS** so elected proposals combine into one verifiable key. Both eventual and perfect variants. (Journal version: *Distributed Computing*, [10.1007/s00446-022-00436-8](https://doi.org/10.1007/s00446-022-00436-8).)

### 6.5 High-threshold ADKG — Das, Xiang, Kokoris-Kogias, Ren (USENIX Security 2023)

**"Practical Asynchronous High-threshold Distributed Key Generation and Distributed Polynomial Sampling," USENIX Security 2023. ePrint [2022/1389](https://eprint.iacr.org/2022/1389).** Targets reconstruction thresholds **larger than `n/3`**: among `n = 3t+1` nodes tolerating `t` malicious, supports any reconstruction threshold `ℓ ≥ t` (up to `n − t`), at `O(κn³)` communication, assuming only the **hardness of discrete log** (no pairings, no trusted setup). Generalizes to distributed sampling of a random polynomial.

### 6.6 Weaker assumptions / standard model / network-agnostic

- **Zhang, Duan, Liu, Zhao, Meng, Liu, Yu, Zhang, Zhu, "Practical Asynchronous Distributed Key Generation: Improved Efficiency, Weaker Assumption, and Standard Model," DSN 2023. ePrint [2022/1678](https://eprint.iacr.org/2022/1678).** Removes the random-oracle reliance of Das et al. and improves efficiency.
- **Bacho, Collins, Liu-Zhang, Loss, "Network-Agnostic Security Comes (Almost) for Free in DKG and MPC," CRYPTO 2023. ePrint [2022/1369](https://eprint.iacr.org/2022/1369).** A single DKG secure under `t_s < n/2` if the network happens to be synchronous *and* `t_a < n/3` if asynchronous.

### 6.7 Batched and communication-optimal ADKG (2023–2026)

- **Jens Groth, Victor Shoup, "Fast Batched Asynchronous Distributed Key Generation," EUROCRYPT 2024. ePrint [2023/1175](https://eprint.iacr.org/2023/1175).** Produces *many* secret shares at amortized low cost — the DKG substrate for the Groth–Shoup asynchronous **threshold-ECDSA signing service** (ePrint [2022/506](https://eprint.iacr.org/2022/506)). Robust, optimal-resilience asynchronous.
- **Abraham, Bacho, Loss, Stern, "Nearly Quadratic Asynchronous DKG from Recursive Consensus," ePrint [2025/006](https://eprint.iacr.org/2025/006)** and **Abraham, Bacho, Stern, "Quadratic Asynchronous DKG from Plain Setup," ePrint [2026/1159](https://eprint.iacr.org/2026/1159)** push ADKG to `O(n²)` communication *and* `O(1)` rounds under only a plain (constant-size public key) setup — the current communication-optimality endpoint for robust ADKG.
- **Feng, Tang, "Asymptotically Optimal Adaptive Asynchronous Common Coin and DKG with Silent Setup," CRYPTO 2025. ePrint [2024/2098](https://eprint.iacr.org/2024/2098).** Optimal-resilient, **adaptively secure** async common coin at `O(λn²)` / `O(1)` rounds from a **public silent setup** (each party posts one short key once), immediately implying quadratic constant-round async DKG.

**secp256k1 note.** ADKG constructions 6.2–6.6 produce a **discrete-log field-element** key (`x` with `X = xG`), exactly the format threshold ECDSA/Schnorr consume, so they instantiate over secp256k1 for the *final key* even where their dealing-verification layer uses pairing-friendly groups for NIZKs.

---

## 7. Proactive security, refresh, and dynamic committees

**Motivation.** A key that lives for years faces "creeping compromise": an attacker who breaks into fewer than `t` parties *this month* and a different `< t` *next month* could, over time, accumulate `≥ t` shares of the *same* secret. Proactive schemes defeat this by periodically **refreshing** the shares — same public key `X`, brand-new randomized shares — so shares from different epochs cannot be combined.

### 7.1 Mobile adversary — Ostrovsky & Yung (1991)

**Rafail Ostrovsky, Moti Yung, "How to Withstand Mobile Virus Attacks," PODC 1991, pp. 51–59.** DOI [10.1145/112600.112605](https://dl.acm.org/doi/10.1145/112600.112605). Origin of the **mobile (proactive) adversary**: rather than a fixed corruption set for all time, the adversary roams from party to party like a virus, constrained by a *rate* — it controls `< t` parties in any window even if it eventually touches all of them. Defense: periodic rebooting/refresh so state learned in one period is useless in the next. (Modern revisit: Eldefrawy et al., ePrint [2013/529](https://eprint.iacr.org/2013/529), PODC 2014.)

### 7.2 Proactive Secret Sharing — Herzberg, Jarecki, Krawczyk, Yung (1995)

**"Proactive Secret Sharing Or: How to Cope With Perpetual Leakage," CRYPTO 1995, LNCS 963, pp. 339–352.** [Springer](https://link.springer.com/chapter/10.1007/3-540-44750-4_27). The canonical PSS mechanics. Time is divided into **epochs**; between epochs the holders run **share renewal**: each party deals a random polynomial with **constant term zero** (a verifiable sharing of 0), and the sum of these "shares of zero" is added into the existing shares. This produces a fresh sharing of the *same* secret on a new random polynomial, so a share captured in epoch `e` cannot be combined with one from epoch `e+1`. Adds **share recovery** (honest parties reconstruct a lost/corrupted party's current-epoch share without exposing the secret) and Feldman-VSS consistency checks to detect faulty contributions. Tolerates a mobile adversary corrupting `< t` of `n` per epoch under honest majority. **Threshold and membership are fixed across epochs** — only the share randomness changes.

### 7.3 General principle for threshold keys

The invariant across all proactive threshold schemes: **`X` stays constant, shares are replaced each epoch.** Security is against an adversary corrupting `< t` per epoch but possibly `> t` across the timeline; since epoch shares differ by a sharing of zero (independent polynomials), cross-epoch knowledge cannot be stitched into `t` consistent shares. Refresh never reconstructs the secret and never changes the on-chain public key/address — ideal for custody keys and validator keys.

### 7.4 CHURP — dynamic committees (CCS 2019)

**Sai Krishna Deepak Maram, Fan Zhang, Lun Wang, Andrew Low, Yupeng Zhang, Ari Juels, Dawn Song, "CHURP: Dynamic-Committee Proactive Secret Sharing," ACM CCS 2019, pp. 2369–2386. ePrint [2019/017](https://eprint.iacr.org/2019/017).** ("CHUrn-Robust Proactive secret sharing.")

Extends PSS to **dynamic committees**: at each epoch the *membership can churn* and the *threshold can change* — an old committee `(n, t)` hands the secret to a new, possibly disjoint committee `(n', t')`. Headline result: **optimal `O(n)` per-node communication** on the optimistic (no-fault) path, with a more expensive dispute/pessimistic path invoked only on detected misbehavior — well suited to a blockchain that can adjudicate accusations on-chain. Technical engine: the secret is held on an **asymmetric bivariate polynomial**, and CHURP uses **dimension-switching** to reduce communication and to change the threshold, plus a KZG-style polynomial commitment hedged against setup failure. The **handoff** has old members generate reduce/full-share data for new members; on-chain storage of commitments drives dispute resolution while bulk `O(n²)` traffic stays off-chain in the optimistic case. Reference design for rotating a threshold key across a changing validator set.

### 7.5 Share redistribution / committee handoff

- **Yvo Desmedt, Sushil Jajodia, "Redistributing Secret Shares to New Access Structures and Its Applications," Tech. report ISSE-TR-97-01, George Mason Univ., 1997.** Old `(m,n)` committee converts its sharing into a fresh `(m',n')` sharing for a new committee: each old holder sub-shares its share to new members, who Lagrange-combine — changing membership *and* threshold without reconstructing the secret.
- **Theodore M. Wong, Chenxi Wang, Jeannette M. Wing, "Verifiable Secret Redistribution for Threshold Sharing Schemes," CMU-CS-02-114 / IEEE SISW 2002.** [PDF](https://www.cs.cmu.edu/~wing/publications/Wong-Wing02b.pdf). Adds Feldman-VSS verifiability to Desmedt–Jajodia, and fixes a subtlety: a naïve combination does not let new holders verify that redistributed shares reconstruct the *same* original secret (the NEW-SHARES-VALID / OLD-SHARES-VALID conditions). The conceptual ancestor of CHURP's handoff.

### 7.5b Dynamic committees, 2022–2026

- **Hu, Zhang, Chen, Zhou, Jiang, Liu, "DyCAPS: Asynchronous Dynamic-committee Proactive Secret Sharing," ePrint [2022/1169](https://eprint.iacr.org/2022/1169).** Brings CHURP-style dynamic-committee resharing to the **asynchronous** setting at cubic communication, supporting both low- and high-threshold resharing across changing committees.
- **Cimatti, De Sclavis, Galano, Giammusso, Iezzi, Muci, Nardelli, Pedicini, "Dynamic-FROST: Schnorr Threshold Signatures with a Flexible Committee," ePrint [2024/896](https://eprint.iacr.org/2024/896).** Combines FROST (secp256k1 Schnorr) with CHURP-style resharing so committee membership *and* threshold can change without changing the public key — directly applicable to long-lived Bitcoin threshold keys. Related: **Kate, Mukherjee, Samanta, Sarkar, "Dyna-hinTS: Silent Threshold Signatures for Dynamic Committees," ePrint [2025/631](https://eprint.iacr.org/2025/631).**

### 7.6 Refresh in CGGMP / CMP (proactive threshold ECDSA)

**Ran Canetti, Rosario Gennaro, Steven Goldfeder, Nikolaos Makriyannis, Udi Peled, "UC Non-Interactive, Proactive, Threshold ECDSA with Identifiable Aborts," ACM CCS 2020, pp. 1769–1787. ePrint [2021/060](https://eprint.iacr.org/2021/060)** (full version; a shorter non-identifiable-abort variant is ePrint [2020/492](https://eprint.iacr.org/2020/492)). Known as **CGGMP20 / CMP**.

UC-secure threshold ECDSA (global-ROM, Strong-RSA + DDH + Paillier), building on Gennaro–Goldfeder and Lindell–Nof, **non-interactive** in that all but the final signing round is preprocessable. It includes a **proactive key-refresh sub-protocol** run between epochs: each party re-randomizes its **additive secret share** (collectively adding a fresh sharing of zero, aggregate key unchanged) *and* rotates its **Paillier key**, ring-Pedersen/auxiliary parameters, and ZK setup. With **identifiable abort** (on failure the honest parties pinpoint a culprit via the per-round ZK proofs), refresh gives proactive security against a per-epoch minority adversary. Threshold and roster are fixed across a refresh (it rotates secrets, not membership). This is the key-rotation building block in production MPC-custody wallets. Lineage: **Lindell–Nof**, CCS 2018, ePrint [2018/987](https://eprint.iacr.org/2018/987); **Gennaro–Goldfeder** ("Fast Trustless Setup"), CCS 2018, ePrint [2019/114](https://eprint.iacr.org/2019/114).

### 7.7 FROST: refresh, repair, enrollment

FROST itself (Komlo–Goldberg, §8.1) is a *signing* protocol; **proactive refresh and committee change are not in the FROST paper** — they are layered onto the same Shamir sharing:

- **Proactive resharing** uses the §7.2 add-a-sharing-of-zero technique.
- **Share repair / recovery** of a single lost share uses the **repairable-threshold-scheme** techniques of **D. R. Stinson & R. Wei, "Combinatorial Repairability for Threshold Schemes," Designs, Codes and Cryptography 2018, ePrint [2016/855](https://eprint.iacr.org/2016/855)** (see also Laing–Stinson survey, [arXiv 1410.7190](https://arxiv.org/abs/1410.7190)): the surviving parties send Lagrange-weighted combinations of their shares to the party that lost its share, which reconstructs *its* share by interpolation without exposing the secret or others' shares.
- **Enrollment (add/remove participant)** and **threshold change** use the redistribution methods of §7.5.
- The **Zcash Foundation `frost` crate** ships a `dkg` module with `part1/part2/part3` plus **refresh** functions (including removing a participant); see §8.5.

---

## 8. Bitcoin-specific: secp256k1, BIP340, ChillDKG, trusted-dealer alternative

### 8.1 FROST DKG (PedPop)

**Chelsea Komlo, Ian Goldberg, "FROST: Flexible Round-Optimized Schnorr Threshold Signatures," SAC 2020, LNCS 12804. ePrint [2020/852](https://eprint.iacr.org/2020/852).** Signing standardized as **RFC 9591** (2024).

FROST's key generation is a **PedPop** protocol = Pedersen DKG + a **proof of possession**. Structure:

- **Round 1.** Each party samples degree-`(t−1)` `f_i`, publishes **Feldman commitments** `(g^{a_{i0}}, …, g^{a_{i,t−1}})`, **and broadcasts a Schnorr proof of knowledge (signature) on its constant term `a_{i0}`**. This PoP is the specific defense against **rogue-key attacks** (choosing your contribution as a function of others' to control the aggregate key).
- **Round 2.** Each party sends each other party its secret share `f_i(ℓ)` over an authenticated/confidential channel; recipients verify against the sender's public commitments. A failed check triggers a complaint, adjudicated by revealing the disputed share to disqualify the cheater.
- **Output.** Signing share `s_i = Σ_ℓ f_ℓ(i)`; group public key `Y = Π_ℓ g^{a_{ℓ0}} = g^x`, never reconstructed; per-party public shares `Y_i = g^{s_i}` for partial-signature verification.

FROST deliberately uses the cheaper (biasable) Pedersen variant rather than GJKR New-DKG, because the bias does not help forge Schnorr signatures (§4.3, §8.2). It is a 2-round DKG.

### 8.2 Modular building blocks: SimplPedPop → EncPedPop → ChillDKG

Layered abstractions from Blockstream Research's `bip-frost-dkg` (authors **Tim Ruffing, Jonas Nick, Sivaram Dhakshinamoorthy**):

- **SimplPedPop** — a simplified PedPop: Pedersen DKG + per-party proof of possession. Assumes **secure (authenticated + confidential) point-to-point channels** and an **external reliable-broadcast / equality mechanism**. A coordinator role aggregates VSS commitments to cut communication.
- **EncPedPop** — wraps SimplPedPop by **encrypting the VSS shares** (pairwise ECDH keys from a long-term host seed + ephemeral nonces), so shares can traverse an untrusted coordinator / insecure links. Removes the need for pre-established secure channels, leaving only the external **equality check (Eq)**.
- **ChillDKG** — makes the protocol **standalone** by instantiating the equality check in-band as **CertEq**: every participant signs the session transcript, and the collection of `n` signatures forms a **success certificate** proving all honest parties agree on identical output. Only an **authenticated channel (not broadcast)** is required.

**Security foundation.** **Hien Chu, Paul Gerhart, Tim Ruffing, Dominique Schröder, "Practical Schnorr Threshold Signatures Without the Algebraic Group Model," CRYPTO 2023, LNCS 14081. ePrint [2023/899](https://eprint.iacr.org/2023/899).** Defines **Olaf = FROST3 + a Pedersen-DKG variant (SimplPedPop)** and proves unforgeability under **AOMDL** (algebraic one-more DL, a falsifiable assumption) in the ROM **without the AGM**. Key insight: an adversary biasing the DKG output does **not** help forge Schnorr signatures, so full simulatability of the DKG is unnecessary — the proof of possession suffices even against a dishonest majority. Related: **Crites, Komlo, Maller, "How to Prove Schnorr Assuming Schnorr," ePrint [2021/1375](https://eprint.iacr.org/2021/1375)** (introduces FROST2 and treats the Schnorr PoP), and the **TS-UF / TS-SUF** unforgeability hierarchy.

### 8.3 ChillDKG in detail

Draft *"ChillDKG: Distributed Key Generation for FROST"* — repo [github.com/BlockstreamResearch/bip-frost-dkg](https://github.com/BlockstreamResearch/bip-frost-dkg), Python 3.12 reference; version **0.2.0 (2024-12-19)** added blame/identifiable-fault and made **Taproot safety** the default. Salient properties:

- **No trusted dealer; no party ever holds `x`.** Deterministic — all randomness derives from a per-participant **host secret key** + session context.
- **Only setup assumption:** signers agree on the co-signer set by their host **public keys**. No pre-existing secure channels and no external consensus/broadcast needed — an authenticated channel + the built-in CertEq check suffice.
- **Recovery data:** the output is reproducible from the host secret key + transcript; recovery data (transcript ‖ success certificate) is self-authenticating and can be kept with an untrusted backup provider.
- **API:** `participant_step1/step2/finalize` (returns `DKGOutput` + `RecoveryData`); coordinator `coordinator_step1/finalize/investigate` (blame); plus `recover()`.
- **Not robust by design:** any single faulty participant can abort. The authors argue robustness in a keygen ceremony is *undesirable* — it would silently degrade `t`-of-`n` to `(t−1)`-of-`(n−1)` and hand a malicious coordinator dangerous power. Signing robustness is layered separately (ROAST, §10).
- **Taproot safety:** unlike plain Pedersen DKG, a malicious participant cannot embed a hidden script path in the threshold key; ChillDKG applies a deterministic BIP341-style tweak to the aggregated constant-term commitment so the output is safe as a Taproot output key.

### 8.4 Interaction with BIP340 (even-Y / x-only keys)

BIP340 public keys are **x-only**, implicitly the point with **even Y**; a signer with an odd-Y key negates its secret before signing. For threshold FROST:

- ChillDKG works internally with 33-byte compressed secp256k1 keys (0x02/0x03), compatible with BIP340 and **BIP327 (MuSig2)** conventions, deferring x-only normalization.
- The negation is handled **at signing time**, not baked destructively into shares. The FROST signing draft ([github.com/siv2r/bip-frost-signing](https://github.com/siv2r/bip-frost-signing)) initializes the tweak/negation context from the **signer public shares**, so correctness does not depend on whether the keygen-produced group key is plain or x-only.
- In the ZF stack this is the **`frost-secp256k1-tr`** crate (BIP340/BIP341-compatible), which conditionally negates values to satisfy x-only semantics and support Taproot tweaks.

### 8.5 Trusted-dealer alternative vs full DKG

- **Trusted dealer:** one party runs Shamir/Feldman VSS, generates `x`, hands out shares, erases `x`. Simpler, fewer rounds, no complaint handling — but a **single point of trust/compromise at setup** (the dealer transiently knows `x`). Appropriate when one operator provisions a wallet and can be trusted for the ceremony, or for testing.
- **Full DKG (Pedersen/PedPop/ChillDKG):** no party ever holds the full secret; trust is distributed from the start. Cost: more rounds, complaint/blame handling, abortability (for non-robust variants). Correct when no trusted setup party is acceptable.

The **ZF `frost` crate offers both**: `trusted_dealer_keygen()` / `split_secret()` for the dealer path, and a `dkg` module (FROST-paper DKG) for the distributed path. DKG needs authenticated + confidential channels; signing needs only authenticated channels.

### 8.6 Implementation survey

| Library | Language | Scheme(s) | DKG / refresh | Notes |
|---|---|---|---|---|
| [ZF FROST](https://github.com/ZcashFoundation/frost) (`frost.zfnd.org`) | Rust | FROST Schnorr (secp256k1, ed25519, ristretto255, p256, ed448) | 3-part DKG (`part1/2/3`), trusted-dealer, refresh module | RFC 9591; v3.0.0 (2025); `frost-secp256k1-tr` for Taproot; `frostd` relay + CLI |
| [Blockstream `secp256k1-zkp`](https://github.com/BlockstreamResearch/secp256k1-zkp) | C | MuSig2, adaptor sigs, experimental FROST | ChillDKG intended companion | fork of libsecp256k1; Rust binding `rust-secp256k1-zkp` |
| [`bip-frost-dkg`](https://github.com/BlockstreamResearch/bip-frost-dkg) | Python (ref) | ChillDKG / SimplPedPop / EncPedPop | full DKG + recovery | BIP draft, reference implementation |
| [bnb-chain `tss-lib`](https://github.com/bnb-chain/tss-lib) | Go | Threshold ECDSA (GG18/GG20), EdDSA | keygen (no dealer) + **resharing** (dynamic groups) | MtA + Paillier; ~9-round signing; forked by SwingbyProtocol, THORChain |
| [ZenGo `multi-party-ecdsa`](https://github.com/ZenGo-X/multi-party-ecdsa) | Rust | Threshold ECDSA (GG18/GG20), Lindell 2-party | keygen | ZenGo also curates [`awesome-tss`](https://github.com/ZenGo-X/awesome-tss) |
| Coinbase Kryptology, Sepior/Blockdaemon, Taurus, DKLs-family | Go / various | Threshold ECDSA (CMP, DKLs) | keygen + proactive refresh | production MPC custody; CGGMP21 with identifiable abort is state of the art |

### 8.7 Recent secp256k1-native DKG advances (2023–2026)

- **Chelsea Komlo, Ian Goldberg, Douglas Stebila, "A Formal Treatment of Distributed Key Generation, and New Constructions," ePrint [2023/292](https://eprint.iacr.org/2023/292).** Modern game-based security definitions for discrete-log DKG that cleanly separate **"DKG with abort"** from **"robust DKG,"** with new constructions. The definitional backbone for FROST-style secp256k1 DKG.
- **Benedikt Bünz, Kevin Choi, Chelsea Komlo, "Golden: Lightweight Non-Interactive Distributed Key Generation," ePrint [2025/1924](https://eprint.iacr.org/2025/1924).** A **non-interactive, publicly verifiable** DKG outputting Shamir shares of `sk ∈ Z_p` and public key `g^sk`, instantiable **over any elliptic curve where discrete-log is hard — including secp256k1**. Uses a two-party *exponent VRF* (Diffie–Hellman NIKE + ZK proof) to derive one-time pads that encrypt shares, avoiding ElGamal/Paillier/class-group overhead (≈223 kB and ~13.5 s/party at n=50 vs 27.8 MB for ElGamal NI-DKG). One of the most directly actionable NI-DKG results for Bitcoin key setup.
- **Baecker, Gerhart, Jarecki, Nazarian, Rausch, Schröder, "Adaptive Distributed Key Generation for Discrete-Log Cryptosystems," ePrint [2026/892](https://eprint.iacr.org/2026/892).** Explicitly motivated by **Bitcoin Taproot/BIP340 and NIST threshold-Schnorr standardization**. Gives DKG with **identifiable abort** tolerating a dishonest majority, and — responding to a CRYPTO'25 impossibility (unique key commitments force a non-falsifiable assumption for adaptive security) — introduces **key-share-hiding** DKG to retain standard-assumption security while composing with adaptively-secure threshold schemes. Among the most secp256k1-relevant DKG papers to date.
- **Bacho, Loss, Stern, Wagner, "HARTS: High-Threshold, Adaptively Secure, and Robust Threshold Schnorr Signatures," ASIACRYPT 2024. ePrint [2024/280](https://eprint.iacr.org/2024/280).** First threshold **Schnorr** scheme that is simultaneously adaptively secure, robust, high-threshold, and asynchronous (`t_c < n/3`), `O(λn² log n)` amortized, single online round — with matching DKG needs. **Bacho, Wagner, "Tightly Secure Threshold Signatures over Pairing-Free Groups," EUROCRYPT 2025. ePrint [2024/1557](https://eprint.iacr.org/2024/1557)** gives tight DDH security in the secp256k1 setting.

### 8.8 Bitcoin pitfall: additive key derivation + presignatures

**Jens Groth, Victor Shoup, "On the Security of ECDSA with Additive Key Derivation and Presignatures," EUROCRYPT 2022. ePrint [2021/1330](https://eprint.iacr.org/2021/1330).** Directly load-bearing for HD wallets: combining **BIP32-style additive key derivation** (deriving child keys `x + Δ`) with **presignatures** (precomputed nonces) in threshold ECDSA opens concrete attacks and weakens security. A threshold-ECDSA Bitcoin wallet that both derives child keys additively and preprocesses signing nonces must account for this interaction — it constrains how a DKG-produced master key may be reused across derived addresses.

---

## 9. Comparison table (representative schemes)

| Scheme | Type | Network | Adversary | Threshold | Rounds | Robust? | Key type | Curve fit |
|---|---|---|---|---|---|---|---|---|
| Shamir 1979 | SS (dealer) | — | — | `t`-of-`n` | — | — | field | any |
| Feldman VSS 1987 | VSS (dealer) | sync/bcast | static | `t`-of-`n` | 1 (+complaint) | detect | field, reveals `g^s` | secp256k1 |
| Pedersen VSS 1991 | VSS (dealer) | sync/bcast | static | `t`-of-`n` | 1 (+complaint) | detect | field, hides `s` | secp256k1 |
| Pedersen DKG 1991 | DKG | sync/bcast | static (biasable) | `t < n/2` | 2 | yes | field | secp256k1 |
| GJKR New-DKG 1999 | DKG | sync/bcast | static | `t < n/2` | 2-phase (≈4) | yes | field, **unbiased** | secp256k1 |
| CGJKR 1999 | DKG | sync/bcast | **adaptive** | `t < n/2` | 2-phase+ | yes | field | secp256k1 |
| Schoenmakers PVSS 1999 | PVSS | bcast/ledger | static | `t`-of-`n` | 1 (non-interactive) | public verify | field (recovers `g^s`) | secp256k1 |
| Kate eVSS 2010 | VSS | sync/bcast | static | `t`-of-`n` | 1 | detect | field | **pairing only** |
| Groth NIDKG 2021 | DKG + reshare | (a)sync/ledger | static | `t`-of-`n` | non-interactive | public verify | BLS group | **pairing only** |
| Aggregatable DKG 2021 | DKG (PVSS) | sync/gossip | static | `t < n/2` | gossip | public verify, no complaints | **group elem `g^s`** | **pairing only** |
| KMS ADKG 2020 | ADKG | async | static | `t < n/3` | `O(f)` | yes | field | secp256k1 |
| Das et al. ADKG 2022 | ADKG | async | static | `t < n/3` | expected O(1)–O(f) | yes | field | secp256k1 |
| Abraham et al. 2021 | ADKG | async | static | `t < n/3`, high-thresh | const expected | yes | field | secp256k1 (pairing NIZK) |
| Herzberg PSS 1995 | proactive | sync | mobile | `t`-of-`n`, fixed committee | per epoch | yes | field | secp256k1 |
| CHURP 2019 | proactive, **dynamic committee** | sync (opt/pess) | mobile | changeable `(n,t)→(n',t')` | opt `O(n)` | yes (dispute path) | field | secp256k1 (KZG commit) |
| CGGMP20 refresh | proactive tECDSA | sync | adaptive, id-abort | fixed roster | per epoch | id-abort | additive+Paillier | secp256k1 |
| FROST PedPop 2020 | DKG | sync/bcast | static (biasable) | `t`-of-`n` | 2 | detect (complaint) | field | secp256k1 / BIP340 |
| ChillDKG 2024 | DKG standalone | authenticated ch. | static | `t`-of-`n` | ≈3 | abort (blame) | field, Taproot-safe | **secp256k1 / BIP340** |

---

## 10. Latest research (2023–2026)

**Systematization.**
- **Renas Bacho, Alireza Kavousi, "SoK: Dlog-Based Distributed Key Generation," IEEE S&P 2025. ePrint [2025/819](https://eprint.iacr.org/2025/819).** The current definitive survey of discrete-log DKG — taxonomizes techniques (VSS vs PVSS, complaint vs public-verify, sync vs async), security notions, and design principles. Best single starting point for this whole area.

**Round-optimality and impossibility.**
- **Jonathan Katz, "Round-Optimal, Fully Secure Distributed Key Generation," CRYPTO 2024. ePrint [2023/1094](https://eprint.iacr.org/2023/1094).** Proves the **impossibility of one-round *unbiased* DKG** (even for weaker security notions), regardless of setup, and gives round-optimal fully-secure protocols with efficiency/setup/assumption trade-offs. Settles the "silent/one-round DKG" question for the unbiased case.
- **Shrestha, Bhat, Kate, Nayak, "Synchronous Distributed Key Generation without Broadcasts," IACR CiC 2024. ePrint [2021/1635](https://eprint.iacr.org/2021/1635).** Initiates the study of DKG communication/round complexity over point-to-point synchronous networks (no broadcast channel).

**Adaptive security (the active frontier).**
- **Renas Bacho, Julian Loss, "Adaptively Secure (Aggregatable) PVSS and Application to Distributed Randomness Beacons," ACM CCS 2023. ePrint [2023/1348](https://eprint.iacr.org/2023/1348).** First adaptive-security proof of any PVSS (under AGM + OMDL), improving adaptively-secure beacon communication to `O(λn²)`.
- **Bacho, Chen, Loss, "Adaptively Secure (Aggregatable) PVSS from Standard Assumptions," ePrint [2026/1100](https://eprint.iacr.org/2026/1100).** Removes the AGM/OMDL crutch — adaptive PVSS from standard assumptions.
- **Bacho, Lenzen, Loss, Ochsenreither, Papachristoudis, "GRandLine: Adaptively Secure DKG and Randomness Beacon with (Log-)Quadratic Communication Complexity," ACM CCS 2024. ePrint [2023/1887](https://eprint.iacr.org/2023/1887).** Adaptively-secure DKG + beacon at (almost) quadratic communication.
- **Feng, Mai, Tang, "Scalable and Adaptively Secure Any-Trust Distributed Key Generation and All-hands Checkpointing," ACM CCS 2024. ePrint [2023/1773](https://eprint.iacr.org/2023/1773)** ([arXiv 2311.09592](https://arxiv.org/abs/2311.09592)). Practical DLog DKG with **(quasi-)linear per-node cost** even under many Byzantine nodes, adaptive security for `< n/2`, by delegating costly work to a small "Any-Trust" group.
- **Bacho, Loss, "Round-Optimal, Fully Secure Distributed Key Generation" line** — see also the DDH adaptive-security threshold-Schnorr work below.

**Asynchronous DKG advances.**
- **Das, Xiang, Kokoris-Kogias, Ren — high-threshold ADKG** (USENIX 2023, ePrint [2022/1389](https://eprint.iacr.org/2022/1389)); **improved-efficiency/standard-model ADKG** (ePrint [2022/1678](https://eprint.iacr.org/2022/1678)).
- **"Practical Asynchronous Distributed Key Reconfiguration," ePrint [2025/149](https://eprint.iacr.org/2025/149)** — dynamic committee reconfiguration in the asynchronous setting (proactive resharing meets ADKG).
- **"Asymptotically Optimal Adaptive Asynchronous Common Coin and DKG with Silent Setup"** (2025) — silent-setup async DKG direction.

**Constant / sub-cubic complexity.**
- **Benny Applebaum, Benny Pinkas, "Distributing Keys and Random Secrets with Constant Complexity," TCC 2024. ePrint [2024/876](https://eprint.iacr.org/2024/876).** Per-party communication that is **constant** (independent of `n`) in a model where broadcast is a public bulletin board / ledger — directly motivated by large-scale blockchain DKG.
- **"Dragon: Decentralization at the cost of Representation after Arbitrary Grouping … Sub-cubic DKG," PODC 2024.** [ACM](https://dl.acm.org/doi/10.1145/3662158.3662771).

**Threshold-Schnorr signing that co-designs with the DKG (Bitcoin-relevant).**
- **Olaf / Chu–Gerhart–Ruffing–Schröder**, CRYPTO 2023, ePrint [2023/899](https://eprint.iacr.org/2023/899) — SimplPedPop + FROST3, no AGM (the ChillDKG foundation).
- **Sparkle — Crites, Komlo, Maller, "Fully Adaptive Schnorr Threshold Signatures," CRYPTO 2023. ePrint [2023/445](https://eprint.iacr.org/2023/445).** First pairing-free DL threshold Schnorr with adaptive security.
- **Twinkle — Bacho, Loss, Tessaro, Wagner, Zhu, "Threshold Signatures from DDH with Full Adaptive Security," EUROCRYPT 2024**; and the DDH adaptive-security line **Glacius (EUROCRYPT 2025)**, **Dazzle (PKC 2025, ePrint [2025/264](https://eprint.iacr.org/2025/264))**.
- **Arctic — "Lightweight and Stateless Threshold Schnorr Signatures," PKC 2025.**
- **ROAST — Ruffing, Ronge, Jin, Schneider-Bensch, Schröder, "ROAST: Robust Asynchronous Schnorr Threshold Signatures," ACM CCS 2022. ePrint [2022/550](https://eprint.iacr.org/2022/550).** Wraps FROST into a **robust, asynchronous** *signing* protocol — the robustness answer ChillDKG deliberately omits for keygen.

**Newer VSS/PVSS building blocks.**
- **Cascudo, David, "PVSS over Class Groups and Applications to DKG and YOSO," EUROCRYPT 2024. ePrint [2023/1651](https://eprint.iacr.org/2023/1651)** — non-interactive dealing/reconstruction of the *actual* secret over class groups.
- **Cascudo, Cozzo, Giunta, "Verifiable Secret Sharing from Symmetric Key Cryptography with Improved Optimistic Complexity," ASIACRYPT 2024. ePrint [2024/838](https://eprint.iacr.org/2024/838)** — optimistic path uses only symmetric primitives (cheaper, more PQ-friendly).
- **Baghery, Knapen, Nicolas, Rahimi, "Pre-Constructed Publicly Verifiable Secret Sharing and Applications," ACNS 2025. ePrint [2025/576](https://eprint.iacr.org/2025/576)** — precomputes sharing material offline to slash online DKG/beacon cost.

**Post-quantum hedge.**
- **Espitau, Niot, Prest, "Flood and Submerse: Distributed Key Generation and Robust Threshold Signature from Lattices," ePrint [2024/959](https://eprint.iacr.org/2024/959).** Lattice-based robust DKG + threshold signature — not discrete-log, but the reference point if hedging secp256k1 against a future PQ migration.

**Threshold ECDSA round reduction.**
- **Lyu, Li, Zhou, Deng, "Threshold ECDSA in Two Rounds," ePrint [2025/1696](https://eprint.iacr.org/2025/1696)** — continued reduction of the interaction cost of threshold ECDSA signing over secp256k1.

**Practitioner takeaways for a secp256k1/Bitcoin system.** The most directly actionable recent work is **Golden** (2025/1924, lightweight NI-DKG over any DL curve), **Baecker et al.** (2026/892, discrete-log DKG with identifiable abort explicitly targeting BIP340), **Komlo–Goldberg–Stebila** DKG definitions (2023/292), **HARTS** (2024/280) and **Dynamic-FROST** (2024/896) on the signing/resharing side, and the Blockstream **bip-frost-dkg** effort for a native Bitcoin standard.

---

## 11. Known attacks and pitfalls

- **GJKR key-biasing (§4.2).** A rushing adversary manipulating the complaint/disqualification round biases the public-key distribution in single-phase Pedersen DKG. Fix: two-phase commit-then-reveal — *or* rely on the CT-RSA 2003 result that Schnorr is secure despite the bias (which is what FROST/ChillDKG do).
- **Rogue-key attacks in DKG.** Without a proof of possession, a party can choose its contribution as a function of others' commitments to control the aggregate key. Fix: each party attaches a **Schnorr PoK of its constant term** (PedPop / SimplPedPop).
- **Complaint-round DoS.** Real, exploited pitfall: **Trail of Bits (Jan 2024) found that ZF FROST's Pedersen DKG did not validate the *number of polynomial coefficients*.** A malicious participant could submit a higher-degree polynomial, silently **inflating the reconstruction threshold** above the intended value — potentially above `n`, making signing impossible and funds unspendable. Fixed in FROST 1.0.0 by checking coefficient count. See [zfnd.org write-up](https://zfnd.org/pedersen-dkg-vulnerability-in-frost-distributed-key-generation-successfully-remediated/). General lesson: verify *every* structural property of a dealing, not just the share-consistency equation.
- **Robustness-vs-security tension in keygen.** ChillDKG deliberately is **not robust** — a robust keygen that excludes a "faulty" party silently changes `t`-of-`n` into `(t−1)`-of-`(n−1)` and empowers a malicious coordinator. Abort-with-blame is the safer choice for key generation; robustness belongs at signing time (ROAST).
- **Share-verification gaps / weak channels.** DKG requires **authenticated** (and, for share transport, **confidential**) channels; a MITM during keygen can substitute keys. EncPedPop/ChillDKG encrypt shares and add an equality check precisely to close this.
- **Trusted-dealer erasure.** The dealer path is only as safe as the dealer's ability to *securely erase* `x` and its randomness after distribution; a compromised or backed-up dealer nullifies the threshold.
- **Even-Y / BIP340 mishandling.** Baking Y-negation destructively into shares at keygen (rather than resolving it at signing from the public shares) leads to correctness bugs; handle it at signing time.
- **ADKG threshold assumption.** Deploying an async DKG at `t ≥ n/3` breaks the agreement layer's impossibility bound; asynchronous deployments must respect `n ≥ 3t+1`.
- **Biased-key downstream.** Even where Schnorr tolerates DKG bias, *other* uses of the same key (VRFs, some encryption, key-derivation) may not — check the reduction for the specific application before reusing a biasable DKG key.
- **Additive key derivation + presignatures (threshold ECDSA).** Groth–Shoup (ePrint 2021/1330, §8.8) show BIP32-style additive child-key derivation combined with precomputed presignatures enables concrete attacks — an HD wallet doing both on a DKG-produced key must account for this.

---

## 12. Open problems

- **Practical adaptive security on secp256k1.** Adaptive DKG/PVSS is still heavier than static; getting adaptive security from standard assumptions (no AGM/OMDL) at low communication for pairing-free secp256k1 is ongoing (GRandLine, Any-Trust DKG, Bacho–Chen–Loss 2026 point the way).
- **Robust, low-round *asynchronous* DKG at scale** with a **field-element** key and no pairings — pushing below `O(κn³)` communication while keeping high-threshold support.
- **One-round / silent DKG.** Katz (2024) rules out one-round *unbiased* DKG; the question of the weakest setup enabling near-one-round DKG (with tolerable bias) for Bitcoin signing is open.
- **Standardization of DKG for FROST.** ChillDKG is a draft BIP without finalized test vectors; a stable, audited, interoperable spec (matching RFC 9591's role for signing) is not yet finished.
- **Dynamic committees for Bitcoin custody.** CHURP-style membership/threshold change is mature in the pairing/blockchain setting but under-deployed for pairing-free secp256k1 wallets; clean, audited resharing that changes both roster and threshold on secp256k1 is a practical gap.
- **Post-quantum DKG.** Lattice/isogeny DKG (e.g. robust CSIDH DKG) is early-stage; a PQ threshold key for Bitcoin's eventual PQ signature would need a matching PQ DKG.
- **Verifiable / auditable DKG ceremonies.** Publicly-auditable transcripts that a wallet user can later verify their key was generated correctly — bridging PVSS public verifiability with real wallet UX.

---

## 13. References

**Secret sharing and VSS**
- Shamir, "How to Share a Secret," CACM 22(11), 1979. https://dl.acm.org/doi/10.1145/359168.359176
- Feldman, "A Practical Scheme for Non-interactive Verifiable Secret Sharing," FOCS 1987. https://dl.acm.org/doi/10.1109/SFCS.1987.4 · https://www.cs.umd.edu/~gasarch/TOPICS/secretsharing/feldmanVSS.pdf
- Pedersen, "Non-Interactive and Information-Theoretic Secure Verifiable Secret Sharing," CRYPTO '91. https://link.springer.com/chapter/10.1007/3-540-46766-1_9
- Kate, Zaverucha, Goldberg, "Constant-Size Commitments to Polynomials and Their Applications," ASIACRYPT 2010. https://link.springer.com/chapter/10.1007/978-3-642-17373-8_11 · https://cacr.uwaterloo.ca/techreports/2010/cacr2010-10.pdf

**Synchronous DKG (discrete-log)**
- Pedersen, "A Threshold Cryptosystem without a Trusted Party," EUROCRYPT '91. https://link.springer.com/chapter/10.1007/3-540-46416-6_47
- Gennaro, Jarecki, Krawczyk, Rabin, "Secure Distributed Key Generation for Discrete-Log Based Cryptosystems," EUROCRYPT '99 / J. Cryptology 2007. https://link.springer.com/chapter/10.1007/3-540-48910-X_21 · https://link.springer.com/article/10.1007/s00145-006-0347-3
- Gennaro, Jarecki, Krawczyk, Rabin, "Revisiting the Distributed Key Generation…," CT-RSA 2003. https://link.springer.com/chapter/10.1007/3-540-36563-X_26
- Canetti, Gennaro, Jarecki, Krawczyk, Rabin, "Adaptive Security for Threshold Cryptosystems," CRYPTO '99. https://link.springer.com/chapter/10.1007/3-540-48405-1_7

**PVSS and PVSS-based DKG**
- Stadler, "Publicly Verifiable Secret Sharing," EUROCRYPT 1996. https://link.springer.com/chapter/10.1007/3-540-68339-9_17
- Schoenmakers, "A Simple Publicly Verifiable Secret Sharing Scheme and Its Application to Electronic Voting," CRYPTO 1999. https://link.springer.com/chapter/10.1007/3-540-48405-1_10
- Cascudo, David, "SCRAPE," ACNS 2017. https://eprint.iacr.org/2017/216
- Cascudo, David, "ALBATROSS," ASIACRYPT 2020. https://eprint.iacr.org/2020/644
- Gentry, Halevi, Krawczyk, Magri, Nielsen, Rabin, Yakoubov, "YOSO," CRYPTO 2021. https://eprint.iacr.org/2021/210
- Cascudo, David, Garms, Konring, "YOLO YOSO," ASIACRYPT 2022. https://eprint.iacr.org/2022/242 · class-groups follow-up https://eprint.iacr.org/2023/1651
- Gurkan, Jovanovic, Maller, Meiklejohn, Stern, Tomescu, "Aggregatable Distributed Key Generation," EUROCRYPT 2021. https://eprint.iacr.org/2021/005
- Groth, "Non-interactive distributed key generation and key resharing," 2021. https://eprint.iacr.org/2021/339

**Asynchronous DKG**
- Kokoris-Kogias, Malkhi, Spiegelman, "Asynchronous Distributed Key Generation…," CCS 2020. https://eprint.iacr.org/2019/1015
- Das, Yurek, Xiang, Miller, Kokoris-Kogias, Ren, "Practical Asynchronous Distributed Key Generation," IEEE S&P 2022. https://eprint.iacr.org/2021/1591
- Abraham, Jovanovic, Maller, Meiklejohn, Stern, Tomescu, "Reaching Consensus for Asynchronous DKG," PODC 2021. https://eprint.iacr.org/2021/1015
- Das, Xiang, Kokoris-Kogias, Ren, "Practical Asynchronous High-threshold DKG…," USENIX Security 2023. https://eprint.iacr.org/2022/1389
- Zhang et al., "Practical Asynchronous DKG: Improved Efficiency, Weaker Assumption, and Standard Model," DSN 2023. https://eprint.iacr.org/2022/1678
- Bacho, Collins, Liu-Zhang, Loss, "Network-Agnostic Security Comes (Almost) for Free in DKG and MPC," CRYPTO 2023. https://eprint.iacr.org/2022/1369
- Groth, Shoup, "Fast Batched Asynchronous Distributed Key Generation," EUROCRYPT 2024. https://eprint.iacr.org/2023/1175 · signing service https://eprint.iacr.org/2022/506
- Abraham, Bacho, Loss, Stern, "Nearly Quadratic Asynchronous DKG from Recursive Consensus," 2025. https://eprint.iacr.org/2025/006 · Abraham, Bacho, Stern, "Quadratic Asynchronous DKG from Plain Setup," 2026. https://eprint.iacr.org/2026/1159
- Feng, Tang, "Asymptotically Optimal Adaptive Asynchronous Common Coin and DKG with Silent Setup," CRYPTO 2025. https://eprint.iacr.org/2024/2098
- "Practical Asynchronous Distributed Key Reconfiguration," 2025. https://eprint.iacr.org/2025/149

**Proactive / dynamic-committee / refresh**
- Ostrovsky, Yung, "How to Withstand Mobile Virus Attacks," PODC 1991. https://dl.acm.org/doi/10.1145/112600.112605 · revisited https://eprint.iacr.org/2013/529
- Herzberg, Jarecki, Krawczyk, Yung, "Proactive Secret Sharing Or: How to Cope With Perpetual Leakage," CRYPTO 1995. https://link.springer.com/chapter/10.1007/3-540-44750-4_27
- Desmedt, Jajodia, "Redistributing Secret Shares to New Access Structures," ISSE-TR-97-01, 1997.
- Wong, Wang, Wing, "Verifiable Secret Redistribution for Threshold Sharing Schemes," CMU-CS-02-114 / IEEE SISW 2002. https://www.cs.cmu.edu/~wing/publications/Wong-Wing02b.pdf
- Maram, Zhang, Wang, Low, Zhang, Juels, Song, "CHURP: Dynamic-Committee Proactive Secret Sharing," CCS 2019. https://eprint.iacr.org/2019/017
- Canetti, Gennaro, Goldfeder, Makriyannis, Peled, "UC Non-Interactive, Proactive, Threshold ECDSA with Identifiable Aborts" (CGGMP/CMP), CCS 2020. https://eprint.iacr.org/2021/060 · shorter variant https://eprint.iacr.org/2020/492
- Lindell, Nof, "Fast Secure Multiparty ECDSA with Practical Distributed Key Generation…," CCS 2018. https://eprint.iacr.org/2018/987
- Gennaro, Goldfeder, "Fast Multiparty Threshold ECDSA with Fast Trustless Setup," CCS 2018. https://eprint.iacr.org/2019/114
- Stinson, Wei, "Combinatorial Repairability for Threshold Schemes," DCC 2018. https://eprint.iacr.org/2016/855 · Laing–Stinson survey https://arxiv.org/abs/1410.7190
- Hu, Zhang, Chen, Zhou, Jiang, Liu, "DyCAPS: Asynchronous Dynamic-committee Proactive Secret Sharing," 2022. https://eprint.iacr.org/2022/1169
- Cimatti et al., "Dynamic-FROST: Schnorr Threshold Signatures with a Flexible Committee," 2024. https://eprint.iacr.org/2024/896 · Kate, Mukherjee, Samanta, Sarkar, "Dyna-hinTS: Silent Threshold Signatures for Dynamic Committees," 2025. https://eprint.iacr.org/2025/631

**Bitcoin / FROST / secp256k1**
- Komlo, Goldberg, "FROST: Flexible Round-Optimized Schnorr Threshold Signatures," SAC 2020. https://eprint.iacr.org/2020/852 · RFC 9591 https://www.rfc-editor.org/rfc/rfc9591
- Crites, Komlo, Maller, "How to Prove Schnorr Assuming Schnorr" (FROST2), 2021. https://eprint.iacr.org/2021/1375
- Chu, Gerhart, Ruffing, Schröder, "Practical Schnorr Threshold Signatures Without the Algebraic Group Model" (Olaf), CRYPTO 2023. https://eprint.iacr.org/2023/899
- Komlo, Goldberg, Stebila, "A Formal Treatment of Distributed Key Generation, and New Constructions," 2023. https://eprint.iacr.org/2023/292
- Bünz, Choi, Komlo, "Golden: Lightweight Non-Interactive Distributed Key Generation," 2025. https://eprint.iacr.org/2025/1924
- Baecker, Gerhart, Jarecki, Nazarian, Rausch, Schröder, "Adaptive Distributed Key Generation for Discrete-Log Cryptosystems," 2026. https://eprint.iacr.org/2026/892
- Groth, Shoup, "On the Security of ECDSA with Additive Key Derivation and Presignatures," EUROCRYPT 2022. https://eprint.iacr.org/2021/1330
- Bacho, Loss, Stern, Wagner, "HARTS: High-Threshold, Adaptively Secure, and Robust Threshold Schnorr Signatures," ASIACRYPT 2024. https://eprint.iacr.org/2024/280 · Bacho, Wagner, "Tightly Secure Threshold Signatures over Pairing-Free Groups," EUROCRYPT 2025. https://eprint.iacr.org/2024/1557
- Blockstream Research, "ChillDKG: Distributed Key Generation for FROST" (BIP draft). https://github.com/BlockstreamResearch/bip-frost-dkg · blog https://blog.blockstream.com/the-key-to-frost-what-is-distributed-key-generation/
- FROST signing BIP (BIP340). https://github.com/siv2r/bip-frost-signing
- ZF FROST (Rust). https://github.com/ZcashFoundation/frost · https://frost.zfnd.org · Pedersen-DKG DoS remediation https://zfnd.org/pedersen-dkg-vulnerability-in-frost-distributed-key-generation-successfully-remediated/
- Blockstream `secp256k1-zkp`. https://github.com/BlockstreamResearch/secp256k1-zkp
- bnb-chain `tss-lib`. https://github.com/bnb-chain/tss-lib · ZenGo `multi-party-ecdsa`. https://github.com/ZenGo-X/multi-party-ecdsa · `awesome-tss` https://github.com/ZenGo-X/awesome-tss

**Latest (2023–2026) and robustness**
- Bacho, Kavousi, "SoK: Dlog-Based Distributed Key Generation," IEEE S&P 2025. https://eprint.iacr.org/2025/819
- Katz, "Round-Optimal, Fully Secure Distributed Key Generation," CRYPTO 2024. https://eprint.iacr.org/2023/1094
- Shrestha, Bhat, Kate, Nayak, "Synchronous Distributed Key Generation without Broadcasts," CiC 2024. https://eprint.iacr.org/2021/1635
- Bacho, Loss, "Adaptively Secure (Aggregatable) PVSS…," CCS 2023. https://eprint.iacr.org/2023/1348 · from standard assumptions https://eprint.iacr.org/2026/1100
- Bacho, Lenzen, Loss, Ochsenreither, Papachristoudis, "GRandLine…," CCS 2024. https://eprint.iacr.org/2023/1887
- Feng, Mai, Tang, "Scalable and Adaptively Secure Any-Trust DKG…," CCS 2024. https://eprint.iacr.org/2023/1773
- Applebaum, Pinkas, "Distributing Keys and Random Secrets with Constant Complexity," TCC 2024. https://eprint.iacr.org/2024/876
- Crites, Komlo, Maller, "Fully Adaptive Schnorr Threshold Signatures" (Sparkle), CRYPTO 2023. https://eprint.iacr.org/2023/445
- Bacho, Loss, Tessaro, Wagner, Zhu, "Twinkle: Threshold Signatures from DDH with Full Adaptive Security," EUROCRYPT 2024. https://link.springer.com/chapter/10.1007/978-3-031-58716-0_15
- Ruffing, Ronge, Jin, Schneider-Bensch, Schröder, "ROAST: Robust Asynchronous Schnorr Threshold Signatures," CCS 2022. https://eprint.iacr.org/2022/550
- Cascudo, David, "PVSS over Class Groups and Applications to DKG and YOSO," EUROCRYPT 2024. https://eprint.iacr.org/2023/1651
- Cascudo, Cozzo, Giunta, "Verifiable Secret Sharing from Symmetric Key Cryptography…," ASIACRYPT 2024. https://eprint.iacr.org/2024/838
- Baghery, Knapen, Nicolas, Rahimi, "Pre-Constructed Publicly Verifiable Secret Sharing and Applications," ACNS 2025. https://eprint.iacr.org/2025/576
- Espitau, Niot, Prest, "Flood and Submerse: Distributed Key Generation and Robust Threshold Signature from Lattices," 2024. https://eprint.iacr.org/2024/959
- Lyu, Li, Zhou, Deng, "Threshold ECDSA in Two Rounds," 2025. https://eprint.iacr.org/2025/1696
- Aumasson, Hamelink, Shlomovits, "A Survey of ECDSA Threshold Signing," 2020. https://eprint.iacr.org/2020/1390

---

*Compiled July 2026. All ePrint numbers and venues verified against IACR/dblp; pre-1996 works (Shamir, Feldman, Pedersen, Ostrovsky–Yung, Herzberg et al., GJKR, CGJKR) predate the ePrint archive and are cited by DOI/Springer/ACM.*
