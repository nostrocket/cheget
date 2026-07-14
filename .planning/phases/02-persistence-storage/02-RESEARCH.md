# Phase 2: Persistence & Storage - Research

**Researched:** 2026-07-14
**Domain:** At-rest secret storage (age/scrypt), encrypted DKG checkpointing, coordinator SQLite (rusqlite), transport-only secp256k1 identity keys + npub derivation, atomic-write discipline
**Confidence:** HIGH (crate versions grounded in `Cargo.lock` + docs.rs; APIs cited from official docs; one MEDIUM open question on rusqlite MSRV)

## Summary

Phase 2 lays the durable-state foundation for `cheget`: a participant store under `~/.cheget/` (identity keypair + per-`(key_id, epoch, seat)` `KeyPackage`/`PublicKeyPackage`, age/scrypt-encrypted, zeroized after use), an encrypted between-round DKG checkpoint store, and a plaintext coordinator SQLite database (roster / transcripts / session logs / policy / churn). Every design lever the user left to "Claude's Discretion" resolves cleanly against the pinned crate stack — no exotic dependencies are required, and two of the four "new" crates (`secp256k1`, `bech32`) are **already in the dependency graph** via rust-bitcoin.

The single organizing principle inherited from Phase 1 is **structural controls over runtime checks**: the non-serializable nonce type (`EphemeralNonces`) already satisfies STOR-02's nonce-exclusion half at the type level, and Phase 2 must not open any persistence path that touches it. The checkpoint store is deliberately typed to accept only `dkg::round{1,2}::SecretPackage` — the "two state machines" separation from PITFALLS Pitfall 1 — so the tidy-but-catastrophic "make signing resumable like ceremonies" instinct is a non-expressible operation. Likewise the transport identity key (D-13) is its own newtype from an independent `OsRng` draw with no conversion function to/from FROST material.

**Primary recommendation:** Build a new `src/store/` module (participant store + checkpoint store + age helpers + atomic-write helper) and a `src/coordinator/` SQLite store, both outside the pure `src/crypto/` core. Use `age`'s one-shot `age::encrypt`/`age::decrypt` with `age::scrypt::Recipient`/`Identity` at `log_n = 18`; wrap all decrypted secret bytes in `age::secrecy`/`zeroize::Zeroizing`; use `rusqlite` with `bundled` + WAL + `user_version` migration gate; derive npubs with the already-present `bech32 0.11.1` (Bech32, **not** Bech32m, HRP `npub`); resolve `~/.cheget/` via a `CHEGET_HOME` override falling back to the `home` crate.

## Architectural Responsibility Map

`cheget` is a single-binary CLI, so "tiers" map to internal layers, not network boundaries. The load-bearing rule is that **persistence never enters the pure crypto core** (`src/crypto/` imports no fs/chain/transport — confirmed in `keygen.rs` header comment).

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Encrypt/decrypt secret bytes (age/scrypt) | Store layer (`src/store/crypto.rs`) | — | age is I/O-adjacent; crypto core must stay pure |
| Participant share files + manifest | Store layer (`src/store/participant.rs`) | CLI (prompts passphrase) | D-05/D-06/D-07; consumed by keygen/sign/rotation |
| DKG round-secret checkpoint | Store layer (`src/store/checkpoint.rs`) | — | D-08/D-10/D-11; typed to `dkg::SecretPackage` only |
| Transport identity keypair + npub | Store layer (`src/store/identity.rs`) | — | D-12/D-13/D-14; independent newtype, no FROST bridge |
| Atomic write + Unix perms | Store layer (`src/store/atomic.rs`) | — | D-07; shared by all encrypted-file writers |
| Coordinator roster/transcript/policy/churn | Coordinator DB layer (`src/coordinator/`) | CLI (coordinator persona) | STOR-03/D-15; **public data only**, plaintext SQLite (D-11) |
| Passphrase acquisition | CLI layer (interactive) / test seam | Store layer (`PassphraseSource` trait) | D-01/D-03; production interactive-only, tests inject in-code |
| Tagging `(key_id, epoch, seat)` | Crypto core (`src/crypto/types.rs`, existing) | Store layer (reuses it) | Types already exist — reuse, do not reinvent |
| Address / share-status from store | CLI (`address`, `share status`) | Store layer (reads plaintext public pkg) | D-05: self-contained, no unlock, reuses `PubkeyEnvelope` |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `age` | **0.11.3** | At-rest encryption of shares, identity key, checkpoints via scrypt passphrase recipient | Project-pinned (PROJECT.md); de-facto standard age impl; `age::scrypt::Recipient`/`Identity` are exactly the passphrase path needed [CITED: docs.rs/age/0.11.3] |
| `rusqlite` | **0.40.1** | Coordinator SQLite store (roster/transcripts/logs/policy/churn) | Project-pinned; `bundled` statically links SQLite 3.53.2 for reproducible builds [CITED: github.com/rusqlite/rusqlite] |
| `secp256k1` | **0.29.1** | Transport-only identity keypair (independent of FROST `k256`) | **Already in graph via rust-bitcoin** [VERIFIED: Cargo.lock]; D-14 wants the C-lib family nostr-sdk uses in Phase 7 |
| `zeroize` | **1.9.0** | `Zeroizing<_>` wrappers for decrypted secret bytes (D-06) | **Already in `Cargo.toml`** [VERIFIED: Cargo.lock]; decrypt-use-drop hygiene |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `bech32` | **0.11.1** | Encode npub (NIP-19) from x-only identity pubkey | **Already in graph via rust-bitcoin** [VERIFIED: Cargo.lock]; D-15 real npubs now |
| `home` | **0.5.x** | Resolve `~` for `~/.cheget/` without a deprecation warning | New dep; see "std::env::home_dir" note below |
| `secrecy` | (re-exported by age as `age::secrecy`) | `SecretString` for `scrypt::Recipient::new` / `Identity::new` | Do **not** add explicitly — use `age::secrecy::SecretString` [CITED: docs.rs/age/0.11.3] |
| `serde` / `serde_json` | 1.x | `manifest.json` (de)serialization | **Already present**; reuse for manifest + `PubkeyEnvelope` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `home` crate | `#[allow(deprecated)] std::env::home_dir()` | Works on 1.85 but is deprecated until 1.87 (un-deprecated in 1.87); would need an `allow` attribute and emits a warning under `-D warnings`. `home` is tiny and rust-lang-maintained (used by cargo/rustup). [CITED: rust-lang/rust#132650] |
| `home` crate | `dirs` / `directories` | Larger dep trees; `home` is minimal and closest to the reproducible-build ethos |
| hand-rolled atomic write | `tempfile` (`NamedTempFile::persist`) / `atomicwrites` | Atomic temp+rename is a well-understood ~40-line primitive; a dep adds surface to audit for the 100-verifier requirement. The commonly-missed bit is the **directory fsync** — document it (see Pitfall 2). Recommend hand-roll. |
| `secp256k1` explicit dep | `bitcoin::secp256k1` re-export | rust-bitcoin re-exports the crate, so no new dep is strictly required. But D-14 explicitly wants the dependency named, and generation needs the `rand` feature — declare it explicitly to control features (see Compatibility note). |
| age scrypt | hand-rolled scrypt+AEAD | **Never** — see Don't Hand-Roll |

**Installation (Cargo.toml additions):**
```toml
# At-rest encryption (shares, identity key, DKG checkpoints). BETA label upstream but API-stable across 0.11.x.
age = "0.11.3"
# Coordinator SQLite (public data only). "bundled" = static SQLite for reproducible builds.
rusqlite = { version = "0.40.1", features = ["bundled"] }
# Transport-only identity keypair (D-14). Same 0.29.1 rust-bitcoin resolves → single copy in the graph.
# "rand" enables OsRng keypair generation; "std" for the OS entropy source.
secp256k1 = { version = "0.29.1", features = ["rand", "std"] }
# npub (NIP-19) encoding. Already in the graph via rust-bitcoin; declare so the intent is explicit.
bech32 = "0.11.1"
# ~/.cheget/ resolution without the 1.85 deprecation warning on std::env::home_dir.
home = "0.5"
```

**Version verification performed:**
- `secp256k1 = 0.29.1`, `bech32 = 0.11.1`, `zeroize = 1.9.0`, `bitcoin = 0.32.101` — confirmed present in `Cargo.lock` [VERIFIED: Cargo.lock].
- `age 0.11.3` module `scrypt` — `Recipient`/`Identity` confirmed on docs.rs [CITED: docs.rs/age/0.11.3/age/scrypt].
- `rusqlite 0.40.1` bundles SQLite 3.53.2 via `libsqlite3-sys 0.38.1` [CITED: github.com/rusqlite/rusqlite]. (0.40.0 release date 2026-05-26.)

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `age` | crates.io | ~5 yrs | high | github.com/str4d/rage | OK | Approved (project pin) |
| `rusqlite` | crates.io | ~10 yrs | very high | github.com/rusqlite/rusqlite | OK | Approved (project pin) |
| `secp256k1` | crates.io | ~8 yrs | very high | github.com/rust-bitcoin/rust-secp256k1 | OK | Approved (already in graph) |
| `bech32` | crates.io | ~7 yrs | very high | github.com/rust-bitcoin/rust-bech32 | OK | Approved (already in graph) |
| `home` | crates.io | ~4 yrs | very high | github.com/rust-lang/cargo (home) | OK | Approved (rust-lang-maintained) |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

> Note: the `package-legitimacy check` seam and `npm/pip/cargo` registry probes could not be executed — the sandbox blocked outbound network to crates.io (`curl` failed; docs.rs reachable via WebFetch only). All five packages are, however, either already resolved in `Cargo.lock` (`secp256k1`, `bech32`) or are long-established, high-download crates confirmed via official docs.rs / GitHub pages. `age`, `rusqlite`, `secp256k1`, `zeroize` are the exact PROJECT.md pins. Treat versions as `[CITED]` pending a `cargo update`/`Cargo.lock` diff at plan time; the planner should confirm `cargo build` resolves them before locking.

## Architecture Patterns

### System Architecture Diagram

```
                          cheget CLI (persona tree — src/cli/mod.rs)
                                        │
       ┌────────────────────────────────┼───────────────────────────────┐
       │ participant persona             │ coordinator persona           │ watcher persona
       ▼                                 ▼                               ▼
  PassphraseSource (D-03)          Coordinator DB layer            (reads plaintext
  ├─ prod: interactive no-echo     (src/coordinator/)               public pkg only —
  │        stdin prompt (D-01)      ├─ open(bundled) + WAL           no unlock, D-05)
  └─ test: in-code passphrase       ├─ user_version migrate          │
           (headless CI seam)       └─ roster / transcripts /        │
       │                               session_logs / policy /       │
       ▼                               churn_ledger  (PUBLIC only)    │
  Store layer (src/store/)              │  no secret material (D-11)  │
  ├─ participant.rs ──────────┐         ▼                            │
  │   manifest.json (plaintext)│    ~/.cheget/coordinator/state.db   │
  │   shares/<key_id>/         │                                      │
  │     epoch-<N>/seat-<NNNN>.age                                     │
  │   pubkey/<key_id>/epoch-<N>.json (plaintext PubkeyEnvelope) ◄─────┘
  ├─ identity.rs   identity.age (secp256k1 secret; npub derived)
  ├─ checkpoint.rs ceremonies/<cid>/<seat>/round-<N>.age (dkg SecretPackage ONLY)
  ├─ crypto.rs     age::encrypt / age::decrypt (scrypt log_n=18) ── Zeroizing boundary
  └─ atomic.rs     tmp → fsync(file) → rename → fsync(dir); 0700/0600; manifest last
       │
       ▼
  Pure crypto core (src/crypto/) — UNCHANGED, imports no fs
  ├─ types.rs   KeyId / Epoch / SeatId   (reused as the (key_id, epoch, seat) tag)
  ├─ nonce.rs   EphemeralNonces          (non-serializable — store MUST NOT touch)
  └─ keygen.rs  dkg::round{1,2}::SecretPackage (the checkpoint store's ONLY input type)
```

Data flow for a secret write: caller hands the store a `KeyPackage` + tag → `serialize()` to bytes (frost `serialization` feature) → wrap in `Zeroizing` → `age::encrypt(scrypt_recipient, bytes)` → `atomic::write(path, ciphertext, 0o600)` → update `manifest.json` **last**. Read reverses it, dropping the `Zeroizing` buffer the instant the operation ends (D-06).

### Recommended Project Structure
```
src/
├── store/                 # NEW — participant durable state (fs + age)
│   ├── mod.rs             # StoreRoot resolution (CHEGET_HOME → home::home_dir), StoreError
│   ├── passphrase.rs      # PassphraseSource trait (D-03) + interactive + in-code impls
│   ├── crypto.rs          # age encrypt/decrypt helpers; scrypt work factor; Zeroizing
│   ├── atomic.rs          # atomic write + Unix perms (D-07)
│   ├── manifest.rs        # manifest.json schema + versioning (D-05)
│   ├── participant.rs     # ParticipantStore: shares, identity, public pkg (STOR-01)
│   ├── identity.rs        # IdentityKeypair newtype + npub (D-12/D-13/D-14/D-15)
│   └── checkpoint.rs      # CheckpointStore: dkg SecretPackage only (STOR-02, D-08..D-11)
├── coordinator/           # NEW — coordinator SQLite (public data only)
│   ├── mod.rs             # open + pragmas + migrate; CoordinatorStore
│   └── schema.rs          # CREATE TABLE ... ; user_version=1 (STOR-03, D-15)
├── crypto/                # UNCHANGED (pure core)
└── cli/                   # extend: participant `share status`, coordinator store wiring
```

### Pattern 1: Passphrase-scrypt one-shot encrypt / decrypt
**What:** Encrypt small secret blobs (a serialized `KeyPackage` is well under a KB) to a passphrase, one-shot into memory. No streaming needed.
**When to use:** Every secret write/read in the participant + checkpoint stores.
**Example:**
```rust
// Source: docs.rs/age/0.11.3 (age::scrypt, age::encrypt/decrypt)
use age::secrecy::SecretString;
use zeroize::Zeroizing;

/// D-09: work factor is a deliberate choice, not the library default.
/// N = 2^log_n. 18 (~256 MiB-equivalent CPU cost, age's own default) is a sound
/// floor for interactive unlock; document the tradeoff. Never 0 or >=64 (panics).
const SCRYPT_LOG_N: u8 = 18;

fn encrypt_secret(passphrase: &SecretString, plaintext: &[u8]) -> Result<Vec<u8>, age::EncryptError> {
    let mut recipient = age::scrypt::Recipient::new(passphrase.clone());
    recipient.set_work_factor(SCRYPT_LOG_N);          // set BEFORE wrapping the file key
    age::encrypt(&recipient, plaintext)               // one-shot Vec<u8> ciphertext
}

fn decrypt_secret(passphrase: &SecretString, ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, age::DecryptError> {
    let identity = age::scrypt::Identity::new(passphrase.clone());
    // Optional: identity.set_max_work_factor(20) to bound attacker-supplied cost.
    let plaintext = age::decrypt(&identity, ciphertext)?;
    Ok(Zeroizing::new(plaintext))                     // D-06: caller drops → zeroized
}
```
Notes: `age::scrypt::Recipient::new` / `Identity::new` take `age::secrecy::SecretString` [CITED: docs.rs/age/0.11.3/age/scrypt/struct.Recipient, struct.Identity]. `set_work_factor(log_n: u8)` where `N = 2^log_n`; panics if `log_n == 0 || log_n >= 64`. A scrypt (passphrase) recipient must be the **sole** recipient — age enforces this; do not mix with x25519 recipients. Prefer the top-level `age::encrypt`/`age::decrypt` helpers over `Encryptor::wrap_output`/`Decryptor` streaming — the payloads are tiny and one-shot keeps the plaintext in a single `Zeroizing` buffer with the shortest lifetime.

### Pattern 2: Typed checkpoint store — the "two state machines" separation (STOR-02)
**What:** The checkpoint store's public API accepts **only** `dkg::round1::SecretPackage` and `dkg::round2::SecretPackage` — never a signing nonce. This is the structural encoding of PITFALLS Pitfall 1 ("do not share a resumable-round-state trait between ceremony and signing").
**When to use:** D-08 capability build + persist/reload test.
**Example:**
```rust
// Source: frost-core CHANGELOG #833 — dkg SecretPackages ARE serializable (they must
// checkpoint between rounds); serialize()/deserialize() from the default `serialization`
// feature. This is the DELIBERATE opposite of EphemeralNonces (non-serializable).
use frost_secp256k1_tr::keys::dkg;

pub struct CheckpointStore { /* root, PassphraseSource */ }

impl CheckpointStore {
    // Note the concrete types — there is no generic `persist<T: Serialize>` that a
    // future contributor could accidentally hand a nonce to.
    pub fn put_round1(&self, cid: &CeremonyId, seat: SeatId, pkg: &dkg::round1::SecretPackage) -> Result<(), StoreError> {
        let bytes = Zeroizing::new(pkg.serialize().map_err(StoreError::Frost)?);
        self.write_encrypted(&self.path(cid, seat, 1), &bytes)   // age + atomic
    }
    pub fn load_round1(&self, cid: &CeremonyId, seat: SeatId) -> Result<dkg::round1::SecretPackage, StoreError> { /* decrypt → deserialize */ }
    pub fn wipe(&self, cid: &CeremonyId) -> Result<(), StoreError> { /* D-10: on success */ }
    // (round2 mirrors round1)
}
```
Notes: frost 3.0's `serialization` feature is on by default and gives `.serialize() -> Result<Vec<u8>, Error>` / `.deserialize(&[u8])` on the dkg secret packages [CITED: frost-core CHANGELOG #833; ASSUMED the `-tr` crate re-exports these methods on the dkg types — verify with `cargo doc` at plan time]. Do **not** derive `Serialize` on any wrapper that could also hold nonce material.

### Pattern 3: Atomic write + restrictive perms (D-07)
**What:** temp file in the **same directory** → write → `fsync(file)` → atomic `rename` → `fsync(dir)`; dir `0700`, file `0600`; manifest updated last.
**Example:**
```rust
// Source: POSIX rename(2) atomicity + fsync durability; std::os::unix::fs for perms.
#[cfg(unix)]
fn write_atomic(final_path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let dir = final_path.parent().expect("share path has a parent");
    let tmp = dir.join(format!(".{}.tmp", unique_suffix()));
    {
        let mut f = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;                       // fsync the file contents
    }
    fs::rename(&tmp, final_path)?;           // atomic replace within the same fs
    File::open(dir)?.sync_all()?;            // fsync the DIRECTORY so the rename is durable
    Ok(())
}
```
Notes: the directory fsync is the commonly-omitted step that makes the rename survive a crash. On non-Unix targets, perms are best-effort — gate perm code with `#[cfg(unix)]` and document Windows as best-effort (see Open Questions). Create the store dir with `DirBuilder::new().mode(0o700)`.

### Anti-Patterns to Avoid
- **A generic `persist<T: Serialize>(...)` in the store.** It would accept a nonce type the day someone adds `Serialize` to it. Type the checkpoint API to the two concrete `dkg::SecretPackage`s (Pattern 2).
- **Deriving `Serialize`/`Deserialize` on any type transitively holding `SigningNonces`.** PITFALLS Pitfall 1 warning sign #1.
- **Any `fn` converting a `KeyPackage`/`SigningShare`/`VerifyingKey` into the identity `SecretKey`/npub, or vice-versa** (D-13, Pitfall 6). Keep the identity key a distinct newtype with no such conversion.
- **age-encrypting the coordinator SQLite DB.** It holds only public roster/transcript data (D-11) — encryption would be false assurance and break `watch`/inspection.
- **Overwriting a share file in place.** Always temp+rename (a crash mid-write must never truncate a live share — Phase 4 rotation depends on this).
- **Updating `manifest.json` before the share file is durably renamed.** Manifest must point only at complete files (D-07).
- **Treating checkpoint/share deletion as a security control.** It is hygiene only (SPEC §11.1, Pitfall 9); the sweep is the revocation.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Passphrase KDF + at-rest AEAD | Custom scrypt + AES-GCM/chacha | `age::scrypt` + `age::encrypt` | Nonce management, KDF params, format versioning, and streaming are all footguns; age is audited and format-stable [CITED: docs.rs/age/0.11.3] |
| npub encoding | Custom base32/checksum | `bech32 0.11.1` (`Bech32`, HRP `npub`) | Checksum + 8→5-bit conversion are error-prone; wrong variant (Bech32m) yields an invalid npub (Pitfall 4) |
| Embedded SQL engine | Custom file format / KV store | `rusqlite` `bundled` | ACID, migrations, querying for roster/churn; static-linked = reproducible |
| Secret-byte memory hygiene | Manual `memset` | `zeroize::Zeroizing` / `age::secrecy::SecretString` | Compiler can elide plain memset; `zeroize` uses volatile writes |
| DKG round-secret serialization | Custom byte layout | frost `.serialize()`/`.deserialize()` (default feature) | Versioned, ciphersuite-tagged; cross-version-safe (Pitfall 15) |
| Home-dir resolution | Parse `$HOME` by hand | `home` crate (+ `CHEGET_HOME` override) | Windows edge cases; `std::env::home_dir` deprecated until 1.87 |

**Key insight:** In a security-reviewable OSS project where 100 people must verify what they run, every hand-rolled crypto/format primitive is extra audit surface with a catastrophic tail. The pinned stack already covers every persistence concern in this phase; the only bespoke code should be the ~40-line atomic-write helper (a well-understood OS primitive) and the store's own module glue.

## Runtime State Inventory

> Phase 2 is greenfield persistence (it *creates* the store), not a rename/refactor. This inventory therefore records what durable state Phase 2 **introduces** and what later phases will mutate — so nothing is retrofitted.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None pre-existing. Phase 2 creates `~/.cheget/` (shares, identity.age, pubkey/*.json, ceremonies/*, coordinator/state.db). No prior on-disk store to migrate. | Create fresh; define schema/manifest versions now so Phase 4/7 extend, not rewrite |
| Live service config | None — no external services in Phase 2 (Nostr relays are Phase 7). | None |
| OS-registered state | None — no daemons/tasks registered by this phase. | None |
| Secrets/env vars | New: `CHEGET_HOME` (store-root override, testability/CI seam — not a passphrase). **No passphrase env var ships** (D-01/D-03). Existing `CHEGET_*` prefix convention (STATE.md). | Document `CHEGET_HOME`; ensure it is read only for root resolution |
| Build artifacts | `Cargo.lock` changes when `age`/`rusqlite`/`secp256k1`/`bech32`/`home` are added; `rusqlite bundled` compiles SQLite (C) → longer first build, needs a C toolchain in CI. | Commit `Cargo.lock`; confirm CI has a C compiler for `libsqlite3-sys` |

## Common Pitfalls

### Pitfall 1: A generic serialize path re-admits the nonce (STOR-02 / PITFALLS #1)
**What goes wrong:** A convenience `persist<T: Serialize>` or a shared "resumable round state" trait spanning ceremony *and* signing lets a `SigningNonces`-bearing type be checkpointed. Reusing a persisted nonce across two sighashes extracts the long-term share — the project's highest-severity bug class.
**Why it happens:** DKG round secrets *are* deliberately checkpointed (they must survive between rounds), and signing nonces live in adjacent code; a tidy uniform abstraction sweeps the nonce in.
**How to avoid:** Type the checkpoint API to the two concrete `dkg::SecretPackage`s only (Pattern 2). `EphemeralNonces` already implements no `Serialize` (verified in `src/crypto/nonce.rs`) — Phase 2 must preserve that and add no serializable wrapper around it. The `tests/ui/nonce_no_serialize.rs` trybuild snapshot already guards the type; add a store-side test that the checkpoint store exposes no nonce-accepting method.
**Warning signs:** `derive(Serialize)` near nonce material; the word "resume"/"checkpoint" in the signing module; a generic persist over `T: Serialize`.

### Pitfall 2: Missing directory fsync makes atomic-rename non-durable (D-07)
**What goes wrong:** `write + rename` without `fsync` on the file *and its parent directory* can leave the rename un-persisted after a crash; Phase 4's verify→persist→delete ordering then operates on a share that silently vanished.
**Why it happens:** Rename atomicity is well-known; directory-entry durability is not.
**How to avoid:** `sync_all()` the file, then open the parent dir and `sync_all()` it after the rename (Pattern 3).
**Warning signs:** atomic-write helper with no `File::open(dir)?.sync_all()`.

### Pitfall 3: Identity key derivable from / into FROST material (D-13 / PITFALLS #6)
**What goes wrong:** Because both live on secp256k1, a helper that turns a share into an npub (or seeds both from one master) collapses the transport/crown-jewel trust separation.
**How to avoid:** `IdentityKeypair` is its own newtype, generated from an independent `secp256k1` `OsRng` draw; provide no `From`/`TryFrom`/conversion to or from any FROST type. Make reuse non-expressible (mirror `EphemeralNonces`).
**Warning signs:** any `impl From<KeyPackage> for IdentityKeypair` or a shared seed.

### Pitfall 4: Wrong bech32 variant for npub (D-15)
**What goes wrong:** NIP-19 uses **bech32** (BIP-173 checksum constant), **not** bech32m. Encoding with `Bech32m` produces a string that other Nostr clients reject.
**How to avoid:** `bech32::encode::<bech32::Bech32>(Hrp::parse("npub")?, &xonly_bytes)` — the 32 raw x-only bytes; the crate does the 8→5-bit conversion internally [CITED: docs.rs/bech32/0.11.1; nips.nostr.com/19].
**Warning signs:** `Bech32m` anywhere near npub; feeding pre-converted 5-bit data.

### Pitfall 5: `secp256k1::SecretKey` is not zeroized on drop
**What goes wrong:** Unlike frost 3.0's `SigningKey` (ZeroizeOnDrop), `secp256k1::SecretKey` does not zeroize its bytes on drop by default. A decrypted identity key can linger in freed memory.
**How to avoid:** Keep the raw 32 secret bytes in a `Zeroizing<[u8;32]>` for storage; construct `SecretKey::from_slice` only at point of use inside a short scope; call `SecretKey::non_secure_erase()` (best-effort) or drop promptly. Document the best-effort nature (matches D-10 secure-delete caveat).
**Warning signs:** a long-lived `SecretKey` field on a struct; secret bytes in a plain `Vec<u8>`.

### Pitfall 6: rusqlite 0.40 MSRV vs the 1.85 pin
**What goes wrong:** rusqlite states its MSRV is "latest stable at release time" [CITED: github.com/rusqlite/rusqlite]; 0.40.1 (May 2026) may not build on 1.85, contradicting the project MSRV.
**How to avoid:** At plan time, run `cargo +1.85.0 build -p cheget --features …` (or check `libsqlite3-sys 0.38.1` MSRV). If it fails, either bump the documented MSRV (weigh against reproducibility) or pin an older rusqlite (e.g. 0.37/0.38) that supports 1.85. See Open Questions.

### Pitfall 7: Passphrase in memory / echoed at prompt (D-01/D-04)
**What goes wrong:** Reading the passphrase into a plain `String`, echoing it, or leaving it resident defeats the interactive-only design.
**How to avoid:** Use a no-echo prompt (e.g. `rpassword`-style read, or termios raw mode) that yields directly into `age::secrecy::SecretString`; confirm-twice-and-match on store creation (D-04); never store it beyond the operation. (Choosing the no-echo crate is a small planner decision — `rpassword` is the standard; verify legitimacy at plan time.)

## Code Examples

### Generate the transport identity keypair and derive its npub (D-12/D-14/D-15)
```rust
// Source: docs.rs/secp256k1/0.29.1 + docs.rs/bech32/0.11.1 + nips.nostr.com/19
use secp256k1::{Secp256k1, SecretKey, Keypair};
use secp256k1::rand::rngs::OsRng;          // requires the `rand` feature
use bech32::{Bech32, Hrp};
use zeroize::Zeroizing;

/// D-13: distinct newtype; NO conversion to/from any FROST type exists.
pub struct IdentityKeypair {
    secret: Zeroizing<[u8; 32]>,           // stored age-encrypted; kept minimal in memory
}

impl IdentityKeypair {
    pub fn generate() -> Self {
        let secp = Secp256k1::new();
        let (sk, _pk) = secp.generate_keypair(&mut OsRng);   // independent OsRng draw
        Self { secret: Zeroizing::new(sk.secret_bytes()) }   // 32-byte secret
    }

    /// npub = bech32 (NOT bech32m), HRP "npub", of the 32-byte x-only pubkey.
    pub fn npub(&self) -> String {
        let secp = Secp256k1::new();
        let sk = SecretKey::from_slice(&*self.secret).expect("32-byte secret");
        let (xonly, _parity) = Keypair::from_secret_key(&secp, &sk).x_only_public_key();
        let hrp = Hrp::parse("npub").expect("valid hrp");
        bech32::encode::<Bech32>(hrp, &xonly.serialize()).expect("bech32 encode")
    }
}
```
[VERIFIED: Cargo.lock — secp256k1 0.29.1, bech32 0.11.1 present] [CITED: docs.rs signatures]. Verify `Keypair::from_secret_key` / `x_only_public_key` exact names with `cargo doc` at plan time (secp256k1 0.29 API — MEDIUM).

### Coordinator SQLite: open, pragmas, migrate
```rust
// Source: docs.rs/rusqlite/0.40 ; rusqlite pragma_update / execute_batch
use rusqlite::Connection;

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;      // crash-safe, single-writer
    conn.pragma_update(None, "synchronous", "NORMAL")?;    // WAL-appropriate durability
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if v < 1 {
        conn.execute_batch(SCHEMA_V1)?;                    // build-full-now
        conn.pragma_update(None, "user_version", 1)?;      // gate future incremental steps
    }
    // future: if v < 2 { conn.execute_batch(MIGRATE_V1_TO_V2)?; ... }
    Ok(())
}
```
[CITED: docs.rs/rusqlite/0.40; github.com/rusqlite/rusqlite]. `pragma_update`/`pragma_query_value`/`busy_timeout`/`execute_batch` are stable rusqlite API — confirm exact names at plan time.

### Recommended coordinator schema (SCHEMA_V1) — public data only (STOR-03, D-11, D-15)
```sql
-- Roster: identifier ↔ npub ↔ status ↔ join/leave epochs (D-15). PUBLIC data.
CREATE TABLE roster (
  key_id        TEXT    NOT NULL,               -- "active" | "standby"
  identifier    TEXT    NOT NULL,               -- hex of frost Identifier.serialize() (stable across refresh)
  seat_index    INTEGER,                        -- 1..=100 convenience (nullable; identifier is authority)
  npub          TEXT    NOT NULL,               -- bech32 npub (real, from D-12 identity keys)
  status        TEXT    NOT NULL,               -- ACTIVE | STANDBY | RETIRED | REMOVED
  join_epoch    INTEGER NOT NULL,
  leave_epoch   INTEGER,                        -- NULL while active
  PRIMARY KEY (key_id, identifier)
);

-- Ceremony transcript: PUBLIC record of each DKG/refresh/enroll (event ids arrive in Phase 7).
CREATE TABLE ceremony_transcripts (
  ceremony_id          TEXT PRIMARY KEY,
  key_id               TEXT    NOT NULL,
  epoch                INTEGER NOT NULL,
  kind                 TEXT    NOT NULL,        -- dkg | refresh | enroll | repair
  group_verifying_key  BLOB,                    -- the pinned P (public); NULL until confirmed
  status               TEXT    NOT NULL,        -- open | complete | aborted
  started_at           INTEGER NOT NULL,        -- unix seconds
  completed_at         INTEGER
);

-- Session logs: PUBLIC record of signing/sweep sessions (no nonces, no partials).
CREATE TABLE session_logs (
  session_id   TEXT PRIMARY KEY,
  key_id       TEXT    NOT NULL,
  epoch        INTEGER NOT NULL,
  kind         TEXT    NOT NULL,               -- sign | sweep
  psbt_txid    TEXT,                           -- or sighash digest; identifies the tx signed
  subset       TEXT,                           -- json array of identifiers that signed
  outcome      TEXT    NOT NULL,               -- success | aborted | timeout
  created_at   INTEGER NOT NULL
);

-- Policy config: single-row (id=1) mirror of SPEC §10 knobs (tables now; engine is Phase 5).
CREATE TABLE policy_config (
  id               INTEGER PRIMARY KEY CHECK (id = 1),
  value_cap        INTEGER,                    -- sats; operator-set (nullable until set)
  churn_budget     INTEGER NOT NULL DEFAULT 50,
  max_epochs       INTEGER NOT NULL DEFAULT 24,
  standby_max_age  INTEGER NOT NULL DEFAULT 7776000  -- 90 days in seconds
);
INSERT INTO policy_config (id) VALUES (1);

-- Churn ledger: distinct former holders since last DKG (feeds Phase 5 `watch`).
CREATE TABLE churn_ledger (
  key_id       TEXT    NOT NULL,
  identifier   TEXT    NOT NULL,
  npub         TEXT,
  left_epoch   INTEGER NOT NULL,
  recorded_at  INTEGER NOT NULL,
  PRIMARY KEY (key_id, identifier, left_epoch)
);
```
Rationale: `identifier` stored as hex of `Identifier.serialize()` is the stable authority (survives refresh — Pitfall 16); `seat_index` is a human convenience. No column holds a share, nonce, or partial (D-11). Policy defaults match SPEC §10. Build-full-now is appropriate for a greenfield table set; the `user_version` gate lets Phase 4/5/7 add columns/tables incrementally.

### manifest.json schema + versioning (D-05)
```jsonc
{
  "schema_version": 1,
  "shares": [
    { "key_id": "active",  "epoch": 0, "seat": "0100…hex", "state": "ACTIVE",  "created_at": 1752000000 },
    { "key_id": "standby", "epoch": 0, "seat": "0100…hex", "state": "STANDBY", "created_at": 1752000100 }
  ]
  // identity keypair + per-(key_id,epoch) public PubkeyEnvelope live at well-known
  // paths, not in the manifest — the manifest indexes only the encrypted shares.
}
```
`state` enum: `ACTIVE | STANDBY | RETIRED` (mirrors ROT-06 / LIFE-03). `seat` is hex of the frost `Identifier`, matching the roster's `identifier`. The public `PubkeyEnvelope` is written plaintext under `pubkey/<key_id>/epoch-<N>.json` reusing the existing `src/cli/address.rs::PubkeyEnvelope` format so `address`/`share status` work from the store with no unlock (D-05, continues Phase 1 D-09).

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `age::Encryptor::with_user_passphrase` | `age::scrypt::Recipient` + `age::encrypt` | age 0.10→0.11 | Explicit scrypt recipient/identity types; cleaner one-shot helpers [CITED: docs.rs/age/0.11.3] |
| dkg SecretPackages in-memory only | dkg SecretPackages serializable | frost-core #833 (2.x) | Enables encrypted between-round checkpointing (STOR-02 DKG half) [CITED] |
| `std::env::home_dir` (deprecated, buggy on Windows) | fixed in 1.85, un-deprecated in 1.87; use `home` crate meanwhile | Rust 1.85 / 1.87 | Use `home` crate to avoid the deprecation warning on the 1.85 pin [CITED: rust-lang/rust#132650] |
| bech32m for all bech32 | NIP-19 keeps original bech32 for npub | — | Wrong variant = invalid npub (Pitfall 4) |

**Deprecated/outdated:**
- `std::env::home_dir` — deprecated through 1.86; prefer the `home` crate on MSRV 1.85.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `dkg::round{1,2}::SecretPackage` in `frost-secp256k1-tr 3.0.0` expose `.serialize()/.deserialize()` (not just serde) | Pattern 2, Code Examples | If serde-only, checkpoint code uses `serde_json`/postcard instead — minor; verify with `cargo doc` |
| A2 | secp256k1 0.29.1 API names: `Keypair::from_secret_key`, `x_only_public_key() -> (XOnlyPublicKey, Parity)`, `SecretKey::secret_bytes()`, `generate_keypair` | Identity code example | Wrong names → compile fix only; verify with `cargo doc` |
| A3 | `bech32::encode::<Bech32>(hrp, &raw_bytes)` performs 8→5-bit conversion internally in 0.11.1 | Pitfall 4, Identity code | If it expects pre-converted `Fe32`, add a `ByteIterExt`/`u5` step — small fix |
| A4 | rusqlite `pragma_update`/`pragma_query_value`/`busy_timeout`/`execute_batch` names are current in 0.40 | SQLite code example | Compile fix only |
| A5 | age scrypt `log_n = 18` is an acceptable interactive default | Pattern 1 | Too low = weaker at-rest; too high = slow unlock. Tunable; document (D-09 discretion) |
| A6 | rusqlite 0.40.1 compiles on MSRV 1.85 | Pitfall 6, Open Q | If not, must pin older rusqlite or bump MSRV — a real planning fork |
| A7 | `secp256k1::SecretKey` (0.29) is not ZeroizeOnDrop; `non_secure_erase()` exists | Pitfall 5 | If it does zeroize, Zeroizing wrapper is belt-and-suspenders — harmless |
| A8 | `age::secrecy::SecretString` is re-exported by age 0.11.3 (secrecy 0.10.3) | Pattern 1 | If not re-exported, add `secrecy = "0.10"` explicitly — trivial |

## Open Questions (RESOLVED at planning)

> All three questions were resolved during planning and implemented as concrete tasks; planning did not proceed on any open assumption.
> - Q1 → **RESOLVED**: 02-01 Task 2 gates on an explicit `cargo +1.85.0 check` with fallback branches (a) keep 0.40.1 / (b) pin older rusqlite / (c) bump MSRV; 02-04 defers schema-locking to it.
> - Q2 → **RESOLVED**: 02-01 Task 1 picks `rpassword` behind the `PassphraseSource` trait, gated by a blocking-human legitimacy checkpoint before install.
> - Q3 → **RESOLVED**: 02-03 Task 1 drives `CheckpointStore` against real `dkg::part1/part2` outputs without faking a between-round pause in `run_inprocess_dkg`.

1. **rusqlite 0.40.1 vs MSRV 1.85** — RESOLVED (see above)
   - What we know: rusqlite MSRV policy is "latest stable at release time"; 0.40.1 is May 2026, project pins Rust 1.85.
   - What's unclear: whether `libsqlite3-sys 0.38.1` + rusqlite 0.40.1 actually build on 1.85.
   - Recommendation: run `cargo +1.85.0 check` with the deps added as the first planning task; if it fails, decide between (a) pinning rusqlite ~0.37/0.38 (still `bundled`, older SQLite) or (b) bumping the documented MSRV. Do not lock the schema until this is resolved.

2. **No-echo passphrase prompt crate (D-01)**
   - What we know: production must read a passphrase with no echo into `SecretString`.
   - What's unclear: which crate (`rpassword` is the de-facto standard; `dialoguer` is heavier).
   - Recommendation: planner picks `rpassword` (verify legitimacy at plan time) behind the `PassphraseSource` trait so tests never touch it.

3. **Checkpoint `(ceremony_id, round, seat)` in-process today (D-08)**
   - What we know: `run_inprocess_dkg` runs all seats in one call with no between-round pause; D-08 says build the capability + a standalone persist/reload test, do **not** wire it into the hot path.
   - Recommendation: the persist/reload test drives `CheckpointStore` directly against real `dkg::part1/part2` outputs (call part1, checkpoint the SecretPackage, reload, feed part2) — proving round-trip fidelity without faking a pause in `run_inprocess_dkg`.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | build | ✓ | 1.96.0 installed / MSRV 1.85 (STATE.md) | — |
| C compiler (cc) | `rusqlite bundled` → `libsqlite3-sys` compiles SQLite | ✓ (Xcode CLT on darwin) | — | System SQLite via non-bundled (worse for reproducibility) |
| crates.io network | fetch `age`/`rusqlite`/`home` | ✗ in sandbox (curl blocked) | — | `cargo build` on the dev host (has network); versions confirmed via docs.rs |
| OS CSPRNG (`OsRng`) | identity keypair + (existing) DKG | ✓ | — | none needed |

**Missing dependencies with no fallback:** none blocking — the sandbox network limit affects only this research session, not the developer's `cargo build`.
**Missing dependencies with fallback:** system C compiler for `bundled` SQLite — present on darwin; ensure CI images include one.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `cargo test` (unit + integration) + `trybuild` (existing, compile-fail) |
| Config file | none — `#[test]` in modules + `tests/` dir |
| Quick run command | `cargo test --lib store::` (or `coordinator::`) |
| Full suite command | `cargo test` (excludes `#[ignore]` n=100) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| STOR-01 | KeyPackage persist→reload roundtrip (encrypt, reload, decrypt, assert equal) via in-code passphrase | unit | `cargo test --lib store::participant::tests::share_roundtrip` | ❌ Wave 0 |
| STOR-01 | Wrong passphrase fails to decrypt (no partial leak) | unit | `cargo test --lib store::crypto::tests::wrong_passphrase_fails` | ❌ Wave 0 |
| STOR-01 | Identity keypair persist→reload; npub is stable + starts `npub1` | unit | `cargo test --lib store::identity::tests::identity_roundtrip_npub` | ❌ Wave 0 |
| STOR-01 | Unix perms: store dir 0700, files 0600 (`#[cfg(unix)]`) | unit | `cargo test --lib store::atomic::tests::perms` | ❌ Wave 0 |
| STOR-01 | `(key_id, epoch, seat)` tagging survives roundtrip; manifest indexes correctly | unit | `cargo test --lib store::manifest::tests::tags` | ❌ Wave 0 |
| STOR-02 | dkg round1/round2 SecretPackage checkpoint persist→reload (D-08) | unit | `cargo test --lib store::checkpoint::tests::dkg_roundtrip` | ❌ Wave 0 |
| STOR-02 | Wipe-on-success removes checkpoint files; keep-on-abort leaves them (D-10) | unit | `cargo test --lib store::checkpoint::tests::wipe_vs_keep` | ❌ Wave 0 |
| STOR-02 | Nonce-exclusion preserved: no store API accepts `EphemeralNonces`/`SigningNonces` | compile-fail / structural | existing `tests/ui/nonce_no_serialize.rs` + review | ✅ (nonce) / ❌ store guard |
| STOR-02 | Atomic write: crash-simulated (leftover tmp) never yields a truncated/corrupt share; manifest points only to complete files | unit | `cargo test --lib store::atomic::tests::atomic_no_partial` | ❌ Wave 0 |
| STOR-03 | Coordinator DB opens, migrates (user_version 0→1), WAL on | unit | `cargo test --lib coordinator::tests::open_migrate` | ❌ Wave 0 |
| STOR-03 | Roster insert/query roundtrip with real npub (D-15) | unit | `cargo test --lib coordinator::tests::roster_roundtrip` | ❌ Wave 0 |
| STOR-03 | Transcript / session_log / policy default / churn insert+query | unit | `cargo test --lib coordinator::tests::tables` | ❌ Wave 0 |
| D-03 | Headless CI path: store built with in-code `PassphraseSource`, full persist/reload with no prompt | integration | `cargo test --test store_headless` | ❌ Wave 0 |
| D-13 | Structural separation: grep/type test that no fn converts FROST↔identity | structural/review | code review + optional `tests/ui/` compile-fail | ❌ Wave 0 |
| (Phase 3) | n=100 persist/reload of full share set through these stores | integration (`#[ignore]`) | `cargo test --release persist_reload_100 -- --ignored` | ❌ (built here, exercised Phase 3) |

### Sampling Rate
- **Per task commit:** `cargo test --lib store::` (or `coordinator::`) — sub-second, no bundled-SQLite rebuild after first.
- **Per wave merge:** `cargo test` (full non-ignored suite) + `cargo clippy -- -D warnings`.
- **Phase gate:** full suite green + a manual `cheget participant share status` from a freshly-created store (no unlock) before `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `src/store/crypto.rs` tests — age roundtrip + wrong-passphrase (covers STOR-01)
- [ ] `src/store/participant.rs` tests — share roundtrip, tagging (STOR-01)
- [ ] `src/store/identity.rs` tests — identity roundtrip + npub format (STOR-01, D-15)
- [ ] `src/store/atomic.rs` tests — perms + atomic-no-partial (STOR-01, D-07)
- [ ] `src/store/manifest.rs` tests — schema/versioning (D-05)
- [ ] `src/store/checkpoint.rs` tests — dkg roundtrip + wipe/keep (STOR-02, D-08/D-10)
- [ ] `src/coordinator/` tests — open/migrate/WAL + table roundtrips (STOR-03)
- [ ] `tests/store_headless.rs` — in-code PassphraseSource CI seam (D-03)
- [ ] Store-side nonce guard (no API accepts nonce material) — complements existing trybuild snapshot
- [ ] `#[ignore]` n=100 persist/reload harness stub (built here; run in Phase 3)

## Security Domain

> `security_enforcement: true`, ASVS L1, block on high. This phase handles secret material at rest — V6/V8 dominate.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | partial | Passphrase confirm-twice on store creation (D-04); no strength policy by decision |
| V3 Session Management | no | No sessions in this phase (signing sessions are Phase 1/later) |
| V4 Access Control | yes | Unix file perms 0700/0600 (D-07); coordinator DB holds no secrets (D-11) |
| V5 Input Validation | yes | Validate manifest `schema_version`, reject unknown; validate decoded frost packages (reuse `PubkeyEnvelope` error handling) |
| V6 Cryptography | yes | `age::scrypt` for at-rest (never hand-roll); `zeroize::Zeroizing` for keys in memory; `OsRng` for identity key |
| V8 Data Protection | yes | Decrypt-use-drop (D-06); zeroize decrypted bytes; best-effort delete with documented SSD/CoW limits (D-10) |
| V14 Configuration | yes | Pinned deps, committed `Cargo.lock`, `bundled` SQLite for reproducibility (SEC-01/02) |

### Known Threat Patterns for {age/secp256k1/SQLite at-rest storage}
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Signing nonce swept into a checkpoint | Information Disclosure / Elevation | Non-serializable nonce type + type-restricted checkpoint API (Pattern 2) |
| Identity key derived from / reused as FROST material | Spoofing / Information Disclosure | Distinct newtype, independent `OsRng`, no conversion fn (D-13) |
| Decrypted share/key lingering in freed memory | Information Disclosure | `Zeroizing`; `SecretKey::non_secure_erase`; short decrypt scope (D-06, Pitfall 5) |
| Crash mid-write corrupts/truncates a live share | Tampering / Denial of Service | Atomic temp+fsync+rename+dir-fsync; manifest last (D-07) |
| Weak scrypt work factor | Information Disclosure | Explicit `log_n` (≥18), documented; `set_max_work_factor` on decrypt (Pattern 1) |
| Secret leaked into coordinator DB | Information Disclosure | Schema holds only public data; no secret columns; DB not encrypted by design (D-11) |
| "Deleted old share = revoked" false assurance | (design) | Every deletion labeled best-effort hygiene; sweep is the revocation (SPEC §11.1, Pitfall 9) |
| Passphrase echoed / resident | Information Disclosure | No-echo prompt into `SecretString`; no env var/flag ships (D-01/D-03) |

## Sources

### Primary (HIGH confidence)
- `Cargo.lock` (in-repo) — confirms `secp256k1 0.29.1`, `bech32 0.11.1`, `zeroize 1.9.0`, `bitcoin 0.32.101` already resolved
- `src/crypto/nonce.rs`, `types.rs`, `keygen.rs`, `cli/address.rs`, `cli/mod.rs`, `Cargo.toml` — existing code to build on
- `SPEC-frost-cli.md` §6.5, §7, §8, §11 — normative storage / nonce / key-separation rules
- `.planning/research/PITFALLS.md` — Pitfalls 1, 6, 9, 16, 18
- `.planning/phases/02-persistence-storage/02-CONTEXT.md` — D-01..D-15 (authoritative decisions)

### Secondary (MEDIUM confidence)
- docs.rs/age/0.11.3 (`age::scrypt::Recipient`/`Identity`, `age::encrypt`/`decrypt`) — API signatures
- docs.rs/secp256k1/0.29.1 — keypair generation / serialization
- docs.rs/bech32/0.11.1 (`fn encode`) + nips.nostr.com/19 — npub = bech32 (not bech32m)
- github.com/rusqlite/rusqlite — `bundled` (SQLite 3.53.2), MSRV policy
- frost-core CHANGELOG (#833) — dkg SecretPackages serializable
- rust-lang/rust#132650, releases.rs/1.85.0 — `home_dir` fix/deprecation timeline

### Tertiary (LOW confidence)
- Exact secp256k1 0.29 / rusqlite 0.40 method names (marked [ASSUMED] A2/A4 — verify with `cargo doc`)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — two crates already in `Cargo.lock`; others are exact PROJECT.md pins confirmed on docs.rs
- Architecture: HIGH — grounded in existing pure-core boundary and D-05..D-13 decisions
- Pitfalls: HIGH — derived from in-repo PITFALLS.md + SPEC normative rules
- API method names: MEDIUM — cited from docs.rs but a few exact identifiers flagged for `cargo doc` confirmation
- rusqlite MSRV: MEDIUM — policy uncertainty vs 1.85 pin (Open Question 1)

**Research date:** 2026-07-14
**Valid until:** 2026-08-14 (stable pinned stack; re-check only if pins bump)
</content>
</invoke>
