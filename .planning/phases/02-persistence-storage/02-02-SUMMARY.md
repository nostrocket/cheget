---
phase: 02-persistence-storage
plan: 02
subsystem: infra
tags: [store, manifest, participant, identity, npub, bech32, age, zeroize, structural-control, STOR-01]

# Dependency graph
requires:
  - phase: 02-01
    provides: "StoreError + StoreRoot, atomic::write_atomic/create_dir_secure (D-07), crypto::encrypt_secret/decrypt_secret (D-06), passphrase::PassphraseSource seam (D-03)"
  - phase: 01-foundation
    provides: "PubkeyEnvelope (address.rs) public-artifact format (D-09), crypto::types tag newtypes, EphemeralNonces structural-control pattern"
provides:
  - "store::manifest — Manifest (schema_version forward-compat + reject unknown/newer) + ShareEntry (key_id, epoch, seat, state, created_at) with add/lookup/remove (D-05)"
  - "store::participant::ParticipantStore — per-(key_id,epoch,seat) age-encrypted KeyPackage persist/reload (byte-equal) + plaintext PubkeyEnvelope public path (no unlock); multi-epoch coexistence"
  - "store::participant::ShareTag — (KeyId, Epoch, SeatId) coordinate reusing crypto::types (D-02)"
  - "store::identity::IdentityKeypair — independent secp256k1 OsRng transport key in Zeroizing<[u8;32]> + npub() (Bech32) + persist/reload; NO FROST<->identity conversion (D-13)"
  - "StoreError::Identity variant for identity reload validation"
affects: [02-04, "coordinator-sqlite-roster (D-15 real npubs)", "phase-04-rotation (multi-epoch shares)"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Versioned serde struct with validate-and-reject-unknown schema_version (Manifest mirrors PubkeyEnvelope forward-compat idiom)"
    - "Secret write order: public envelope + encrypted share atomically first, manifest LAST (D-07)"
    - "Decrypt-use-drop: decrypted bytes in Zeroizing scoped to the operation (D-06)"
    - "Structural control by non-expressibility: no From/TryFrom between IdentityKeypair and any FROST type, proven by a trybuild compile-fail .stderr snapshot (mirrors nonce.rs)"

key-files:
  created:
    - "src/store/manifest.rs"
    - "src/store/participant.rs"
    - "src/store/identity.rs"
    - "tests/ui/identity_no_frost_conversion.rs"
    - "tests/ui/identity_no_frost_conversion.stderr"
  modified:
    - "src/store/mod.rs"
    - "tests/compile_fail.rs"

# Decisions
decisions:
  - "npub() is infallible (returns String, uses expect on true invariants); reload validation (32-byte + valid scalar) is pushed into IdentityKeypair::load so npub never panics on external input"
  - "Seat filename + manifest seat field both use lowercase hex of Identifier.serialize() (matches the coordinator roster identifier authority, Pitfall 16) rather than a numeric seat index"
  - "put_share writes the plaintext public PubkeyEnvelope as well as the encrypted share, so a single call leaves address/status working with no unlock"
  - "Bech32-vs-Bech32m guarded at runtime in the unit test via CheckedHrpstring (npub must validate Bech32 and FAIL Bech32m)"

# Metrics
metrics:
  duration: "~30m"
  tasks_completed: 3
  files_created: 5
  files_modified: 2
  completed: 2026-07-14

status: complete
---

# Phase 2 Plan 2: Participant Secret Store + Transport Identity Summary

Delivered the participant's durable secret store (STOR-01): a plaintext manifest indexing per-`(key_id, epoch, seat)` age-encrypted `KeyPackage` shares, an unlock-free plaintext `PubkeyEnvelope` public path reusing the Phase 1 address format, and a transport-only `IdentityKeypair` newtype that is structurally incapable of being derived from or converted to any FROST key material.

## What was built

**Task 1 — `store::manifest` (commit 04dc888).** `Manifest { schema_version, shares }` following the `PubkeyEnvelope` versioned-serde idiom: `from_json_bytes` rejects a `0`/unknown/newer `schema_version` with `StoreError::Schema` rather than silently misparsing (RESEARCH V5). `ShareEntry` carries `(key_id, epoch, seat, state, created_at)`; `seat` is lowercase hex of the frost `Identifier`, and `ShareState` serializes as `ACTIVE|STANDBY|RETIRED` (ROT-06/LIFE-03). `add_entry` replaces on matching tag (no duplicates), `lookup`/`remove` operate on the `crypto::types` tag tuple. Per D-05 the manifest indexes only the encrypted shares — identity + public envelope live at well-known paths.

**Task 2 — `store::participant::ParticipantStore` (commit e51bc47).** `put_share` serializes the `KeyPackage`, wraps it in `Zeroizing`, `encrypt_secret`s it, and `write_atomic`s to `shares/<key_id>/epoch-<N>/seat-<hex>.age` (0600), writing the plaintext public `PubkeyEnvelope` to `pubkey/<key_id>/epoch-<N>.json` first and updating `manifest.json` **last** (D-07). `load_share` reverses it with the decrypted bytes held in a `Zeroizing` buffer dropped at function end (D-06). `load_public_envelope` never touches the passphrase source — the public path works on a locked store. `ShareTag` reuses `crypto::types::{KeyId, Epoch, SeatId}`. Multiple epochs coexist (no deletion of old epochs — pruning is Phase 4).

**Task 3 — `store::identity::IdentityKeypair` (commit 3c1d954).** A `Zeroizing<[u8;32]>` secret generated from an independent `secp256k1::rand::OsRng` draw on the C-lib secp256k1 curve (a different RNG and a different curve crate from FROST's k256). `npub()` derives the 32-byte x-only key and encodes it with the **Bech32** checksum (not Bech32m) under HRP `npub`, building a live `SecretKey` only inside a short scope (Pitfall 5). `persist`/`load` round-trip under the single store passphrase (D-02/D-12), with reload validating length and scalar validity via the new `StoreError::Identity`. Crucially, no `From`/`TryFrom`/conversion to or from any FROST type exists — reuse of FROST material as the identity key is non-expressible (D-13, T-02-05), proven by `tests/ui/identity_no_frost_conversion.rs` whose `.stderr` snapshot pins the missing-`From` reason.

## Verification

- `cargo test --lib store::` — 12/12 green (incl. `manifest::tags`, `manifest::rejects_unknown_schema_version`, `participant::share_roundtrip`, `identity::identity_roundtrip_npub`).
- `cargo test --test compile_fail` — both trybuild guards green (nonce non-serializable + identity no-FROST-conversion).
- `cargo clippy --lib -- -D warnings` — clean; `IdentityKeypair` unit test confirms the public envelope reads under a WRONG passphrase (no unlock) while the share does not decrypt, and that the npub validates under Bech32 but fails Bech32m.

## Threat mitigations applied

| Threat ID | Mitigation | Where |
|-----------|-----------|-------|
| T-02-05 | Distinct newtype, independent OsRng, no FROST↔identity conversion + trybuild guard | `identity.rs`, `tests/ui/identity_no_frost_conversion.rs` |
| T-02-06 | `encrypt_secret` before write; decrypt returns `Zeroizing` dropped at op end | `participant.rs` put_share/load_share |
| T-02-07 | Atomic write then manifest updated LAST | `participant.rs` put_share |
| T-02-08 | `bech32::encode::<Bech32>` (not Bech32m), HRP npub; runtime CheckedHrpstring guard | `identity.rs` npub + test |
| T-02-09 | Raw secret in `Zeroizing<[u8;32]>`; `SecretKey` built in short scope only | `identity.rs` |

## Deviations from Plan

### Auto-added functionality

**1. [Rule 2 - Missing error surface] Added `StoreError::Identity` variant**
- **Found during:** Task 3
- **Issue:** `IdentityKeypair::load` must validate that decrypted material is 32 bytes and a well-formed secp256k1 scalar, but no honest error face existed (Manifest/Schema are semantically wrong).
- **Fix:** Added `StoreError::Identity(String)` with a Display arm. The `mod.rs` doc already anticipates 02-02 identity populating store error variants.
- **Files modified:** `src/store/mod.rs`
- **Commit:** 3c1d954

### Design choices (within plan latitude)

- `npub()` returns `String` (infallible) with the fallible length/scalar validation pushed into `load()`; this keeps the D-15 roster-building call site clean and guarantees npub never panics on decrypted input.
- Share filenames use `seat-<hex>.age` (hex of `Identifier.serialize()`) rather than the plan's placeholder `seat-<NNNN>`, matching the manifest `seat` field and the coordinator roster identifier authority (Pitfall 16).

## Known Stubs

None. All three modules are fully wired with green tests; no placeholder/empty-value data paths were introduced.

## Deferred Issues

- Pre-existing clippy lint `needless_borrows_for_generic_args` in `tests/dkg_100_correctness.rs:55` (Phase 1 code) still fails `cargo clippy --all-targets -- -D warnings`. Out of scope (unrelated file, already logged in `deferred-items.md` by 02-01). Store lib code is clean under `-D warnings`.
- The scrypt work factor (log_n=18, a Wave 1 D-09 decision) makes the encrypt/decrypt unit tests slow in debug builds (`share_roundtrip` ≈ 110s, `identity_roundtrip_npub` ≈ 54s). Correct behavior, not a defect; noted for CI timeout budgeting.

## Self-Check: PASSED

- Files exist: `src/store/manifest.rs`, `src/store/participant.rs`, `src/store/identity.rs`, `tests/ui/identity_no_frost_conversion.rs`, `tests/ui/identity_no_frost_conversion.stderr` — all present.
- Commits exist: 04dc888, e51bc47, 3c1d954 — all in `git log`.
