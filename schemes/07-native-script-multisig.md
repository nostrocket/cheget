# Native Script-Level Multisignature & Threshold Spending on Bitcoin

*The on-chain "threshold" approach that needs NO threshold cryptography — a contrast to threshold ECDSA/Schnorr/FROST.*

Last updated: 2026-07-03. Primary sources are BIPs, Bitcoin Optech, Bitcoin Core docs, the Miniscript specification and rust-miniscript. Cross-reference the cryptographic threshold-signature documents in this series for MuSig2/FROST detail.

---

## 1. Overview & Framing: two very different meanings of "threshold"

Bitcoin has supported *k-of-n* spending since 2012, but there are two fundamentally different ways to achieve it, and they are constantly conflated:

| | **Native script multisig (this document)** | **Cryptographic threshold signatures (MuSig2 / threshold ECDSA / FROST)** |
|---|---|---|
| Where the "k-of-n" lives | In **Bitcoin Script**, enforced by network consensus | In a **cryptographic protocol** run off-chain among signers |
| What the chain sees | The policy, all *n* public keys, and *k* signatures (script-path) | A **single ordinary-looking signature** on one aggregate key |
| Key generation | Independent — each signer just makes a keypair, no interaction | Distributed Key Generation (DKG) or interactive setup |
| Signing | Each signer signs independently; no rounds between them | Interactive multi-round protocol (or preprocessing) |
| Trust model | Consensus-enforced; no cryptographic protocol to get wrong | Relies on the security of the MPC/threshold scheme |
| Accountability | **Which keys signed is visible on-chain** (audit feature / privacy cost) | Indistinguishable — you cannot tell who signed, or that it was multisig at all |

The core insight: **native multisig moves the threshold logic onto the blockchain**, where it is transparent, verifiable, and flexible, at the cost of larger transactions and worse privacy. **Threshold cryptography moves the threshold logic off-chain**, producing a single signature that is small, private, and indistinguishable from a single-signer spend, at the cost of interactive protocols, DKG, and a more complex trust/implementation model.

Taproot (2021) blurs the line: a Taproot **key-path** spend can carry a MuSig2/FROST aggregate signature (looks single-sig — see the crypto docs), while the **script-path** can still hold a fully transparent native multisig as a fallback. This document focuses on the native, script-enforced mechanisms and contrasts them with the cryptographic approach throughout.

---

## 2. Mechanism deep dive

### 2.1 Bare multisig — P2MS + `OP_CHECKMULTISIG` (BIP 11)

**BIP:** [BIP 11 — M-of-N Standard Transactions](https://github.com/bitcoin/bips/blob/master/bip-0011.mediawiki) (standardized Jan 2012).

The original threshold primitive. The output script (`scriptPubKey`) directly embeds every public key:

```
OP_m <pubkey_1> <pubkey_2> ... <pubkey_n> OP_n OP_CHECKMULTISIG
```

Example 2-of-3: `OP_2 <pk1> <pk2> <pk3> OP_3 OP_CHECKMULTISIG`.

Spending witness/scriptSig:

```
OP_0 <sig_1> <sig_2>          # OP_0 is the dummy element (see the bug below)
```

**The `OP_CHECKMULTISIG` off-by-one "dummy element" bug.** `OP_CHECKMULTISIG` pops **one more element off the stack than it should**. It was meant to consume the *n* pubkeys, the *m* signatures, and the two count values; instead it pops one extra item. Because this bug shipped in the very first Bitcoin release and old coins depend on it, it can never be fixed — so every multisig spend must prepend a throwaway "dummy" stack element. [BIP 147 (Dealing with dummy stack element malleability)](https://github.com/bitcoin/bips/blob/master/bip-0147.mediawiki) made this dummy a consensus-enforced NULL: it must be `OP_0` (an empty byte vector), removing a malleability vector. In a P2WSH context the dummy is the empty push `00`.

**Limits & pitfalls of bare multisig:**
- **20-key hard cap.** `OP_CHECKMULTISIG` supports at most 20 public keys (limit set in Bitcoin Core's `script.h`). Each public key in a checkmultisig also counts as 20 toward the legacy 201-opcode / sigops accounting.
- **Non-standard beyond 3 keys.** Bare P2MS is only *relay-standard* (`IsStandard`) up to **3 pubkeys**. Larger bare multisig can be mined but won't propagate through default nodes.
- **Terrible footprint & privacy.** All pubkeys sit in the output script forever, bloating the UTXO set, and are visible before spending. This is why bare multisig is essentially obsolete for custody; it survives mostly as a data-embedding vector and inside P2SH/P2WSH.

### 2.2 P2SH multisig (BIP 16)

**BIP:** [BIP 16 — Pay to Script Hash](https://github.com/bitcoin/bips/blob/master/bip-0016.mediawiki) (activated April 2012).

P2SH hides the script behind a hash. The output commits only to `HASH160(redeemScript)`:

```
scriptPubKey:  OP_HASH160 <20-byte script hash> OP_EQUAL
```

The full multisig lives in the **redeem script**, revealed only at spend time:

```
redeemScript:  OP_2 <pk1> <pk2> <pk3> OP_3 OP_CHECKMULTISIG
scriptSig:     OP_0 <sig1> <sig2> <redeemScript>
```

Benefits over bare multisig: sender pays a short, fixed-size address (`3...`); the pubkeys/policy stay off-chain until spent; larger k-of-n is standard.

**Limits & pitfalls:**
- **520-byte redeem-script cap.** The redeem script is pushed onto the stack, and Bitcoin's max element push size is 520 bytes. With 33-byte compressed keys this caps P2SH multisig at **15-of-15** (15-of-15 is ~513 bytes and *just* fits). So the practical universe is "up to 15 keys," not 20.
- On-chain footprint at spend time still reveals the entire redeem script (all *n* pubkeys) plus *k* signatures — large and non-private.

### 2.3 P2WSH multisig — SegWit v0 (BIP 141 / BIP 143)

**BIPs:** [BIP 141 — Segregated Witness (consensus)](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki), [BIP 143 — Transaction Signature Verification for v0 witness programs](https://github.com/bitcoin/bips/blob/master/bip-0143.mediawiki).

SegWit moves the redeem script and signatures into the **witness**, which enjoys the **witness discount** (witness bytes cost 1 weight unit vs 4 for base bytes → ~75% cheaper). The output is a 32-byte SHA-256 witness-script hash:

```
scriptPubKey:  OP_0 <32-byte SHA256(witnessScript)>   # bech32 "bc1q..." address
witness:       0 <sig1> <sig2> <witnessScript>
witnessScript: OP_2 <pk1> <pk2> <pk3> OP_3 OP_CHECKMULTISIG
```

Key improvements:
- **520-byte P2SH limit no longer applies.** P2WSH scripts are bounded by a **3,600-byte standardness policy limit** and a **10,000-byte consensus limit**, comfortably allowing up to the 20-key `OP_CHECKMULTISIG` maximum.
- **256-bit hash** (SHA-256 vs P2SH's HASH160) → collision-resistant even for scripts with attacker-influenced keys.
- **BIP 143** introduced a new sighash digest that fixes the O(n²) signature-hashing problem and prevents fee/value blindness during hardware-wallet signing.
- Can be wrapped as **P2SH-P2WSH** (nested SegWit, `3...` address) for backward compatibility with senders that don't understand bech32.

This (`wsh(multi(...))` / `wsh(sortedmulti(...))`) was the dominant institutional and collaborative-custody multisig format from ~2019 until Taproot adoption grew.

### 2.4 Taproot — key-path vs script-path (BIP 340 / 341 / 342)

**BIPs:** [BIP 340 — Schnorr signatures](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki), [BIP 341 — Taproot: SegWit v1 spending rules](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki), [BIP 342 — Validation of Taproot Scripts (Tapscript)](https://github.com/bitcoin/bips/blob/master/bip-0342.mediawiki). Activated November 2021.

A Taproot output is a single 32-byte tweaked public key `Q`:

```
Q = P + H_TapTweak(P || merkle_root) · G
```

- **P** = the internal public key (a single key, or a MuSig2/FROST **aggregate** of many keys — see the crypto docs).
- **merkle_root** = root of a Merkle tree of alternative spending scripts (the MAST).

Two ways to spend:

**Key-path spend.** Provide one 64/65-byte BIP-340 Schnorr signature for `Q`. The witness is a *single element*. This is **indistinguishable from any other Taproot single-sig spend** — an observer cannot tell whether it was one key or a 3-of-5 institutional cold wallet cooperating via MuSig2. This is the privacy win, and where native multisig meets threshold crypto: put a threshold/aggregate key at `P` and cooperative spends look single-sig.

**Script-path spend.** Reveal one leaf script, its inputs, and a **control block** (1 byte version+parity + 32-byte internal key + 32×tree-depth Merkle path; the tree can be up to depth 128). Only the *revealed* leaf is exposed — all sibling scripts stay hidden behind their hashes. This is where transparent, native k-of-n multisig lives as the "uncooperative"/fallback path.

**MAST / TapScript trees** let you commit to *many* independent spending conditions and reveal only the one you use. A typical custody design: key-path = MuSig2 of the everyday signers (private, cheap); script-path leaves = explicit `multi_a` thresholds and timelocked recovery branches (transparent, only revealed if the cooperative path fails).

### 2.5 Tapscript k-of-n: `OP_CHECKSIGADD` and `multi_a` (BIP 342)

Tapscript **disables `OP_CHECKMULTISIG` and `OP_CHECKMULTISIGVERIFY`** (they behave like `OP_RETURN` — fail and terminate). Their replacement is **`OP_CHECKSIGADD`** (opcode `0xba`).

`OP_CHECKSIGADD` pops three stack items — a signature, a `CScriptNum` accumulator `n`, and a public key. If the signature is valid it pushes `n+1`; if the signature is an empty vector it pushes `n` unchanged. It is "functionally equivalent to `OP_ROT OP_SWAP OP_CHECKSIG OP_ADD` but in one byte." This lets you **accumulate a count of valid signatures** and compare to a threshold:

```
<pk1> OP_CHECKSIG <pk2> OP_CHECKSIGADD ... <pkn> OP_CHECKSIGADD <k> OP_NUMEQUAL
```

Witness supplies, for each key position, either a signature or an empty vector — so a k-of-n `multi_a` spend always pushes *n* witness items (one per key), which subtly **leaks n** (the total number of keys) even for the used branch.

**Why the change (vs `OP_CHECKMULTISIG`):**
- **Batch verifiability.** BIP-340 Schnorr + `CHECKSIGADD`'s one-signature-per-key structure enables batch signature validation, unlike the "try each sig against each key" `CHECKMULTISIG` loop.
- **Clean per-opcode accounting.** Tapscript **removes the 10,000-byte script limit and the 201-opcode limit**, replacing global sigop counting with a **per-input sigops budget = 50 + witness size in bytes**; each executed signature check costs 50. This ties signature-checking cost to the fees paid.

The descriptor form is **`multi_a(k, KEY_1, ..., KEY_n)`** (and `sortedmulti_a`), standardized in [BIP 387 — Tapscript Multisig Output Script Descriptors](https://github.com/bitcoin/bips/blob/master/bip-0387.mediawiki). `multi_a`/`sortedmulti_a` are only valid inside a `tr()` (tapscript) context.

### 2.6 Threshold via Taproot script trees: two encodings, a real trade-off

There are two ways to express k-of-n inside Taproot script paths, with a genuine efficiency/privacy trade-off:

1. **Single `multi_a` accumulator leaf.** One leaf `<pk1> CHECKSIG <pk2> CHECKSIGADD ... k NUMEQUAL`. Compact tree (one leaf), but the spend pushes an item *per key* (revealing *n*), and includes all *n* pubkeys in the revealed script.

2. **Enumerate k-of-k leaves (combinatorial MAST).** Put every k-subset of the n keys as its own `k`-of-`k` leaf in the tree — e.g. a 2-of-3 becomes three leaves `{pk1&pk2}, {pk1&pk3}, {pk2&pk3}`. A spend reveals only the leaf actually used, exposing **only the k keys that signed** and hiding the rest. Smaller per-spend witness for the revealed leaf and better privacy about *n*.

The cost of enumeration is **combinatorial blowup**: the number of leaves is C(n,k), which explodes for large wallets (e.g. 7-of-11 = 330 leaves). So the guidance is: enumerate for small n (privacy + witness efficiency win), use `multi_a` for larger n (avoid leaf explosion). And for the *cooperative* case, prefer neither — use a **MuSig2/FROST key-path** so the normal spend is a single indistinguishable signature, keeping the script tree purely as an uncooperative fallback.

---

## 3. Miniscript & Output Descriptors — how wallets express k-of-n and complex policies

Two complementary technologies turn human spending intent into exact Bitcoin Script and back:

- **Output Descriptors** ([BIP 380](https://github.com/bitcoin/bips/blob/master/bip-0380.mediawiki) and the family [BIP 381–386](https://github.com/bitcoin/bips), [BIP 387](https://github.com/bitcoin/bips/blob/master/bip-0387.mediawiki)) solve the **portability/backup** problem: a string that fully specifies a wallet's scripts, keys, and derivation paths so any wallet can rederive every address and know what to sign. Relevant fragments: `multi`/`sortedmulti` ([BIP 383](https://github.com/bitcoin/bips/blob/master/bip-0383.mediawiki)) for `CHECKMULTISIG`-based multisig, `multi_a`/`sortedmulti_a` ([BIP 387](https://github.com/bitcoin/bips/blob/master/bip-0387.mediawiki)) for tapscript, `tr()` ([BIP 386](https://github.com/bitcoin/bips/blob/master/bip-0386.mediawiki)) for Taproot, and wrappers `sh()`/`wsh()`.
- **Miniscript** (the [Miniscript spec, bitcoin.sipa.be/miniscript](https://bitcoin.sipa.be/miniscript/); rust-miniscript; merged into Bitcoin Core descriptors via PRs [#24147](https://github.com/bitcoin/bitcoin/pull/24147)/[#24148](https://github.com/bitcoin/bitcoin/pull/24148)) solves the **analysis/composition** problem. It is a typed, structured subset of Script where every expression has a known size, a known satisfaction (spend) set, and a guaranteed one-to-one mapping to Script bytecode.

`sortedmulti`/`sortedmulti_a` lexicographically sort the keys so all cosigners deterministically derive the same script/address regardless of key order — the standard for interoperable hardware-wallet multisig.

**Descriptor examples:**
- `wsh(sortedmulti(2,xpubA/0/*,xpubB/0/*,xpubC/0/*))` — a portable 2-of-3 P2WSH wallet.
- `tr(MUSIG_OR_NUMS_INTERNAL, sortedmulti_a(2,A,B,C))` — Taproot with a script-path 2-of-3.
- `tr(internal_key, {multi_a(2,A,B,C), and_v(v:pk(D),older(2016))})` — 2-of-3 now **OR** a single key D after a ~2-week relative timelock.

**Policy compilation.** A high-level *policy* (e.g. `thresh(2, pk(A), pk(B), pk(C))` or `or(99@thresh(2,...), 1@and(pk(D),older(52560)))`) is compiled to the *optimal* miniscript by **minimizing expected witness weight for the most likely spending branch** (the weights `99@`/`1@` express probabilities). The same policy can compile to different Script depending on expected usage. Wallets then know, for every branch, the exact worst-case fee and exactly what to request from each signer.

The `thresh(k, sub_1, ..., sub_n)` fragment is more general than `multi`: the sub-expressions can themselves be arbitrary conditions (other thresholds, timelocks, hashlocks), enabling nested/heterogeneous thresholds that pure threshold cryptography cannot express.

---

## 4. Timelocks + thresholds: recovery, inheritance & decaying multisig

Native script's headline advantage over threshold crypto is expressing **arbitrary spending logic**, especially timelocked fallbacks.

- **`OP_CHECKLOCKTIMEVERIFY` (CLTV)** — [BIP 65](https://github.com/bitcoin/bips/blob/master/bip-0065.mediawiki) — absolute timelock (block height or Unix time): "not spendable until date/height X."
- **`OP_CHECKSEQUENCEVERIFY` (CSV)** — [BIP 112](https://github.com/bitcoin/bips/blob/master/bip-0112.mediawiki), using the relative-locktime semantics of the `nSequence` field ([BIP 68](https://github.com/bitcoin/bips/blob/master/bip-0068.mediawiki)) — relative timelock: "not spendable until N blocks/time *after this coin was confirmed*." CSV underpins "dead-man's-switch" inactivity recovery.

In Miniscript these become `after(n)` (CLTV) and `older(n)` (CSV).

**Canonical "decaying multisig" / inheritance policy** — *2-of-3 now, OR 1-of-3 after 1 year*:

```
or_d(
  multi(2, primary, cosigner, backup),          # normal: 2-of-3 anytime
  and_v(v:older(52560), thresh(1, ...heirs...))  # fallback: 1 key after ~1 year (52560 blocks)
)
```

Real products built on exactly this:
- **Liana** (Wizardsardine) is built entirely on Miniscript timelock recovery: primary spending path plus one or more decaying/recovery paths that activate after a period of key inactivity.
- **Nunchuk** ships generalized Miniscript (2026), enabling timelocked inheritance, tiered business-treasury approvals, and non-custodial escrow with a compact on-chain footprint; its Group Wallets use Miniscript out of the box.
- **Unchained** and **Casa** offer structured inheritance/recovery products; Unchained has published detailed analyses of the tradeoffs of Miniscript timelock wallets (e.g. the need to periodically "refresh" coins so timelocks don't silently mature).

**Important pitfall** (Blockstream's "Don't Mix Your Timelocks"): mixing absolute (CLTV/height-or-time) and relative (CSV) or height-vs-time timelocks in one branch is a footgun; and coins whose recovery timelocks are counting down must be periodically moved/refreshed to reset the clock.

Threshold cryptography, by contrast, **cannot express any of this on its own** — a FROST or threshold-ECDSA group produces one signature and has no notion of "after 1 year" or "OR this other condition." Timelock/recovery logic always requires script. This is a decisive reason many custody designs are *hybrid*: threshold key on the key-path for private everyday spends, native script leaves for timelocked recovery.

---

## 5. Covenants relevant to threshold custody (brief)

Covenants restrict *where* coins can move next, enabling vaults and programmatic custody. As of mid-2026 these remain **proposals, not activated consensus rules**; the 2022–2024 "covenants yes/no" debate has shifted to "which combination."

- **`OP_CHECKTEMPLATEVERIFY` (CTV)** — [BIP 119](https://github.com/bitcoin/bips/blob/master/bip-0119.mediawiki). Commits a UTXO to a specific template of its next transaction. Enables simple vaults, congestion-control batching, and non-interactive channels. Saw renewed activation discussion through 2025–2026 but no confirmed timeline.
- **`OP_VAULT` / `OP_VAULT_RECOVER`** — [BIP 345](https://github.com/bitcoin/bips/blob/master/bip-0345.mediawiki). Purpose-built dynamic vaults with an enforced withdrawal delay and a recovery path; designed to compose with CTV.
- **`OP_CAT` revival** — [BIP 347](https://github.com/bitcoin/bips/blob/master/bip-0347.mediawiki). Re-enabling concatenation unlocks general covenants and script-based verification (and underpins some BitVM-style constructions).
- **LNHANCE** bundles CTV + `OP_CHECKSIGFROMSTACK` (CSFS) + `OP_INTERNALKEY`, targeting Lightning improvements.

Relevance to threshold custody: covenants would let a *vault* enforce "funds can only move to the pre-approved cold address after a delay," which today must be approximated with pre-signed transactions + multisig + timelocks. They complement rather than replace k-of-n.

---

## 6. PSBT — the coordination format for multi-party native signing

**BIPs:** [BIP 174 — PSBT v0](https://github.com/bitcoin/bips/blob/master/bip-0174.mediawiki), [BIP 370 — PSBT v2](https://github.com/bitcoin/bips/blob/master/bip-0370.mediawiki).

Because native multisig requires *k* independent signatures collected from separate devices/parties, there must be a standard container to pass a half-signed transaction around. That is the PSBT. It defines **six roles**: **Creator, Updater, Signer, Combiner, Finalizer, Extractor**. In a multisig flow a coordinator (Sparrow, Nunchuk, Caravan, Specter) builds the PSBT and plays most roles; each **hardware wallet is a Signer**; the **Combiner** merges the independently-produced partial signatures; the **Finalizer** assembles the final `scriptSig`/witness.

The PSBT carries prev-UTXO values, derivation paths, and the redeem/witness/tapscript so an **air-gapped** signer can verify amounts, fees, and change addresses on its own screen without touching the internet. **BIP 370 (PSBT v2)** lets inputs/outputs be added incrementally (better for coin-selection and interactive construction) and adds explicit version fields; Sparrow and Nunchuk support v2 in primary workflows, though BIP 174 remains the interoperability baseline. Taproot fields for PSBT were added by [BIP 371](https://github.com/bitcoin/bips/blob/master/bip-0371.mediawiki).

Threshold-signature schemes need their *own* coordination protocol (nonce exchange, partial signatures over the MPC network) — PSBT is the analogue but for on-chain multisig, and it is a mature, cross-vendor, air-gap-friendly standard, which is a real operational advantage of the native approach.

---

## 7. Comparison table: native multisig vs threshold ECDSA vs FROST/MuSig2

| Property | **Native multisig (P2WSH `multi` / Tapscript `multi_a`, script-path)** | **Threshold ECDSA (MPC, e.g. GG20/CMP)** | **FROST / MuSig2 (Schnorr, Taproot key-path)** |
|---|---|---|---|
| On-chain object | *k* signatures + full script + *n* pubkeys (script-path) | **1 ECDSA signature** on one aggregate key | **1 Schnorr signature** on one aggregate key |
| Looks like single-sig on-chain? | **No** — policy & keys visible | **Yes** (pre-Taproot P2PKH/P2WPKH single-sig) | **Yes** — indistinguishable from any Taproot key-path spend |
| Reveals *n*, *k*, and *which* keys signed | **Yes** (script-path); accountability by design | No | No |
| Distributed key generation (DKG) | **Not needed** — independent keypairs | **Required** (interactive DKG) | Required for FROST; MuSig2 needs key aggregation but no secret-sharing DKG |
| Signing interaction rounds | **None between signers** — each signs independently; PSBT passed around | Multiple interactive rounds | 2 rounds (MuSig2/FROST), can pre-process nonces |
| Arbitrary policies (timelocks, OR, nesting) | **Yes** — full Script/Miniscript expressiveness | No (signature only) | No (signature only) — needs script for anything beyond k-of-n |
| Accountability / audit trail | **On-chain** (which signers signed is provable) | None on-chain | None on-chain |
| Consensus/crypto risk | Consensus-enforced; **no novel crypto protocol** | Depends on MPC scheme security & implementation | Depends on scheme; Schnorr aggregation well-studied |
| Per-spend footprint (relative, k-of-n) | **Largest** — grows with *k* and *n* | Smallest (single sig) | Smallest (single sig) |
| Fees (see §8) | Highest, scales with *k*,*n* | Low, ~single-sig | Low, ~single-sig |
| Backup complexity | Must back up **descriptor + all n xpubs**; lose descriptor → hard to recover | Back up key shares | Back up key shares (+ any script tree) |
| Maturity / tooling | **Very mature**: PSBT, descriptors, every major HWW | Production but heavier; audited MPC libs | MuSig2 shipping; FROST maturing 2024–2026 |
| Cross-institution setups | Excellent — each party keeps its own key, no shared protocol | Needs shared MPC infra/coordination | Needs shared protocol/coordination |

Hybrid designs (Taproot) get the best of both: **key-path = MuSig2/FROST** for private, cheap cooperative spends; **script-path = native `multi_a` + timelocks** as a transparent, consensus-enforced fallback.

---

## 8. Footprint, fees & privacy — the quantitative contrast

Approximate spend-input sizes (single input; figures are indicative, exact values depend on signature grinding and key count):

- **Taproot key-path (single or MuSig2/FROST aggregate):** witness = one 64-byte Schnorr sig → input ≈ **57–58 vbytes**. Cheapest and most private; a 3-of-5 cooperating via MuSig2 costs the same as one signer.
- **P2WSH 2-of-3 (`CHECKMULTISIG`):** witness carries 2 sigs (~72 B each) + ~105-byte witness script → input ≈ **~104 vbytes** (after the 75% witness discount). Larger than key-path, and grows with *k* and *n*.
- **Tapscript `multi_a` 2-of-3 (script-path):** the revealed leaf script (all 3 x-only pubkeys + opcodes) + 2 sigs + control block (≥33 B). Comparable to or a bit larger than P2WSH per spend, but only paid when the *uncooperative* path is used; the cooperative key-path is far cheaper.
- **Legacy P2SH 2-of-3:** no witness discount → the largest and most expensive of the practical options.

**Privacy/fungibility.** Native script-path spends **broadcast the policy** — everyone learns it was a 2-of-3, sees the pubkeys, and can link the wallet's coins by their distinctive script. This harms fungibility and enables clustering/surveillance. Threshold signatures and Taproot key-path spends produce a **single signature indistinguishable** from ordinary payments, so the existence, threshold, and membership of the multisig never touch the chain. The Taproot key-path is the native ecosystem's answer to this privacy gap.

**Accountability is the mirror image.** For many custody/compliance settings, *seeing which keys signed on-chain* is a **feature** — it gives an auditable, cryptographically provable record of who authorized a spend, which threshold signatures deliberately erase.

---

## 9. Real-world usage

- **Hardware-wallet multisig.** Coldcard, Trezor, Ledger, BitBox, Jade, etc. sign native multisig via PSBT. Coldcard pioneered air-gapped PSBT-on-SD-card multisig. `sortedmulti`/`sortedmulti_a` descriptors give deterministic cross-vendor addresses.
- **Collaborative custody.** **Unchained** and **Casa** (2-of-3 / 3-of-5 where the provider holds one key), **Nunchuk** (self-hosted collaborative wallets, encrypted coordination chat, generalized Miniscript), **Liana** (Miniscript timelock recovery). All rely on native multisig + PSBT rather than MPC, precisely because independent keys + no DKG + on-chain accountability suit multi-party, cross-organization custody.
- **Exchange / institutional cold storage.** Large k-of-n (e.g. 3-of-5, 4-of-7) P2WSH or Taproot wallets with geographically distributed hardware and PSBT signing ceremonies; some institutions instead use MPC/threshold-ECDSA products (Fireblocks-style) for privacy and single-sig footprint — a direct instance of the trade-off.
- **Bitcoin federations.** **Liquid** (Blockstream) secures peg-in bitcoin with an **11-of-15 multisig** across 15 functionary HSMs (watchmen), plus a **timelocked Emergency Withdrawal Procedure**: after a timelock expires, a separate Blockstream-held emergency backup multisig (in deep cold storage) can recover funds. This is native script threshold + timelock at federation scale — no threshold cryptography.

---

## 10. Pitfalls & limits (summary)

- **`OP_CHECKMULTISIG` dummy-element bug** — must prepend `OP_0`/empty (BIP 147); a classic implementation gotcha.
- **Standardness limits** — bare P2MS standard only to 3 keys; larger needs P2SH/P2WSH.
- **Script-size limits** — P2SH 520-byte redeem script caps ~15 keys; P2WSH lifts this (3,600 policy / 10,000 consensus); Tapscript removes the 10,000-byte and 201-opcode limits, replacing them with a per-input sigops budget (50 + witness bytes).
- **`multi_a` leaks *n*** — pushes one witness item per key; use enumerated k-of-k leaves for small n to hide the unused keys, at the cost of C(n,k) leaf blowup.
- **Backup complexity** — you must retain the full **descriptor + every xpub**; losing the descriptor (not just keys) can make funds hard/impossible to recover. Address/derivation complexity is the top operational risk in multisig custody.
- **Timelock footguns** — don't mix absolute/relative or height/time timelocks in one branch; decaying-multisig coins must be periodically refreshed before recovery timelocks mature (Unchained/Blockstream analyses).
- **Privacy/fungibility cost** — script-path multisig publishes the policy and links coins; use Taproot key-path (MuSig2/FROST) for private cooperative spends.
- **No native arbitrary logic in threshold crypto** — any timelock/OR/recovery still needs script, so most robust designs are hybrid.

---

## 11. When to use which

- **Choose native multisig when** you need on-chain accountability/audit, arbitrary policies (timelocks, recovery, nested conditions), cross-institution setups where each party independently holds a key with **no DKG or interactive protocol**, maximum implementation maturity, and simplest trust model (consensus-enforced, no novel crypto).
- **Choose threshold signatures (MPC/FROST/MuSig2) when** you prioritize **privacy/fungibility** and **minimal on-chain footprint/fees**, want spends indistinguishable from single-sig, and can operate the interactive signing/DKG infrastructure.
- **Choose hybrid Taproot when** you want both: threshold/aggregate key on the **key-path** for private, cheap everyday spends, plus native `multi_a` + timelock **script-path** leaves for transparent, consensus-enforced recovery/fallback.

---

## References

**Core consensus BIPs**
- [BIP 11 — M-of-N Standard Transactions (bare multisig)](https://github.com/bitcoin/bips/blob/master/bip-0011.mediawiki)
- [BIP 16 — Pay to Script Hash (P2SH)](https://github.com/bitcoin/bips/blob/master/bip-0016.mediawiki)
- [BIP 141 — Segregated Witness (consensus)](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki)
- [BIP 143 — Transaction Signature Verification for v0 witness programs](https://github.com/bitcoin/bips/blob/master/bip-0143.mediawiki)
- [BIP 147 — Dealing with dummy stack element malleability](https://github.com/bitcoin/bips/blob/master/bip-0147.mediawiki)
- [BIP 340 — Schnorr Signatures for secp256k1](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
- [BIP 341 — Taproot: SegWit v1 spending rules](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki)
- [BIP 342 — Validation of Taproot Scripts (Tapscript, OP_CHECKSIGADD)](https://github.com/bitcoin/bips/blob/master/bip-0342.mediawiki)

**Timelocks**
- [BIP 65 — OP_CHECKLOCKTIMEVERIFY (CLTV)](https://github.com/bitcoin/bips/blob/master/bip-0065.mediawiki)
- [BIP 68 — Relative lock-time using consensus-enforced sequence numbers](https://github.com/bitcoin/bips/blob/master/bip-0068.mediawiki)
- [BIP 112 — OP_CHECKSEQUENCEVERIFY (CSV)](https://github.com/bitcoin/bips/blob/master/bip-0112.mediawiki)

**Descriptors & Miniscript**
- [BIP 380 — Output Script Descriptors (General)](https://github.com/bitcoin/bips/blob/master/bip-0380.mediawiki)
- [BIP 383 — `multi()` / `sortedmulti()` descriptors](https://github.com/bitcoin/bips/blob/master/bip-0383.mediawiki)
- [BIP 386 — `tr()` Taproot descriptors](https://github.com/bitcoin/bips/blob/master/bip-0386.mediawiki)
- [BIP 387 — `multi_a()` / `sortedmulti_a()` Tapscript multisig descriptors](https://github.com/bitcoin/bips/blob/master/bip-0387.mediawiki)
- [BIP 388 — Wallet Policies for Descriptor Wallets](https://github.com/bitcoin/bips/blob/master/bip-0388.mediawiki)
- [Miniscript specification (bitcoin.sipa.be/miniscript)](https://bitcoin.sipa.be/miniscript/)
- [rust-miniscript](https://github.com/rust-bitcoin/rust-miniscript) · [python-bip380](https://github.com/darosior/python-bip380)
- Bitcoin Core Miniscript integration PRs [#24147](https://github.com/bitcoin/bitcoin/pull/24147), [#24148](https://github.com/bitcoin/bitcoin/pull/24148)

**PSBT**
- [BIP 174 — Partially Signed Bitcoin Transaction Format (v0)](https://github.com/bitcoin/bips/blob/master/bip-0174.mediawiki)
- [BIP 370 — PSBT Version 2](https://github.com/bitcoin/bips/blob/master/bip-0370.mediawiki)
- [BIP 371 — Taproot fields for PSBT](https://github.com/bitcoin/bips/blob/master/bip-0371.mediawiki)
- [Bitcoin Core PSBT documentation](https://github.com/bitcoin/bitcoin/blob/master/doc/psbt.md)

**Covenants**
- [BIP 119 — OP_CHECKTEMPLATEVERIFY (CTV)](https://github.com/bitcoin/bips/blob/master/bip-0119.mediawiki)
- [BIP 345 — OP_VAULT](https://github.com/bitcoin/bips/blob/master/bip-0345.mediawiki)
- [BIP 347 — OP_CAT](https://github.com/bitcoin/bips/blob/master/bip-0347.mediawiki)
- [Bitcoin Optech — Vaults](https://bitcoinops.org/en/topics/vaults/) · [OP_CHECKTEMPLATEVERIFY](https://bitcoinops.org/en/topics/op_checktemplateverify/)

**Bitcoin Optech & general**
- [Optech — Output Script Descriptors](https://bitcoinops.org/en/topics/output-script-descriptors/) · [PSBT](https://bitcoinops.org/en/topics/psbt/)
- [Optech Newsletter #385 — 2025 Year-in-Review](https://bitcoinops.org/en/newsletters/2025/12/19/)

**Custody / real-world**
- [Liquid Federation multisig (Blockstream Help Center)](https://help.blockstream.com/hc/en-us/articles/900002386446-How-does-the-Liquid-Federation-s-multisig-work)
- [Liana Wallet](https://lianawallet.com/) · [Nunchuk — Miniscript: Programmable Bitcoin (2026)](https://nunchuk.io/blog/miniscript-programmable-bitcoin)
- [Unchained — Tradeoffs of Miniscript timelock wallets](https://www.unchained.com/blog/examining-the-tradeoffs-of-miniscript-timelock-wallets)
- [Blockstream — Don't Mix Your Timelocks](https://medium.com/blockstream/dont-mix-your-timelocks-d9939b665094)

**Reference/education**
- [learnmeabitcoin — P2MS](https://learnmeabitcoin.com/technical/script/p2ms/) · [P2SH](https://learnmeabitcoin.com/technical/script/p2sh/) · [P2WSH](https://learnmeabitcoin.com/technical/script/p2wsh/)
- [Bitcoin Wiki — OP_CHECKMULTISIG](https://en.bitcoin.it/wiki/OP_CHECKMULTISIG)
- [Bitcoin Optech Taproot workshop — Tapscript notebook](https://github.com/bitcoinops/taproot-workshop/blob/master/2.3-tapscript.ipynb)
