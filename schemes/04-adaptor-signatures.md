# Adaptor Signatures & Scriptless Scripts

*Deep research survey — current to 2026.*
*Scope: adaptor signatures ("signature encryption" to a hidden witness), scriptless scripts, and their composition with threshold/multi-party signing, as applied to Bitcoin.*

---

## 1. Overview: why adaptor signatures matter for Bitcoin

An **adaptor signature** is a cryptographic gadget that ties the act of producing a valid digital signature to the *revelation of a secret witness*. Informally, a signer publishes a **pre-signature** (also called an "encrypted" or "adaptor" signature) that is bound to a public statement `Y = y·G` of a hard relation. The pre-signature is **not** a valid signature. Two things can then happen, and they are two sides of the same coin:

- Anyone holding the **witness** `y` can **adapt** the pre-signature into a fully valid signature.
- Anyone who sees both the pre-signature and the completed (published) signature can **extract** the witness `y`.

This "sign ⟺ reveal a secret" equivalence is the entire trick. It lets Bitcoin express conditional payments — "you get this coin iff you reveal `y`" — **without any special on-chain script**. The condition lives inside the signature math, not in Bitcoin Script. On-chain the transaction looks like an utterly ordinary single-key (or multisig/Taproot) spend. This property is what Andrew Poelstra named **scriptless scripts**: "a contract enforced by a blockchain which does not actually contain any code — the signature scheme alone enforces the conditions."

**Why this is a big deal for Bitcoin specifically:**

- **No new opcodes, no soft fork.** Adaptor signatures work *today* with Bitcoin's existing signature schemes: ECDSA (pre-Taproot outputs) and BIP-340 Schnorr (Taproot). The DLC community shipped ECDSA adaptor signatures years before Taproot.
- **On-chain indistinguishability / privacy.** Because the completed signature is a bog-standard signature, an atomic swap, a Lightning hop, or a DLC settlement is indistinguishable on-chain from a normal payment. HTLCs, by contrast, expose a hash and a preimage on-chain and correlate the two legs of a payment by a shared hash.
- **Fungibility & fee savings.** No hashlock script means smaller witnesses and no protocol-specific on-chain fingerprint.
- **Composability with aggregation.** Adaptor logic composes cleanly with MuSig2 (n-of-n Schnorr), FROST (t-of-n Schnorr), and threshold ECDSA, because the adaptor is just an offset applied to the (aggregate) nonce.

**The canonical applications**, all of which replace an explicit script with a scriptless signature condition:

| Application | Classic construction | Scriptless (adaptor) construction |
|---|---|---|
| Cross-chain atomic swap | Two HTLCs sharing a hash preimage | Two adaptor sigs sharing a secret scalar `y` |
| Lightning payment routing | HTLC (hashlock + timelock) | **PTLC** (point-locked): 2-of-2 MuSig2 + adaptor |
| Oracle / event contracts | Custom script | **DLC** (Discreet Log Contract): oracle signature *is* the witness |
| Payment channels | Script-heavy commitment txs | **Generalized channels**: adaptor-based punishment |
| CoinSwap / coin mixing | HTLC-based | Adaptor-based, unlinkable |

Poelstra's original motivation came from **Mimblewimble**, a design that allows *only* signatures with no script enforcement at all — forcing him to ask what smart-contract functionality could be squeezed out of signatures alone. The answer turned out to apply directly to Bitcoin. (Poelstra, *Scriptless Scripts*, MIT Bitcoin Expo, March 2017; Blockstream Research; [slides](https://download.wpsoftware.net/bitcoin/wizardry/mw-slides/2017-03-mit-bitcoin-expo/slides.pdf).)

---

## 2. Formal definition

The first *standalone, game-based* definition of adaptor signatures as a primitive in its own right was given by **Aumayr, Ersoy, Erwig, Faust, Hostáková, Maffei, Moreno-Sanchez & Riahi**, "Generalized Channels from Limited Blockchain Scripts and Adaptor Signatures," **ASIACRYPT 2021** (ePrint [2020/476](https://eprint.iacr.org/2020/476)). A generic construction from identification schemes followed in **Erwig, Faust, Hostáková, Maitra & Riahi**, "Two-Party Adaptor Signatures from Identification Schemes," **PKC 2021** (ePrint [2021/150](https://eprint.iacr.org/2021/150)).

### 2.1 The hard relation

An adaptor signature is defined *relative to a hard relation* `R`. A pair `(Y, y) ∈ R` consists of a **statement** `Y` and a **witness** `y`, where given only `Y` it is computationally infeasible to find `y`. In the Bitcoin setting `R` is almost always the **discrete-log relation** on secp256k1:

```
R = { (Y, y) : Y = y·G }
```

so a "statement" is just a curve point and the "witness" is its discrete log. This is exactly what makes the primitive Bitcoin-native: the witness is a secp256k1 scalar, the statement is a secp256k1 point, and revealing the witness is the same kind of secret an ordinary key or nonce already is.

### 2.2 The four algorithms

An adaptor signature scheme `ΠR` extends an ordinary signature scheme `Σ = (Gen, Sign, Verify)` with four algorithms:

- **`preSign(sk, m, Y) → σ̃`** — produce a **pre-signature** on message `m` bound to statement `Y`.
- **`preVerify(pk, m, Y, σ̃) → 0/1`** — publicly check that `σ̃` is a well-formed pre-signature for `(m, Y)`.
- **`adapt(σ̃, y) → σ`** — using witness `y`, turn the pre-signature into a **full signature** `σ` that `Verify(pk, m, σ)` accepts.
- **`extract(σ̃, σ, Y) → y`** — from a matching pre-signature/signature pair, recover the witness `y`.

### 2.3 The security / correctness properties

**(a) Pre-signature correctness.** An honestly generated pre-signature verifies under `preVerify`, adapts (with the right witness) to a signature that passes `Verify`, and from that adapted signature `extract` recovers the correct witness. Formally, for `(Y,y) ∈ R`: `preVerify(pk,m,Y, preSign(sk,m,Y)) = 1`, `Verify(pk,m, adapt(σ̃,y)) = 1`, and `extract(σ̃, adapt(σ̃,y), Y) = y`.

**(b) aEUF-CMA (adaptor existential unforgeability under chosen-message attack).** The core unforgeability notion. An adversary with a signing oracle **and** a pre-signing oracle, who is additionally *given a challenge pre-signature* `σ̃` on the target message `m*` for a statement `Y*` (with `(Y*,y*) ∈ R`), still cannot produce a valid *full* signature on `m*` — **even though it holds the pre-signature** — as long as it does not know the witness `y*`. Intuitively: a pre-signature plus everything short of the witness must not be enough to finish the signature.

**(c) Pre-signature adaptability.** *Any* valid pre-signature (even a maliciously generated one that merely passes `preVerify`) can be adapted with a valid witness into a valid full signature. This is what guarantees the "receiver" side of a swap actually gets paid once they learn the secret — it must hold even against a cheating counterparty who produced the pre-signature.

**(d) Witness extractability.** If an adversary produces a *full* signature `σ` on `m*` for which it was given a pre-signature `σ̃` on statement `Y*`, then `extract(σ̃, σ, Y*)` yields a valid witness `y*` with `(Y*, y*) ∈ R`. This is the "seller" side: whoever completes the signature *necessarily leaks* the secret to the pre-signature holder.

Properties (c) and (d) are the atomicity engine: adaptability guarantees the buyer can always claim once they have the secret; extractability guarantees the seller always learns the secret the instant the buyer claims. Together they make a swap **atomic** — either both legs happen or neither does.

---

## 3. Schnorr vs ECDSA adaptor constructions

### 3.1 Schnorr (clean, native to Taproot)

Recall a BIP-340 Schnorr signature on message `m` under key `P = x·G` is `(R, s)` with nonce `R = k·G` and
```
s = k + e·x,   where  e = H(R ‖ P ‖ m),
```
verified by `s·G == R + e·P`.

The adaptor construction over statement `T = t·G` is beautifully simple — the witness `t` is just an **additive offset on the nonce**:

- **preSign:** sample nonce `k`, set `R = k·G`, form the *adapted nonce* `R' = R + T`, compute the challenge over the adapted nonce `e = H(R' ‖ P ‖ m)`, and output pre-signature `s' = k + e·x` (together with `R`, or equivalently `R'` and `T`).
- **preVerify:** check `s'·G == R + e·P` with `e = H((R+T) ‖ P ‖ m)`. (Note `s'` alone is *not* a valid Schnorr signature because the verification equation uses `R'=R+T` as the nonce but `s'` only "covers" `R`.)
- **adapt:** given `t`, set `s = s' + t`. Now `(R', s)` is a valid BIP-340 signature, because `s·G = s'·G + t·G = R + e·P + T = R' + e·P`. ✓
- **extract:** given the completed `(R', s)` and the pre-signature `s'`, recover `t = s − s'`.

Everything stays inside the group; there are **no auxiliary zero-knowledge proofs**, no extra assumptions beyond the discrete-log/Schnorr security already relied upon. This is why Schnorr adaptor signatures are considered the "clean" case and why Taproot makes them first-class. (Technical walkthrough: [conduition.io — "The Riddles of Adaptor Signatures"](https://conduition.io/scriptless/adaptorsigs/); [Suredbits — Schnorr Applications: Scriptless Scripts](https://suredbits.com/schnorr-applications-scriptless-scripts/).)

### 3.2 ECDSA (harder — needs a ZK proof)

ECDSA has no clean additive-nonce structure: a signature is `(r, s)` with `r = x-coord(k·G)` and `s = k⁻¹(H(m) + r·x)`. The multiplicative inversion of `k` blocks the "just add the witness" trick. The DLC-community construction (Fournier; standardized in [`dlcspecs/ECDSA-adaptor.md`](https://github.com/discreetlogcontracts/dlcspecs/blob/master/ECDSA-adaptor.md)) works over secp256k1 with SHA-256 and repairs this with a **DLEQ (discrete-log-equality) proof**:

- **encrypt / preSign:** with signing key `x`, encryption/adaptor point `Y = y·G`, and message hash: sample `k`, compute `R_a = k·G` and `R = k·Y`, take `r = x-coord(R)`, and form the "adaptor scalar" `s_a = k⁻¹(H(m) + r·x)`. Attach a **DLEQ proof** `π` (a Fiat–Shamir–transformed sigma protocol) that `log_G(R_a) = log_Y(R) = k` — i.e. that the *same* `k` was used against both `G` and `Y`. Without this proof the receiver cannot trust that decrypting with `y` will yield a valid signature.
- **decrypt / adapt:** given witness `y`, set `s = s_a · y⁻¹ (mod n)` and apply the BIP-62 low-`s` rule. This is now a valid ECDSA signature.
- **recover / extract:** given `s_a` and the published `s`, recover `y = s_a · s⁻¹ (mod n)`.

**Cost and caveat.** The DLEQ proof is the extra machinery Schnorr does not need. Crucially the spec warns: *"each adaptor signature leaks the Diffie–Hellman key for the signing key `X` and the encryption key `Y`… This scheme is applicable to DLCs only and should not be applied in another context without careful analysis."* This is the recurring theme: **ECDSA adaptor signatures are subtler and context-sensitive**, whereas Schnorr's are structurally clean.

**Two-party ECDSA.** For 2-of-2 (e.g. CoinSwap, two-party Lightning), the pre-signature is produced *jointly* without either party holding the full ECDSA key. Approaches build on **Lindell-style two-party ECDSA** and **DKLs (Doerner–Kondi–Lee–shelat)** multiplication, wrapping the joint signing so that the co-signed value is a pre-signature bound to `Y` plus a ZK proof convincing the receiver that the embedded discrete log yields the desired witness. (Erwig et al., PKC 2021, ePrint [2021/150](https://eprint.iacr.org/2021/150), give the generic ID-scheme framework instantiated for both Schnorr and ECDSA and the two-party variants.)

---

## 4. Threshold & multi-party adaptor signatures

Adaptor signatures compose with signature *aggregation* and *thresholdization* because the adaptor is, at bottom, an offset on the (aggregate) nonce or an offset on the combined signature — orthogonal to how the signing key is shared.

### 4.1 MuSig2 (n-of-n Schnorr) adaptor

In MuSig2 the signers jointly produce an aggregate nonce `R_agg` and aggregate key `P_agg`. To make an **adaptor MuSig2 signature**, the adaptor point `T` is added to the aggregate nonce before the challenge is hashed (`R' = R_agg + T`), and each signer contributes a partial pre-signature `s'_i = k_i + e·a_i·x_i`. The sum `s' = Σ s'_i` is the joint pre-signature; adapting with `t` and extracting `t` work exactly as in the single-key Schnorr case. This is precisely how **PTLCs** are realized: a PTLC output is a **plain 2-of-2 MuSig2** output, and "the hashlock is only implicitly added to the output when a partial signature is received" (Blockstream Research [`scriptless-scripts/multi-hop-locks.md`](https://github.com/BlockstreamResearch/scriptless-scripts/blob/master/md/multi-hop-locks.md)). Note the usual MuSig2 concurrency discipline (nonce handling / ROS-attack avoidance) still applies and is compatible with the adaptor offset.

### 4.2 FROST (t-of-n Schnorr) adaptor

FROST generalizes to a `t`-of-`n` threshold. The adaptor offset is applied to the group nonce commitment, and partial signatures are combined via Lagrange interpolation over the participating subset; the group signature that results is adapted/extracted identically. Blockstream's `secp256k1-zkp` carries FROST work ([PR #138](https://github.com/BlockstreamResearch/secp256k1-zkp/pull/138)) alongside its adaptor modules.

### 4.3 Threshold ECDSA adaptor

Combining the adaptor property with **threshold ECDSA** (GG18/GG20, DKLs, CMP/Lindell) is the hardest case: it inherits both the multiplicative-nonce awkwardness of ECDSA *and* the interactive MPC of threshold signing, and requires the DLEQ-style proof to be produced in a distributed manner. Feasible but heavy; this is the frontier where the ECDSA "leaks the DH key" caveat and the definitional gaps below matter most.

### 4.4 Dedicated threshold/multi adaptor literature

- **"Threshold/Multi Adaptor Signature and Their Applications in Blockchains"** — Electronics (MDPI) 13(1):76, 2024 ([doi](https://doi.org/10.3390/electronics13010076)). Introduces both **threshold adaptor signatures** (t-of-n) and **multi-adaptor signatures**, with formal security models and instantiations over **Schnorr** and post-quantum **Dilithium**, targeting atomic swaps and fair exchange.
- **"Consecutive Adaptor Signature Scheme: From Two-Party to N-Party Settings"** — ePrint [2024/241](https://eprint.iacr.org/2024/241). Generalizes the two-party adaptor to *chains* of `N` parties, matching multi-hop payment structure.
- **"A Multi-Party, Multi-Blockchain Atomic Swap Protocol with Universal Adaptor Secret"** — arXiv [2406.16822](https://arxiv.org/pdf/2406.16822). A *universal* adaptor secret so that one witness settles many legs across many chains at once.
- **"Universal Adaptor Signatures from Blackbox Multi-party Computation"** — Springer 2025 ([chapter](https://link.springer.com/chapter/10.1007/978-3-031-88661-4_16)). Builds adaptor signatures for arbitrary relations generically from MPC.

**Security caveat for threshold composition.** The subtle point is that the adaptor property interacts with *nonce generation and witness extraction*: a scheme that is a secure threshold signature and a secure adaptor signature *in isolation* is not automatically secure when combined — extraction gives every pre-signature holder an equation relating nonces and the witness, and any nonce misuse (reuse across sessions, biased/predictable nonces) turns extraction into **key leakage**. See §6.

---

## 5. Applications with protocol sketches

### 5.1 Cross-chain atomic swap (Schnorr)

Alice has BTC on chain A, Bob has coins on chain B; they want to swap atomically.

1. Bob picks secret `y`, publishes statement `Y = y·G` to Alice.
2. Each party funds a 2-of-2 output on its chain. Alice gives Bob a **pre-signature** (bound to `Y`) on the transaction that pays Bob on chain A; Bob gives Alice a **pre-signature** (bound to the same `Y`) on the transaction paying Alice on chain B.
3. Bob knows `y`, so he **adapts** Alice's pre-signature and claims his coins on chain A — publishing a complete signature.
4. Alice **extracts** `y` from that published signature (`y = s − s'`), then adapts Bob's pre-signature and claims on chain B.

Either both legs complete or (with timelock refunds) neither does — and both chains just see ordinary signatures. No shared hash links the two legs, unlike HTLC swaps.

### 5.2 Lightning PTLCs (point time-locked contracts)

PTLCs replace HTLCs as the routing primitive. Instead of every hop sharing one hash `H(preimage)`, each hop is locked to a *different curve point*, decorrelating the route.

- The recipient chooses scalar `z`, sends `z·G` to the sender.
- For each hop `i` the sender derives a **left lock** `Lᵢ` and a **right lock** `Rᵢ = Lᵢ + yᵢ·G`; each hop's right lock is the next hop's left lock (a chain of points).
- Each hop's payment is a 2-of-2 MuSig2 output; a node forwards by handing over an **adaptor (partial) pre-signature** bound to its lock point.
- When the recipient claims, she combines her secret `z` with the last node's partial signature; each upstream node then learns its right-lock secret by **subtracting signatures**, computes its left-lock secret, and propagates upstream. Only the sender ultimately learns `z` — a **proof of payment**.

Benefits over HTLCs: **payment decorrelation** (intermediaries can't tell they're on the same route), smaller/indistinguishable on-chain footprint, and fee savings. PTLCs slot naturally into **eltoo**-style symmetric channels (no per-update revocation needed). (Bitcoin Optech [PTLC topic](https://bitcoinops.org/en/topics/ptlc/); [Suredbits PTLC PoC](https://suredbits.com/ptlc-proof-of-concept/).)

### 5.3 Discreet Log Contracts (DLCs)

DLCs (Dryja, MIT DCI, 2018 — [paper](https://adiabat.github.io/dlc.pdf)) let two parties bet on a real-world event adjudicated by an **oracle**, entirely scriptlessly.

- Before the event the oracle publishes its public key `P` and a **public nonce** `R` it will use.
- For each possible outcome the oracle's *future* signature is a *predictable point*: `S_outcome = R + H(outcome)·P`. This point is used as the **adaptor statement** `Y` for that outcome.
- The two parties exchange adaptor pre-signatures on the set of settlement transactions, one per outcome, each bound to the corresponding outcome-point.
- When the event happens the oracle broadcasts a single scalar signature `s` for the actual outcome. That scalar is exactly the **witness** to the outcome-point, so the party favored by that outcome can adapt and claim.

The oracle never touches the contract, learns nothing about it, and need not even know the bet exists ("discreet"). Cooperatively only two transactions hit the chain and they look like a normal 2-of-2 spend, so outside observers cannot detect the contract. The ECDSA variant uses the `dlcspecs` scheme in §3.2; Taproot enables the Schnorr variant. (See also **"From Multi-sig to DLCs: Modern Oracle Designs on Bitcoin,"** arXiv [2602.09822](https://arxiv.org/pdf/2602.09822).)

### 5.4 Generalized channels & CoinSwap

- **Generalized channels** (Aumayr et al., ASIACRYPT 2021) use adaptor signatures to build the channel *punishment* mechanism scriptlessly, so a single channel construction supports arbitrarily many off-chain applications on top of a minimal-script blockchain like Bitcoin.
- **CoinSwap / coin mixing** uses adaptor signatures to make two payments atomic and unlinkable, improving fungibility without an on-chain fingerprint.

---

## 6. Security model, definitional issues & the foundations work

Adaptor-signature security definitions turned out to be **surprisingly tricky**, and a sequence of papers from 2022–2026 revealed that the widely used original notions were *insufficient* — they failed to rule out schemes that are dangerous in exactly the applications adaptor signatures were invented for.

### 6.1 Stronger Security & Generic Constructions (INDOCRYPT 2022)

**Dai, Okamoto & Yamamoto**, "Stronger Security and Generic Constructions for Adaptor Signatures," INDOCRYPT 2022, ePrint [2022/1687](https://eprint.iacr.org/2022/1687). Two findings:

1. The "scriptless" framing (pre-signature derived from an ordinary signature) **restricts instantiability** — it blocks constructions from BLS or NIST PQC candidates.
2. **Security gaps:** the then-current notions **did not rule out a class of insecure schemes**, and **on-chain unlinkability** of adaptor signatures had never been formalized. They give stronger notions and generic constructions from *any* signature scheme + hard relation.

### 6.2 Foundations of Adaptor Signatures (EUROCRYPT 2024) — the headline result

**Gerhart, Schröder, Soni & Thyagarajan**, "Foundations of Adaptor Signatures," **EUROCRYPT 2024**, ePrint [2024/1809](https://eprint.iacr.org/2024/1809). This is the definitive "the definitions were wrong" paper. Key results:

- **Witness extractability is too weak as stated.** They exhibit a scheme that **satisfies the existing witness-extractability and unforgeability definitions yet lets an adversary derive *two distinct valid signatures* from a single pre-signature.** Under the old definition this is not forbidden.
- **This breaks real applications.** Using such a (definitionally "secure") scheme, they **break the unforgeability of Blind Conditional Signatures** — the primitive underlying **payment-channel hubs / Lightning-style anonymous payment hubs**. So the gap is not academic.
- **A prior reduction is incorrect.** The earlier claim that OMDL-secure adaptor signatures generically yield secure blind conditional signatures **does not hold**.
- **Relations among primitives.** They put adaptor signatures on firmer foundations, clarifying what they imply and require and how the notions relate — establishing the "foundations" the title promises.

Takeaway: **treat pre-2024 adaptor-signature security claims with care**, especially in composed protocols (hubs, threshold, blind).

### 6.3 Witness hiding for NP relations (ASIACRYPT 2024)

**Liu, Tzannetos & Zikas**, "Adaptor Signatures: New Security Definition and a Generic Construction for NP Relations," **ASIACRYPT 2024**, ePrint [2024/1051](https://eprint.iacr.org/2024/1051). Observation: the adapted signature is *published on-chain*, and the old definitions let it **leak the witness** to the whole world. For DL statements that is usually fine (the witness is ephemeral), but for richer relations it is fatal. They add a **witness-hiding** property, and construct witness-hiding adaptor signatures for **any NP relation** from **one-way functions** (via a weak trapdoor commitment — "trapdoor commitment with a specific adaptable message" — instantiated on the Hamiltonian-cycle problem).

### 6.4 Blind adaptor signatures, revisited (2026)

**"Blind Adaptor Signatures, Revisited: Stronger Security Definitions and Their Construction toward Practical Applications,"** ePrint [2026/060](https://eprint.iacr.org/2026/060). Continues the foundations program into the *blind* setting, strengthening definitions with practical hub applications in mind.

### 6.5 Formal-methods framework: LedgerLocks (CCS 2023)

**Tairi, Moreno-Sanchez & Schneidewind**, "LedgerLocks: A Security Framework for Blockchain Protocols Based on Adaptor Signatures," **CCS 2023**, ePrint [2023/1315](https://eprint.iacr.org/2023/1315). Rather than fix the primitive, this fixes *how protocols reason about it*. It introduces **AS-locked transactions** (a transaction whose publication is bound to knowing a secret) and a **Universal-Composability ledger functionality `G_LedgerLocks`** with built-in support for them, letting designers of channels/swaps/DLCs/wallets/mixers work with a clean abstraction and focus on blockchain-specific concerns rather than re-proving the crypto each time.

---

## 7. Known pitfalls & attacks

- **Nonce reuse ⇒ key extraction.** As with all Schnorr/ECDSA signing, reusing or biasing a nonce is catastrophic. In the adaptor setting the danger is amplified: extractability already hands the pre-signature holder a linear equation in the nonce and witness, so any additional nonce leakage collapses to private-key recovery.
- **Two-signatures-from-one-pre-signature (the Foundations attack).** Old witness-extractability did not forbid a pre-signature adaptable to *two* different valid signatures; this breaks blind-conditional-signature / hub constructions (§6.2). Use the strengthened definitions.
- **Witness leakage on-chain (non-DL relations).** The adapted signature can reveal the witness publicly; harmless for ephemeral DL secrets, dangerous otherwise — motivating witness hiding (§6.3).
- **ECDSA "leaks the DH key."** The `dlcspecs` ECDSA adaptor leaks the Diffie–Hellman value of the signing and encryption keys; the spec explicitly restricts it to DLCs "without careful analysis" for other uses (§3.2).
- **Missing/forged DLEQ proof (ECDSA).** Omitting or mis-verifying the DLEQ proof lets a malicious signer hand over a pre-signature that does *not* decrypt to a valid signature under the promised witness — breaking pre-signature adaptability. The proof is mandatory, not optional.
- **Randomness / random-self-reducibility subtleties.** Several works note that "secret randomness concealed in the signature" can be exposed in ways not captured by naïve definitions; some propose using relations with random self-reducibility rather than injecting extra randomness (INDOCRYPT 2022; the New-Security-Definition line).
- **Unlinkability was unformalized.** On-chain unlinkability — that the completed signature can't be tied back to its pre-signature/protocol — was not a defined goal until Dai et al.

---

## 8. Latest research 2023–2026 (snapshot)

- **2023 — LedgerLocks (CCS):** UC framework for AS-based protocols; `G_LedgerLocks`. ([2023/1315](https://eprint.iacr.org/2023/1315))
- **2024 — Foundations of Adaptor Signatures (EUROCRYPT):** definitional gaps exposed; two-sigs-from-one-presig; breaks blind conditional signatures; corrects OMDL claim. ([2024/1809](https://eprint.iacr.org/2024/1809))
- **2024 — Witness-hiding AS for NP relations (ASIACRYPT):** from OWFs, Hamiltonian-cycle trapdoor commitments. ([2024/1051](https://eprint.iacr.org/2024/1051))
- **2024 — Threshold/Multi Adaptor Signatures (MDPI Electronics):** t-of-n + multi, Schnorr & Dilithium.
- **2024 — Consecutive Adaptor Signatures, two-party → N-party.** ([2024/241](https://eprint.iacr.org/2024/241))
- **2024 — Efficient ECDSA-Based Adaptor Signature for Batched Atomic Swaps:** fewer/no ZK proofs in pre-signing for batched swaps. ([2024/140](https://eprint.iacr.org/2024/140))
- **2024 — Multi-party, multi-blockchain atomic swap with universal adaptor secret.** (arXiv [2406.16822](https://arxiv.org/pdf/2406.16822))
- **2024 — SQIAsignHD:** isogeny (SQIsignHD) adaptor signature, QROM-secure. (Renan & Kutas, arXiv [2404.09026](https://arxiv.org/pdf/2404.09026); ePrint 2024/561)
- **2025 — Universal Adaptor Signatures from Blackbox MPC.** ([Springer](https://link.springer.com/chapter/10.1007/978-3-031-88661-4_16))
- **2026 — Blind Adaptor Signatures, Revisited:** stronger definitions for hub applications. ([2026/060](https://eprint.iacr.org/2026/060))
- **2026 — "From Multi-sig to DLCs: Modern Oracle Designs on Bitcoin"** survey. (arXiv [2602.09822](https://arxiv.org/pdf/2602.09822))

**Post-quantum context (Bitcoin relevance = future-proofing L2, not on-chain today):**
- **LAS** — first lattice adaptor signature, Module-SIS/Module-LWE, enables PQ payment channels & swaps with no extra on-chain cost (Esgin, Ersoy & Erkin, FC 2021, ePrint [2020/1345](https://eprint.iacr.org/2020/1345)).
- **"Post-Quantum Adaptor Signatures and Payment Channel Networks"** (ESORICS 2020, ePrint [2020/845](https://eprint.iacr.org/2020/845)).
- **Randomized EdDSA adaptor** (ScienceDirect, 2024) and **isogeny** variants (SQIAsignHD; Compact Adaptor Signatures from Isogenies).
- Relevance to Bitcoin is prospective: these matter if/when Bitcoin's signature scheme migrates, but the *scriptless* off-chain applications carry over.

---

## 9. Implementations

- **`BlockstreamResearch/secp256k1-zkp`** — C library with **ECDSA adaptor signatures** ([PR #117](https://github.com/BlockstreamResearch/secp256k1-zkp/pull/117), by jesseposner), **DLEQ proofs**, **MuSig2**, and **FROST** ([PR #138](https://github.com/BlockstreamResearch/secp256k1-zkp/pull/138)); BIP-340 Schnorr adaptor work also tracked here.
- **`rust-secp256k1-zkp`** / crate **`secp256k1-zkp`** — Rust bindings exposing adaptor signatures and DLEQ ([docs.rs](https://docs.rs/secp256k1-zkp)); ECDSA-adaptor tracking in [rust-bitcoin/rust-secp256k1 #292](https://github.com/rust-bitcoin/rust-secp256k1/issues/292).
- **`discreetlogcontracts/dlcspecs`** — the DLC specification suite, including the normative [`ECDSA-adaptor.md`](https://github.com/discreetlogcontracts/dlcspecs/blob/master/ECDSA-adaptor.md) scheme (secp256k1, SHA-256, DLEQ sigma proof).
- **`BlockstreamResearch/scriptless-scripts`** (and the original `ElementsProject/scriptless-scripts`) — the [`multi-hop-locks.md`](https://github.com/BlockstreamResearch/scriptless-scripts/blob/master/md/multi-hop-locks.md) PTLC/multi-hop specification.
- **Lightning:** PTLC proofs-of-concept and design work across LDK, eclair, and lnd; PTLCs depend on the adaptor + MuSig2 tooling above. Pre-Taproot deployments can use ECDSA adaptors today; eltoo/Taproot deployments use Schnorr. (Bitcoin Optech [PTLC](https://bitcoinops.org/en/topics/ptlc/); Suredbits [PoC](https://suredbits.com/ptlc-proof-of-concept/).)
- **DLC implementations:** `rust-dlc`, `p2pderivatives` / `cfd-dlc-js`, and Suredbits' `bitcoin-s`.

---

## 10. Comparison / summary table

| Aspect | Schnorr adaptor (Taproot) | ECDSA adaptor (legacy) |
|---|---|---|
| Extra machinery | None (nonce offset) | DLEQ / sigma ZK proof required |
| Assumptions | DL / Schnorr security | DL + soundness of DLEQ proof |
| Adapt | `s = s' + t` | `s = s_a · y⁻¹` |
| Extract | `t = s − s'` | `y = s_a · s⁻¹` |
| Bitcoin availability | Taproot (BIP-340), 2021+ | Available pre-Taproot (deployed in DLCs) |
| Leakage caveat | Minimal | Leaks DH key of signing & encryption keys |
| Aggregation | MuSig2 / FROST natural | Threshold ECDSA (heavy MPC) |

| Primitive / paper | Venue | ePrint | Contribution |
|---|---|---|---|
| Scriptless scripts | MIT Bitcoin Expo 2017 | — | Poelstra: original concept |
| Generalized Channels | ASIACRYPT 2021 | 2020/476 | First standalone AS definition (aEUF-CMA, adaptability, extractability) |
| Two-Party AS from ID schemes | PKC 2021 | 2021/150 | Generic construction; 2-party Schnorr & ECDSA |
| Stronger Security & Generic Constructions | INDOCRYPT 2022 | 2022/1687 | Unlinkability; ruled-out insecure schemes; BLS/PQC |
| LedgerLocks | CCS 2023 | 2023/1315 | UC framework, AS-locked txs |
| Foundations of AS | EUROCRYPT 2024 | 2024/1809 | Definitional gaps; breaks blind conditional sigs |
| Witness-hiding AS (NP) | ASIACRYPT 2024 | 2024/1051 | Witness hiding from OWFs, any NP relation |
| Threshold/Multi AS | MDPI 2024 | — | t-of-n & multi; Schnorr + Dilithium |
| VTS (timed) | CCS 2020 | 2020/1563 | Time-locked signatures; BLS/Schnorr/ECDSA |
| LAS (post-quantum) | FC 2021 | 2020/1345 | Lattice adaptor sig; PQ channels/swaps |

**Verifiable Timed (Adaptor) Signatures.** Thyagarajan, Bhat, Malavolta, Döttling, Kate & Schröder, "Verifiable Timed Signatures Made Practical," **CCS 2020**, ePrint [2020/1563](https://eprint.iacr.org/2020/1563): time-lock a signature so it can be *forcibly extracted* after time `T` (via time-lock puzzles + a cut-and-choose validity proof), instantiated for BLS/Schnorr/ECDSA. **Verifiable Timed Adaptor Signatures** extend this and cut the verification cost from `O(n)` to `O(1)`, giving swaps/DLCs a built-in "the secret will become available by time T" guarantee without an on-chain timelock.

---

## 11. Open problems

- **Consolidating the "right" definition.** Post-Foundations, the community has *several* strengthened notions (witness hiding, unlinkability, stronger extractability, blind variants). A single agreed, composable definition — and re-verification of deployed DLC/PTLC/hub code against it — is still in progress.
- **Efficient, provably secure threshold ECDSA adaptors.** Combining threshold ECDSA MPC + DLEQ + the strengthened notions, without the DH-key leakage caveat, remains heavy and under-specified.
- **PTLC deployment at scale.** PTLCs are spec'd and prototyped but not yet the default routing primitive across the Lightning implementations; the eltoo/Taproot dependency chain is the gating factor.
- **Practical post-quantum adaptors for Bitcoin.** Lattice/isogeny adaptors work in theory but are large/slow; their relevance is contingent on Bitcoin's own signature migration.
- **Universal / cross-chain adaptor secrets.** Batching many swap legs under one witness (universal adaptor secret) needs careful security analysis to avoid one compromised leg leaking the universal secret.
- **Blind conditional signatures / hubs.** The Foundations paper showed the old route was broken; secure, efficient anonymous-payment-hub constructions are an active target (see 2026/060).

---

## 12. References

**Foundational concept**
- A. Poelstra, *Scriptless Scripts*, MIT Bitcoin Expo, Mar 2017. [slides](https://download.wpsoftware.net/bitcoin/wizardry/mw-slides/2017-03-mit-bitcoin-expo/slides.pdf) · [L2 summit 2018 slides](https://download.wpsoftware.net/bitcoin/wizardry/mw-slides/2018-05-18-l2/slides.pdf) · [ElementsProject/scriptless-scripts](https://github.com/ElementsProject/scriptless-scripts)

**Formal definitions & constructions**
- Aumayr, Ersoy, Erwig, Faust, Hostáková, Maffei, Moreno-Sanchez, Riahi. *Generalized Channels from Limited Blockchain Scripts and Adaptor Signatures.* ASIACRYPT 2021. ePrint [2020/476](https://eprint.iacr.org/2020/476).
- Erwig, Faust, Hostáková, Maitra, Riahi. *Two-Party Adaptor Signatures from Identification Schemes.* PKC 2021. ePrint [2021/150](https://eprint.iacr.org/2021/150).
- Dai, Okamoto, Yamamoto. *Stronger Security and Generic Constructions for Adaptor Signatures.* INDOCRYPT 2022. ePrint [2022/1687](https://eprint.iacr.org/2022/1687).
- Gerhart, Schröder, Soni, Thyagarajan. *Foundations of Adaptor Signatures.* EUROCRYPT 2024. ePrint [2024/1809](https://eprint.iacr.org/2024/1809).
- Liu, Tzannetos, Zikas. *Adaptor Signatures: New Security Definition and a Generic Construction for NP Relations.* ASIACRYPT 2024. ePrint [2024/1051](https://eprint.iacr.org/2024/1051).
- *Blind Adaptor Signatures, Revisited.* ePrint [2026/060](https://eprint.iacr.org/2026/060).

**Threshold / multi-party**
- *Threshold/Multi Adaptor Signature and Their Applications in Blockchains.* Electronics (MDPI) 13(1):76, 2024. [doi](https://doi.org/10.3390/electronics13010076).
- *Consecutive Adaptor Signature Scheme: From Two-Party to N-Party Settings.* ePrint [2024/241](https://eprint.iacr.org/2024/241).
- *A Multi-Party, Multi-Blockchain Atomic Swap Protocol with Universal Adaptor Secret.* arXiv [2406.16822](https://arxiv.org/pdf/2406.16822).
- *Universal Adaptor Signatures from Blackbox Multi-party Computation.* Springer 2025. [chapter](https://link.springer.com/chapter/10.1007/978-3-031-88661-4_16).

**Frameworks & applications**
- Tairi, Moreno-Sanchez, Schneidewind. *LedgerLocks: A Security Framework for Blockchain Protocols Based on Adaptor Signatures.* CCS 2023. ePrint [2023/1315](https://eprint.iacr.org/2023/1315).
- T. Dryja. *Discreet Log Contracts.* MIT DCI, 2018. [paper](https://adiabat.github.io/dlc.pdf).
- *From Multi-sig to DLCs: Modern Oracle Designs on Bitcoin.* arXiv [2602.09822](https://arxiv.org/pdf/2602.09822).
- *Efficient ECDSA-Based Adaptor Signature for Batched Atomic Swaps.* ePrint [2024/140](https://eprint.iacr.org/2024/140).

**Timed**
- Thyagarajan, Bhat, Malavolta, Döttling, Kate, Schröder. *Verifiable Timed Signatures Made Practical.* CCS 2020. ePrint [2020/1563](https://eprint.iacr.org/2020/1563).

**Post-quantum**
- Esgin, Ersoy, Erkin. *Post-Quantum Adaptor Signature for Privacy-Preserving Off-Chain Payments (LAS).* FC 2021. ePrint [2020/1345](https://eprint.iacr.org/2020/1345).
- *Post-Quantum Adaptor Signatures and Payment Channel Networks.* ESORICS 2020. ePrint [2020/845](https://eprint.iacr.org/2020/845).
- Renan, Kutas. *SQIAsignHD: SQIsignHD Adaptor Signature.* arXiv [2404.09026](https://arxiv.org/pdf/2404.09026) (ePrint 2024/561).

**Specs & implementations**
- [`discreetlogcontracts/dlcspecs` — ECDSA-adaptor.md](https://github.com/discreetlogcontracts/dlcspecs/blob/master/ECDSA-adaptor.md)
- [`BlockstreamResearch/scriptless-scripts` — multi-hop-locks.md](https://github.com/BlockstreamResearch/scriptless-scripts/blob/master/md/multi-hop-locks.md)
- [`BlockstreamResearch/secp256k1-zkp` — ECDSA adaptor PR #117](https://github.com/BlockstreamResearch/secp256k1-zkp/pull/117) · [FROST PR #138](https://github.com/BlockstreamResearch/secp256k1-zkp/pull/138)
- [`rust-secp256k1-zkp` docs](https://docs.rs/secp256k1-zkp) · [rust-bitcoin/rust-secp256k1 #292](https://github.com/rust-bitcoin/rust-secp256k1/issues/292)
- Bitcoin Optech: [PTLCs](https://bitcoinops.org/en/topics/ptlc/) · Suredbits: [Scriptless Scripts](https://suredbits.com/schnorr-applications-scriptless-scripts/), [PTLC PoC](https://suredbits.com/ptlc-proof-of-concept/)
- conduition.io: [*The Riddles of Adaptor Signatures*](https://conduition.io/scriptless/adaptorsigs/)

---

*Compiled from IACR ePrint, ASIACRYPT/EUROCRYPT/PKC/CCS/INDOCRYPT proceedings, Blockstream Research, and the DLC/Lightning specifications. Where an author list or ePrint ID could not be independently confirmed, the entry is described by title, venue, and identifier for verification.*
