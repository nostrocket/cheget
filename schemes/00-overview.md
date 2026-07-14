# Threshold Signature Schemes for Bitcoin — Landscape Overview

> A survey of every class of threshold and multi-party signing scheme applicable to
> Bitcoin, synthesized from deep research into the primary cryptographic literature
> (IACR ePrint, CRYPTO/EUROCRYPT/CCS/S&P/PKC, IETF RFCs, and Bitcoin BIPs) current to
> mid-2026. Each scheme *type* has its own detailed document; this file is the map.

---

## 1. The core question

Bitcoin validates two signature algorithms, both over the **secp256k1** curve:

- **ECDSA** — legacy, P2PKH, P2SH, P2WPKH, P2WSH (pre-Taproot).
- **BIP340 Schnorr** — Taproot (BIP341/342), key-path and Tapscript.

A *threshold signature scheme* (TSS) lets **t-of-n** parties jointly produce **one ordinary
signature** under a single public key, such that fewer than *t* parties can neither sign nor
recover the key. The decisive property for Bitcoin is that the output is a **standard
signature** — 64/71–72 bytes, verified by unmodified consensus rules, and **indistinguishable
on-chain from a single-signer spend**. This gives privacy, fungibility, and minimal fees while
distributing custody. The alternative — expressing the policy in **Bitcoin Script** — is simpler
and more flexible but publishes the whole policy on-chain.

Everything below is a way to resolve that tension, or a building block for it.

---

## 2. The seven scheme types (document index)

| # | Document | Class | Bitcoin curve/target | Native on-chain? |
|---|----------|-------|----------------------|------------------|
| 1 | [`01-threshold-ecdsa.md`](01-threshold-ecdsa.md) | Threshold **ECDSA** (t-of-n) | secp256k1 / legacy + SegWit | ✅ single ECDSA sig |
| 2 | [`02-threshold-schnorr-frost.md`](02-threshold-schnorr-frost.md) | Threshold **Schnorr** / FROST (t-of-n) | secp256k1 / Taproot BIP340 | ✅ single 64-byte sig |
| 3 | [`03-multisignatures.md`](03-multisignatures.md) | **Multisignatures** (n-of-n, key aggregation) | secp256k1 / Taproot BIP340 | ✅ single 64-byte sig |
| 4 | [`04-adaptor-signatures.md`](04-adaptor-signatures.md) | **Adaptor signatures** / scriptless scripts | secp256k1 / ECDSA + Schnorr | ✅ ordinary sig + hidden witness |
| 5 | [`05-dkg-and-vss.md`](05-dkg-and-vss.md) | **DKG / VSS** (key-generation building block) | secp256k1 | — (setup layer) |
| 6 | [`06-pairing-based-bls.md`](06-pairing-based-bls.md) | **Pairing-based BLS** threshold | BLS12-381 / BN254 (**not** secp256k1) | ❌ Bitcoin cannot verify pairings |
| 7 | [`07-native-script-multisig.md`](07-native-script-multisig.md) | **Native script** k-of-n (no threshold crypto) | Bitcoin Script | ✅ policy published on-chain |

The seven divide into three tiers:

- **Cryptographic threshold signing that Bitcoin accepts natively** — types 1, 2, 3, 4. These
  are the heart of the subject: an off-chain multi-party protocol yields a signature the chain
  cannot distinguish from single-sig.
- **The building block underneath all of them** — type 5 (DKG/VSS): how the shared key is created
  and maintained without a trusted dealer. Every scheme in 1–3 depends on it.
- **The contrast cases** — type 6 (BLS: the cleanest threshold scheme but *not verifiable on
  Bitcoin*) and type 7 (native script: no cryptography, but on-chain footprint and full policy
  flexibility). These bound the design space on either side.

---

## 3. Cross-scheme comparison matrix

The signing schemes that Bitcoin verifies natively, plus the two contrast cases:

| Property | Threshold ECDSA | FROST (Schnorr) | MuSig2 (multisig) | Adaptor sigs | Native script | BLS threshold |
|---|---|---|---|---|---|---|
| **Threshold** | t-of-n | t-of-n | n-of-n only | wraps any of these | k-of-n | t-of-n |
| **Bitcoin curve** | secp256k1 | secp256k1 | secp256k1 | secp256k1 | secp256k1 | BLS12-381 ❌ |
| **Signature type** | standard ECDSA | BIP340 Schnorr | BIP340 Schnorr | standard sig | k sigs in script | pairing (n/a) |
| **On-chain footprint** | 1 sig (hidden) | 1 sig (hidden) | 1 sig (hidden) | 1 sig (hidden) | full policy + k sigs | cannot verify |
| **Signing rounds** | 4–6 → **2** (2025) | 2 (1 preprocessable) | 2 | +offset on host | 0 (independent) | **1**, non-interactive |
| **DKG required?** | yes (heavy: Paillier/CL setup) | yes (or dealer) | **no** (independent keys) | inherits host | **no** | yes |
| **Deterministic sig?** | no | no | no (MuSig-DN yes) | no | per-signer | **yes** (unique) |
| **Robust / identifiable abort** | GG20/CGGMP: yes | ROAST wrapper: yes | no | no | n/a (consensus) | native (verifiable partials) |
| **Adaptive security** | CGGMP proactive | static; adaptive is frontier | static (AGM+ROM) | scheme-dependent | n/a | achievable |
| **Standardization** | NIST IR 8214C (2026) | RFC 9591 + BIP 445 (draft) | **BIP327** (final) | dlcspecs | BIPs 11/16/141/341/342/387 | IETF draft; RFC 9380 |
| **Main assumption** | DL + Paillier / OT / CL | DL (ROM; AGM for some) | AOMDL (AGM+ROM) | DL + host scheme | consensus | co-CDH / gap-DH (pairing) |
| **Hardest part** | MtA share conversion + ZK proofs | nonce binding vs ROS | 2-nonce trick vs ROS | ECDSA needs DLEQ proof | script size / privacy | not verifiable on BTC |

Key reading of the matrix:

- **Schnorr-family schemes are strictly nicer than ECDSA** for threshold work: Schnorr's
  linearity makes distributed signing (FROST) and adaptor construction almost trivial, while
  ECDSA's `k⁻¹(H(m)+r·d)` structure forces the whole **multiplicative-to-additive (MtA)**
  machinery and heavy zero-knowledge proofs. Taproot is the reason FROST/MuSig2 now dominate new
  designs; threshold ECDSA persists only because it also serves **pre-Taproot** addresses and the
  vast installed base.
- **Multisig (n-of-n) vs true threshold (t-of-n):** MuSig2 needs *no DKG* — each party keeps its
  own key and all must sign — which is why it standardized first (BIP327) and shipped in
  production. FROST adds genuine *t-of-n* fault tolerance at the cost of a DKG.
- **Adaptor signatures are orthogonal, not competing:** they *decorate* any of the host schemes
  (ECDSA, Schnorr, MuSig2, FROST) with a hidden-witness condition, enabling PTLCs, DLCs, and
  atomic swaps with no new opcodes.
- **BLS is the cleanest scheme and the worst Bitcoin fit** — a genuine impossibility, not an
  engineering gap (§5).

---

## 4. How the pieces fit together

```
                        ┌─────────────────────────────────────────┐
                        │   DKG / VSS  (doc 05)  — the setup layer  │
                        │   Pedersen · GJKR · ChillDKG · ADKG ·     │
                        │   proactive refresh (CHURP, CGGMP)        │
                        └───────────────┬───────────────────────────┘
                                        │ produces a secp256k1 shared key
              ┌─────────────────────────┼─────────────────────────┐
              ▼                         ▼                          ▼
   ┌────────────────────┐   ┌────────────────────┐    ┌────────────────────┐
   │ Threshold ECDSA 01 │   │ Threshold Schnorr  │    │  Multisig (n-of-n) │
   │  (legacy + SegWit) │   │  FROST 02 (Taproot)│    │  MuSig2 03(Taproot)│
   └─────────┬──────────┘   └─────────┬──────────┘    └─────────┬──────────┘
             │                        │                         │
             └───────────┬────────────┴─────────────┬───────────┘
                         ▼                           ▼
              ┌────────────────────┐     produces one on-chain signature that
              │ Adaptor sigs  04   │     Bitcoin verifies with NO change and
              │ (PTLC/DLC/swap)    │     that looks identical to single-sig
              └────────────────────┘
   ─────────────────────────────────────────────────────────────────────────
   CONTRAST:  Native script k-of-n (07) — no crypto, policy on-chain, flexible
   CONTRAST:  BLS threshold (06) — cleanest scheme, but NOT verifiable on Bitcoin
```

DKG (doc 05) is the shared foundation: threshold ECDSA, FROST, and (optionally) multisig all sit
on top of it. Adaptor signatures (doc 04) sit *on top of* the signing schemes. The two contrast
documents bound the space — one shows what Bitcoin can't do (BLS), the other what it could
always do without any cryptography (script).

---

## 5. The most important findings across all seven documents

1. **Schnorr/Taproot changed the game.** FROST (Komlo–Goldberg, SAC 2020) and MuSig2
   (Nick–Ruffing–Seurin, CRYPTO 2021 → BIP327) exploit Schnorr's linearity for clean, concurrently
   secure, two-round signing. Threshold ECDSA remains far more complex and is favored only for
   pre-Taproot compatibility. **For new Bitcoin custody targeting Taproot, FROST (t-of-n) or MuSig2
   (n-of-n) is the state of the art.**

2. **The ROS problem is the recurring villain for two-round schemes.** Naive two-round
   Schnorr/multisig is broken by Wagner/ROS (Benhamouda et al., EUROCRYPT 2021) once concurrent
   sessions exceed ~log₂p. MuSig2 and FROST both defeat it with the **two-nonce / binding-factor**
   trick — this is *why* MuSig2 needs 2+ nonces and *why* FROST binds nonces to the signer set.

3. **Threshold ECDSA's real-world failures were all implementation ZK-proof bugs, not scheme
   breaks.** Alpha-Rays, TSSHOCK (Black Hat 2023), and Fireblocks BitForge (CVE-2023-33241/33242)
   all trace to dropped or mis-encoded Paillier range/soundness proofs — even against a *UC-secure*
   CGGMP21 implementation. The lesson: the load-bearing ZK proofs are not optional.

4. **The field converged on two-round threshold ECDSA in late 2025** (Trout 2025/1666; "Threshold
   ECDSA in Two Rounds" 2025/1696, both CCS'25) via **class groups / non-interactive
   multiplication**, and 2025/1696 is the first threshold-optimal scheme to provably avoid the
   Groth–Shoup presignature security loss — directly relevant to any presigning deployment.

5. **Adaptive security is the live theoretical frontier for threshold Schnorr.** Sparkle (CRYPTO
   2023) claimed it (only to t/2, with AGM); Meier's "Plausible Attack" (CRYPTO 2025) proved a
   *barrier* — FROST, Sparkle, Lindell'22 can't be shown fully adaptively secure without new
   assumptions — spawning a wave of DDH-based, AGM-free schemes (Twinkle, HARTS, Glacius) and a
   fresh static+adaptive proof for Sparkle (ePrint 2026/431). **Adaptive-secure schemes are not yet
   BIP340 drop-in.**

6. **The 2024 "Foundations of Adaptor Signatures" (EUROCRYPT 2024) showed the original adaptor
   definitions were insufficient** — a scheme could meet them yet admit two valid signatures from
   one pre-signature, breaking Lightning-style payment-hub unforgeability. Pre-2024 composed-protocol
   security claims should be treated with care.

7. **DKG subtlety that everyone gets wrong:** the GJKR key-*biasing* attack on Pedersen DKG is real,
   but the same authors proved threshold **Schnorr tolerates the bias** — which is exactly why
   FROST/ChillDKG safely use the *cheaper, biasable* DKG (rigorously reconfirmed by the AGM-free
   Olaf proof, 2023/899). Separately, ChillDKG is deliberately **non-robust** (abort-with-blame),
   on the principle that a robust keygen silently degrades t-of-n; robustness belongs at *signing*
   time (ROAST), not keygen.

8. **BLS is a true impossibility for Bitcoin, not a missing feature.** secp256k1 is deliberately
   pairing-hostile, Script has no `OP_PAIRING`/EC/bignum opcodes, and consensus ossification rules
   out a BLS12-381 precompile of the Ethereum EIP-2537 kind. Every system that actually moves BTC
   trustlessly uses a *secp256k1* threshold scheme (threshold ECDSA or FROST); BLS appears only
   off-chain, or conceivably via a SNARK adjudicated by BitVM. **`OP_CAT` enables covenants but
   still cannot compute a pairing.**

9. **The strongest custody designs in 2026 are hybrid Taproot:** a MuSig2/FROST aggregate key on
   the private, cheap **key-path** for everyday cooperative spends, with transparent,
   consensus-enforced native `multi_a` + CLTV/CSV **timelock leaves** in the script tree as
   recovery fallbacks (Liana, Nunchuk, Liquid). This unifies documents 2, 3, and 7: threshold
   crypto for the common case, native script for what crypto *cannot* express (timelocks, ORs,
   recovery logic).

---

## 6. Choosing a scheme (decision guide)

| If you need… | Use | Why |
|---|---|---|
| t-of-n over **Taproot** (new deployments) | **FROST** (doc 02) | 2-round, private, robust via ROAST |
| n-of-n cooperative key over Taproot | **MuSig2 / BIP327** (doc 03) | no DKG, standardized, in production |
| t-of-n over **pre-Taproot** (legacy/SegWit) | **Threshold ECDSA** (doc 01) | only native option for ECDSA outputs |
| Atomic swaps / PTLCs / DLCs | **Adaptor signatures** (doc 04) | conditional payments, no new opcodes |
| On-chain **accountability** or complex policy (timelocks, ORs, inheritance) | **Native script** (doc 07) | consensus-enforced, auditable, flexible |
| Both privacy *and* recovery logic | **Hybrid Taproot** (docs 2/3 + 7) | key-path aggregate + script-path fallback |
| A pairing-based threshold sig | **BLS** (doc 06) — but **not on Bitcoin** | usable only off-chain / via SNARK bridge |
| To generate/maintain the shared key | **DKG/VSS** (doc 05) | required setup for docs 1–3 |

---

## 7. Standardization & maturity snapshot (mid-2026)

- **BIP327 (MuSig2)** — final; shipped in libsecp256k1 v0.6.0, Ledger, Lightning Loop.
- **RFC 9591 (FROST)** — published; but `FROST(secp256k1)` is *not* BIP340-compatible on its own —
  Bitcoin needs the separate **BIP 445 (bip-frost-signing)** + **ChillDKG** work (both Draft).
- **NIST IR 8214C** — finalized 2026-01-20; threshold ECDSA explicitly in scope.
- **dlcspecs** — normative adaptor-signature scheme for DLCs (ECDSA variant flagged "DLCs only").
- **Native multisig** — BIPs 11/16/141/342/387 all active; covenants (CTV/119, OP_VAULT/345,
  OP_CAT/347) proposed but **unactivated** as of mid-2026.

---

## 8. Open problems (aggregated)

- Fully **adaptively secure** threshold Schnorr that is BIP340 drop-in and AGM-free.
- **Robust, asynchronous DKG** over secp256k1 that is efficient at wallet scale (ChillDKG is
  non-robust by design; ADKG needs t < n/3).
- **Presigning without security loss** in practice (Groth–Shoup); interaction with BIP32
  derivation.
- Rigorous **composition** proofs for threshold + adaptor + tweaking stacks (post-2024 foundations).
- Standardization gap: BIP 445 / ChillDKG still Draft; no finalized BIP340-native t-of-n signing.
- Practical **post-quantum** threshold signing for a future Bitcoin soft-fork (lattice multisig:
  Squirrel, Chipmunk, DualMS — none Bitcoin-native).

---

*Each linked document contains per-scheme deep dives (authors, venue, year, ePrint IDs, protocol
rounds, security models, costs, known attacks, implementations), in-family comparison tables, and
full reference lists. Start with this overview, then descend into the document for the scheme type
you need.*
