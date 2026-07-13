---
phase: quick
plan: 260713-kgv
type: execute
wave: 1
depends_on: []
files_modified:
  - README.md
autonomous: true
requirements: []
must_haves:
  truths:
    - "A README.md exists at the repo root describing cheget accurately"
    - "The README states the fixed parameters as 51-of-100 (t=51, n=100) everywhere"
    - "Phase 1 capabilities are described as implemented; transport/persistence/rotation/lifecycle are marked planned"
    - "No invented install instructions, crates.io publication, or non-existent features appear"
  artifacts:
    - "README.md"
  key_links:
    - "README status claims map 1:1 to what Phase 1 actually shipped (bridge, in-process DKG, in-process signing to confirmed regtest key-spend, Transport seam)"
---

<objective>
Create a top-level `README.md` at the repo root for `cheget` — the 51-of-100 FROST
Taproot signing CLI. It does not exist yet.

Purpose: Give a reader an accurate, non-overclaiming orientation: what the project
is, what actually works today (Phase 1), and what is planned but not yet built.

Output: `README.md` at repo root.
</objective>

<execution_context>
@$HOME/.claude/gsd-core/workflows/execute-plan.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@Cargo.toml
@src/lib.rs
@src/cli/mod.rs
</context>

<accuracy_constraints>
These are hard constraints on README content. Violating any is a defect.

- **Fixed parameters:** always `51-of-100`, `t=51`, `n=100`. NEVER 501/1000. (Note: the
  `Cargo.toml` `description` field is stale at "501-of-1000" — do NOT copy it; do NOT edit
  Cargo.toml in this task.)
- **What is DONE (Phase 1 only) — describe in present tense as working:**
  - The frost↔rust-bitcoin key bridge (`src/bridge/taproot.rs`): FROST `VerifyingKey`
    → 33-byte SEC1 → x-only → `XOnlyPublicKey` → BIP341 P2TR address (merkle root `None`),
    pinned by a byte-level round-trip / known-answer test (`tests/bridge_roundtrip.rs`).
  - In-process DKG (simulated participants, no transport) producing a group
    `PublicKeyPackage` whose verifying key is the Taproot internal key `P`, with
    client-side key confirmation (`src/crypto/`).
  - In-process two-round FROST signing with the Taproot tweak
    (`sign_with_tweak` / `aggregate_with_tweak(…, None)`) producing a 64-byte BIP340
    signature that verifies against the output key `Q`, finalizing a PSBT and broadcasting
    a CONFIRMED key-spend on regtest (`src/session/`, crown-jewel test
    `tests/inproc_sign_100.rs::inproc_sign_confirmed_regtest_key_spend_51_of_100`, `#[ignore]`
    / on-demand).
  - The `Transport` trait + in-memory/in-process stub (`src/transport/`) — the
    architectural seam later ceremony phases run against; no relay/Nostr code exists yet.
  - The `ChainBackend` trait + Bitcoin Core RPC and Esplora impls + key-spend sighash
    helper (`src/chain/`).
  - Structural security controls present from Phase 1: a non-serializable nonce type
    (won't compile if you try to persist it), display-before-sign sighash recompute,
    tweak/aggregate verified against `Q`.
  - The clap persona tree (participant / coordinator / watcher); today only
    `watcher address` is fully wired end-to-end from the CLI — keygen/sign handlers are
    the in-process/test-driven paths. Describe CLI status honestly; do not imply a
    polished multi-command UX that isn't wired.
- **What is PLANNED / NOT built — mark clearly as future (Phases 2–7):**
  persistence & at-rest encryption (age/scrypt, SQLite), n=100 DKG at scale, membership
  rotation (refresh/enroll/repair), key lifecycle (standby key, sweep, rollover), policy
  watcher, reproducible-build/audit hardening, and ALL real transport (Nostr, offline file
  mode). No relay code exists yet.
- **Security claims — keep accurate:** nonces live in memory only and are never persisted
  (enforced by a non-serializable type); no individual ever holds the full key; Nostr
  identity keys (future) are transport-only and never derived from FROST material. Do NOT
  claim any property the code does not yet enforce.
- **Do NOT invent:** installation instructions beyond what the repo supports
  (`cargo build --release` / `cargo test` are fine; there is NO crates.io release, NO
  `cargo install cheget`, NO release binaries, NO homebrew). Do not fabricate benchmarks
  beyond those recorded in STATE.md if cited (full 51/100 regtest key-spend ~9.9s, DKG
  group-key proof ~4.4s) — attribute them as measured local timings, optional to include.
- Reference `SPEC-frost-cli.md` (draft design) and the ROADMAP for the full end-state
  vision, clearly separated from current status.
</accuracy_constraints>

<tasks>

<task type="auto">
  <name>Task 1: Write repo-root README.md</name>
  <files>README.md</files>
  <action>
Create `README.md` at the repo root with these sections, honoring every rule in
`<accuracy_constraints>` above:

1. Title + one-line description: `cheget` — a single-binary Rust CLI letting a fixed
   51-of-100 group jointly control one Bitcoin Taproot address via FROST threshold
   Schnorr signatures (RFC 9591, secp256k1, BIP340/341 key-path spend), on-chain
   indistinguishable from single-sig.
2. "Core value" — the four properties (any 51 can spend; no individual holds the key;
   membership rotates with zero on-chain footprint; past compromise is revocable by
   sweep to a standby key), phrased as the project GOAL. Immediately follow with a clear
   "Status" note that only the crypto core is built so far.
3. "Status: what works today" — a bullet or table listing the Phase 1 DONE items from
   `<accuracy_constraints>`, each described in present tense with the source module and,
   where relevant, the pinning test. Make clear this is in-process (single host, no
   transport, no persistence).
4. "Planned (not yet built)" — a bullet list of Phases 2–7 items marked clearly as
   future, cross-referencing the ROADMAP. State plainly that NO transport/relay/Nostr
   code exists yet and there is no persistence layer yet.
5. "Architecture" — brief layered module map from `src/lib.rs` (bridge, crypto, chain,
   transport, session, cli) and the load-bearing `Transport` trait seam idea. Keep it
   short; link to `SPEC-frost-cli.md` and `.planning/ROADMAP.md` for depth.
6. "Building & testing" — `cargo build --release` and `cargo test`; note the heavy
   full-scale tests are `#[ignore]` and run on demand (mention the crown-jewel test name).
   Note MSRV 1.85 and that `Cargo.lock` is committed for reproducibility. Do NOT invent
   install/distribution steps.
7. "Security model (current)" — nonces never persisted (non-serializable type),
   display-before-sign, no individual holds the key, tweak/aggregate verified against `Q`.
   Only claim what the code enforces today.
8. "License" — MIT OR Apache-2.0 (from Cargo.toml).

Write plain GitHub-flavored Markdown. Do not edit any file other than README.md.
  </action>
  <verify>
    <automated>test -f README.md && grep -q "51-of-100" README.md && ! grep -Eq "501-of-1000|501 of 1000|t = 501|t=501|n = 1000|n=1000" README.md && ! grep -Eiq "cargo install cheget|crates\.io|homebrew|brew install" README.md && echo OK</automated>
  </verify>
  <done>
README.md exists at repo root; uses 51-of-100 (t=51/n=100) exclusively with no
501/1000 references; describes Phase 1 as done and Phases 2–7 as planned; contains no
invented install/distribution claims; security claims match what the code enforces.
  </done>
</task>

</tasks>

<verification>
- `test -f README.md` passes.
- `grep -c '51-of-100' README.md` ≥ 1; no 501/1000 matches.
- No crates.io / cargo install / homebrew claims.
- Manual read confirms Phase 1 vs planned separation is unambiguous and no capability is overclaimed.
</verification>

<success_criteria>
A reader lands on the repo, understands what cheget aims to be, sees exactly what is
implemented today (Phase 1 crypto core, in-process, no transport/persistence) versus
what is planned, and encounters no false or overclaimed capability.
</success_criteria>

<output>
Create `.planning/quick/260713-kgv-add-a-readme/260713-kgv-SUMMARY.md` when done.
</output>
