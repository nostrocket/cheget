---
phase: 260715-msx
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - README.md
autonomous: true
requirements:
  - DOC-README-CLI
  - DOC-README-PHASE2

must_haves:
  truths:
    - "README's CLI tree lists share-status (participant) and roster (coordinator) alongside the existing commands."
    - "README documents share-status and roster usage accurately: read-only inspectors, share-status never prompts for a passphrase, roster reads the coordinator's public SQLite store."
    - "The 'Three commands run end-to-end' count is corrected to match the actual number of CLI commands."
    - "README no longer claims 'no persistence yet' / 'Phase 1 only' as blanket status; Phase 2 (persistence & storage) is presented as implemented."
    - "Phase 2 is removed from the 'Planned (not yet built)' list."
    - "README does NOT overstate: it still states keygen writes only the public package, sign runs an in-process simulate-all-seats DKG, and no CLI flow persists a secret share yet."
  artifacts:
    - README.md
  key_links:
    - "Every documented command/flag matches `./target/release/cheget <...> --help` output."
---

<objective>
Update README.md so its CLI documentation matches the currently available commands in the release binary, and correct the now-stale "no persistence / Phase 1 only" framing (Phase 2 has shipped).

Purpose: The README currently omits the two Phase 2 read-only inspector commands (`participant share-status`, `coordinator roster`) and frames the whole project as Phase-1-only with no persistence. Phase 2 (persistence & storage layer) is implemented. The docs must reflect reality while preserving the README's honest, precise tone and NOT overstating what shipped.

Output: An updated README.md — scope is README.md only. No source or other docs.
</objective>

<execution_context>
@$HOME/.claude/gsd-core/workflows/execute-plan.md
@$HOME/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@README.md

## Authoritative CLI surface (verified 2026-07-15 from `./target/release/cheget <...> --help`)

The executor MUST re-verify each command/flag below against `./target/release/cheget <...> --help` before writing — do not invent flags.

- `cheget participant keygen --out <OUT> [--ceremony <C>] [--seats <N>] [--threshold <T>] [--full] [--key-id <ID> (default active)]`
- `cheget participant sign [--session <S>] [--psbt <FILE>] [--key <K> (default active)] [--seats <N>] [--threshold <T>] [--full] [--network <bitcoin|testnet|signet|regtest> (default regtest)] [--yes]`
- `cheget participant share-status [--home <PATH>]`  — NEW (Phase 2, 02-04): lists held shares by reading the plaintext manifest; NO unlock, never prompts for a passphrase (D-05)
- `cheget coordinator keygen ...` (identical flags to participant keygen)
- `cheget coordinator sign ...` (identical flags to participant sign)
- `cheget coordinator roster [--key-id <ID> (default active)] [--home <PATH>]`  — NEW (Phase 2, 02-04): lists the roster from the coordinator's public SQLite store (STOR-03)
- `cheget watcher address --pubkey <FILE> [--network <bitcoin|testnet|signet|regtest> (default bitcoin)]`

## Precision guardrails — do NOT overstate

- keygen still writes ONLY the public PublicKeyPackage envelope; no secret share is persisted.
- sign still runs an in-process simulate-all-seats DKG; it does NOT consume a persisted secret share.
- There is currently NO CLI command that writes/persists a secret share, so `share-status` reads a manifest that normal CLI flows do not yet populate. Phase 2 delivered the storage LAYER + the two read-only inspector commands; the persisted-key ceremony/signing flow is still later-phase work.
- Phase 2 delivered: age/scrypt participant store, encrypted between-round ceremony checkpointing, coordinator SQLite store.
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add share-status and roster to the CLI surface section and correct the command count</name>
  <files>README.md</files>
  <action>
First run `./target/release/cheget participant share-status --help`, `./target/release/cheget coordinator roster --help`, and (to confirm no flag drift) `--help` on the other commands, to confirm the exact subcommand names and flags before editing.

In the "CLI surface today" section:

1. Update the clap persona tree (the fenced text block) to add the two Phase-2 commands. Under `participant` add a `share-status` leaf; under `coordinator` add a `roster` leaf. Give each a short inline gloss consistent with the existing entries' style — e.g. share-status: read-only; lists held shares from the plaintext manifest; never unlocks or prompts for a passphrase. roster: read-only; lists the roster from the coordinator's public SQLite store.

2. Add a short usage bullet for each new command, matching the existing bullet style used for `watcher address` / `keygen` / `sign` (a one-to-two sentence description plus a fenced `text` usage line). For share-status document `cheget participant share-status [--home <PATH>]` and state plainly that it does NOT prompt for a passphrase and performs no decryption (it reads a plaintext manifest, D-05). For roster document `cheget coordinator roster [--key-id <ID>] [--home <PATH>]` and state it reads the coordinator's public SQLite store. Note that both are read-only inspectors introduced in Phase 2.

3. Correct the count in the sentence that currently reads "Three commands run end-to-end from the command line...". Re-count the actual CLI commands from the verified surface and state the correct number (do not leave "Three"). Preserve the existing caveat that the ceremony commands (keygen, sign) still run an in-process simulate-all-seats DKG with no persisted secret share.

Keep the honest, precise tone. Do NOT claim share-status reflects shares written by a normal CLI flow — no CLI command persists a secret share yet, so note the manifest it reads is not populated by current keygen/sign flows.
  </action>
  <verify>
    <automated>test -x ./target/release/cheget && ./target/release/cheget participant share-status --help >/dev/null && ./target/release/cheget coordinator roster --help >/dev/null && grep -q 'share-status' README.md && grep -q 'roster' README.md && ! grep -q 'Three commands run end-to-end' README.md</automated>
  </verify>
  <done>The CLI tree and usage bullets in README.md include share-status and roster with accurate read-only descriptions; the stale "Three commands run end-to-end" count is corrected to the real number; no invented flags appear (all match `--help`).</done>
</task>

<task type="auto">
  <name>Task 2: Correct the stale "no persistence / Phase 1 only" framing and move Phase 2 into what-works</name>
  <files>README.md</files>
  <action>
Update the project-status framing so it reflects that Phase 2 (persistence & storage) has shipped, WITHOUT overstating (see the precision guardrails in context).

1. Status banner (top of README): revise the blockquote that says only the Phase-1 cryptographic core is implemented and there is "no persistence yet". State that the persistence & storage LAYER (Phase 2) is now implemented — age/scrypt participant store, encrypted between-round ceremony checkpointing, coordinator SQLite store — plus the two read-only inspector commands. Keep the honest caveat that there is still no networking / relay / transport layer, and that no CLI flow yet persists or consumes a secret share (the persisted-key ceremony/signing flow is later-phase work).

2. Any other blanket "no persistence" / "all current state lives in memory" / "Phase 1 only" claims in the body (including the "Core value" paragraph, the "Status: what works today" heading/intro, the mainnet safety blockquote, and the "Security model (current)" closing paragraph): soften from "no persistence exists" to the accurate statement that the storage layer exists but is not yet wired into a fund-custody signing flow. Do not remove the genuine safety warnings about real funds — the only mainnet-safe action today is still offline address derivation.

3. In the "Planned (not yet built)" list, remove the "Phase 2 — Persistence & storage" bullet (it has shipped) and remove the lead-in clause asserting "there is no persistence layer yet". Keep Phases 3–7 as planned. Add a concise mention of the shipped persistence layer to the "what works today" area (either a new short paragraph/row or an addition to the existing status section) so the capability is documented as implemented.

4. Preserve every precision guardrail: keygen writes only the public package; sign runs an in-process simulate-all-seats DKG and does not consume a persisted secret share; no CLI command persists a secret share yet. Do not claim end-to-end persisted-key custody works.

Re-read the edited sections after changes to confirm no contradiction remains (e.g., one section calling persistence "planned" while another calls it "shipped").
  </action>
  <verify>
    <automated>! grep -q 'Phase 2 — Persistence' README.md && ! grep -qi 'no persistence layer yet' README.md && grep -qi 'age/scrypt' README.md && grep -qi 'simulate-all-seats' README.md</automated>
  </verify>
  <done>README no longer frames the project as Phase-1-only-with-no-persistence; Phase 2 is documented as shipped (storage layer + read-only inspectors) and removed from the Planned list; precision guardrails are intact (only public package written, in-process simulate-all-seats DKG, no secret-share persistence via CLI); genuine real-funds safety warnings preserved; no internal contradiction between sections.</done>
</task>

</tasks>

<verification>
- `./target/release/cheget participant share-status --help` and `./target/release/cheget coordinator roster --help` succeed, confirming the documented commands exist.
- Every command/flag documented in README.md matches the verified `--help` surface (no invented flags).
- README's CLI section documents share-status and roster; the command count is corrected.
- README presents Phase 2 as shipped without overstating, and removes it from the Planned list.
- No section contradicts another on persistence status.
</verification>

<success_criteria>
- README.md accurately documents the current CLI surface (7 subcommands including share-status and roster).
- The "Three commands run end-to-end" count is corrected.
- The stale "no persistence / Phase 1 only" framing is corrected; Phase 2 moved from Planned into what-works.
- Precision guardrails intact: no claim that any CLI flow persists/consumes a secret share.
- Honest, precise tone preserved; scope limited to README.md.
</success_criteria>

<output>
Create `.planning/quick/260715-msx-update-readme-md-cli-documentation-to-ma/260715-msx-SUMMARY.md` when done.
</output>
