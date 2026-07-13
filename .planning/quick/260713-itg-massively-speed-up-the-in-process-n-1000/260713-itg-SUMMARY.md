---
quick_id: 260713-itg
title: Speed up in-process n=100 FROST DKG simulation
status: complete
mode: quick
commit: 9bc25e4
files_changed:
  - src/crypto/keygen.rs
  - Cargo.toml
  - Cargo.lock
---

# Quick Task 260713-itg: Speed up the in-process n=100 FROST DKG simulation

## What changed

Parallelized the per-seat work in
`src/crypto/keygen.rs::run_inprocess_dkg_with_rng` with `rayon` and eliminated
the O(n^2) per-seat round-1 package-map clone. No audited crypto path was
touched — only `src/crypto/keygen.rs`, `Cargo.toml`, and `Cargo.lock` changed.

- **Round 1 (unchanged, sequential):** still a `for i in 1..=max_signers` loop
  threading `&mut *rng`. It now collects `r1_secret` into an id-ordered
  `Vec<(Identifier, round1::SecretPackage)>` (was a `BTreeMap`); `r1_pkgs`
  stays a `BTreeMap`.
- **Round 2 (now parallel):** `r1_secret.into_par_iter().map_with(r1_pkgs.clone(), …)`.
  Each rayon worker clones the round-1 pool **once**, then per seat
  `pool.remove(&id)` yields "all others", calls `dkg::part2`, and re-inserts
  its own package. Results collected into an order-preserving
  `Vec<Result<…, KeygenError>>`, then resolved sequentially in ascending-id
  order (first `?` → smallest-id Frost error). Round-2 packages fanned out by
  recipient sequentially (cheap).
- **Round 3 (now parallel):** same `map_with` clone-once + remove/re-insert-self
  pool trick; `r2_for_me` read via a shared borrow of `r2_by_recipient`. D-11
  `into_even_y(None)` applied to **both** `kp` and `pubkeys` inside the parallel
  closure. Results collected into an order-preserving Vec.
- **Group-key confirmation (sequential, deterministic):** walks the ordered
  round-3 results smallest-id first; the first sets the reference group key,
  any later seat whose `verifying_key()` differs returns
  `GroupKeyMismatch { seat }` (smallest-id mismatch), then
  `group.ok_or(KeygenError::Empty)`.

Preserved exactly: `KeygenError::{Frost, Identifier, Empty, GroupKeyMismatch{seat}}`
semantics, D-11 even-Y normalization on both packages, client-side group-key
confirmation, and per-share culprit attribution (**no batch verification added**).

`Cargo.toml`: added `rayon = "=1.12.0"` (exact pin) and a `[profile.release]`
with `lto = "fat"`, `codegen-units = 1`, `opt-level = 3`. `Cargo.lock` records
rayon 1.12.0 + rayon-core 1.13.0 + crossbeam-{deque,epoch} (crossbeam-utils was
already transitively present).

## Determinism decision (documented in code)

`dkg::part1` is the **only** round that consumes the RNG, so round 1 stays
sequential and threads the caller-supplied RNG through seats in exactly the
prior order — bit-for-bit reproducible failures are preserved on **both** the
`_with_rng` and `OsRng` paths. `dkg::part2` / `dkg::part3` take no RNG argument
and are pure deterministic functions of round-1 output, so parallelizing them
cannot affect reproducibility. Error attribution is made independent of thread
scheduling by collecting each parallel round into an order-preserving indexed
`Vec<Result<…>>` and resolving failures sequentially in ascending-id order, so
the smallest-id Frost error and the smallest-id `GroupKeyMismatch { seat }` are
returned deterministically.

## target-cpu=native reproducibility note

`[profile.release]` carries a comment forbidding `target-cpu=native`: it would
break the "100 people must verify what they run" reproducible-build
requirement. The nightly benchmark machine may opt in **locally/ephemerally**
via `RUSTFLAGS="-C target-cpu=native"`, never committed.

## Benchmark — medium scale, release build (t=101, n=200)

Measured against the **library** fn `cheget::crypto::run_inprocess_dkg(101, 200)`
via a throwaway `#[test]` timer (created, run, deleted — repo tree left clean).
The `dkg_100_correctness` test was intentionally **not** used for before/after
because its loop is inlined and already parallel and does not call the library
fn. Host: 11 cores.

| Variant                          | Wall clock         |
| -------------------------------- | ------------------ |
| BEFORE (sequential, single run)  | **219.9 s**        |
| AFTER (parallel, best-of-3)      | **33.2 s**         |

**Speedup ≈ 6.6×.** Observed core utilization (AFTER, via `/usr/bin/time -l`):
1010.54 user + 5.54 sys over 106.26 real ≈ **9.6 of 11 cores** engaged. The gap
from the ~9.6× ideal is the sequential round 1 (RNG threading), the sequential
fan-out/confirmation passes, and rayon scheduling overhead; the parallel version
also removes the O(n^2) clone churn the sequential path still pays.

BEFORE/AFTER method: measured AFTER with the change in place, then
`git stash push src/crypto/keygen.rs` (keeping Cargo.toml/Cargo.lock so rayon
still resolved), rebuilt `--release`, measured BEFORE, then `git stash pop`. The
full `--ignored` n=100 tests were **not** run.

## Verification

`cargo build`, `cargo build --release` (new fat-LTO profile), and
`cargo clippy --lib` all clean with no warnings on the changed code.

Fast tests — all passed:

| Test suite                        | Result       |
| --------------------------------- | ------------ |
| `dkg_small`                       | ok (2 tests) |
| `inproc_sign`                     | ok (7 tests) |
| `bridge_roundtrip`                | ok (3 tests) |
| `sign_adversarial`                | ok (3 tests) |
| `transport_stub`                  | ok (4 tests) |
| `chain_backend_conformance`       | ok (2 tests) |
| `compile_fail`                    | ok (1 test)  |

## cargo-deny

No `deny.toml` / cargo-deny config exists in the repo, so **no allow-list edit
was needed**. rayon and its transitive crossbeam deps introduce no new
duplicate-crate concern requiring an allow-list.

## Deviations from plan

None — plan executed as written.

## Self-Check: PASSED

- `src/crypto/keygen.rs`, `Cargo.toml`, `Cargo.lock` present and committed in `9bc25e4`.
- Scratch bench (`tests/_scratch_bench_itg.rs`) removed; `git status` shows only the
  3 intended source files (staged/committed) plus pre-existing unrelated dirty/untracked
  planning artifacts, which were deliberately not staged.
