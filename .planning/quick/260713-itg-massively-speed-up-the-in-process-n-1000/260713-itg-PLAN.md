---
quick_id: 260713-itg
title: Speed up in-process n=100 FROST DKG simulation
status: ready
mode: quick
must_haves:
  truths:
    - "rounds 2 and 3 of run_inprocess_dkg_with_rng run in parallel across seats via rayon"
    - "round 1 stays sequential so the caller-supplied RNG threads through seats identically (bit-for-bit reproducible failures preserved)"
    - "rounds 2/3 consume NO RNG (dkg::part2/part3 take no rng argument) so parallelizing them cannot affect determinism of either the _with_rng or OsRng path"
    - "GroupKeyMismatch { seat } still returns a mismatching seat, chosen deterministically (smallest id) regardless of thread scheduling"
    - "Frost / Identifier / Empty error variants and D-11 into_even_y(None) normalization and client-side group-key confirmation are all preserved"
    - "the O(n^2) per-seat map clone in rounds 2 and 3 is replaced by a per-worker clone-once + remove/re-insert-self pool"
    - "no audited crypto path is modified — only src/crypto/keygen.rs, Cargo.toml, Cargo.lock"
  artifacts:
    - "src/crypto/keygen.rs — rayon-parallel rounds 2/3, no per-seat cloning"
    - "Cargo.toml — rayon exact-pinned dependency + [profile.release] lto=fat/codegen-units=1/opt-level=3"
    - "Cargo.lock — updated with rayon (committed, reproducible)"
  key_links:
    - "src/crypto/keygen.rs::run_inprocess_dkg_with_rng"
    - "tests/dkg_100_correctness.rs (reference parallel pattern already used in the ignored test)"
    - "tests/inproc_sign_100.rs (TSIG_SIGN_T/TSIG_SIGN_N — calls library run_inprocess_dkg via common::run_confirmed_key_spend)"
---

# Quick Task 260713-itg: Speed up the in-process n=100 FROST DKG simulation

## Goal

Massively reduce wall-clock time of the single-process n=100 FROST DKG simulation
in `src/crypto/keygen.rs::run_inprocess_dkg_with_rng` by parallelizing the
embarrassingly-parallel per-seat work with `rayon` and eliminating O(n^2)
allocation churn — WITHOUT touching audited crypto or weakening any security
invariant.

## Critical context (already verified)

- `dkg::part1(id, n, t, rng)` is the ONLY round that consumes the RNG.
  `dkg::part2(secret, &others)` and `dkg::part3(secret2, &r1_others, r2_for_me)`
  take NO rng — they are pure deterministic functions of round-1 output. This is
  the key to determinism: **rounds 2 and 3 can be parallelized unconditionally
  (both the `_with_rng` and OsRng paths) and reproducibility is untouched** because
  no randomness flows through them. Round 1 stays sequential so the caller RNG
  threads through seats in exactly the current order.
- The ignored test `tests/dkg_100_correctness.rs` ALREADY demonstrates the
  intended pattern with `std::thread::scope`: clone the round-1 package map ONCE
  per worker, then for each seat `remove` its own entry (giving "all others"),
  call part2/part3, then re-`insert` it. Port that pattern into the library using
  rayon's `map_with` (per-worker cloned state). Do NOT edit that test file.
- `tests/inproc_sign_100.rs` → `common::run_confirmed_key_spend(t,n)` →
  `run_inprocess_dkg(t,n)` (the library fn), so `TSIG_SIGN_T=101 TSIG_SIGN_N=200
  cargo test --release --test inproc_sign_100 -- --ignored` exercises the code we
  change. NOTE: that test also spawns a regtest bitcoind (constant overhead). The
  `dkg_100_correctness` test does NOT call the library (its loop is inlined and
  already parallel) so it is NOT a valid before/after for our change — use a
  scratch bench or inproc_sign_100 for timing (see Task 3).
- rayon latest = 1.12.0. Pin EXACT: `rayon = "=1.12.0"`. crossbeam is already in
  Cargo.lock (transitively). No deny.toml exists → no allow-list edit needed (note it).
- 11 cores on this host.

## Tasks

### Task 1 — Parallelize rounds 2/3 with rayon + kill O(n^2) clones
**files:** `src/crypto/keygen.rs`, `Cargo.toml`, `Cargo.lock`
**action:**
- Add `rayon = "=1.12.0"` to `[dependencies]` in Cargo.toml (exact pin). Run a
  build so Cargo.lock records it; commit Cargo.lock.
- Rewrite the body of `run_inprocess_dkg_with_rng`:
  - **Round 1:** keep the existing sequential `for i in 1..=max_signers` loop
    (threads `&mut *rng`). Collect into `Vec<(Identifier, round1::SecretPackage)>`
    for r1_secret and keep `r1_pkgs: BTreeMap<Identifier, round1::Package>`.
  - **Round 2 (parallel):** `r1_secret.into_par_iter().map_with(r1_pkgs.clone(),
    |pool, (id, secret)| { let self_pkg = pool.remove(&id)...; let (s2, sent) =
    dkg::part2(secret, pool)...; pool.insert(id, self_pkg); Ok((id, s2, sent)) })`
    collecting into an **order-preserving** `Vec<Result<..., KeygenError>>` (Vec
    par-iter is indexed → order preserved). Then sequentially unwrap in order so
    the FIRST (smallest-id) Frost error is the one returned (deterministic).
  - Fan round-2 packages out by recipient into
    `r2_by_recipient: BTreeMap<Id, BTreeMap<Id, round2::Package>>` (sequential,
    cheap) and build `r2_secret: Vec<(Id, round2::SecretPackage)>`.
  - **Round 3 (parallel):** same `map_with(r1_pkgs.clone(), ...)` pool trick;
    `r2_for_me = r2_by_recipient.get(&id).unwrap_or(&empty)` (shared read borrow).
    Inside: `dkg::part3(...)`, then D-11 `kp.into_even_y(None)` /
    `pubkeys.into_even_y(None)`. Collect order-preserving
    `Vec<Result<(Id, KeyPackage, PublicKeyPackage), KeygenError>>`.
  - **Group-key confirmation (sequential, deterministic):** iterate the ordered
    round-3 results (smallest id first); the first result's pubkeys sets `group`;
    every subsequent seat whose `verifying_key()` differs returns
    `KeygenError::GroupKeyMismatch { seat: id }`. Build `key_packages` BTreeMap.
    `group.ok_or(KeygenError::Empty)`.
- Add a code comment documenting the determinism decision: "Round 1 is the only
  RNG-consuming round and stays sequential to preserve exact RNG threading /
  reproducible failures; rounds 2 & 3 consume no randomness (pure functions of
  round-1 output) so rayon parallelism leaves determinism untouched. Error
  attribution (Frost errors, GroupKeyMismatch seat) is made deterministic by
  collecting into order-preserving Vecs and resolving the first/smallest-id
  failure sequentially."
- Do NOT add batch verification. Do NOT touch nonces/bridge/signing. Do NOT edit
  frost-* internals.
**verify:** `cargo build`, `cargo build --release`, `cargo clippy --lib` clean.
**done:** `run_inprocess_dkg_with_rng` uses rayon for rounds 2/3 with per-worker
pool clones; all error semantics + D-11 + confirmation preserved.

### Task 2 — Release-profile tuning
**files:** `Cargo.toml`
**action:** Append:
```toml
[profile.release]
lto = "fat"
codegen-units = 1
opt-level = 3
# NOTE (reproducible builds): do NOT add target-cpu=native here — it breaks the
# 100-verifier reproducibility requirement. The nightly measurement machine may
# opt in locally/ephemerally via RUSTFLAGS="-C target-cpu=native", never committed.
```
**verify:** `cargo build --release` succeeds with the new profile.
**done:** profile present with the commented target-cpu note.

### Task 3 — Verify + demonstrate medium-scale speedup
**files:** (none committed — benchmarking + test runs only)
**action:**
- Run fast tests (must pass): `cargo test --test dkg_small`,
  `cargo test --test inproc_sign`, `cargo test --test bridge_roundtrip`,
  `cargo test --test sign_adversarial`, `cargo test --test transport_stub`,
  `cargo test --test chain_backend_conformance`, `cargo test --test compile_fail`.
- Medium-scale before/after (isolate the LIBRARY fn — do NOT rely on
  dkg_100_correctness, its loop is already parallel and does not call the lib):
  write a throwaway bench in the SCRATCHPAD (outside the repo tree) e.g. a tiny
  `examples/`-style or a `#[test]` you add-then-remove, OR simplest: a scratch
  integration invocation timing `tsig::crypto::run_inprocess_dkg(101, 200)` with
  `std::time::Instant`. Time AFTER (parallel). Then `git stash` the keygen.rs
  change, rebuild, time BEFORE (sequential), `git stash pop`. Keep the bench file
  in the scratchpad only; leave the repo tree clean. Report both wall-clock
  numbers and observed core utilization (11 cores available).
- Do NOT run the full `--ignored` n=100 tests.
- Confirm no deny.toml exists (nothing to allow-list) — note it.
**verify:** all listed fast tests green; before/after numbers captured.
**done:** speedup demonstrated at t=101/n=200; timings recorded in SUMMARY.

## Out of scope / hard constraints
- No changes outside `src/crypto/keygen.rs`, `Cargo.toml`, `Cargo.lock`.
- No batch verification (preserves per-share culprit attribution / SIGN-06).
- No nonce/bridge/signing/verify-against-Q changes.
- No target-cpu=native committed. No editing the ignored test files.
