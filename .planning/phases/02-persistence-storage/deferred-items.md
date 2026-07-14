# Deferred Items — Phase 02

Out-of-scope discoveries logged during execution (not fixed — see SCOPE BOUNDARY).

| Discovered in | Item | Location | Notes |
|---------------|------|----------|-------|
| 02-01 | Pre-existing clippy lint `needless_borrows_for_generic_args` fails `cargo clippy --tests -- -D warnings` | `tests/dkg_100_correctness.rs:55:70` (`&mut rng`) | Phase 1 code (last touched commit c537bf0), not introduced by 02-01. `cargo clippy --lib -- -D warnings` is clean; the store modules have no clippy warnings. Newer clippy (1.96) flags an older borrow. Trivial `&mut rng` → `rng` fix; deferred as unrelated. |
| 02-04 | Pre-existing clippy lint `needless_borrows_for_generic_args` under `cargo clippy --tests` | `src/store/checkpoint.rs:305:54` and `:360:53` | 02-03 code, not touched by 02-04. `cargo clippy --lib --bins -- -D warnings` (excludes tests) is clean including all 02-04 files. Trivial borrow-removal fix; deferred as unrelated to 02-04. |
