---
phase: 02-persistence-storage
reviewed: 2026-07-14T09:41:09Z
depth: standard
files_reviewed: 19
files_reviewed_list:
  - src/store/mod.rs
  - src/store/atomic.rs
  - src/store/crypto.rs
  - src/store/passphrase.rs
  - src/store/manifest.rs
  - src/store/participant.rs
  - src/store/identity.rs
  - src/store/checkpoint.rs
  - src/coordinator/mod.rs
  - src/coordinator/schema.rs
  - src/cli/mod.rs
  - src/lib.rs
  - Cargo.toml
  - tests/compile_fail.rs
  - tests/store_headless.rs
  - tests/store_checkpoint_n100.rs
  - tests/ui/checkpoint_no_nonce.rs
  - tests/ui/checkpoint_no_generic_persist.rs
  - tests/ui/identity_no_frost_conversion.rs
findings:
  critical: 2
  warning: 3
  info: 4
  total: 9
status: issues_found
---

# Phase 2: Code Review Report

**Reviewed:** 2026-07-14T09:41:09Z
**Depth:** standard
**Files Reviewed:** 19
**Status:** issues_found

## Summary

Phase 2 delivers the at-rest persistence layer: atomic crash-safe writes, age/scrypt
encryption, a passphrase seam, the participant share store + manifest, the transport
identity key, encrypted DKG checkpoints, and the coordinator public SQLite store. The
security-critical invariants that this phase exists to enforce are, for the most part,
soundly implemented and — impressively — enforced structurally: nonces are non-persistable
(concrete-typed checkpoint API + compile-fail snapshots), the transport identity has no
FROST conversion (compile-fail snapshot), decrypted secrets are returned in `Zeroizing`,
age is the sole scrypt recipient with an explicit work factor, and the atomic write
sequence (temp → fsync → rename → dir-fsync) is correct including the commonly-omitted
directory fsync.

However, adversarial review surfaces two BLOCKER-class defects that undercut the phase's
own stated threat model. First, `ParticipantStore` joins the caller-supplied `KeyId`
string directly into filesystem paths with **no** path-component validation — the exact
directory-traversal class the code deliberately guards against for `CeremonyId` (T-02-12),
left unguarded here. Second, the coordinator schema migration — in a module whose entire
premise is crash-safe state (T-02-17) — is **not atomic**: a crash mid-migration can leave
the DB permanently unopenable. Three warnings and four info items follow.

## Critical Issues

### CR-01: Unvalidated `KeyId` enables directory traversal in the share/public store paths

**File:** `src/store/participant.rs:193-206` (also the write sites `:98`, `:146`)
**Issue:**
`KeyId` is `pub struct KeyId(pub String)` (`src/crypto/types.rs:13`) with `From<&str>` /
`From<String>` and a public inner field — it is freely constructible from *any* string.
`share_path` and `public_path` join that string straight into the on-disk path:

```rust
fn share_path(&self, tag: &ShareTag) -> PathBuf {
    self.root
        .join("shares")
        .join(&tag.key_id.0)              // <-- unvalidated attacker-influenceable string
        .join(format!("epoch-{}", tag.epoch.0))
        .join(format!("seat-{}.age", seat_hex(&tag.seat)))
}
```

A `KeyId` of `"../../../../home/user/.ssh"` (or any `..`/separator payload) escapes the
store subtree, so `put_share` / `put_public_envelope` become an arbitrary-location file
**write** primitive and `load_share` an arbitrary-location read. This is precisely the
tampering surface the codebase treats as real for ceremony ids — `CeremonyId::new`
(`src/store/checkpoint.rs:83`) rejects anything that is not `[A-Za-z0-9_-]` with an
explicit T-02-12 rationale — but `KeyId` (which flows from ceremony/coordinator metadata in
later phases) gets no equivalent gate. `epoch` (a `u64`) and `seat` (fixed-width hex) are
safe; only `key_id` is unvalidated. The asymmetry is the bug.

**Fix:** Validate the key-id as a safe single path component before it ever reaches a
`join`, mirroring `CeremonyId`. Either gate at the store boundary:

```rust
fn safe_key_component(key_id: &KeyId) -> Result<&str, StoreError> {
    let s = &key_id.0;
    let ok = !s.is_empty()
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !ok {
        return Err(StoreError::Manifest(format!(
            "key_id {s:?} is not a safe path component"
        )));
    }
    Ok(s)
}
```

…and call it in `share_path`/`public_path` (making them fallible), or — better — move the
validation into `KeyId`'s constructor so an unsafe `KeyId` is non-constructible, matching
the `CeremonyId` pattern.

### CR-02: Coordinator schema migration is not atomic — a crash mid-migration bricks the DB

**File:** `src/coordinator/mod.rs:332-340`, `src/coordinator/schema.rs:23-79`
**Issue:**
`migrate` applies the whole v1 schema and then bumps `user_version` as two separate,
unwrapped operations:

```rust
if v < 1 {
    conn.execute_batch(SCHEMA_V1)?;                       // five CREATE TABLEs + a seed INSERT
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
}
```

`SCHEMA_V1` contains no `BEGIN;`/`COMMIT;`, so each DDL statement auto-commits
individually. If the process is interrupted after (say) `CREATE TABLE roster` but before
`user_version` is set to 1, the next `open()` sees `user_version == 0`, re-runs
`execute_batch(SCHEMA_V1)`, and fails on `CREATE TABLE roster` (`table roster already
exists`) — permanently. In a module whose stated design goal is crash-safe single-writer
coordinator state (T-02-17, `WAL`+`synchronous=NORMAL`), a migration that is itself not
crash-safe is a correctness defect and an availability/data-loss risk.

**Fix:** Wrap schema application and the version bump in one transaction so they commit
atomically (SQLite DDL is transactional):

```rust
if v < 1 {
    let tx = conn.unchecked_transaction()?;   // or conn.transaction()? with &mut
    tx.execute_batch(SCHEMA_V1)?;
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    tx.commit()?;
}
```

Alternatively embed `BEGIN;`/`COMMIT;` around the statements in `SCHEMA_V1` and set
`user_version` inside the same batch. Also consider `CREATE TABLE IF NOT EXISTS` as
defence-in-depth for re-run idempotency.

## Warnings

### WR-01: Interactive passphrase lands in an un-zeroized `String`, contradicting the module invariant

**File:** `src/store/passphrase.rs:83`, `:92-97`
**Issue:**
The module doc states the passphrase is "read directly into a `SecretString` via
`rpassword` so the passphrase is never echoed and **never lands in a plain `String`**."
But `rpassword::prompt_password` returns `io::Result<String>`, so `entered`, `first`, and
`second` are plaintext `String`s that hold the store passphrase and are dropped **without
zeroization** (freed heap retains the bytes):

```rust
let first = rpassword::prompt_password("New store passphrase: ")?;   // plaintext String
let second = rpassword::prompt_password("Confirm store passphrase: ")?;
if first != second { ... }
Ok(SecretString::from(first))   // copies into SecretString; the String bytes are not wiped
```

This is the one passphrase that unlocks the identity key **and** every share, so residual
plaintext copies are exactly the memory-hygiene class the project takes seriously elsewhere
(`Zeroizing` everywhere for secrets). The stated invariant is not actually held.

**Fix:** Wrap each read in `Zeroizing` immediately and build the `SecretString` from it,
so the transient buffer is wiped on drop; on the mismatch path drop both before returning:

```rust
let first = Zeroizing::new(rpassword::prompt_password("New store passphrase: ")?);
let second = Zeroizing::new(rpassword::prompt_password("Confirm store passphrase: ")?);
if *first != *second {
    return Err(StoreError::Age("passphrases did not match".into()));
}
Ok(SecretString::from(first.clone()))
```

(Or update the doc comment to drop the "never lands in a plain String" claim — but wiping
is the right fix given the phase's hygiene posture.)

### WR-02: `rusqlite` pinned to 0.37, contradicting the project's documented/researched pin (0.40.1)

**File:** `Cargo.toml:43`
**Issue:**
The project stack doc (CLAUDE.md, "Recommended Stack") pins `rusqlite = 0.40.1` as the
audited/researched version, but the manifest declares `rusqlite = { version = "0.37",
features = ["bundled"] }`. Given the phase's reproducible-build and "100 people must verify
what they run" requirements, silently diverging from the researched, version-compatibility-
checked pin is a governance/quality gap (the divergence is undocumented in the Cargo.toml
comment, which only says "wired in 02-04").

**Fix:** Either bump to the researched `0.40.1` pin, or add an explicit note in the
Cargo.toml comment and in the phase decisions recording why 0.37 was chosen and that the
version-compat matrix was re-verified for it.

### WR-03: `create_dir_secure` does not tighten permissions on a pre-existing directory

**File:** `src/store/atomic.rs:43-56`
**Issue:**
`create_dir_secure` early-returns `Ok(())` when `path.is_dir()` is already true, and only
applies `0700` on fresh creation. If the store root (or any intermediate directory) already
exists with looser permissions — e.g. `~/.cheget` pre-created `0755` by another tool or an
umask mishap — the store silently accepts it and never tightens it. Share/identity/
checkpoint *files* are `0600`, so contents stay protected, but directory listings (seat
structure, epoch/ceremony layout, which key-ids exist) become readable by group/other,
leaking metadata the `0700` design intends to hide.

**Fix:** When the directory already exists on Unix, verify/enforce the mode rather than
trusting it:

```rust
#[cfg(unix)]
if path.is_dir() {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(path)?.permissions().mode() & 0o777;
    if mode != 0o700 {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    return Ok(());
}
```

## Info

### IN-01: `foreign_keys=ON` is a no-op — no table declares a FOREIGN KEY

**File:** `src/coordinator/mod.rs:145`, `src/coordinator/schema.rs:23-79`
**Issue:** `open` sets `foreign_keys=ON` and the module doc lists it as a crash-safety
control, but `SCHEMA_V1` defines no `FOREIGN KEY` constraints, so the pragma enforces
nothing. This is harmless today but misleading — a reader may assume referential integrity
is enforced between (e.g.) `ceremony_transcripts.epoch` and roster/churn rows.
**Fix:** Either add the intended foreign keys (e.g. churn/roster ↔ ceremony), or drop the
pragma and the doc claim until relationships exist. Documenting it as "reserved for a later
phase's constraints" is acceptable.

### IN-02: Manifest filename literal duplicated across the instance and static read paths

**File:** `src/store/participant.rs:181-191`
**Issue:** `read_manifest` hardcodes `root.join("manifest.json")` while `manifest_path`
independently hardcodes `self.root.join("manifest.json")`. The two indices must agree; a
future rename of the manifest file could update one and miss the other, silently splitting
the read paths.
**Fix:** Extract a `const MANIFEST_FILE: &str = "manifest.json";` (or a shared helper) and
use it in both places.

### IN-03: Env-var mutation in a unit test without serialization; comment claims a guarantee cargo does not provide

**File:** `src/store/mod.rs:171-182`
**Issue:** `resolve_honors_home_override` sets/removes the global `CHEGET_HOME` env var and
the comment asserts "single-threaded test." `cargo test` runs tests in the same binary
concurrently by default, so this is only safe as long as no other test in the lib
unit-test binary reads `CHEGET_HOME` — a latent flake if one is ever added.
**Fix:** Serialize env-touching tests (a shared `Mutex`/`serial_test`), or restructure
`resolve()` to accept an injected lookup so the test needs no global mutation.

### IN-04: `secp256k1::SecretKey` intermediates in the identity path are not zeroized

**File:** `src/store/identity.rs:64` (`generate`), `:78-80` (`npub`), `:120` (`load`)
**Issue:** The raw secret is kept in `Zeroizing<[u8;32]>`, but each operation reconstructs a
live `secp256k1::SecretKey`/`Keypair` (which the module itself notes is *not* `ZeroizeOnDrop`,
Pitfall 5). Those transient key objects leave un-wiped copies of the scalar on the stack
when they drop. This is inherent to the `secp256k1` C-binding crate and is acknowledged in
the doc, so it is informational, not a defect — but it is a residual leak worth tracking.
**Fix:** Minimise the count/lifetime of live `SecretKey` reconstructions (e.g. derive and
cache the x-only pubkey once at construction so `npub()` need not rebuild a `SecretKey`
each call), and document the residual as a known `secp256k1` limitation.

---

_Reviewed: 2026-07-14T09:41:09Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
