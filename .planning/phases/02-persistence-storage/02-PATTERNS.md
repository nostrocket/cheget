# Phase 2: persistence-storage - Pattern Map

**Mapped:** 2026-07-14
**Files analyzed:** 13 (10 new, 3 modified)
**Analogs found:** 10 / 13 (3 have no in-repo analog — greenfield fs/SQL; use RESEARCH.md)

The codebase has **no filesystem, no DB, and no encryption code today** — `src/crypto/`
and `src/bridge/` are pure by design, and the only disk touch is `std::fs::write` of a
plaintext JSON envelope in `cli/keygen.rs`. So the reusable analogs are *idiom* analogs
(error enums, trait seams, Zeroizing newtypes, serde envelopes, module layout), not
same-role file analogs. The three genuinely new capabilities (atomic fs write, SQLite)
have no analog and must follow RESEARCH.md Pattern 3 / Code Examples verbatim.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/store/mod.rs` | module-root / config | request-response | `src/crypto/mod.rs` (+ `ChainError` idiom) | idiom-match |
| `src/store/passphrase.rs` | trait / provider | request-response | `src/transport/mod.rs` `Transport`, `src/chain/mod.rs` `ChainBackend` | idiom-match (exact seam shape) |
| `src/store/crypto.rs` | utility / service | transform (bytes→ciphertext) | `src/crypto/nonce.rs` (Zeroizing) + `KeygenError` idiom | partial (idiom only) |
| `src/store/atomic.rs` | utility | file-I/O | **none** | no analog |
| `src/store/manifest.rs` | model / serde | file-I/O / CRUD | `src/cli/address.rs` `PubkeyEnvelope` | role-match |
| `src/store/participant.rs` | service / store | CRUD (file-per-share) | `src/cli/address.rs` `address_from_pubkey_file` + `cli/keygen.rs::run` write | role-match |
| `src/store/identity.rs` | model / newtype | transform | `src/crypto/nonce.rs` `EphemeralNonces` + `types.rs` newtypes | exact (structural-control) |
| `src/store/checkpoint.rs` | service / store | CRUD / file-I/O | `src/crypto/nonce.rs` (type-restriction) + `crypto/keygen.rs` dkg types | exact (structural-control) |
| `src/coordinator/mod.rs` | service / store | CRUD | **none** (no DB code) | no analog |
| `src/coordinator/schema.rs` | config / migration | CRUD | **none** (no SQL) | no analog |
| `src/lib.rs` (MOD) | module-root | — | existing `pub mod` block in `src/lib.rs` | exact |
| `src/cli/mod.rs` (MOD) | route | request-response | existing persona tree in `src/cli/mod.rs` | exact |
| `Cargo.toml` (MOD) | config | — | existing `[dependencies]` block | exact |

## Pattern Assignments

### `src/store/mod.rs` (module-root / config, request-response)

**Analog:** `src/crypto/mod.rs` (module doc + `pub mod` + `pub use` re-export) and the
`ChainError` enum in `src/chain/mod.rs`.

**Module-root layout pattern** (`src/crypto/mod.rs` lines 1-31): a load-bearing doc
comment stating the layer + purity/boundary rule, then `pub mod` declarations, then a
flat `pub use` re-export surface. Copy this shape: doc the store layer boundary
("persistence never enters the pure crypto core"), declare the submodules, re-export
`ParticipantStore`, `CheckpointStore`, `IdentityKeypair`, `PassphraseSource`, `StoreError`.

**StoreError enum pattern** — this is the single most-repeated idiom in the repo; copy it
exactly. `src/chain/mod.rs` lines 30-58:
```rust
#[derive(Debug)]
pub enum ChainError {
    Rpc(String),
    Descriptor(String),
    Unsupported(&'static str),
}
impl std::fmt::Display for ChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainError::Rpc(m) => write!(f, "bitcoin core rpc error: {m}"),
            /* ... */
        }
    }
}
impl std::error::Error for ChainError {}
```
`StoreError` should wrap: `Io(std::io::Error)`, `Age(String)` (or `age::EncryptError` /
`age::DecryptError`), `Frost(frost::Error)`, `Json(serde_json::Error)`,
`Manifest(...)`, `Sqlite(rusqlite::Error)` — following the exact manual
`Debug`/`Display`/`std::error::Error` triple. Note `KeygenError`
(`crypto/keygen.rs:28-62`) and `EnvelopeError` (`cli/address.rs:73-102`) use the
identical idiom with a nested `frost::Error` variant — reuse that `Frost(frost::Error)`
+ `Serialize(frost::Error)` split for the serialize/deserialize error faces.

**StoreRoot resolution:** no analog (first env-var/home read in the repo). Per RESEARCH
§Recommended Project Structure + Runtime State Inventory: `CHEGET_HOME` override →
`home::home_dir()` fallback → `~/.cheget/`. Follow the existing `CHEGET_*` prefix
convention noted in RESEARCH.

---

### `src/store/passphrase.rs` (trait / provider, request-response)

**Analog:** `src/transport/mod.rs::Transport` (lines 114-127) and
`src/chain/mod.rs::ChainBackend` (lines 78-100) — the project's established
**trait-seam + swappable-impls** pattern, which is exactly D-03's "abstraction with a
production impl and a test impl."

**Trait seam pattern** (`transport/mod.rs` lines 114-127): a small doc comment explaining
*why* it is a seam and what must NOT leak through it, then a minimal trait. Mirror this:
```rust
// analog: transport/mod.rs:120-127
pub trait Transport {
    fn publish(&self, envelope: Envelope) -> EnvelopeId;
    fn subscribe(&self, filter: &Filter) -> Vec<Envelope>;
}
```
`PassphraseSource` should be equally minimal — one method returning
`age::secrecy::SecretString` (D-03). The doc must state the load-bearing rule: **the
production impl is interactive-only; no env var / CLI flag for the passphrase ships**
(D-01/D-03), mirroring how the `Transport` doc states "no `nostr-sdk` type leaks."

**Prod + test impl pattern:** `chain/mod.rs` provides two impls behind one trait
(`CoreRpcBackend`, `EsploraBackend`, lines 20-25 re-exported). Follow it: an interactive
no-echo prompt impl (prod, gated so tests never link it) and an in-code
`SecretString` impl (headless CI seam). RESEARCH Open Q2 flags `rpassword` as the
no-echo crate to verify at plan time.

---

### `src/store/crypto.rs` (utility / service, transform)

**Analog:** idiom-only — `src/crypto/nonce.rs` for the `Zeroizing` boundary; RESEARCH
Pattern 1 for the age API.

**Zeroizing return pattern** (`crypto/nonce.rs` lines 35, 42, 56): decrypted secret bytes
must come back wrapped so the caller's drop zeroizes them. `nonce.rs` wraps its inner
secret as `Zeroizing<SigningNonces>` and constructs it with `Zeroizing::new(nonces)`
(line 56). Apply identically: `decrypt_secret(...) -> Result<Zeroizing<Vec<u8>>, StoreError>`
(RESEARCH Pattern 1, lines 169-174).

**age one-shot pattern:** no analog — follow RESEARCH Pattern 1 verbatim
(`age::scrypt::Recipient::new` + `set_work_factor(SCRYPT_LOG_N=18)` + `age::encrypt`;
`age::scrypt::Identity::new` + `age::decrypt`). Error variants feed `StoreError::Age`.

**Wrong-passphrase test** (RESEARCH Test Map STOR-01): `decrypt` with a mismatched
passphrase must return `Err`, not partial plaintext.

---

### `src/store/atomic.rs` (utility, file-I/O)

**Analog: NONE.** The repo has no atomic-write, `fsync`, or Unix-perms code — the only
disk write is `std::fs::write(&args.out, &json)` (`cli/keygen.rs:78`), which is
non-atomic and unhardened (fine for a plaintext public artifact, wrong for a share).

**Follow RESEARCH Pattern 3 verbatim** (lines 209-224): temp file in the *same dir* →
`OpenOptions … .mode(0o600).create_new(true)` → `write_all` → `sync_all()` (file) →
`fs::rename` → `File::open(dir)?.sync_all()` (directory fsync — the commonly-omitted
durability step, Pitfall 2). Gate perms with `#[cfg(unix)]`; `DirBuilder::new().mode(0o700)`
for the store dir. Manifest updated **last** (D-07). Tests: `perms` and
`atomic_no_partial` (RESEARCH Test Map STOR-01/D-07).

---

### `src/store/manifest.rs` (model / serde, file-I/O / CRUD)

**Analog:** `src/cli/address.rs::PubkeyEnvelope` (lines 61-124) — the repo's one
serde-struct-with-versioning + `from_x`/`decode` + typed error pattern.

**Versioned serde struct pattern** (`cli/address.rs` lines 61-70):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubkeyEnvelope {
    pub key_id: String,
    #[serde(default)]
    pub epoch: u64,
    pub pubkey_package_hex: String,
}
```
The `#[serde(default)] epoch` field is the existing forward-compat idiom — the manifest's
`schema_version: u32` (RESEARCH manifest.json schema, lines 419-428) plays the same role.
Validate `schema_version` on load and reject unknown (RESEARCH V5 Input Validation).

**Constructor + decoder pattern** (`cli/address.rs` lines 104-124): `from_package(...)`
builds the envelope, `decode_package()` reverses it, both returning the typed
`EnvelopeError`. Mirror with manifest add/lookup/remove entry methods returning
`StoreError`. Manifest indexes only the encrypted shares — identity key + public
`PubkeyEnvelope` live at well-known paths, not in the manifest (RESEARCH lines 426-427).

**`state` enum:** `ACTIVE | STANDBY | RETIRED` (RESEARCH line 430; mirrors ROT-06/LIFE-03).
`seat` stored as hex of the frost `Identifier`, matching the coordinator roster `identifier`.

---

### `src/store/participant.rs` (service / store, CRUD file-per-share)

**Analog:** `src/cli/address.rs::address_from_pubkey_file` (lines 127-136) for the
read→deserialize→decode flow, and `cli/keygen.rs::run` (lines 66-85) for the
serialize→write flow.

**Read-file → deserialize → decode pattern** (`cli/address.rs` lines 127-136):
```rust
let raw = std::fs::read(path).map_err(EnvelopeError::Io)?;
let envelope: PubkeyEnvelope = serde_json::from_slice(&raw).map_err(EnvelopeError::Json)?;
let pkg = envelope.decode_package()?;
```
The share read path mirrors this but inserts decrypt: `atomic/std::fs::read` →
`decrypt_secret` (→ `Zeroizing<Vec<u8>>`) → `KeyPackage::deserialize(&bytes)`.

**Serialize → write pattern** (`cli/keygen.rs` lines 76-78): the public-package half reuses
`PubkeyEnvelope::from_package(...)` + `serde_json::to_vec_pretty` and writes the
**plaintext** public envelope under `pubkey/<key_id>/epoch-<N>.json` (D-05) so
`address`/`share status` work with no unlock. The **secret** half instead does
`key_package.serialize()` → `Zeroizing` → `crypto::encrypt_secret` → `atomic::write(..,0o600)`
→ update manifest last (RESEARCH data-flow, lines 128).

**Tagging tuple:** reuse `crypto::types::{KeyId, Epoch, SeatId}` (`crypto/types.rs`
lines 13, 43, 62) for `(key_id, epoch, seat)` — do NOT reinvent (CONTEXT lines 38-40,
D-02 tag lives inside each encrypted payload). Path layout
`shares/<key_id>/epoch-<N>/seat-<NNNN>.age` (D-05). Multiple epochs held simultaneously.

**Decrypt-use-drop (D-06):** every secret load returns `Zeroizing<...>` scoped to the
operation — same discipline as `EphemeralNonces` consume-by-value (`nonce.rs` lines 66-72).

---

### `src/store/identity.rs` (model / newtype, transform) — STRUCTURAL CONTROL

**Analog:** `src/crypto/nonce.rs::EphemeralNonces` (whole file, esp. lines 42, 51-72) —
the project's canonical **structural-control newtype**. This is an *exact* pattern match:
D-13 wants transport/FROST separation enforced the same way nonce non-reuse is enforced.

**Zeroizing newtype with NO forbidden operations** (`crypto/nonce.rs` line 42):
```rust
pub struct EphemeralNonces(Zeroizing<SigningNonces>);
```
`EphemeralNonces` implements **no** `Serialize`/`Deserialize`/`Clone`, and the reuse
failure mode is a *compile-time impossibility* (module doc lines 11-27). Mirror this for
`IdentityKeypair`:
```rust
// analog shape: nonce.rs:42 ; RESEARCH Code Examples lines 307-327
pub struct IdentityKeypair { secret: Zeroizing<[u8; 32]> }
```
**The load-bearing rule (D-13, Pitfall 3):** provide **no** `From`/`TryFrom`/conversion
to or from *any* FROST type (`KeyPackage`, `SigningShare`, `VerifyingKey`). Reuse of
FROST material as the identity key must be *non-expressible*, exactly as reusing a nonce
is non-expressible. Generate from an **independent** `secp256k1::rand::OsRng` draw
(RESEARCH lines 313-317) — a different RNG *and* a different curve crate (`secp256k1`
C-lib, not frost's `k256`), reinforcing D-14 separation at the dependency level.

**npub derivation:** RESEARCH lines 320-326 — `bech32::encode::<Bech32>` (NOT `Bech32m`,
Pitfall 4), HRP `npub`, over the 32 x-only bytes. `secp256k1::SecretKey` is NOT
ZeroizeOnDrop (Pitfall 5) — keep the raw secret in `Zeroizing<[u8;32]>`, build
`SecretKey::from_slice` only inside a short scope.

**Doc the control like nonce.rs does:** `nonce.rs` lines 24-27 point at the reviewable
compile-fail artifact (`tests/ui/nonce_no_serialize.rs`) *instead of* an assertion. Add
the parallel store-side structural guard (RESEARCH Wave 0 Gaps: "no fn converts
FROST↔identity").

---

### `src/store/checkpoint.rs` (service / store, CRUD) — STRUCTURAL CONTROL

**Analog:** `src/crypto/nonce.rs` (type-restriction discipline) + the concrete dkg types
in `src/crypto/keygen.rs` (lines 112, 129-137, 151 — `dkg::round1::SecretPackage`,
`dkg::round2::SecretPackage`).

**Type-restricted API — the anti-generic control (Pattern 2 / Pitfall 1).** The
checkpoint store must expose **concrete** methods per round, never a
`persist<T: Serialize>`:
```rust
// RESEARCH Pattern 2 lines 193-200 ; input types from crypto/keygen.rs:112/151
pub fn put_round1(&self, cid: &CeremonyId, seat: SeatId, pkg: &dkg::round1::SecretPackage) -> Result<(), StoreError>;
pub fn load_round1(&self, cid: &CeremonyId, seat: SeatId) -> Result<dkg::round1::SecretPackage, StoreError>;
// round2 mirrors; wipe(cid) on success (D-10)
```
This is the deliberate *inverse* of `EphemeralNonces`: dkg `SecretPackage`s **are**
serializable (they must survive between rounds — frost-core #833) and so get concrete
persist methods; `SigningNonces` is non-serializable and gets none. Because there is no
generic persist and `EphemeralNonces` has no `Serialize` (`nonce.rs`), a nonce is
*non-expressible* as a checkpoint input — that is the whole control. Add the store-side
test that no checkpoint method accepts nonce material (RESEARCH Test Map STOR-02 / Wave 0).

**Serialize→encrypt→atomic reuse** (RESEARCH lines 195-196): `pkg.serialize()` →
`Zeroizing` → `crypto::encrypt_secret` (same store passphrase, D-09) →
`atomic::write` under `ceremonies/<cid>/<seat>/round-<N>.age` (D-11).

**Do NOT touch `run_inprocess_dkg`** — D-08 / RESEARCH Open Q3: `crypto/keygen.rs`
(lines 71-210) runs all rounds in one call with no pause; build the capability + a
standalone persist/reload test that calls `dkg::part1`, checkpoints the SecretPackage,
reloads, feeds `part2`. Do not refactor keygen to fake a pause, and keep persistence
out of `src/crypto/` (purity rule, `keygen.rs` lines 14-16).

---

### `src/coordinator/mod.rs` (service / store, CRUD)

**Analog: NONE** — no SQLite/DB code exists in the repo.

**Follow RESEARCH Code Examples verbatim** (lines 331-356): `Connection::open` →
`pragma_update` (WAL / synchronous=NORMAL / foreign_keys=ON) → `busy_timeout(5s)` →
`migrate` via `user_version` gate (0→1, build-full-now). Wrap `rusqlite::Error` into
`StoreError::Sqlite` using the repo's manual error-enum idiom (see `ChainError`,
`chain/mod.rs:30-58`). **Public data only — never age-encrypt the DB** (D-11, RESEARCH
Anti-Patterns line 230). **Resolve MSRV Open Q1 first** (`cargo +1.85.0 check` with
rusqlite 0.40.1 `bundled`) before locking the schema.

**Roster populated with real npubs** (D-15): each simulated seat's `IdentityKeypair::npub()`
fills its roster row (identifier ↔ npub ↔ status ↔ join/leave epochs).

---

### `src/coordinator/schema.rs` (config / migration, CRUD)

**Analog: NONE.** Use RESEARCH SCHEMA_V1 verbatim (lines 358-416): `roster`,
`ceremony_transcripts`, `session_logs`, `policy_config` (single-row id=1),
`churn_ledger`. `identifier` = hex of `Identifier.serialize()` (stable across refresh,
Pitfall 16); `seat_index` is human convenience. No column holds a share/nonce/partial
(D-11). Policy defaults match SPEC §10. `user_version = 1` gate for Phase 4/5/7 growth.

---

## Shared Patterns

### Error enum (manual Debug + Display + std::error::Error)
**Source:** `src/chain/mod.rs` lines 30-58 (also `crypto/keygen.rs:28-62`,
`cli/address.rs:73-102`).
**Apply to:** `StoreError` (store) and the coordinator error surface — every new module.
This is a universal repo idiom: `#[derive(Debug)]` enum, hand-written `impl Display`
match arm per variant, empty `impl std::error::Error {}`. Nest source errors as variants
(`Frost(frost::Error)`, `Io(std::io::Error)`, `Json(serde_json::Error)`). Do NOT pull in
`thiserror` — the repo hand-rolls this everywhere.
```rust
#[derive(Debug)]
pub enum ChainError { Rpc(String), /* ... */ Unsupported(&'static str) }
impl std::fmt::Display for ChainError { /* match self { .. } */ }
impl std::error::Error for ChainError {}
```
CLI handlers return `CliResult = Result<(), Box<dyn std::error::Error>>` (`cli/mod.rs:17`)
so any `impl Error` propagates with `?`.

### Zeroizing decrypt-use-drop
**Source:** `src/crypto/nonce.rs` lines 35, 42, 56 (`Zeroizing<SigningNonces>`,
`Zeroizing::new(nonces)`).
**Apply to:** `store/crypto.rs` (decrypt returns `Zeroizing<Vec<u8>>`),
`store/participant.rs`, `store/checkpoint.rs`, `store/identity.rs` (`Zeroizing<[u8;32]>`).
Decrypted secret bytes live in `Zeroizing` scoped to the single operation (D-06).

### Structural control (non-expressible unsafe operation)
**Source:** `src/crypto/nonce.rs` (whole file; doc lines 11-27, consume-by-value `sign`
lines 66-72).
**Apply to:** `store/identity.rs` (no FROST↔identity conversion, D-13) and
`store/checkpoint.rs` (no generic persist; concrete dkg types only, Pitfall 1). The
control is a *type/API shape that makes the bug non-expressible*, documented by pointing
at a reviewable compile-fail/structural test — never a runtime assertion or comment.

### Tagging newtypes
**Source:** `src/crypto/types.rs` lines 13 (`KeyId`), 43 (`Epoch`), 62 (`SeatId`);
re-exported at `crypto/mod.rs:31`.
**Apply to:** `store/participant.rs`, `store/manifest.rs`, `store/checkpoint.rs`,
`coordinator/*`. Reuse the existing `(key_id, epoch, seat)` tuple — do not reinvent.

### serde envelope + versioning + hex(serialize())
**Source:** `src/cli/address.rs` lines 61-124 (`PubkeyEnvelope`, `from_package`,
`decode_package`, `#[serde(default)]` forward-compat field).
**Apply to:** `store/manifest.rs` (`schema_version` field, validate-and-reject-unknown)
and the participant store's in-store public package — **reuse `PubkeyEnvelope` directly**
(D-05, single address-derivation path) rather than a parallel type.

### Module-root doc + pub mod + pub use
**Source:** `src/crypto/mod.rs` lines 1-31; `src/chain/mod.rs` lines 1-26;
`src/transport/mod.rs` lines 1-16.
**Apply to:** `src/store/mod.rs`, `src/coordinator/mod.rs`, and the `src/lib.rs` module
map. Doc states the layer + boundary rule, then `pub mod`, then a flat `pub use` surface.
Update `src/lib.rs` (lines 21-30 add `pub mod store;` + `pub mod coordinator;` and a
module-map bullet each).

### Trait seam with prod + test impls
**Source:** `src/transport/mod.rs` lines 114-127 (`Transport`), `src/chain/mod.rs` lines
78-100 (`ChainBackend`, two impls re-exported lines 24-25).
**Apply to:** `store/passphrase.rs::PassphraseSource` (interactive prod impl + in-code
test impl, D-03).

### CLI: clap Args + resolve helper + run() + persona subcommand tree
**Source:** `src/cli/keygen.rs` lines 25-86 (`#[derive(Args)]` struct, `resolve_tn`
helper, `pub fn run(args) -> CliResult` validating then delegating to the library);
`src/cli/mod.rs` lines 29-84 (persona `enum` + per-persona subcommand `enum` + `run`
match dispatch).
**Apply to:** `src/cli/mod.rs` (MOD) — add `ParticipantCmd::ShareStatus` (self-contained
from store, no unlock, D-05) and coordinator store subcommands; add matching `run` arms.
Keep "CLI does no work itself — it only routes to the library" (`cli/mod.rs:8`).

### Cargo.toml commented dependency block
**Source:** `Cargo.toml` lines 17-35 — each dep has a one-line rationale comment; features
declared inline (`zeroize = { version = "1.9.0", features = ["zeroize_derive"] }`).
**Apply to:** add `age`, `rusqlite` (`bundled`), `secp256k1` (`rand`,`std`), `bech32`,
`home` following RESEARCH Installation block (lines 59-71) with the same comment style.

## No Analog Found

| File | Role | Data Flow | Reason | Use Instead |
|------|------|-----------|--------|-------------|
| `src/store/atomic.rs` | utility | file-I/O | No fs/fsync/perms code exists (crypto core is pure; only `std::fs::write` of plaintext JSON) | RESEARCH Pattern 3 (lines 209-224) |
| `src/coordinator/mod.rs` | service/store | CRUD | No SQLite/DB code in repo | RESEARCH Code Examples (lines 331-356) |
| `src/coordinator/schema.rs` | config/migration | CRUD | No SQL in repo | RESEARCH SCHEMA_V1 (lines 358-416) |

For the three no-analog files, RESEARCH.md carries verified, citable code and is the
authoritative source. The error-enum, Zeroizing, and module-layout *idioms* above still
apply (e.g. `StoreError::Sqlite(rusqlite::Error)` follows `ChainError`).

## Metadata

**Analog search scope:** `src/crypto/`, `src/bridge/`, `src/chain/`, `src/transport/`,
`src/session/`, `src/cli/`, `Cargo.toml`, `src/lib.rs`.
**Files scanned:** 23 source files (full `src` tree enumerated); 6 read in full
(`crypto/types.rs`, `crypto/nonce.rs`, `crypto/keygen.rs`, `cli/address.rs`,
`cli/keygen.rs`, `cli/mod.rs`) + 4 for idiom confirmation (`chain/mod.rs`,
`transport/mod.rs`, `crypto/mod.rs`, `lib.rs`, `Cargo.toml`).
**Pattern extraction date:** 2026-07-14
