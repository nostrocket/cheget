---
phase: quick-260713-kxi
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - Cargo.toml
  - Cargo.lock
  - src/main.rs
  - src/lib.rs
  - src/cli/mod.rs
  - src/cli/keygen.rs
  - src/crypto/nonce.rs
  - tests/*.rs
  - tests/common/mod.rs
  - tests/ui/nonce_no_serialize.rs
  - tests/ui/nonce_no_serialize.stderr
  - README.md
  - SPEC-frost-cli.md
  - .claude/CLAUDE.md
  - .planning/PROJECT.md
  - .planning/ROADMAP.md
  - .planning/REQUIREMENTS.md
  - .planning/STATE.md
  - .planning/research/*.md
  - .planning/phases/01-crypto-bridge-in-process-signing/*.md
  - .planning/quick/**/*.md
autonomous: true
requirements: [QUICK-260713-kxi]

must_haves:
  truths:
    - "cargo build produces a binary named `cheget` (target/debug/cheget); `cargo run -- --help` shows `cheget` as program name"
    - "cargo build, cargo build --release, and cargo clippy --lib are clean"
    - "The fast test suite passes, including the regenerated compile_fail .stderr fixture"
    - "The override env-var interface is CHEGET_SIGN_T/N and CHEGET_DKG_T/N (no TSIG_ prefix remains anywhere)"
    - "No tracked file references the crate/binary/project name `tsig` (only the unrelated `scriptSig` substring survives, in schemes/*.md)"
  artifacts:
    - Cargo.toml (package/bin/lib name = cheget)
    - Cargo.lock (regenerated local package entry name = cheget)
    - target/debug/cheget
    - tests/ui/nonce_no_serialize.stderr (references cheget::)
  key_links:
    - "Cargo.toml [lib] name `cheget` <-> every `use cheget::` / `cheget::` path in src/main.rs and tests/*.rs (crate-path resolution)"
    - "trybuild fixture tests/ui/nonce_no_serialize.rs `cheget::crypto::EphemeralNonces` <-> its .stderr expected output"
    - "env call sites (std::env::var / env_u16) <-> the CHEGET_* names documented in test doc-comments"
---

<objective>
Rename the project — crate, binary, lib, override env-var prefix, and ALL prose (including completed planning history) — from `tsig` to `cheget`.

Purpose: A mechanical, decision-free rename. Every decision was made by the user (see task spec); introduce no ambiguity, reduce no scope. Same breadth as the prior t=501→t=51 rescale quick task: live source, tests, and completed `.planning/` history are all rewritten.

Output: A project that builds and tests clean under the name `cheget`, with a `cheget` binary and `CHEGET_*` env overrides, and zero surviving `tsig` project-name references in tracked files.
</objective>

<execution_context>
@$HOME/.claude/gsd-core/workflows/execute-plan.md
@$HOME/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.claude/CLAUDE.md
@Cargo.toml

Known false positive (do NOT change): `scriptSig` in `schemes/*.md` matches `tsig`
case-insensitively but is an unrelated Bitcoin-script term. `.planning/config.json`
contains no project title. `implementations-resharing.md` has zero real hits.
No source file or GSD task-directory slug contains `tsig` — this is content-only,
there are NO file/dir renames. If you discover a slug that does, FLAG it (do not rename it).
</context>

<tasks>

<task type="auto">
  <name>Task 1: Rename crate/binary identity, crate-path imports, CLI name, and env-var interface (compile-affecting)</name>
  <files>Cargo.toml, Cargo.lock, src/main.rs, src/lib.rs, src/cli/mod.rs, src/cli/keygen.rs, src/crypto/nonce.rs, tests/dkg_small.rs, tests/inproc_sign.rs, tests/inproc_sign_100.rs, tests/bridge_roundtrip.rs, tests/sign_adversarial.rs, tests/transport_stub.rs, tests/chain_backend_conformance.rs, tests/dkg_100_correctness.rs, tests/regtest_fixture.rs, tests/common/mod.rs, tests/ui/nonce_no_serialize.rs, tests/ui/nonce_no_serialize.stderr</files>
  <action>
Manifest: in Cargo.toml set `[package] name = "cheget"`, `[[bin]] name = "cheget"`, `[lib] name = "cheget"`. Leave path, version, edition, rust-version, all deps, and dev-deps untouched; the description string contains no "tsig" wording so leave it intact.

Crate-path imports (compile-breaking): replace every `use tsig::...` and every bare `tsig::...` path with the `cheget::` equivalent. Sites: src/main.rs (the `use tsig::cli::Cli;` import) plus all tests/*.rs and tests/common/mod.rs. Grep `\btsig::` to enumerate every occurrence and change all of them.

CLI / binary name: in src/cli/mod.rs change the clap attribute `#[command(name = "tsig", ...)]` to `name = "cheget"`, and the adjacent doc-comment header. Verify there is no other `bin_name`/usage-string literal.

Env-var override interface: rename the four override vars at their call sites AND in the doc-comment/prose that names them — TSIG_SIGN_T→CHEGET_SIGN_T, TSIG_SIGN_N→CHEGET_SIGN_N, TSIG_DKG_T→CHEGET_DKG_T, TSIG_DKG_N→CHEGET_DKG_N. Call sites are the `std::env::var("...")` calls in tests/inproc_sign_100.rs and the `env_u16("...")` calls in tests/dkg_100_correctness.rs; prose mentions are the `//!` doc-comments in tests/inproc_sign_100.rs and tests/dkg_100_correctness.rs.

trybuild fixture: in tests/ui/nonce_no_serialize.rs change `tsig::crypto::EphemeralNonces` to `cheget::crypto::EphemeralNonces`. The expected-output file tests/ui/nonce_no_serialize.stderr embeds the old crate path — after all source edits, regenerate it with `TRYBUILD=overwrite cargo test --test compile_fail`, then re-run WITHOUT the env to confirm it passes. (Hand-editing every `tsig`→`cheget` in the .stderr is an acceptable fallback only if regeneration is unavailable.)

Rebuild the lockfile: run `cargo build` so the local package entry in Cargo.lock (`name = "tsig"`) is regenerated to `cheget`. Do NOT hand-edit Cargo.lock dependency entries or hashes.

Also update any remaining prose `tsig` mentions inside these already-open source files (e.g. the `//! ... tsig ...` header in src/main.rs, src/lib.rs, src/cli/keygen.rs, src/crypto/nonce.rs, and any `tsig keygen` / `tsig watcher` invocation examples in doc-comments) to `cheget`.
  </action>
  <verify>
    <automated>cargo build 2>&1 | tail -3 && test -x target/debug/cheget && cargo build --release 2>&1 | tail -3 && cargo clippy --lib 2>&1 | tail -3 && cargo run -- --help 2>&1 | grep -qi '^Usage: cheget\|cheget' && cargo test --test compile_fail --test dkg_small --test inproc_sign --test bridge_roundtrip --test sign_adversarial --test transport_stub --test chain_backend_conformance 2>&1 | tail -15 && test -z "$(grep -rn 'TSIG_' src/ tests/)" && test -z "$(grep -rn '\btsig::' src/ tests/)"</automated>
  </verify>
  <done>cargo build/build --release/clippy --lib are clean; target/debug/cheget exists; `cargo run -- --help` names the program `cheget`; the listed fast tests pass including the regenerated compile_fail .stderr; no `TSIG_` or `tsig::` remains under src/ or tests/.</done>
</task>

<task type="auto">
  <name>Task 2: Rewrite all remaining prose (live docs, SPEC, README, planning history) and run the final audit</name>
  <files>README.md, SPEC-frost-cli.md, .claude/CLAUDE.md, .planning/PROJECT.md, .planning/ROADMAP.md, .planning/REQUIREMENTS.md, .planning/STATE.md, .planning/research/ARCHITECTURE.md, .planning/research/FEATURES.md, .planning/research/PITFALLS.md, .planning/research/SUMMARY.md, .planning/phases/01-crypto-bridge-in-process-signing/*.md, .planning/quick/260713-jqs-change-fixed-frost-parameters-from-t-501/*.md, .planning/quick/260713-itg-massively-speed-up-the-in-process-n-1000/*.md, .planning/quick/260713-kgv-add-a-readme/*.md</files>
  <action>
Replace every project-name/CLI reference to `tsig` with `cheget` across all prose tiers, including completed planning history. This includes:
- Titles/headers: `.planning/PROJECT.md` `# tsig — 51-of-100 FROST Taproot Signing CLI` → `# cheget — ...`; the `.claude/CLAUDE.md` `## Project **tsig — ...**` header and all mentions.
- CLI invocation examples in README.md, SPEC-frost-cli.md, and any planning doc (`tsig keygen`, `tsig watcher address ...`, `tsig address --network`) → `cheget ...`.
- The `TSIG_*` env-var names wherever they appear in prose (docs, planning history) → `CHEGET_*`.
- All Phase-1 artifacts under .planning/phases/01-... and all quick-task docs under .planning/quick/** that mention `tsig` (PLAN/SUMMARY/RESEARCH/REVIEW/VALIDATION/VERIFICATION/CONTEXT/DISCUSSION-LOG).

Handle case variants: lowercase `tsig` and any Title-case `Tsig`/`TSIG` in prose (map to `cheget`/`Cheget`/`CHEGET` as the casing demands). Do NOT touch: dependency names/versions, the BIP341 KAT vector bytes/address, the fee-math `/1000`, the git remote, or the `scriptSig` term in schemes/*.md (unrelated Bitcoin-script word that matches `tsig` case-insensitively). `.planning/config.json` contains no project title — no change expected; confirm rather than assume.

If any GSD task-directory slug or filename contains `tsig`, FLAG it in the summary — do NOT rename directories/slugs (content-only task).
  </action>
  <verify>
    <automated>test -z "$(grep -rin 'tsig' . 2>/dev/null | grep -v '/target/' | grep -v 'Cargo.lock' | grep -vi 'scriptsig')" && test -z "$(grep -rn 'TSIG_' . 2>/dev/null | grep -v '/target/')"</automated>
  </verify>
  <done>`grep -rin 'tsig' .` (excluding /target/, Cargo.lock, and the `scriptSig` false positive) returns nothing in tracked files; `grep -rn 'TSIG_'` (excluding /target/) is empty. Any deliberately-kept survivor is listed in the summary with a reason.</done>
</task>

</tasks>

<verification>
- Build: `cargo build`, `cargo build --release`, `cargo clippy --lib` all clean.
- Binary identity: `target/debug/cheget` exists; `cargo run -- --help` shows `cheget` as the program name.
- Tests: `compile_fail` (with regenerated .stderr), `dkg_small`, `inproc_sign`, `bridge_roundtrip`, `sign_adversarial`, `transport_stub`, `chain_backend_conformance` pass.
- Env interface: no `TSIG_` anywhere; overrides respond to `CHEGET_*`.
- Final audit: `grep -rin 'tsig' .` (minus /target/, Cargo.lock dep hashes, and `scriptSig`) is empty across tracked files.
</verification>

<success_criteria>
The project builds, tests, and lints clean under the name `cheget`; the produced binary is `cheget`; env overrides use the `CHEGET_*` prefix; and no `tsig` project-name reference survives in any tracked file (the only tolerated match is the unrelated `scriptSig` substring in schemes/*.md).
</success_criteria>

<output>
Create `.planning/quick/260713-kxi-rename-project-from-tsig-to-cheget/260713-kxi-SUMMARY.md` when done.
</output>
