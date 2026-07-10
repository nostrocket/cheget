# Pitfalls Research

**Domain:** FROST threshold Schnorr (RFC 9591 / BIP340) + Bitcoin Taproot key-path + Nostr transport, at t=501/n=1000
**Researched:** 2026-07-10
**Confidence:** HIGH (spec-derived + curated threshold-signature failure-mode literature in-repo)

> These are the concrete, domain-specific ways `tsig`-class systems go catastrophically
> wrong. Each expands a normative rule from SPEC §11 (or a known failure mode from the
> threshold-Schnorr / Taproot literature) into: what breaks, why people do it, structural
> prevention, early-warning signs, and the milestone (M1–M5, SPEC §13) that must close it.
>
> "Catastrophic" here has a precise meaning: **key extraction** (adversary reconstructs the
> group secret), **fund loss** (invalid or wrong-destination signature broadcast), or
> **silent divergence** (the group can no longer produce a valid signature for its own
> address). Ordinary bugs are moderate/minor.

---

## Critical Pitfalls

### Pitfall 1: Persisting or reusing signing nonces (the key-extraction bug class)

**What goes wrong:**
A signer's FROST partial signature is `sᵢ = dᵢ_nonce + ρᵢ·eᵢ_nonce + λᵢ·e·dᵢ`. The nonce
pair `(dᵢ, eᵢ)` is the *only* secret masking the long-term share `dᵢ`. If the same
committed nonce pair is ever used to sign **two different messages** (two different
sighashes, i.e. two different challenges `e`), the adversary — including a malicious
coordinator, who legitimately sees every `sᵢ` — has two linear equations in the same
unknowns and solves directly for the share `dᵢ`. Collect 501 extracted shares from one
epoch and the group key is reconstructed forever. This is not a theoretical edge: it is the
single documented "reset/replay" catastrophe of randomized two-round FROST (scheme survey
§7; Arctic's entire motivation, §3.15).

The trap fires through *innocuous-looking persistence*:
- Writing the round-1 nonce secret to disk "so a crashed signer can resume."
- A crash-then-resign loop where the process reloads the same `SigningNonces`.
- Retrying a timed-out session by **reusing the same `SigningCommitments`** against a
  new `SigningPackage` (SPEC §6.5 step 5 — the coordinator rebuilds the package, challenge
  changes, nonce is the same).
- Checkpointing "all ceremony state" generically and sweeping the nonce in with it (the
  DKG round secrets *are* checkpointed between rounds — §8 — so a naive "persist round
  state" abstraction persists signing nonces too).

**Why it happens:**
Every other piece of ceremony state in this system is *deliberately* persisted for
resumability (SPEC §5: "all ceremony commands are resumable, checkpointed after every
round"). The nonce is the one violent exception, and it lives in the same code paths as the
things that must be saved. The natural, tidy engineering instinct — "make signing resumable
like everything else" — is exactly the bug.

**How to avoid (structural, not disciplinary):**
- **Type-level non-persistence.** Wrap `SigningNonces` in a newtype that does **not**
  implement `Serialize`/`Deserialize` and holds `zeroize::Zeroizing` inner state. If it
  cannot be serialized, it cannot be checkpointed — the compiler enforces the rule. This is
  the highest-leverage structural control in the whole project.
- **Separate the two state machines.** Ceremony (DKG/refresh) state is resumable and
  encrypted-checkpointed; signing-session state is *ephemeral and in-memory only*. Do not
  share a "resumable round state" trait between them.
- **New session on any restart or timeout.** A session identifier is minted fresh; a
  restarted/timed-out session is a **new** session with freshly generated nonces and a
  possibly different 501-subset. Never a resume (SPEC §6.5.5).
- **Bind the nonce lifetime to the process.** Generate in round 1, consume in round 2,
  drop (zeroize) immediately after producing `sᵢ`. No field, cache, or map outlives the
  single round-2 call.
- **Adversarial test that must fail closed:** a test that attempts to serialize
  `SigningNonces` should not compile; a test that reuses commitments across two
  `SigningPackage`s with different sighashes must be rejected before any `sᵢ` is emitted.

**Warning signs:**
- Any `derive(Serialize)` on a type transitively containing nonce material.
- The word "resume" or "checkpoint" appearing in the signing (not ceremony) module.
- A signing session that can be re-entered with the same session id.
- `SigningCommitments` stored in a map keyed by anything reused across sessions.
- Crash-recovery logic in the `sign join` path.

**Phase to address:** **M1** (structural type design must be right from the first signing
code) and re-verified in **M5** (external review of §6.5 + explicit nonce-reuse attempt in
the adversarial suite). This is SPEC §11.2 — the highest-severity rule in the spec.

---

### Pitfall 2: frost → rust-bitcoin key-bridge errors (x-only parity, wrong sighash, internal vs output key)

**What goes wrong:**
The bridge from `frost_secp256k1_tr`'s verifying key to a spendable Taproot address is a
dense knot of conventions, and getting *any* strand wrong yields either an unspendable
address (funds locked forever) or an invalid signature (fund movement fails, or worse, a
subtly wrong one). The classic failure modes, from BIP340/341 semantics (scheme survey §2):

1. **x-only truncation dropping parity.** FROST's verifying key is a 33-byte SEC1 point.
   BIP340 keys are 32-byte x-only with an implied even-Y. Slicing off the parity byte
   without letting the signing layer track the sign-flip (`gacc` bookkeeping, scheme survey
   §2.1) produces a signature that does not verify under the even-Y key.
2. **Internal key `P` vs output key `Q` confusion.** The FROST verifying key is the Taproot
   **internal** key `P`. The *address* commits to the **output** key `Q = P + t·G`,
   `t = H_taproot(P)` (SPEC §3). Building the address from `Q` but verifying the signature
   against `P` (or vice versa), or importing the wrong one into the watch-only descriptor,
   silently breaks spending.
3. **Wrong sighash.** BIP341 key-spend requires
   `SighashCache::taproot_key_spend_signature_hash` with the **default** sighash type
   (`SIGHASH_DEFAULT`), over *all* input prevouts (Taproot signs the whole prevout set).
   Using `SIGHASH_ALL` (0x01) appended, a legacy/segwit-v0 sighash, or omitting a prevout
   changes the challenge `e` and the signature is invalid.
4. **RFC 9591 ≠ BIP340 out of the box.** Plain `FROST(secp256k1, SHA-256)` uses the
   RFC-9591 ciphersuite hash, *not* BIP340's tagged key-prefixed challenge. Only the
   `-tr` (Taproot) crate applies the BIP340 challenge + tweak. Reaching for
   `frost-secp256k1` instead of `frost-secp256k1-tr` produces signatures that will never
   verify on-chain (scheme survey §8).

**Why it happens:**
Three independent convention systems (frost serialization, BIP340 x-only/parity, BIP341
tweak) meet at one function, and each looks individually plausible. rust-bitcoin's API will
happily build an address from whatever `XOnlyPublicKey` you hand it and happily hash
whatever sighash type you request — the mistake is silent until a real spend fails on
regtest (best case) or mainnet (worst case). The parity/tweak sign-flip is described in the
literature as "a notorious implementation footgun — get it from BIP 445 / secp256k1-zkp,
don't hand-roll" (scheme survey §7).

**How to avoid (structural):**
- **Pin the bridge with a byte-level round-trip test on day one** (SPEC §9, Active
  requirement): frost verifying key → 33-byte SEC1 → 32-byte x-only → `XOnlyPublicKey` →
  `Address::p2tr(secp, internal, None, network)`, and independently recompute `Q` and the
  address by hand; assert byte equality against a known-answer vector (BIP86 test vectors
  are the reference for the untweaked path).
- **Never hand-roll the tweak or parity.** Use `frost_secp256k1_tr::aggregate_with_tweak`
  / `sign_with_tweak` for the signing side and `Address::p2tr(..., None, ...)` for the
  address side — the two must agree by construction, not by coincidence. (See Pitfall 7 for
  the aggregation half.)
- **One canonical bridge function, one canonical sighash function.** Exactly one place
  converts key→address and exactly one computes the sighash from a PSBT. Everything else
  calls those. No ad-hoc `XOnlyPublicKey::from_slice` scattered around.
- **Sign a real regtest key-spend in M1** with 501 in-process simulated participants and
  broadcast it — the only test that proves all four strands simultaneously.
- **Known-answer regression vectors** committed to the repo so a dependency bump can't
  silently shift the convention.

**Warning signs:**
- `from_slice`/manual byte slicing of a public key outside the one bridge function.
- Any sighash type other than default in the key-spend path.
- Address derived without the taproot tweak, or signature verified against `P` not `Q`.
- Using `frost-secp256k1` (non-`-tr`) anywhere.
- The round-trip test asserts "it runs" rather than a hard-coded expected address string.

**Phase to address:** **M1** — this is the milestone's explicit purpose ("Proves the
frost↔rust-bitcoin bridge", SPEC §13.1). If M1 does not end with a broadcast, confirmed
regtest key-spend, the bridge is not proven.

---

### Pitfall 3: Mixed-epoch shares producing garbage signatures; sessions not bound to (key_id, epoch)

**What goes wrong:**
After a refresh/enroll, every holder's share is re-randomised: the group key `P` is
identical, but the *individual* share values `dᵢ` change (that is the whole point of
proactivization — old and new shares do not combine, SPEC §2 epoch-mixing property). If a
signing session accidentally mixes a round-2 partial from an epoch-`k` share with partials
from epoch-`k+1` shares, the Lagrange interpolation is over inconsistent points and the
aggregate signature is **garbage** — it will not verify under `Q`. Worse failure modes:
- The library may fail with an opaque internal error, or
- A stale share file from a prior epoch is loaded by a participant who didn't complete the
  last refresh, and they silently contribute a doomed partial, aborting the whole 501-way
  session with no clear culprit — a liveness disaster at n=1000.

**Why it happens:**
At steady state every participant holds **two shares** (ACTIVE + STANDBY, SPEC §4), and
across the key's life they accumulate historical epoch files. Share files are just bytes on
disk tagged `(key_id, epoch, identifier)`. Without hard binding, "load my share and sign"
picks the wrong file, and the failure surfaces far from the cause (at aggregation, not at
load).

**How to avoid (structural):**
- **Bind the session to `(key_id, epoch)` in the session-open event** and have every
  participant reject — loudly, early, before round 1 — any session whose `(key_id, epoch)`
  does not match a share they hold (SPEC §3, §6.5). "Fail early with a clear error" is a
  normative requirement, precisely because the library's own failure is opaque.
- **Tag every share file and every SigningPackage** with `(key_id, epoch)` and refuse to
  mix. The identifier alone is insufficient — a seat's identifier survives refresh.
- **Make the epoch a required, typed field** on the share, not a filename convention that
  can be renamed or misparsed.
- **Adversarial test (SPEC §13.5):** feed a session one epoch-`k` and 500 epoch-`k+1`
  partials; assert early rejection with an identifiable participant, never a garbage
  signature reaching `aggregate_with_tweak`.

**Warning signs:**
- Share files identified by `key_id` only, or by identifier only.
- A signing session that loads "the latest share" heuristically.
- Aggregation errors that don't name the offending seat/epoch.
- No epoch field in the session-open / SigningPackage schema.

**Phase to address:** **M3** (epoch bookkeeping is introduced with rotation) with the
adversarial mixed-epoch test in **M5**. The session-binding *schema* should be laid down in
**M2** so it doesn't need retrofitting.

---

### Pitfall 4: Skipping the mandatory client-side same-key postcondition check after refresh

**What goes wrong:**
A refresh/enroll re-runs a DKG-style protocol that *must* preserve the group verifying key
`P` (same address). If the refresh is buggy, malicious, or fed inconsistent inputs, the
resulting `PublicKeyPackage` can have a **different** verifying key — meaning the new shares
control a *different* address than the funds sit at. If participants trust the coordinator's
assertion ("refresh succeeded") instead of each independently checking
`new_pubkey == old_pubkey`, the group can complete a refresh, delete their old shares
(hygiene step, SPEC §6.3), and discover afterward that **nobody can spend the funds** — the
old key is gone, the new key controls a different address. This is unrecoverable fund loss
by protocol, not by theft.

**Why it happens:**
The refresh output *looks* successful — you get a valid `KeyPackage`. The verifying-key
mismatch is only visible if you explicitly compare against the pre-refresh value, and it is
tempting to let the coordinator (who aggregates confirmations) be the one to check. SPEC
§11.3 makes this "mandatory and client-side (never trust the coordinator's word for it)"
precisely because the trust boundary is the whole point: the coordinator is untrusted.

**How to avoid (structural):**
- **Every participant asserts `new PublicKeyPackage.verifying_key() == pinned old
  verifying_key` before accepting the new share and before deleting the old one** (SPEC
  §6.3 postcondition). Abort-and-discard on mismatch. This mirrors the proven pattern in
  production MPC libraries — coinbase/cb-mpc's refresh *hard-checks* `new_key.Q ==
  current_key.Q` (resharing survey §4).
- **Order of operations matters:** verify same-key → persist new share → *then* delete old
  share. Never delete-before-verify.
- **The pinned old key is local**, taken from the participant's own stored
  `PublicKeyPackage`, not from the ceremony-open event (which the coordinator controls).
- **Coordinator's aggregate check is a redundant belt, not the braces.** The client check
  is authoritative.

**Warning signs:**
- The same-key comparison exists only in coordinator code.
- Old share deleted before new key verified.
- The "old key" in the comparison is read from a ceremony event rather than local storage.
- Refresh "success" reported without a per-client verifying-key assertion in the logs.

**Phase to address:** **M3** — this is a headline M3 deliverable ("same-key postcondition
tests", SPEC §13.3). The test must include a *tampered-refresh* case that produces a
different key and confirm every client aborts.

---

### Pitfall 5: Roster-pinning failures — accepting events from npubs outside the pinned roster

**What goes wrong:**
Authenticity in this system is the Nostr event signature (BIP340 over the event) *is* the
envelope signature (SPEC §7). The roster is a pinned set of npubs, hash-committed in every
ceremony-open event. If the client accepts a `round1-package`, `signature-share`, or
`session-control` event from an npub **not** in the pinned roster — because it trusted the
relay's filtering, or because it never checked, or because it re-derived the roster from a
mutable source — then a non-member (or a compromised relay injecting events) can:
- Inject rogue DKG contributions to bias/break keygen,
- Impersonate the coordinator to drive a blind-sign (see Pitfall 9),
- Force spurious aborts (DoS), or
- Slip an extra "member" into the effective signing set.

**Why it happens:**
Relays *look* like they gatekeep (especially with NIP-42 AUTH restricting connections to
roster npubs), so it is tempting to treat relay-side AUTH as the access control. But SPEC
§7 is explicit: "Relays are never trusted to filter." NIP-42 AUTH is a liveness/DoS
convenience, not a security boundary — a hostile or buggy relay can still deliver anything.

**How to avoid (structural):**
- **Client-side roster verification on every inbound event**, unconditionally: the event's
  author npub must be in the pinned roster and the event's BIP340 signature must verify.
  Discard silently otherwise (SPEC §7, §11.4). This is a single choke-point filter through
  which *all* inbound events pass before any handler sees them.
- **Pin by hash.** The roster hash is committed in the ceremony-open event; clients verify
  the roster they hold matches that hash. Membership changes only via enroll/refresh
  ceremonies, "never by relay fiat" (SPEC §11.4).
- **Never treat NIP-42 AUTH as authorization.** Document it as DoS-reduction only.
- **Coordinator is just another roster npub** — its events get the same treatment; there is
  no privileged "coordinator bypass."
- **Adversarial test (SPEC §13.5):** a malicious relay injects a well-formed event from a
  non-roster npub; assert it is dropped before reaching any protocol handler.

**Warning signs:**
- Event handlers that trust `nostr-sdk` subscription results without re-checking author.
- Roster loaded from a relay query rather than pinned locally + hash-verified.
- Any code path where an event's effect depends on relay-side AUTH having filtered it.
- The coordinator's events handled by a different (more trusting) path than participants'.

**Phase to address:** **M2** (the transport and roster-pinning are built here) with the
malicious-relay injection test in **M5**.

---

### Pitfall 6: Nostr identity key reused as / derived from FROST material

**What goes wrong:**
Both the Nostr identity key and the FROST group/share material live on **secp256k1**. It is
trivially easy — and strictly forbidden (SPEC §11.6a) — to derive one from the other, or
reuse one as the other, "to save a keypair." Consequences:
- If the Nostr identity key is derived from a FROST share, compromise of the (necessarily
  more exposed, always-online, used-on-every-event) transport key leaks information about,
  or directly is, secret share material.
- If a single key serves both roles, the transport signature (public on every relay) and
  the FROST partial signatures are correlated, and the clean separation between "transport
  layer" and "key security" collapses — a relay observer or coordinator gains leverage they
  should never have.

**Why it happens:**
Convenience and apparent elegance: one keypair per member "is simpler." Same curve, same
libraries, `tsig init` is generating a keypair anyway — why not reuse it for the share?
Because the transport key is deliberately low-trust and high-exposure, and the share is the
crown jewel. Mixing trust tiers on one key destroys the model.

**How to avoid (structural):**
- **Generate the Nostr identity keypair independently at `tsig init`** with its own RNG
  draw, stored separately, never fed into any DKG/dealer/refresh input (SPEC §7, §11.6a).
- **No shared derivation seed.** The FROST share comes from the ceremony; the Nostr key
  comes from `init`. There is no common master secret from which both descend.
- **Type separation.** Nostr identity keys and FROST key material are distinct types with
  no conversion function between them. Make "turn my share into an npub" impossible to
  express.
- **Audit for cross-use** in M5: grep/type-check that no FROST secret ever flows into
  nostr-sdk key construction and vice versa.

**Warning signs:**
- A single "identity" abstraction used for both signing partials and event signing.
- Any function converting a share/`KeyPackage` into a nostr `Keys`/`SecretKey`.
- A master seed that generates both the transport key and share-related randomness.

**Phase to address:** **M2** (Nostr identity introduced with transport) — get the
separation right when `tsig init` is first written; re-audit in **M5**.

---

### Pitfall 7: Taproot tweak applied inconsistently during aggregation (`aggregate_with_tweak(…, None)`)

**What goes wrong:**
This is the aggregation-side twin of Pitfall 2. The signature must verify under the
**tweaked output key** `Q`, so the tweak has to be applied consistently across the whole
signing pipeline:
- Participants sign with `round2::sign_with_tweak`, and
- The coordinator aggregates with `aggregate_with_tweak(…, merkle_root: None)`.

Mismatches that silently produce an invalid (or wrong-key) signature:
- Aggregating with plain `aggregate` (no tweak) while participants used `sign_with_tweak`,
  or the reverse — the sign-flip / tweak bookkeeping doesn't line up.
- Passing a `Some(merkle_root)` when the design is **key-only, BIP86-style, merkle root
  `None`** (SPEC §3). Any non-`None` merkle root computes a *different* `Q`, hence a
  different address, hence a signature that doesn't match the funded address.
- Verifying the aggregate against `P` (internal) instead of `Q` (output) — the check passes
  in code but the on-chain spend fails.

**Why it happens:**
`frost-secp256k1-tr` exposes both tweaked and untweaked aggregation, and the tweak argument
is easy to get wrong (`None` vs `Some`). The untweaked path may even superficially "work" in
a unit test that verifies against the internal key, hiding the bug until a real spend.

**How to avoid (structural):**
- **Single signing pipeline that always uses the tweaked path** with `merkle_root: None`
  hard-wired (this design has no script tree — SPEC §1 non-goals exclude script-path
  spends). Don't expose the untweaked functions at all in the app layer.
- **Coordinator verifies the aggregate BIP340 signature against `Q`** (the output key /
  address key), not `P`, before finalizing the PSBT (SPEC §6.5.4). This catch is the last
  line before broadcast.
- **The M1 broadcast test** exercises exactly this path end-to-end; a signature that
  verifies against `Q` and confirms on regtest proves the tweak is consistent.
- **Assert `merkle_root == None`** structurally (the type carries no merkle root) so a
  future contributor can't pass `Some`.

**Warning signs:**
- Both `aggregate` and `aggregate_with_tweak` reachable from app code.
- Any `Some(merkle_root)` in the aggregation call.
- Final verification against the internal key `P`.
- Unit tests that verify partials/aggregate against `P` and never against the actual
  address key `Q`.

**Phase to address:** **M1** (part of proving the bridge; inseparable from Pitfall 2).

---

### Pitfall 8: Blind signing — participants trusting the coordinator's sighash instead of recomputing from the PSBT

**What goes wrong:**
FROST partial signing signs whatever challenge `e = H(R, Q, m)` the coordinator's
`SigningPackage` implies. If participants sign the *message/sighash the coordinator hands
them* without independently recomputing it from the PSBT, a **compromised coordinator can
get a 501-quorum to blind-sign an arbitrary transaction** — draining all funds to an
attacker address — while showing each participant a benign summary. At n=1000 the whole
security story rests on "no single party holds the key," but blind signing hands the
coordinator effective unilateral control (SPEC §11.7). This is the highest-value attack on
the running system after nonce reuse.

**Why it happens:**
It is dramatically easier to have the coordinator compute the sighash once and distribute
it — recomputing per input from the PSBT on 501 devices is more code and more UX friction.
The `--yes` flag and "just sign it" pressure at ceremony scale push toward blind signing.

**How to avoid (structural):**
- **Participants recompute the sighash from the PSBT locally** and compare against what
  they are being asked to sign; they sign the *result of their own computation*, never a
  coordinator-supplied hash (SPEC §6.5.3, §11.7). The coordinator sends the PSBT, not a
  sighash.
- **Display-before-sign is mandatory:** each participant is shown human-readable
  outputs/amounts/fee (recomputed locally) and must ack before round 2 runs (SPEC §5, §6.5,
  Active requirement). `--yes` should be available only for automated/regtest contexts and
  loudly flagged; it must never be the ceremony default.
- **The PSBT is the source of truth**, parsed independently per signer; the coordinator's
  role is transport, not authority.
- **Adversarial test:** a malicious coordinator sends a PSBT whose displayed summary differs
  from the actual outputs, or sends a mismatched sighash; assert participants detect and
  refuse.

**Warning signs:**
- The `SigningPackage`/session-control event carries a precomputed sighash that participants
  sign directly.
- Participants don't parse the PSBT themselves.
- `--yes` is the documented normal path, or there's no human-ack gate at all.
- No local recompute-and-compare step before `sign_with_tweak`.

**Phase to address:** **M1** (the sighash recompute + display gate should exist in the very
first signing flow, even in-process) hardened and adversarially tested in **M5**. Do not
defer display-before-sign to M5 — retrofitting it into an established coordinator-authoritative
flow is far more error-prone.

---

### Pitfall 9: Treating share deletion as a security control (it is not — the sweep is)

**What goes wrong:**
The residual risk of this entire design is stark and normative: **501 shares from any one
past epoch reconstruct the key forever** (SPEC §11.1). If the team (or the docs, or the
operators) come to believe that deleting old shares after refresh *revokes* the compromised
holders, they will under-invest in the sweep machinery and over-trust rotation. A retained
insider who simply *kept* their epoch-`k` share is untouched by any amount of refreshing —
refresh only defends against *external gradual* compromise (an adversary who must
re-compromise devices each epoch). The moment a compromise coalition of 501-in-one-epoch is
plausible, only an **on-chain sweep to the standby key** actually protects the funds.

**Why it happens:**
"We deleted the old shares" *feels* like revocation, and the refresh flow does delete old
`KeyPackage`s for hygiene (SPEC §6.3). It is a very natural but false inference. Verifiable
erasure on member hardware is assumed impossible (SPEC §1 non-goals, §11.1) — you cannot
prove a member deleted their share, and you must assume some didn't.

**How to avoid (structural / design-discipline):**
- **No security claim anywhere rests on deletion.** Every deletion in the codebase and docs
  is labeled "hygiene, best-effort, not a security control" (SPEC §6.3, §8). Bake this into
  the RETIRED-state comments and the transcript.
- **The sweep is a first-class, always-ready operation**, not an emergency scramble: the
  STANDBY key is pre-generated and kept refreshed so a sweep is *a signing session, not a
  ceremony* (SPEC §4, §6.6). Build it early enough that it is trusted.
- **Policy engine forces sweeps** on value cap, churn budget, and max epochs — both prize
  and coalition-pool are bounded, and *either alone is insufficient* (SPEC §10). The watcher
  must nag until a fresh standby exists post-sweep (SPEC §6.6).
- **Threat-model docs and UX** state plainly that rotation ≠ revocation; the sweep is the
  revocation. Prevents operators from delaying a due sweep because "we just refreshed."

**Warning signs:**
- Any comment/doc implying refresh or deletion "removes" or "revokes" a compromised holder.
- Sweep treated as an emergency path rather than routine, or standby key allowed to go
  stale (violating `standby_max_age`).
- Policy thresholds set as if churn budget alone (or value cap alone) were sufficient.
- The watcher not escalating when no standby exists after a sweep.

**Phase to address:** Conceptual discipline from **M1** (in the threat-model docs), enforced
operationally in **M4** (standby key + sweep flow + policy engine + watcher are the M4
deliverables, SPEC §13.4).

---

### Pitfall 10: Pointing an n=1000 ceremony at a public relay (bans, rate limits, retention loss, non-resumable at scale)

**What goes wrong:**
A DKG/refresh at n=1000 is O(n²): round 2 alone is ~999 directed packages **per sender**,
≈10⁶ events, ≈1 GB per ceremony (SPEC §8). Pointing that at a public Nostr relay gets the
whole roster **rate-limited or IP-banned mid-round**, and the ceremony stalls with partial
state scattered across relays. Related scale failures:
- **Retention misconfig:** a relay that prunes events (default retention) before all 1000
  participants have fetched their directed packages loses share deltas — participants can't
  complete `dkg::part3`, and the ceremony is stuck.
- **Rate-limit self-DoS:** even self-hosted relays with default limits throttle the
  round-2 burst; clients that publish 999 events in a tight loop trip the limiter.
- **Non-resumable ceremonies:** at 10⁶ events, *any* interruption (relay restart, network
  blip, a participant's laptop closing) is near-certain over the ceremony's wall-clock. If
  the ceremony can't resume idempotently from where it stopped, it must restart from
  scratch — and at this scale a from-scratch restart may never converge.

**Why it happens:**
Nostr *looks* like "just publish events," and public relays are the frictionless default in
every tutorial. The O(n²) blowup is invisible at the 2-of-3 / 3-of-5 scale where FROST is
usually demonstrated (e.g. Chainflip's 100-of-150 is already an order of magnitude smaller
in event count). Default relay configs are tuned for social-media traffic, not ceremony
bursts.

**How to avoid (structural / operational):**
- **Operators MUST run ≥3 dedicated self-hosted relays** (strfry / nostr-rs-relay), never a
  public relay for ceremonies (SPEC §7, §11, Constraints). Public relays may carry only
  low-volume session-control/sweep events as extra redundancy.
- **Relay configs explicitly raise rate limits and retention** for the ceremony event kinds
  before any n=1000 run; document the required strfry settings as part of operator setup
  (SPEC §8).
- **Clients publish round-2 in paced batches**, not a 999-event burst (SPEC §8).
- **Ceremonies are resumable and idempotent per `(ceremony_id, round, seat)`** via Nostr
  event-id dedup (SPEC §5, §13.2) — this is the structural defense against interruption at
  scale; readers merge and dedup across relays so a single reachable honest relay suffices.
- **Containerized n=1000 load test in M2** with relay rate-limit tuning is an explicit
  milestone deliverable (SPEC §13.2) — do not discover the O(n²) wall in production.
- **Offline `--in/--out` file mode** is a first-class fallback for when relays fail entirely
  (SPEC §7).

**Warning signs:**
- A public relay URL in any ceremony config or default.
- Round-2 publish loop with no pacing/batching.
- No idempotent dedup key `(ceremony_id, round, seat)` — resumption re-processes or
  duplicates.
- Relay retention/rate-limit left at defaults.
- No load test above ~dozens of simulated participants before real deployment.

**Phase to address:** **M2** (real transport, DKG at n=1000, relay tuning, resumable
ceremonies — the entire milestone, SPEC §13.2).

---

## Moderate Pitfalls

### Pitfall 11: FROST is not robust — one malformed/absent share aborts a 501-way session (liveness at scale)

**What goes wrong:**
FROST provides *identifiable abort* but **not robustness** (scheme survey §1.4, §3.6): a
single participant who drops out or sends a malformed partial aborts the whole signing
session. At n=1000 selecting *exactly* 501, the probability that all 501 stay live and
correct through two rounds is low; naive "pick 501, sign" loops will thrash.

**Why it happens:**
FROST's non-robustness is invisible at 2-of-3. At 501-of-1000 it dominates operational
reality.

**How to avoid:**
- Over-provision the liveness poll (select more than 501 live candidates; SPEC §6.5.2 says
  "select 501 live participants" — in practice poll a margin and finalize 501 from those
  who actually commit).
- On timeout/abort, start a **new session** (fresh nonces, possibly different subset — never
  reuse commitments, Pitfall 1) rather than retrying with the same set.
- Consider ROAST-style session management (scheme survey §3.8) if disruptor-resilience
  becomes a real problem — but note this is a coordinator meta-protocol, not a change to the
  crypto. Not required by the spec; flag as a hardening option.

**Warning signs:** signing sessions that hang on one non-responsive seat; retry loops that
reuse the same 501 and the same commitments.

**Phase to address:** **M1** (session/abort semantics) and stress-tested in **M2** (real
participants dropping out).

---

### Pitfall 12: Enroll without an immediate refresh — helpers retain delta knowledge, epoch boundary unclean

**What goes wrong:**
Enroll issues a share to a new seat via the repair/RTS technique: ≥501 helpers compute
delta contributions (SPEC §6.4). Those helpers now *know* deltas about the new seat's share.
If enroll is not immediately followed by a refresh, that knowledge lingers and the epoch
boundary is not clean — the new member's share is partially known to a helper coalition.

**Why it happens:**
Enroll and refresh feel like separate operations; batching "enroll now, refresh later" seems
efficient.

**How to avoid:**
- **Every enroll is immediately followed by a refresh in the same ceremony window** so the
  helpers' delta knowledge is proactivized away (SPEC §6.4). Batch semantics: enroll k
  members, then one refresh — but the refresh is not optional or deferrable.
- Encode this as a single atomic coordinator ceremony (`enroll` internally chains the
  refresh), not two independently invokable steps.

**Warning signs:** an `enroll` command that completes and increments epoch without a refresh;
docs describing enroll and refresh as independently schedulable.

**Phase to address:** **M3** (enroll + refresh, SPEC §13.3).

---

### Pitfall 13: Standby key neglect — its epoch-1 holders are a future dangerous coalition

**What goes wrong:**
The STANDBY key is generated in advance and is the sweep destination. But its own epoch-1
holders are a 501-coalition-in-one-epoch just like any active epoch (SPEC §4). If the
standby is generated once and never refreshed, sweeping to it may move funds into the hands
of a coalition that has had all the time in the world to form.

**Why it happens:**
"Standby" sounds passive; it's easy to forget it needs the same rotation cadence as active.

**How to avoid:**
- Keep the standby **refreshed on the same cadence as active** (SPEC §4).
- `standby_max_age` (default 90 d) forces regeneration; the watcher enforces it (SPEC §10).
- After a sweep, the watcher nags until a fresh standby exists (SPEC §6.6).

**Warning signs:** standby generated once at setup and never refreshed; `standby_max_age`
unset or ignored; no post-sweep standby-regeneration nag.

**Phase to address:** **M4** (lifecycle + policy, SPEC §13.4).

---

### Pitfall 14: NIP-44 misuse — confidential DKG/dealer/enroll payloads sent in the clear or mis-encrypted

**What goes wrong:**
DKG round-2 shares, dealer share exports, and enroll/repair deltas are **secret** and must
be NIP-44 v2 encrypted to the recipient's npub inside the signed event (SPEC §7). Sending
them in plaintext content (the default for public messages like round-1 packages and
signature shares) leaks share material to every relay and observer. Conversely, encrypting
the *public* messages needlessly, or getting the NIP-44 padding wrong, can leak payload
sizes (which shares vs. which are dummies).

**Why it happens:**
The event schema has both public-by-design and confidential message classes; a uniform
"encrypt everything" or "encrypt nothing" is simpler than getting the per-class distinction
right.

**How to avoid:**
- **Per-message-class encryption policy**, explicit in the schema: round-2 DKG bundles /
  dealer exports / enroll-repair deltas → NIP-44 v2 to recipient npub; round-1 packages,
  commitments, signature shares → plaintext (public by design). SPEC §7 enumerates these.
- Rely on **NIP-44 v2 padding** to hide share-payload sizes; don't roll custom padding.
- Optionally gift-wrap (NIP-59) for roster/metadata privacy — not required for key security,
  so don't let it block the ceremony.

**Warning signs:** a share/delta bundle in plaintext event content; the same
encrypt-or-not decision applied uniformly to all event kinds; custom padding logic.

**Phase to address:** **M2** (event schema + NIP-44, SPEC §13.2).

---

### Pitfall 15: Library/serialization version skew across 1000 heterogeneous clients

**What goes wrong:**
All 1000 participants serialize/deserialize `frost-secp256k1-tr` types (via the
`serialization` feature, base64 in event content, SPEC §7). If clients run different
`frost-core`/`frost-secp256k1-tr` versions, serialization formats or the ciphersuite can
diverge, producing packages that some clients can't parse or that yield inconsistent
results — a ceremony-wide failure that's brutal to debug across 1000 machines.

**Why it happens:**
1000 independent operators building/updating on their own schedules; a minor version bump
that changes a wire format.

**How to avoid:**
- **Pin exact library versions** (`Cargo.lock` committed, `frost-secp256k1-tr ≥3.0` pinned
  precisely) and gate the ceremony on a version handshake (the ceremony-open event declares
  the required version; clients on the wrong version refuse to join).
- **Reproducible builds** so "same version" means "same binary" (SPEC §11.8) — 1000 people
  can verify they run identical code.
- `cargo audit` / `cargo deny` in CI to catch a transitive bump.

**Warning signs:** no version field in ceremony-open; `Cargo.lock` not committed or deps
using `^`/`~` ranges that float; participants on mixed versions with no handshake.

**Phase to address:** **M2** (version handshake in the ceremony schema) and **M5**
(reproducible builds, pinned/audited deps).

---

## Minor Pitfalls

### Pitfall 16: Identifier reuse / collision when enrolling replacements

**What goes wrong:** A seat's `frost::Identifier` (1..=1000, u16) survives refresh; an
enrolled replacement gets the vacated or a fresh identifier, and the identifier space may
exceed 1000 historical values while the live set stays ≤1000 (SPEC §3). Accidentally
assigning a live identifier to two seats, or reusing a still-active one, corrupts Lagrange
interpolation.
**How to avoid:** the coordinator's SQLite roster is the single authority for
identifier↔npub↔status; allocate new identifiers from a monotonic counter, never recycle a
live one. **Phase:** M3.

### Pitfall 17: Dealer-mode state not destroyed / not recorded as a trust event

**What goes wrong:** In `keygen dealer` mode the dealer sees the full secret momentarily
(SPEC §2, §6.2). If dealer state isn't destroyed (best-effort) and the event isn't recorded
in the ceremony transcript (who, where, what hardware), the trust trade-off is invisible and
unauditable. **How to avoid:** zeroize dealer material after export; write the trust event
to the transcript (SPEC §11.5); recommend an air-gapped machine; keep DKG mode as the
no-dealer path. **Phase:** M1 (dealer keygen) / M2 (DKG).

### Pitfall 18: At-rest share encryption / zeroize gaps

**What goes wrong:** Shares written unencrypted, or share material left in memory (not
zeroized) after use, defeats the at-rest protection (SPEC §8). **How to avoid:** age/scrypt
passphrase encryption for all share files; `zeroize` on all in-memory secret material
(shares *and* nonces); the non-serializable nonce newtype from Pitfall 1 uses `Zeroizing`.
**Phase:** M1 (share storage) / M5 (audit).

### Pitfall 19: Over-claiming adaptive security

**What goes wrong:** FROST/Olaf have no clean *adaptive*-corruption proof (the Meier
barrier, scheme survey §3.16, §6). Marketing the system as adaptively secure over-claims.
**How to avoid:** in threat-model docs, state FROST is proven for *static, concurrent*
security (TS-UF-1) and that adaptive corruption is a modeling caveat mitigated *operationally*
by epoch-mixing refresh — not by a proof. This is a documentation-accuracy issue, not a code
bug. **Phase:** M1 docs / M5 review.

### Pitfall 20: Replayed / stale envelopes accepted as fresh

**What goes wrong:** Without replay protection, an old signed event (a prior round's package)
replayed by a relay could be mistaken for a fresh contribution. **How to avoid:** event
`id` + tags (`ceremony_id`, `round`, `seat`) give replay protection and idempotent dedup
(SPEC §7); bind every accepted event to the current `(ceremony_id, round)` and reject
out-of-window events. **Phase:** M2, adversarial test in M5 (replayed envelopes, SPEC §13.5).

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Make signing sessions "resumable like ceremonies" | Uniform state machine, less code | Persists nonces → key-extraction bug class | **Never** |
| Coordinator computes the sighash once and distributes it | Less per-client code, simpler UX | Enables blind-sign fund theft by a compromised coordinator | **Never** |
| Delete old shares and call it revocation | Feels like a security win, no sweep needed | False security; retained insiders untouched; funds exposed | **Never** |
| Use a public relay to "get started" | Zero ops setup | Ban/rate-limit at n=1000; retention loss mid-ceremony | Only for tiny (<~dozen) dev tests, never real ceremonies |
| Skip the byte-level bridge round-trip test | Ship M1 faster | Silent unspendable address or invalid signature on mainnet | **Never** |
| One keypair for Nostr identity + FROST share | One fewer key to manage | Cross-tier compromise; correlation; model collapse | **Never** |
| Verify same-key only on the coordinator | Less client code | Coordinator can lie; funds move to a dead key on buggy refresh | **Never** |
| Float dependency versions (`^3.0`) | Auto-patch updates | Wire-format skew across 1000 clients; non-reproducible builds | Only pre-M2 prototyping |
| Defer display-before-sign to M5 | Faster M1 signing flow | Retrofitting into coordinator-authoritative flow is error-prone | Discouraged — build the gate in M1 |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| `frost-secp256k1-tr` → rust-bitcoin | Hand-rolling x-only/parity/tweak; using non-`-tr` crate | Use `sign_with_tweak`/`aggregate_with_tweak(…, None)`; one pinned bridge fn; verify against `Q` |
| rust-bitcoin sighash | Wrong sighash type / omitting a prevout | `taproot_key_spend_signature_hash`, `SIGHASH_DEFAULT`, all prevouts |
| Taproot address | Building from internal key `P`, or `Some(merkle_root)` | `Address::p2tr(secp, P, None, network)`; funds sit at `Q` |
| nostr-sdk relays | Trusting relay-side NIP-42 AUTH as authorization | Client-side roster+signature check on every inbound event |
| nostr-sdk payloads | Plaintext DKG/dealer/enroll secrets | NIP-44 v2 to recipient npub for confidential classes only |
| Bitcoin Core RPC | Importing wrong descriptor / not watch-only | `tr(<internal-key>)` watch-only descriptor so Core tracks the address |
| SQLite roster | Recycling a live identifier for a new seat | Monotonic identifier allocation; roster is single authority |
| age/zeroize | Encrypting shares but leaving nonces/plaintext in memory | Zeroize all secret material; non-serializable nonce newtype |

---

## Performance / Scale Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| O(n²) round-2 blowup | Relay bans, throttling, GB of events | Dedicated relays, raised limits, paced batches | n≈100s; catastrophic at n=1000 (~10⁶ events, ~1 GB) |
| Non-resumable ceremony | A single interruption forces full restart | Idempotent dedup per `(ceremony_id, round, seat)` | Any ceremony whose wall-clock exceeds MTBF of 1000 laptops |
| Relay retention pruning | Late fetchers can't complete part3 | Raise retention for ceremony kinds until all fetched | Whenever retention < ceremony duration |
| Exactly-501 selection | Sessions abort on any single dropout | Over-provision liveness poll; new session on abort | n=1000, real participants (FROST non-robust) |
| Unbatched publish loop | Self-inflicted rate-limit trip | Paced round-2 batches | Round-2 burst of ~999 events/sender |

---

## Security Mistakes (domain-specific)

| Mistake | Risk | Prevention |
|---------|------|------------|
| Persist/reuse signing nonces | **Key extraction** (share recovery from 2 sigs) | Non-serializable, zeroized nonce type; new session on restart (§11.2) |
| Blind-sign coordinator's sighash | **Fund theft** by compromised coordinator | Recompute sighash from PSBT locally; display-before-sign (§11.7) |
| Trust deletion as revocation | Funds exposed to retained-insider coalition | Sweep to standby is the only revocation (§11.1) |
| Skip client same-key check | Funds move to a dead key on buggy/malicious refresh | Client-side `new==old` verifying-key assert before delete (§11.3) |
| Accept non-roster events | Rogue contributions, impersonation, DoS | Client-side roster hash + signature check on every event (§11.4) |
| Reuse Nostr key as FROST material | Cross-tier compromise; correlation | Independent generation; no derivation; type separation (§11.6a) |
| Mix epochs in a session | Garbage signature / opaque abort | Bind session to `(key_id, epoch)`; reject early (§3, §6.5) |
| Wrong tweak/parity in bridge | Unspendable address / invalid sig | Pinned round-trip test; verify against `Q`; `-tr` crate (§9) |

---

## "Looks Done But Isn't" Checklist

- [ ] **Signing works in a demo:** Often missing — verify nonces are *non-serializable* and
  a session restart mints fresh nonces + new session id (not a resume).
- [ ] **Bridge produces an address:** Often missing — verify a broadcast, *confirmed* regtest
  key-spend and a byte-level round-trip against a known-answer vector.
- [ ] **Refresh completes:** Often missing — verify *every client* asserts same verifying key
  before deleting the old share, with a tampered-refresh test that aborts.
- [ ] **Aggregation returns a signature:** Often missing — verify it validates against the
  *output* key `Q` (not internal `P`) and confirms on-chain.
- [ ] **Events flow over Nostr:** Often missing — verify non-roster events are dropped
  client-side even when the relay delivers them.
- [ ] **Ceremony runs at small n:** Often missing — verify n=1000 load test with relay tuning,
  paced batches, and interrupt-then-resume idempotency.
- [ ] **Display-before-sign shows a summary:** Often missing — verify the summary is
  *recomputed locally from the PSBT*, not rendered from coordinator-supplied fields.
- [ ] **Sweep flow exists:** Often missing — verify standby is pre-generated *and kept
  refreshed*, and the watcher nags until a fresh standby exists post-sweep.
- [ ] **Shares encrypted at rest:** Often missing — verify in-memory zeroize of shares *and*
  nonces, not just file encryption.
- [ ] **Deps pinned:** Often missing — verify committed `Cargo.lock`, reproducible build, and
  a ceremony version handshake rejecting mismatched clients.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Nonce reuse detected pre-broadcast | MEDIUM | Abort session, discard partials, new session with fresh nonces; audit for key exposure |
| Nonce reuse exploited (share/key leaked) | **HIGH** | Emergency sweep to standby; treat active key as fully compromised; forensics on the leak path |
| Bridge bug found before funding | LOW | Fix bridge fn + round-trip vector; regenerate address; nothing on-chain yet |
| Bridge bug found after funding (unspendable) | **HIGH/terminal** | If truly unspendable, funds may be unrecoverable — why M1 must prove the bridge before any real funds |
| Refresh produced wrong key, old shares deleted | **HIGH/terminal** | If old shares gone and new key ≠ address key, funds locked — why the client same-key check must precede deletion |
| Relay ban mid-ceremony | LOW/MEDIUM | Switch to backup self-hosted relays or offline file mode; resume via idempotent dedup |
| Blind-sign fund theft | **HIGH** | On-chain loss, likely irreversible; the display-before-sign gate is the only prevention |
| Retained-insider coalition suspected | MEDIUM | Trigger policy sweep to standby immediately; regenerate standby |

---

## Pitfall-to-Phase (Milestone) Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| 1. Nonce persistence/reuse | M1 (structural), M5 (review) | Nonce type won't serialize; reuse attempt rejected pre-`sᵢ` |
| 2. Bridge parity/sighash/key errors | M1 | Byte-level round-trip vector + confirmed regtest spend |
| 3. Mixed-epoch shares | M3 (schema in M2), M5 test | Early identifiable rejection, never a garbage aggregate |
| 4. Skip client same-key check | M3 | Tampered-refresh test aborts on every client |
| 5. Roster-pinning failure | M2, M5 test | Non-roster injected event dropped client-side |
| 6. Nostr key reused as FROST | M2, M5 audit | No conversion path between key types |
| 7. Tweak inconsistent in aggregation | M1 | Aggregate verifies against `Q`; merkle root `None` enforced |
| 8. Blind signing | M1 (gate), M5 test | Local PSBT recompute; mismatched-summary test refuses |
| 9. Deletion-as-revocation | M1 (docs), M4 (sweep) | Sweep is routine; standby pre-gen; policy forces sweeps |
| 10. Public relay / scale | M2 | n=1000 load test, paced batches, resume-after-interrupt |
| 11. FROST non-robustness | M1, M2 stress | Over-provisioned poll; new session on abort |
| 12. Enroll without refresh | M3 | Enroll atomically chains refresh |
| 13. Standby neglect | M4 | `standby_max_age` enforced; post-sweep nag |
| 14. NIP-44 misuse | M2 | Per-class encryption policy verified |
| 15. Version skew | M2, M5 | Version handshake; pinned lock; reproducible build |
| 16. Identifier reuse | M3 | Monotonic allocation from roster authority |
| 17. Dealer state/trust | M1/M2 | Zeroize + transcript trust-event record |
| 18. At-rest/zeroize gaps | M1, M5 | Encrypted shares + zeroized secrets audit |
| 19. Adaptive over-claim | M1 docs, M5 | Threat-model states static-only proof + epoch-mix mitigation |
| 20. Replayed envelopes | M2, M5 test | `(ceremony_id, round, seat)` dedup; out-of-window reject |

---

## Sources

- **SPEC-frost-cli.md** (draft v0.1, 2026-07-09) — normative §11 security considerations,
  §6.5 signing / nonce discipline, §9 Bitcoin bridge, §10 policy rationale, §13 milestones.
  [HIGH — normative project spec]
- **schemes/02-threshold-schnorr-frost.md** (surveyed through ePrint 2026/431) — ROS/Drijvers
  attack and binding factors (§1.2), BIP340 x-only/parity/tweak footguns (§2, §7), FROST
  non-robustness + identifiable abort (§1.4), Arctic nonce-reuse motivation (§3.15), Meier
  adaptive-security barrier (§3.16, §6), BIP 445 / secp256k1-zkp "don't hand-roll the tweak."
  [HIGH — curated academic survey]
- **implementations-resharing.md** — ZF `frost-secp256k1-tr` refresh/repair/enroll API and
  the "cannot change threshold" limit (§2.7/§3.1), coinbase/cb-mpc `new_key.Q ==
  current_key.Q` hard-check pattern (§4), threshold-library implementation-bug CVEs
  (Alpha-Rays, TSSHOCK/BitForge) as evidence that impl bugs — not the math — extract keys (§7).
  [HIGH — curated implementation survey]
- **.planning/PROJECT.md** — security model, epoch discipline, key decisions. [HIGH]

---
*Pitfalls research for: FROST Taproot signing CLI (tsig) at t=501/n=1000*
*Researched: 2026-07-10*
