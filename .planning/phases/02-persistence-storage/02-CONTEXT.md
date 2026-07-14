# Phase 2: Persistence & Storage - Context

**Gathered:** 2026-07-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Lay down the **durable-state foundation** the ceremony and transport layers build on, so no
durable state is retrofitted later. Three concerns:

1. **Participant store** (`~/.cheget/`) — the identity keypair plus per-`(key_id, epoch, seat)`
   `KeyPackage`+`PublicKeyPackage`, age/scrypt-encrypted at rest and zeroized in memory after
   use (STOR-01).
2. **Encrypted between-round ceremony checkpointing** — DKG round secrets checkpointed encrypted
   between rounds of the same ceremony; signing nonces remain the sole never-persisted exception
   (STOR-02).
3. **Coordinator SQLite store** (rusqlite) — roster (identifier ↔ npub ↔ status ↔ join/leave
   epochs), ceremony transcripts, session logs, policy config, churn ledger (STOR-03).

**Requirements in scope:** STOR-01, STOR-02, STOR-03.

**Explicitly NOT in this phase:** real network transport / Nostr wire code (Phase 7 — but the
transport-only identity *keypair* is generated and stored here, see D-10..D-12); membership
rotation ceremonies that *write* new epochs (Phase 4 — Phase 2 only provides the multi-epoch
store they write into); sweep/lifecycle/policy engine behavior (Phase 5 — Phase 2 provides the
policy-config and churn-ledger *tables*, not the watcher logic); n=100 DKG scaling proof (Phase 3
— though Phase 3 criterion 3, persist/reload the full n=100 set through these stores, exercises
what Phase 2 builds).

**Key carry-forward from Phase 1 (do not re-litigate):**
- **D-09 (Phase 1):** public artifacts (`PublicKeyPackage`) on disk as plaintext are fine; the
  age/scrypt-encrypted **secret** store is exactly what Phase 2 adds. Phase 2's plaintext manifest
  + in-store public package (D-05) is the direct continuation of this line.
- **Nonce-exclusion is already done at the type level:** `src/crypto/nonce.rs::EphemeralNonces`
  implements no `Serialize`/`Deserialize`/`Clone` and is consumed by value. Phase 2's STOR-02 work
  is *only* the DKG-round-secret half; it MUST NOT add any persistence path that touches nonces.
- **Tagging types already exist:** `src/crypto/types.rs` defines `KeyId`, `Epoch`, `SeatId`
  (= `frost::Identifier`) in the pure crypto core — the exact `(key_id, epoch, seat)` tuple
  STOR-01 tags shares by. Reuse them; do not re-invent.

</domain>

<decisions>
## Implementation Decisions

### Passphrase / unlock UX
- **D-01:** The participant supplies the age/scrypt passphrase via an **interactive no-echo stdin
  prompt on every command** that touches a secret. No env var and no unlock agent in the shipped
  binary — no passphrase lingers in the process environment or memory beyond the operation.
- **D-02:** **One store passphrase** unlocks the identity keypair and every held share. The
  `(key_id, epoch, seat)` tagging lives *inside* each encrypted payload, so per-key isolation is
  not needed for correctness. (Per-key/per-STANDBY passphrases were considered and rejected as
  unnecessary friction.)
- **D-03:** Decryption goes through a **`PassphraseSource` abstraction** (trait/closure) at the
  store API layer. Production CLI wires it to the interactive prompt **only**; tests construct the
  store directly with an in-code passphrase. **No env var or CLI flag for the passphrase ships in
  the binary** — the production surface stays interactive-only while headless CI (e.g. the Phase 3
  n=100 persist/reload check) can still drive the store.
- **D-04:** Store creation prompts **twice and requires a match** before writing; **no
  minimum-strength policy**. A lost passphrase means the share is unrecoverable (no reset) — this
  MUST be documented at creation time. Confirm-twice prevents typo-lockout without paternalism.

### On-disk store layout (participant)
- **D-05:** **File-per-share tree + plaintext manifest.** One age file per `(key_id, epoch, seat)`
  (e.g. `~/.cheget/shares/<key_id>/epoch-<N>/seat-<NNNN>.age`), plus a plaintext `manifest.json`
  indexing what's held (`key_id`, `epoch`, `seat`, `state`, `created-at`). The public
  `PublicKeyPackage` is also written **plaintext** in the tree so `cheget watcher address` and
  `share status` work **from the store alone, with no unlock and no separate `--pubkey` file**
  (continues Phase 1 D-09). Rotation (Phase 4) writes a **new epoch dir alongside** the old; the
  store therefore must hold multiple epochs simultaneously (ROT-06 steady state ≈ 2: one ACTIVE,
  one STANDBY). A single encrypted blob and a participant-side SQLite were both rejected.
- **D-06:** **Decrypt-use-drop per operation.** Decrypted secret material (`KeyPackage`, identity
  key) is loaded at the point of use, wrapped in `zeroize::Zeroizing`, and dropped/zeroized the
  instant the operation finishes — shortest exposure window, mirrors the `EphemeralNonces`
  consume-by-value discipline. Satisfies STOR-01 "zeroized in memory after use".
- **D-07:** **Atomic writes + restrictive perms.** Write to a temp file, fsync, atomic rename into
  place; store dir `0700`, files `0600` (Unix). The **manifest is updated last** so it never
  points at a half-written file. A crash mid-write never corrupts or truncates a share — required
  because Phase 4 rotation depends on a crash-survivable verify→persist→delete ordering.

### Checkpoint lifecycle (STOR-02, DKG-round-secret half)
- **D-08:** **Build the capability, wire it at the seam.** Phase 2 builds an encrypted checkpoint
  store (write/read/clear `dkg::round1::SecretPackage` & `dkg::round2::SecretPackage` keyed by
  ceremony) and exercises it with a **dedicated persist/reload test**. It is NOT wired into the
  hot in-process `run_inprocess_dkg` (which runs all seats in one call with no between-round
  pause); do **not** refactor that path to fake a pause. Real between-round use arrives in Phase 7
  when a ceremony spans processes/restarts over transport.
- **D-09:** Checkpoints are age/scrypt-encrypted under the **same store passphrase** as shares.
  The key is passphrase-derived so checkpoints **survive a restart** (re-prompting on resume,
  consistent with interactive-only D-01). A separate ceremony passphrase was rejected as extra
  friction.
- **D-10:** **Wipe on ceremony success; keep on abort/interruption.** A completed DKG securely
  deletes its round secrets; an interrupted one keeps them so it can **resume per
  `(ceremony_id, round, seat)`** — aligns with Phase 7's idempotent-resume requirement (TRAN-08).
- **D-11:** Checkpoints live in the **participant store**, ceremony-scoped subdir
  (`~/.cheget/ceremonies/<ceremony_id>/<seat>/round-<N>.age`), under the same passphrase and
  atomic-write discipline as shares. The **coordinator SQLite holds only the public transcript**
  (STOR-03) — round secrets are per-participant material and stay with the participant persona.

### Identity keypair scope
- **D-12:** **Generate and store the transport-only identity keypair now.** A dedicated
  secp256k1 keypair, independent of FROST material, age/scrypt-encrypted in the store's identity
  slot. Satisfies STOR-01 literally; Phase 7 just reads it. Unused until transport exists.
- **D-13:** **Separation enforced structurally, not by assertion.** The identity key is its own
  newtype generated from an **independent `OsRng` draw**, with **no API anywhere** that derives it
  from (or into) a FROST share/verifying key. Reuse is a non-expressible operation — same
  discipline as the byte-level-only bridge and the non-serializable nonce. (A belt-and-suspenders
  runtime `identity_pubkey != any FROST key` assertion was considered and judged weaker/later;
  structural separation is the control.)
- **D-14:** Use the **C-lib `secp256k1` crate already in the dependency graph** (via rust-bitcoin)
  — the same family `nostr-sdk` uses in Phase 7 (no re-keying later), and a different crate from
  frost's `k256` (reinforcing separation at the dependency level). Store the raw keypair; wrap in
  `nostr-sdk`'s `Keys` in Phase 7. Pulling in `nostr-sdk` now was rejected (drags transport into
  Phase 2, against the transport-last ordering).
- **D-15:** The **coordinator roster populates real npubs now**, derived from the generated
  identity keys. In the in-process simulation each simulated seat's identity fills its roster row
  (identifier ↔ npub ↔ status ↔ join/leave epochs), making the roster — and the eventual Phase 7
  roster-hash-commit (TRAN-05) — real and testable from Phase 2.

### Claude's Discretion
Left to research/planning against the `age` 0.11.x, `rusqlite` 0.40.x, `zeroize` 1.9.x and
`secp256k1` APIs (grounded in PROJECT.md pins + SPEC), unless a decision above constrains them:
- Exact `age::scrypt` work-factor / recipient-identity wiring and the `Zeroizing` boundaries.
- The concrete `manifest.json` schema and versioning field.
- Secure-delete mechanism for wiped checkpoints (best-effort overwrite vs plain unlink) — note the
  well-known limits of secure-delete on modern filesystems/SSDs; document rather than over-engineer.
- **Coordinator SQLite specifics (STOR-03) not covered above:** the full table schema
  (roster, ceremony transcripts, session logs, policy config, churn ledger), the migration
  strategy (build-full-now vs incremental migrations), DB file location (likely `~/.cheget/` or a
  coordinator-specific dir), single-writer discipline, and exactly what a "ceremony transcript"
  and "session log" record contain. Prefer `rusqlite` `bundled` for reproducible builds (per
  CLAUDE.md). These were flagged but not deep-dived — planner has latitude, guided by the
  requirement text and the persona boundary (D-11: no secret material in the coordinator DB).
- Old-epoch retention/pruning policy beyond "hold ≥2 simultaneously" (the active pruning happens
  in Phase 4 rotation; Phase 2 need only *support* multiple epochs).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design & spec (authoritative)
- `SPEC-frost-cli.md` — full design. Most relevant to Phase 2: **§6.5** signing/nonce discipline
  (nonces are the sole never-persisted exception — STOR-02), the **at-rest storage / age-scrypt**
  and **Nostr↔FROST key-separation** normative rules (D-12..D-14), and **§7** event/kind schema
  context for what the coordinator transcript will eventually carry.
- `.planning/research/PITFALLS.md` — implementation pitfalls; relevant here for nonce-persistence
  and zeroization hazards.
- `implementations-resharing.md` — companion research on resharing/repair (epoch semantics that
  the store's per-epoch tagging must support in Phase 4).

### Project planning
- `.planning/PROJECT.md` — locked crate stack + pins (`age` 0.11.3, `zeroize` 1.9.0,
  `rusqlite` 0.40.1 `bundled`, `secp256k1` via rust-bitcoin), Key Decisions table (do not
  re-litigate).
- `.planning/REQUIREMENTS.md` — STOR-01, STOR-02, STOR-03 (the requirements this phase satisfies);
  STOR-04 already complete in Phase 1.
- `.planning/ROADMAP.md` — Phase 2 success criteria (3 criteria); Phase 3 criterion 3
  (persist/reload the full n=100 set through these stores) exercises Phase 2's output at scale.
- `.planning/phases/01-crypto-bridge-in-process-signing/01-CONTEXT.md` — Phase 1 decisions,
  especially D-09 (public-artifact-on-disk line) which Phase 2 continues.

### Existing code to build on / not break (from codebase scout)
- `src/crypto/types.rs` — `KeyId`, `Epoch`, `SeatId` tagging newtypes. **Reuse for the
  `(key_id, epoch, seat)` tuple.**
- `src/crypto/nonce.rs` — `EphemeralNonces`, the non-serializable nonce type. **STOR-02's
  nonce-exclusion is already satisfied here; do not add any persistence path that touches nonces.**
- `src/cli/address.rs` — `PubkeyEnvelope` (existing plaintext public-artifact JSON: hex of
  `PublicKeyPackage.serialize()` + `key_id` + `epoch`). The participant store's in-store public
  package (D-05) should reuse / align with this format so `address` derivation stays single-path.
- `src/crypto/keygen.rs` — `run_inprocess_dkg` and the `dkg::round1/round2::SecretPackage` types
  that the checkpoint store (D-08) must serialize/encrypt. Note the "purity" comment: keygen
  imports no filesystem code — keep persistence in the store/CLI layer, not the crypto core.
- `src/cli/mod.rs` — persona tree (`participant`/`coordinator`/`watcher`); Phase 2 adds the
  participant store commands and the coordinator store.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`KeyId` / `Epoch` / `SeatId`** (`src/crypto/types.rs`): the exact tagging tuple STOR-01 needs.
- **`PubkeyEnvelope`** (`src/cli/address.rs`): existing public-artifact serialization; the in-store
  public package should reuse this format (single address-derivation path).
- **`EphemeralNonces`** (`src/crypto/nonce.rs`): the structural nonce-exclusion control is already
  in place — Phase 2 inherits it for free and must preserve it.
- **`zeroize` dependency** already in `Cargo.toml` (with `zeroize_derive`) for the decrypt-use-drop
  wrappers (D-06).

### Established Patterns
- **Crypto core is pure** (no fs/chain/transport imports) — persistence belongs in a new store
  module + CLI layer, never in `src/crypto/`.
- **Public-plaintext / secret-encrypted split** (Phase 1 D-09) is the organizing principle of the
  participant store (D-05).
- **Structural controls over runtime checks** — the project's ethos (non-serializable nonce,
  byte-level-only bridge) directly shapes D-13 (type-level identity separation).

### Integration Points
- New participant store module ← consumed by `keygen`/`sign`/(Phase 4) rotation and (Phase 7)
  transport; exposes `PassphraseSource` (D-03).
- New coordinator SQLite store ← consumed by the coordinator persona; roster feeds Phase 7
  roster-pinning/hash-commit (D-15).
- Checkpoint store ← wired into real ceremonies in Phase 7 (D-08); Phase 2 ships it + a
  persist/reload test.
- `Cargo.toml` gains `age`, `rusqlite` (bundled), and an explicit `secp256k1` dependency (D-14),
  plus likely a home-dir resolver for `~/.cheget/`.

</code_context>

<specifics>
## Specific Ideas

- "The production surface stays interactive-only" — the `PassphraseSource` abstraction (D-03) is
  specifically so that testability never becomes a shipped attack surface.
- The identity key's separation from FROST material should be **impossible to violate**, not
  merely checked (D-13) — treat it as a fourth structural control in the spirit of Phase 1's four.
- The store should be **self-contained**: address + status work from `~/.cheget/` with no external
  file and no unlock (D-05).

</specifics>

<deferred>
## Deferred Ideas

- **Coordinator SQLite schema depth** — full table definitions, migration strategy, transcript &
  session-log record shapes, and DB location were flagged as gray areas but left to planning
  (see Claude's Discretion). Not deferred to a later *phase* — they are in Phase 2 scope (STOR-03),
  just not deep-dived in this discussion.
- **Old-epoch pruning policy** — active pruning is Phase 4 (rotation); Phase 2 only supports
  holding multiple epochs.
- **Unlock agent / passphrase caching** — explicitly rejected for Phase 2 (D-01). Revisit only if
  long-ceremony re-prompt friction proves painful in practice; would be a deliberate future
  security-tradeoff decision, not a Phase 2 addition.

### Reviewed Todos (not folded)
None — no pending todos matched this phase.

</deferred>

---

*Phase: 2-persistence-storage*
*Context gathered: 2026-07-14*
