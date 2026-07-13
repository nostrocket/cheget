---
phase: quick-260713-jqs
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - .claude/CLAUDE.md
  - .planning/PROJECT.md
  - .planning/ROADMAP.md
  - .planning/REQUIREMENTS.md
  - .planning/STATE.md
  - SPEC-frost-cli.md
  - .planning/research/ARCHITECTURE.md
  - .planning/research/FEATURES.md
  - .planning/research/PITFALLS.md
  - .planning/research/STACK.md
  - .planning/research/SUMMARY.md
  - .planning/phases/01-crypto-bridge-in-process-signing/
  - .planning/quick/260713-itg-massively-speed-up-the-in-process-n-1000/
  - src/lib.rs
  - src/cli/mod.rs
  - src/cli/keygen.rs
  - src/cli/sign.rs
  - src/crypto/keygen.rs
  - src/crypto/nonce.rs
  - src/session/liveness.rs
  - src/transport/envelope.rs
  - src/chain/mod.rs
  - src/chain/esplora.rs
  - tests/dkg_100_correctness.rs
  - tests/inproc_sign_100.rs
  - tests/inproc_sign.rs
  - tests/dkg_small.rs
  - tests/common/mod.rs
autonomous: true
requirements: []

must_haves:
  truths:
    - "Every FROST threshold/membership reference reads t=51, n=100 across live docs, planning history, source, and tests"
    - "The two full-scale test files are renamed to _100 and their crown-jewel functions renamed to _100 / _51_of_100"
    - "The #[ignore] rationale reflects the real n=100 cost (not multi-CPU-hour), backed by a measured wall-clock"
    - "cargo build, build --release, clippy --lib are clean and the full fast suite + the renamed 51/100 tests pass"
    - "Every surviving 501/1000 literal is a deliberate non-membership number (fee math, versions, KAT, amounts)"
  artifacts:
    - tests/dkg_100_correctness.rs
    - tests/inproc_sign_100.rs
  key_links:
    - "Fee math (/1000 sat conversions), crate/version/MSRV numbers, and the BIP341/86 KAT vector must remain UNCHANGED"
    - "tests/common/mod.rs and inproc_sign.rs / dkg_small.rs doc cross-references point at the renamed _100 files"
---

<objective>
Change the project's fixed FROST parameters from t=501 / n=1000 to t=51 / n=100 everywhere — live docs, completed planning history, source, and tests — renaming the 1000-scale test files/functions to _100, correcting the now-false #[ignore] cost rationale, and running the full verification suite.

Purpose: 51/100 is still a strict majority (51 > 50) matching the prior 501-of-1000 intent, at a scale whose in-process DKG completes in seconds-to-minutes rather than multi-CPU-hours. Threshold is permanently t=51, n=100 after this.

Output: All membership/threshold references updated surgically; renamed test binaries passing at 51/100; a summary documenting the #[ignore] decision, measured timings, and every deliberately-kept 501/1000 literal.
</objective>

<execution_context>
@$HOME/.claude/gsd-core/workflows/execute-plan.md
@$HOME/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.claude/CLAUDE.md

# Guardrail — this is SURGICAL editing, NOT blind find-replace.
# The literals 1000 / 501 / 100 appear in many non-parameter places.
# Change a number ONLY when it denotes the FROST threshold t (=501→51) or membership size n (=1000→100).
# NEVER change, and treat as UNTOUCHABLE:
#   - Fee math: src/chain/core_rpc.rs:100 `to_sat() / 1000` (sat/kvB→sat/vB) and similar rate math in src/chain/esplora.rs, src/chain/mod.rs — millisat/kilo conversions.
#   - Dependency / crate versions, MSRV (1.85), toolchain (1.96.0) in Cargo.toml / Cargo.lock.
#   - The BIP341/BIP86 KAT vector (hardcoded bc1p… address, pubkey/hash bytes) in src/bridge/taproot.rs, tests/bridge_roundtrip.rs, tests/vectors/*.json — INCLUDING PublicKeyPackage::new(..., Some(501)) at tests/bridge_roundtrip.rs:125, which is a fixed vector min_signers, NOT the group threshold — LEAVE IT.
#   - Block heights, satoshi amounts, timeouts, ports, byte sizes, any other non-membership quantity.
#   - Audited crypto internals (frost-core / frost-secp256k1-tr), the nonce type, bridge parity/even-Y logic, verify-against-Q, SIGN-06 culprit handling, and the recent rayon DKG parallelization — do NOT alter behaviour.
# When a specific 1000/501 is genuinely ambiguous, LEAVE it and list it for human review in the summary.
</context>

<tasks>

<task type="auto">
  <name>Task 1: Update live source-of-truth docs</name>
  <files>.claude/CLAUDE.md, .planning/PROJECT.md, .planning/ROADMAP.md, .planning/REQUIREMENTS.md, .planning/STATE.md, SPEC-frost-cli.md, .planning/research/ARCHITECTURE.md, .planning/research/FEATURES.md, .planning/research/PITFALLS.md, .planning/research/STACK.md, .planning/research/SUMMARY.md</files>
  <action>Grep each file for the membership/threshold literals (501, 1000, and n-derived prose) and edit ONLY the ones denoting FROST t or n. Set t = 51 and n = 100. Update n-derived prose: in .claude/CLAUDE.md change "Fixed parameters: t = 501, n = 1000" → "t = 51, n = 100", the core-value "A group of 1000…" → "A group of 100…", "1000 people must be able to verify" → "100 people", and the ceremony figure "~10⁶ events (~1 GB)" — that count is n² at n=1000, so at n=100 it drops ~two orders of magnitude to ~10⁴ events; rewrite to a consistent order-of-magnitude figure and mark it an estimate. Apply the same "501-of-1000" → "51-of-100" and "any 501" → "any 51", "group of 1000" → "group of 100" prose fixes wherever they appear across PROJECT.md, ROADMAP.md, REQUIREMENTS.md, STATE.md (including the Core value line), SPEC-frost-cli.md, and the five research/*.md files. Honour the guardrail in <context> — do NOT touch fee math, crate/version/MSRV numbers, the KAT vector, or any non-membership quantity; leave genuinely-ambiguous literals and note them for the summary.</action>
  <verify>grep -rnE '\b(501|1000)\b' .claude/CLAUDE.md .planning/PROJECT.md .planning/ROADMAP.md .planning/REQUIREMENTS.md .planning/STATE.md SPEC-frost-cli.md .planning/research/ — manually confirm every remaining hit is a deliberate non-membership number (version/MSRV/etc.)</verify>
  <done>All live docs read t=51 / n=100; core-value and verifier-count prose reflect 100; ceremony event estimate rescaled with an "estimate" note; no membership literal of 501/1000 remains; guardrailed literals untouched.</done>
</task>

<task type="auto">
  <name>Task 2: Rewrite completed planning artifacts (Phase 1 + quick-task prose)</name>
  <files>.planning/phases/01-crypto-bridge-in-process-signing/ (01-01…01-05 PLAN.md + SUMMARY.md, 01-CONTEXT.md, 01-DISCUSSION-LOG.md, 01-RESEARCH.md, 01-REVIEW.md, 01-UAT.md, 01-VALIDATION.md, 01-VERIFICATION.md), .planning/quick/260713-itg-massively-speed-up-the-in-process-n-1000/ (260713-itg-PLAN.md, 260713-itg-SUMMARY.md)</files>
  <action>Per the user decision to rewrite history, grep every file under the Phase-1 directory and the 260713-itg quick-task directory for membership/threshold literals and n-derived prose, changing t=501→51, n=1000→100, "501-of-1000"→"51-of-100", "n=1000"→"n=100" and similar. In 01-UAT.md, additionally update the pending run commands to point at the RENAMED test binaries (`--test inproc_sign_100`, `--test dkg_100_correctness`) and read "51-of-100" (Task 4 performs the renames; reference the new names here). EXCEPTION: do NOT rename the 260713-itg directory — its slug is an internal GSD task identifier that STATE/tracking references; renaming risks corrupting GSD state. Only edit prose INSIDE its .md files. Note this preserved-directory-name exception explicitly in the summary. Honour the <context> guardrail; leave version/KAT/fee literals and ambiguous cases (list them).</action>
  <verify>grep -rnE '\b(501|1000)\b' .planning/phases/01-crypto-bridge-in-process-signing/ .planning/quick/260713-itg-massively-speed-up-the-in-process-n-1000/ — confirm survivors are deliberate non-membership numbers; confirm the itg directory name is unchanged (`ls .planning/quick/ | grep 260713-itg`)</verify>
  <done>All Phase-1 artifacts and the itg quick-task .md prose read 51/100; 01-UAT.md commands reference the renamed _100 binaries and "51-of-100"; the 260713-itg directory name is unchanged; exception noted for summary.</done>
</task>

<task type="auto">
  <name>Task 3: Update source code (doc-comments, default roster sizes, help text)</name>
  <files>src/lib.rs, src/cli/mod.rs, src/cli/keygen.rs, src/cli/sign.rs, src/crypto/keygen.rs, src/crypto/nonce.rs, src/session/liveness.rs, src/transport/envelope.rs, src/chain/mod.rs, src/chain/esplora.rs</files>
  <action>Inspect each 501/1000 hit in context and change only membership/threshold occurrences. Known hits: src/lib.rs:1,3 (title + "t=501, n=1000" doc → 51/100); src/cli/mod.rs:19 (title comment); src/cli/keygen.rs:18-19 (const FULL_THRESHOLD: u16 = 501 → 51, const FULL_SEATS: u16 = 1000 → 100) and :36 help text; src/cli/sign.rs:28-29 (same two consts → 51 / 100) and :52 help text; src/crypto/keygen.rs:5,8,70 (doc-comments referencing n=1000 / 501-of-1000 / "t = 501, n = 1000"); src/crypto/nonce.rs:7 ("501 extracted shares" → "51 extracted shares"); src/session/liveness.rs:10 ("t=501/n=1000" → "t=51/n=100"); src/transport/envelope.rs:23 ("n = 1000" → "n = 100"); src/chain/mod.rs:15 and src/chain/esplora.rs:4 (doc-comment "n=1000 confirm path" → "n=100 confirm path"). DO NOT touch src/chain/core_rpc.rs:100 `to_sat() / 1000` (fee conversion) or the analogous rate math in esplora.rs / mod.rs — those are millisat/kilo, not membership. Do NOT alter crypto internals, the nonce type, bridge logic, or rayon parallelization.</action>
  <verify>cargo build 2>&1 | tail -5 (compiles); grep -rnE '\b(501|1000)\b' src/ — confirm the only survivor is the fee-math `/ 1000` in core_rpc.rs (and any legitimate rate math), NOT any FROST parameter</verify>
  <done>FULL_THRESHOLD=51 / FULL_SEATS=100 in both cli/keygen.rs and cli/sign.rs; all source doc-comments and help text read 51/100; fee math untouched; cargo build succeeds.</done>
</task>

<task type="auto">
  <name>Task 4: Rename full-scale tests, update env defaults, fix cross-references and #[ignore] language</name>
  <files>tests/dkg_100_correctness.rs (from dkg_1000_correctness.rs), tests/inproc_sign_100.rs (from inproc_sign_1000.rs), tests/inproc_sign.rs, tests/dkg_small.rs, tests/common/mod.rs</files>
  <action>Run `git mv tests/dkg_1000_correctness.rs tests/dkg_100_correctness.rs` and `git mv tests/inproc_sign_1000.rs tests/inproc_sign_100.rs`. In inproc_sign_100.rs: rename fn `inproc_sign_confirmed_regtest_key_spend_501_of_1000` → `inproc_sign_confirmed_regtest_key_spend_51_of_100`; change CHEGET_SIGN_T default 501→51 and CHEGET_SIGN_N default 1000→100; update the module doc-comment (title "t=501/n=1000", the run command `--test inproc_sign_1000` → `--test inproc_sign_100`, and "default 501/1000" → "default 51/100"). In dkg_100_correctness.rs: rename fn `dkg_1000_all_shares_verify_to_one_group_key` → `dkg_100_all_shares_verify_to_one_group_key`; change CHEGET_DKG_N default 1000→100 and CHEGET_DKG_T default 501→51; update "all 1000 KeyPackages" prose → "all 100 KeyPackages" and the "t=501, n=1000" doc. In tests/inproc_sign.rs: update line 10 doc pointing at `tests/inproc_sign_1000.rs` → `tests/inproc_sign_100.rs`, and the over_provisioned_poll_size assertions at lines 79-84 that use FROST membership values (501, 1000) and the "501-of-1000 polls a 51-seat margin" message — recompute for 51/100 (e.g. over_provisioned_poll_size(51, 100) with the corresponding margin) keeping the assertion's intent; inspect the (501, 510) case similarly. In tests/dkg_small.rs:3 update "real t=501/n=1000 correctness" → "real t=51/n=100". In tests/common/mod.rs:93 update the doc reference to the renamed n=100 file. For the #[ignore] attribute strings in BOTH renamed files, do NOT decide removal here — just correct the now-false "multi-CPU-hour / nightly" language to describe the real n=100 cost (Task 5 measures and finalizes the ignore decision). Honour the guardrail; leave the bridge_roundtrip.rs Some(501) KAT value untouched.</action>
  <verify>cargo build --tests 2>&1 | tail -5 (all test targets compile with new names); git status --short (shows the two renames as R); grep -rnE '\b(501|1000)\b' tests/ — confirm survivors are the KAT Some(501) in bridge_roundtrip.rs and any non-membership number only</verify>
  <done>Both test files renamed via git mv; crown-jewel fns renamed to _100 / _51_of_100; env defaults 51/100; all doc cross-references and the poll-size assertions updated; #[ignore] language corrected (decision deferred to Task 5); test targets compile.</done>
</task>

<task type="auto">
  <name>Task 5: Full verification — build/clippy/suite, measure 51/100 timings, finalize #[ignore], grep audit</name>
  <files>tests/inproc_sign_100.rs, tests/dkg_100_correctness.rs (only if removing #[ignore])</files>
  <action>Run and require clean: `cargo build`, `cargo build --release`, `cargo clippy --lib`. Run the full fast suite and require pass: dkg_small, inproc_sign, bridge_roundtrip, sign_adversarial, transport_stub, chain_backend_conformance, compile_fail. MEASURE actual full-scale wall-clock at 51/100 for both renamed tests: `cargo test --release --test inproc_sign_100 -- --ignored --nocapture` and `cargo test --release --test dkg_100_correctness -- --ignored --nocapture`; confirm both PASS (crown-jewel confirmed regtest key-spend + DKG-correctness/one-group-key proof). #[ignore] DECISION: if a test completes under ~90s in release, you MAY remove its #[ignore] and fold it into the normal suite (desirable); only if fast — otherwise keep it ignored with the corrected n=100-cost language from Task 4. Record the decision and the measured times per test in the summary. FINAL AUDIT: `grep -rnE '\b(501|1000)\b' src/ tests/` and across the docs touched in Tasks 1-2; list every surviving hit and confirm each is a legitimate non-membership number (fee `/1000`, KAT Some(501), crate/version/MSRV). Do NOT add dependencies; keep Cargo.lock consistent.</action>
  <verify>cargo build && cargo build --release && cargo clippy --lib all exit 0; fast suite passes; both `--test inproc_sign_100 --ignored` and `--test dkg_100_correctness --ignored` pass; final grep survivors all explained</verify>
  <done>All builds + clippy clean; full fast suite green; both renamed 51/100 tests pass with recorded wall-clock times; #[ignore] decision made and justified per measured time; grep audit lists every kept 501/1000 as a deliberate non-membership number; no new deps, Cargo.lock unchanged.</done>
</task>

</tasks>

<verification>
- cargo build, cargo build --release, cargo clippy --lib — clean.
- Fast suite (dkg_small, inproc_sign, bridge_roundtrip, sign_adversarial, transport_stub, chain_backend_conformance, compile_fail) — pass.
- Renamed 51/100 full-scale tests (inproc_sign_100, dkg_100_correctness) — pass, timed.
- `grep -rnE '\b(501|1000)\b'` over src/, tests/, and touched docs — every survivor is a deliberate non-membership number.
- git status shows the two test files as renames (R), not delete+add.
</verification>

<success_criteria>
- Fixed FROST parameters are permanently t=51, n=100 across live docs, planning history, source, and tests.
- Full-scale tests renamed to _100 with crown-jewel functions _100 / _51_of_100 and env defaults 51/100.
- The #[ignore] rationale reflects real n=100 cost with a measured wall-clock, and any ignore removal is justified by a sub-~90s release time.
- Guardrailed literals (fee math, versions/MSRV, KAT vector, crypto internals, rayon work) are provably untouched.
- Summary documents: the #[ignore] decision + measured times, the preserved 260713-itg directory-name exception, and every deliberately-kept 501/1000 literal.
</success_criteria>

<output>
Create `.planning/quick/260713-jqs-change-fixed-frost-parameters-from-t-501/260713-jqs-SUMMARY.md` when done.
</output>
