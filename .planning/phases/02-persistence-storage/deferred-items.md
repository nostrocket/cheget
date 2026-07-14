# Deferred Items — Phase 02

Out-of-scope discoveries logged during execution (not fixed — see SCOPE BOUNDARY).

| Discovered in | Item | Location | Notes |
|---------------|------|----------|-------|
| 02-01 | Pre-existing clippy lint `needless_borrows_for_generic_args` fails `cargo clippy --tests -- -D warnings` | `tests/dkg_100_correctness.rs:55:70` (`&mut rng`) | Phase 1 code (last touched commit c537bf0), not introduced by 02-01. `cargo clippy --lib -- -D warnings` is clean; the store modules have no clippy warnings. Newer clippy (1.96) flags an older borrow. Trivial `&mut rng` → `rng` fix; deferred as unrelated. |
