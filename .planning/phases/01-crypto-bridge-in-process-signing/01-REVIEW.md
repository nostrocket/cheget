---
phase: 01-crypto-bridge-in-process-signing
reviewed: 2026-07-10T13:05:12Z
depth: standard
files_reviewed: 27
files_reviewed_list:
  - src/bridge/mod.rs
  - src/bridge/taproot.rs
  - src/chain/core_rpc.rs
  - src/chain/esplora.rs
  - src/chain/mod.rs
  - src/chain/sighash.rs
  - src/cli/address.rs
  - src/cli/keygen.rs
  - src/cli/mod.rs
  - src/cli/sign.rs
  - src/crypto/keygen.rs
  - src/crypto/mod.rs
  - src/crypto/nonce.rs
  - src/crypto/sign.rs
  - src/crypto/types.rs
  - src/lib.rs
  - src/main.rs
  - src/session/display.rs
  - src/session/liveness.rs
  - src/session/mod.rs
  - src/transport/envelope.rs
  - src/transport/inmemory.rs
  - src/transport/mod.rs
  - tests/bridge_roundtrip.rs
  - tests/chain_backend_conformance.rs
  - tests/inproc_sign.rs
  - tests/sign_adversarial.rs
findings:
  critical: 0
  warning: 3
  info: 4
  total: 7
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-07-10T13:05:12Z
**Depth:** standard
**Files Reviewed:** 27
**Status:** issues_found

## Summary

This is the crypto-bridge / in-process-signing phase of `tsig`. I reviewed the
FROST→rust-bitcoin bridge, the crypto core (DKG, non-serializable nonces,
tweaked aggregation), the chain backends, the two-round signing session, the
transport stub, and the CLI, plus the key adversarial/KAT tests. The library and
all test targets type-check cleanly offline (`cargo check --lib`,
`cargo check --tests`).

I specifically stress-tested the five highest-severity invariants called out in
CLAUDE.md/SPEC. All five hold up:

1. **Nonces never serialized/persisted** — `EphemeralNonces` wraps
   `Zeroizing<SigningNonces>`, derives no `Clone`/`Serialize`/`Deserialize`, is
   consumed by value in `sign()`, and is proven non-serializable by a `trybuild`
   compile-fail snapshot. No serialization path exists. Solid.
2. **Even-Y invariant + `from_slice` confinement** — `internal_key_xonly`
   rejects odd-Y with `BridgeError::OddY` (never blindly strips the prefix); a
   grep confirms `XOnlyPublicKey::from_slice` appears only in `bridge/taproot.rs`.
3. **Verify against tweaked `Q`, not `P`** — `verify_against_q` uses
   `output_key_q` and the session calls it after aggregation; a test asserts the
   signature verifies against `Q` and specifically fails against `P`.
4. **Display-before-sign recompute** — `display_and_ack` independently
   recomputes the sighash from the PSBT via the single canonical helper and
   refuses on mismatch even with `--yes`. Adversarial test covers it.
5. **Nostr key separation** — no Nostr code exists in this phase; N/A.

The end-to-end confirmed regtest key-spend test exercises the full crypto path
(DKG → bridge → descriptor import → PSBT → session → aggregate-with-tweak →
verify-Q → broadcast → confirm), which is strong evidence the on-chain-critical
path is correct.

No BLOCKER/critical defects were found. The findings below are robustness and
quality issues — most latent (the affected code is not on a value-bearing path
in Phase 1), but each is a real gap worth fixing before later phases build on it.

## Structural Findings (fallow)

No `<structural_findings>` block was provided with this review; none to report.

## Narrative Findings (AI reviewer)

## Warnings

### WR-01: `estimate_fee` truncates fractional sat/vB, can yield a zero/under fee

**File:** `src/chain/core_rpc.rs:100`
**Issue:** The Core RPC fee conversion is
`FeeRate::from_sat_per_vb(btc_per_kvb.to_sat() / 1000)`. `estimatesmartfee`
reports BTC/kvB; `.to_sat()` gives sat/kvB, and the integer `/ 1000` truncates
toward zero. Any fee rate with a fractional sat/vB component is silently rounded
down (e.g. 1500 sat/kvB = 1.5 sat/vB → 1 sat/vB, a 33% underestimate), and any
rate below 1000 sat/kvB (< 1 sat/vB) collapses to `FeeRate::from_sat_per_vb(0)`
= a zero fee rate. A zero/under-estimate produces a transaction that will not
relay or confirm. This also diverges from the Esplora backend, which preserves
precision. No spend path consumes this in Phase 1, so it is latent, but it feeds
the Phase 5 sweep.
**Fix:** Round to nearest (or up) and/or use the higher-precision constructor:
```rust
// round up so we never underpay the estimate
let sat_per_kvb = btc_per_kvb.to_sat();
let sat_per_kwu = FeeRate::from_sat_per_kwu(sat_per_kvb * 1000 / 4); // or:
Ok(FeeRate::from_sat_per_vb((sat_per_kvb + 999) / 1000))
```
Prefer a kwu- or kvb-based constructor that keeps sub-sat/vB resolution rather
than an integer `/ 1000`.

### WR-02: `sign` command never checks the PSBT actually spends the derived group address

**File:** `src/cli/sign.rs:95-140`
**Issue:** The handler runs a fresh in-process DKG (a *new* random group key per
invocation), prints the derived group address, then signs the supplied PSBT
regardless of whether that PSBT's inputs pay to the derived address. The
display-before-sign gate only asserts that the *recomputed* sighash equals the
coordinator's — but here coordinator and signer are the same code over the same
PSBT, so that check is trivially always true and validates nothing about the
key. A user who feeds any unrelated PSBT gets a syntactically valid but
economically meaningless signature over a key that does not control the coins.
There is no guard that `psbt.inputs[i].witness_utxo.script_pubkey` matches the
group address's `script_pubkey()`.
**Fix:** Before signing, verify every input's `witness_utxo.script_pubkey`
equals `addr.script_pubkey()` and error out otherwise:
```rust
let group_spk = addr.script_pubkey();
for (i, inp) in psbt.inputs.iter().enumerate() {
    let spk = inp.witness_utxo.as_ref()
        .map(|o| &o.script_pubkey)
        .ok_or_else(|| format!("input {i} missing witness_utxo"))?;
    if spk != &group_spk {
        return Err(format!(
            "input {i} does not pay to the derived group address {addr}; \
             Phase 1 signs only self-consistent group PSBTs"
        ).into());
    }
}
```

### WR-03: over-provisioned liveness defense is documented but not wired into the session

**File:** `src/session/liveness.rs:49-52`, `src/session/mod.rs:234-259`
**Issue:** `liveness.rs` documents (and unit-tests) `over_provisioned_poll_size`
as the Pitfall-11 anti-dropout defense — "the coordinator polls this many, then
`poll_and_select` finalizes exactly `t`." But `SigningSession::liveness_select`
never calls it; it polls the *entire* roster (`for seat in
self.id_of_seat.keys()`) and then takes the first `t`. In Phase 1 (all seats
respond in-process) this is harmless — polling all `n` is a superset of the
margin — but the function is effectively dead code in `src/` (only referenced by
tests), and the module contract diverges from the session's actual behavior. At
n=100 this becomes a real design question (poll all 100 and wait vs. poll 57),
and a Phase-7 implementer inheriting this seam may assume the margin logic runs
when it does not.
**Fix:** Either wire the margin into `liveness_select` (poll
`over_provisioned_poll_size(t, n)` seats rather than all `n`) or update the
`liveness.rs` module doc to state that the session polls the full roster and
`over_provisioned_poll_size` is a helper for the future relay transport. Remove
the dead helper from `src/` if it is not used by the session.

## Info

### IN-01: display gate re-runs (and re-prints) once per seat inside round 2

**File:** `src/session/mod.rs:339-353`
**Issue:** `round2` loops over every selected seat and calls `display_and_ack`
each iteration with the identical `tx`/`prevouts`/`input_index`. The sighash is
recomputed and the full spend summary is printed to stderr `t` times per input
(51× at full scale) — redundant work and noisy output. This is a
simulate-all-seats artifact; a real per-participant flow recomputes once.
**Fix:** Recompute/print the summary once per input (before the per-seat loop),
keeping the recompute check but not repeating the human-facing render `t` times.

### IN-02: interactive ack prints a misleading "automation/regtest bypass" line

**File:** `src/cli/sign.rs:132-140`, `src/session/display.rs:148`
**Issue:** In interactive mode the CLI prompts via `prompt_ack()`, and on success
calls `session.run(true)`. Inside the gate, `yes = true` always prints
`"[--yes] acknowledgement bypassed (automation/regtest)"` — inaccurate when the
operator actually acknowledged at the CLI prompt. Additionally, for a multi-input
PSBT the operator acks once against a single aggregate summary and then all
inputs sign, without a per-input confirmation.
**Fix:** Distinguish "human already acked" from "--yes automation bypass" (e.g.
pass an enum or suppress the bypass banner when the CLI obtained an interactive
ack), and consider rendering per-input summaries for multi-input PSBTs.

### IN-03: clippy needless-borrow warning in a test

**File:** `tests/dkg_100_correctness.rs`
**Issue:** `cargo clippy` reports one warning ("the borrowed expression
implements the required traits") in this test target. Cosmetic, but the project
mandates a clean `cargo clippy` in CI.
**Fix:** Apply the suggested `clippy --fix` (drop the unnecessary `&`).

### IN-04: minor duplication / hand-rolled hex

**File:** `src/cli/address.rs:156-172`, `src/crypto/keygen.rs:136-165`
**Issue:** (a) `hex_encode` allocates a `format!("{b:02x}")` per byte in a loop —
a small hand-rolled hex codec that duplicates functionality the workspace could
share. (b) `confirm_group_key` re-checks each seat's verifying key against the
group key, which `run_inprocess_dkg` already asserts inline (lines 136-142);
the standalone function is only meaningful for packages sourced outside the
in-process DKG. Neither is a defect — noting for maintainability.
**Fix:** Optionally use a shared hex helper and document that `confirm_group_key`
is the check for externally-sourced packages (the in-process path is
self-checking).

---

_Reviewed: 2026-07-10T13:05:12Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
