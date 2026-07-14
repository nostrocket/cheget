//! `CheckpointStore` — encrypted between-round DKG checkpoints (STOR-02).
//!
//! This module is the deliberate **inverse** of [`crate::crypto::EphemeralNonces`]
//! and is the structural half of STOR-02 that concerns *DKG round secrets*:
//!
//! * a DKG `dkg::round{1,2}::SecretPackage` MUST survive between rounds (a
//!   ceremony can pause / crash / resume), so it **is** serializable and gets
//!   concrete persist methods here;
//! * a signing nonce ([`SigningNonces`](frost_secp256k1_tr::round1::SigningNonces))
//!   MUST NEVER survive a round — reuse across two sighashes leaks the long-lived
//!   share (the highest-severity key-extraction bug class). It is non-serializable
//!   and gets **no** method here.
//!
//! The load-bearing control is the **shape of the API**, not any runtime check:
//! [`CheckpointStore`] exposes only `put_round1`/`load_round1`/`put_round2`/
//! `load_round2`, each typed to a *concrete* `dkg::round{1,2}::SecretPackage`.
//! There is deliberately **no** generic `persist<T: Serialize>`. Because a nonce
//! is not serializable and there is no generic sink, a nonce is a *non-expressible*
//! checkpoint input (Pitfall 1). The reviewable proof of that lives in the
//! compile-fail snapshots `tests/ui/checkpoint_no_nonce.rs` and
//! `tests/ui/checkpoint_no_generic_persist.rs`, mirroring
//! `tests/ui/nonce_no_serialize.rs`.
//!
//! On-disk layout (D-11 — checkpoints live in the participant store, reusing the
//! same store passphrase, D-09):
//!
//! ```text
//! <root>/ceremonies/<ceremony-id>/<seat-hex>/round-1.age
//! <root>/ceremonies/<ceremony-id>/<seat-hex>/round-2.age
//! ```
//!
//! Each `put_*` is `serialize()` → [`Zeroizing`] → [`encrypt_secret`] (store
//! passphrase, D-09) → [`write_atomic`] (crash-safe, D-07); each `load_*` reverses
//! it (decrypt → deserialize) with the plaintext scoped to the call (D-06).
//!
//! Wipe semantics (D-10): [`CheckpointStore::wipe`] is called on ceremony
//! **success** and removes that ceremony's checkpoint directory; an aborted or
//! interrupted ceremony is simply never wiped, so its files remain and the
//! ceremony can resume per `(ceremony_id, round, seat)`. Deletion is labelled
//! **best-effort hygiene**, never a security control — the on-chain sweep is the
//! real revocation (SPEC §11.1, Pitfall 9).
//!
//! **Purity boundary (D-08):** `run_inprocess_dkg` / `src/crypto/keygen.rs` are
//! NOT modified. Persistence stays out of the pure crypto core; the checkpoint
//! capability is wired at the seam and only ever handles already-produced FROST
//! bytes.

use std::io;
use std::path::{Path, PathBuf};

use frost_secp256k1_tr as frost;
use frost::keys::dkg;
use zeroize::Zeroizing;

use super::atomic::{create_dir_secure, write_atomic};
use super::crypto::{decrypt_secret, encrypt_secret};
use super::manifest::seat_hex;
use super::passphrase::PassphraseSource;
use super::StoreError;
use crate::crypto::types::SeatId;

/// The subdirectory (under the store root) that holds all ceremony checkpoints.
const CEREMONIES_DIR: &str = "ceremonies";

/// Identifier for one DKG ceremony — the top-level key under which a ceremony's
/// per-seat round checkpoints are grouped.
///
/// Constructed through [`CeremonyId::new`], which validates that the id is a
/// single safe path component (non-empty; only ASCII alphanumerics, `-` and `_`).
/// This keeps a caller-supplied id from escaping the `ceremonies/` subtree via
/// `/`, `\`, or `..` (a path-traversal tampering surface, T-02-12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyId(String);

impl CeremonyId {
    /// Build a ceremony id, rejecting anything that is not a safe single path
    /// component.
    ///
    /// Allowed: a non-empty string of ASCII alphanumerics plus `-` and `_`. Any
    /// path separator, `..`, whitespace, or control/other byte is rejected with
    /// [`StoreError::Io`] (`InvalidInput`) so a hostile id can never write outside
    /// the ceremony subtree.
    pub fn new(id: impl Into<String>) -> Result<Self, StoreError> {
        let id = id.into();
        let ok = !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !ok {
            return Err(StoreError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ceremony id must be a non-empty [A-Za-z0-9_-] path component",
            )));
        }
        Ok(Self(id))
    }

    /// The validated id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CeremonyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Encrypted between-round DKG checkpoint store (STOR-02).
///
/// Holds the resolved store root plus the single [`PassphraseSource`] (D-09 — the
/// SAME store passphrase that unlocks shares and the identity key). Does not touch
/// the filesystem until a `put_*` / `load_*` / `wipe` call runs.
///
/// The public surface is intentionally minimal and *concrete* — see the module
/// docs: no generic persist, and no method whose input is (or could hold) nonce
/// material.
pub struct CheckpointStore {
    root: PathBuf,
    passphrase: Box<dyn PassphraseSource>,
}

impl CheckpointStore {
    /// Open a checkpoint store at `root`, acquiring the passphrase through
    /// `passphrase`.
    pub fn new(root: impl Into<PathBuf>, passphrase: Box<dyn PassphraseSource>) -> Self {
        Self {
            root: root.into(),
            passphrase,
        }
    }

    /// Checkpoint a seat's round-1 `SecretPackage` (encrypted, crash-safe).
    ///
    /// `serialize()` → [`Zeroizing`] → [`encrypt_secret`] (store passphrase) →
    /// [`write_atomic`] to `ceremonies/<cid>/<seat>/round-1.age`.
    pub fn put_round1(
        &self,
        cid: &CeremonyId,
        seat: SeatId,
        pkg: &dkg::round1::SecretPackage,
    ) -> Result<(), StoreError> {
        let plaintext = Zeroizing::new(pkg.serialize().map_err(StoreError::Serialize)?);
        self.put_round(cid, seat, 1, &plaintext)
    }

    /// Load and decrypt a seat's round-1 `SecretPackage`.
    ///
    /// The decrypted bytes live in a [`Zeroizing`] buffer scoped to this call and
    /// are wiped when it returns (D-06). A wrong passphrase yields
    /// [`StoreError::Age`] with no recovered plaintext.
    pub fn load_round1(
        &self,
        cid: &CeremonyId,
        seat: SeatId,
    ) -> Result<dkg::round1::SecretPackage, StoreError> {
        let plaintext = self.load_round(cid, seat, 1)?;
        dkg::round1::SecretPackage::deserialize(&plaintext).map_err(StoreError::Frost)
    }

    /// Checkpoint a seat's round-2 `SecretPackage` (encrypted, crash-safe).
    ///
    /// Mirrors [`CheckpointStore::put_round1`] for
    /// `ceremonies/<cid>/<seat>/round-2.age`.
    pub fn put_round2(
        &self,
        cid: &CeremonyId,
        seat: SeatId,
        pkg: &dkg::round2::SecretPackage,
    ) -> Result<(), StoreError> {
        let plaintext = Zeroizing::new(pkg.serialize().map_err(StoreError::Serialize)?);
        self.put_round(cid, seat, 2, &plaintext)
    }

    /// Load and decrypt a seat's round-2 `SecretPackage`.
    ///
    /// Mirrors [`CheckpointStore::load_round1`].
    pub fn load_round2(
        &self,
        cid: &CeremonyId,
        seat: SeatId,
    ) -> Result<dkg::round2::SecretPackage, StoreError> {
        let plaintext = self.load_round(cid, seat, 2)?;
        dkg::round2::SecretPackage::deserialize(&plaintext).map_err(StoreError::Frost)
    }

    /// Wipe a ceremony's checkpoint directory on **success** (D-10).
    ///
    /// Removes `ceremonies/<cid>` and everything under it. An absent directory is
    /// a no-op (idempotent). Callers invoke this ONLY after the ceremony has
    /// completed successfully; an aborted/interrupted ceremony is simply never
    /// wiped, so its round files remain for a `(ceremony_id, round, seat)` resume.
    ///
    /// This is **best-effort hygiene, not a security control**: a successful
    /// wipe does not guarantee the secret is unrecoverable from the underlying
    /// media — true revocation is the on-chain sweep (SPEC §11.1, Pitfall 9).
    pub fn wipe(&self, cid: &CeremonyId) -> Result<(), StoreError> {
        let dir = self.ceremony_dir(cid);
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StoreError::Io(e)),
        }
    }

    // ---- internal helpers (no generic persist is EVER exposed) ----

    /// Encrypt `plaintext` under the store passphrase and atomically write it to
    /// the `(cid, seat, round)` path. Private, and only reachable from the two
    /// concrete `put_round{1,2}` methods.
    fn put_round(
        &self,
        cid: &CeremonyId,
        seat: SeatId,
        round: u8,
        plaintext: &[u8],
    ) -> Result<(), StoreError> {
        let passphrase = self.passphrase.passphrase()?;
        let ciphertext = encrypt_secret(&passphrase, plaintext)?;
        let path = self.round_path(cid, seat, round);
        create_dir_secure(parent_of(&path)?)?;
        write_atomic(&path, &ciphertext)
    }

    /// Read and decrypt the `(cid, seat, round)` checkpoint, returning the
    /// plaintext in a [`Zeroizing`] buffer scoped to the caller.
    fn load_round(
        &self,
        cid: &CeremonyId,
        seat: SeatId,
        round: u8,
    ) -> Result<Zeroizing<Vec<u8>>, StoreError> {
        let passphrase = self.passphrase.passphrase()?;
        let ciphertext = std::fs::read(self.round_path(cid, seat, round)).map_err(StoreError::Io)?;
        decrypt_secret(&passphrase, &ciphertext)
    }

    /// `<root>/ceremonies/<cid>`.
    fn ceremony_dir(&self, cid: &CeremonyId) -> PathBuf {
        self.root.join(CEREMONIES_DIR).join(cid.as_str())
    }

    /// `<root>/ceremonies/<cid>/<seat-hex>/round-<N>.age`.
    fn round_path(&self, cid: &CeremonyId, seat: SeatId, round: u8) -> PathBuf {
        self.ceremony_dir(cid)
            .join(seat_hex(&seat))
            .join(format!("round-{round}.age"))
    }
}

/// The parent directory of `path`, or an `InvalidInput` I/O error.
fn parent_of(path: &Path) -> Result<&Path, StoreError> {
    path.parent().ok_or_else(|| {
        StoreError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "checkpoint path has no parent directory",
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::passphrase::InCodePassphrase;
    use std::collections::BTreeMap;

    /// A unique scratch store root under the system temp dir (mirrors the
    /// participant-store test helper; avoids a tempfile dev-dependency).
    fn temp_root() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("cheget-cp-{}-{}-{}", std::process::id(), nanos, n))
    }

    fn store_at(root: &Path) -> CheckpointStore {
        CheckpointStore::new(
            root.to_path_buf(),
            Box::new(InCodePassphrase::new("checkpoint-test-passphrase")),
        )
    }

    /// Drive REAL `dkg::part1` → checkpoint round1 → reload → `dkg::part2` →
    /// checkpoint round2 → reload, proving the persisted path is byte-faithful and
    /// the ceremony proceeds identically to the unpersisted path. Uses a small
    /// (t=2, n=3) ceremony — the code is generic over `(t, n)` (D-01), and
    /// `run_inprocess_dkg` proves the full path elsewhere; nothing here fakes a
    /// pause inside `run_inprocess_dkg` (D-08).
    #[test]
    fn dkg_roundtrip() {
        let n: u16 = 3;
        let t: u16 = 2;
        let mut rng = frost::rand_core::OsRng;

        // Round 1 across all seats (real part1 output).
        let mut r1_secret: BTreeMap<SeatId, dkg::round1::SecretPackage> = BTreeMap::new();
        let mut r1_pkgs: BTreeMap<SeatId, dkg::round1::Package> = BTreeMap::new();
        for i in 1..=n {
            let id: SeatId = i.try_into().expect("nonzero seat id");
            let (secret, pkg) = dkg::part1(id, n, t, &mut rng).expect("part1");
            r1_secret.insert(id, secret);
            r1_pkgs.insert(id, pkg);
        }

        let seat: SeatId = 1u16.try_into().unwrap();
        let root = temp_root();
        let store = store_at(&root);
        let cid = CeremonyId::new("ceremony-roundtrip-1").unwrap();

        // Checkpoint seat 1's round-1 secret, then reload it byte-faithfully.
        let orig1 = r1_secret.remove(&seat).unwrap();
        store.put_round1(&cid, seat, &orig1).unwrap();
        let loaded1 = store.load_round1(&cid, seat).unwrap();
        assert_eq!(
            loaded1.serialize().unwrap(),
            orig1.serialize().unwrap(),
            "round-1 SecretPackage must persist→reload byte-equal"
        );

        // The RELOADED round-1 secret drives part2 exactly like the fresh one.
        let mut others: BTreeMap<SeatId, dkg::round1::Package> = r1_pkgs.clone();
        others.remove(&seat);
        let (secret2, _sent) = dkg::part2(loaded1, &others).expect("part2 from reloaded round-1");

        // Checkpoint round-2 and reload it byte-faithfully.
        store.put_round2(&cid, seat, &secret2).unwrap();
        let loaded2 = store.load_round2(&cid, seat).unwrap();
        assert_eq!(
            loaded2.serialize().unwrap(),
            secret2.serialize().unwrap(),
            "round-2 SecretPackage must persist→reload byte-equal"
        );

        // A wrong passphrase cannot decrypt a checkpoint.
        let locked = CheckpointStore::new(
            root.clone(),
            Box::new(InCodePassphrase::new("a-different-passphrase")),
        );
        assert!(
            locked.load_round1(&cid, seat).is_err(),
            "wrong passphrase must not decrypt a checkpoint"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// wipe-on-success removes a ceremony's round files; keep-on-abort leaves them
    /// so the ceremony can resume (D-10).
    #[test]
    fn wipe_vs_keep() {
        let n: u16 = 2;
        let t: u16 = 2;
        let mut rng = frost::rand_core::OsRng;
        let seat: SeatId = 1u16.try_into().unwrap();
        let (secret, _pkg) = dkg::part1(seat, n, t, &mut rng).expect("part1");

        let root = temp_root();
        let store = store_at(&root);

        // --- keep-on-abort: a ceremony that is never wiped retains its files ---
        let kept = CeremonyId::new("ceremony-aborted").unwrap();
        store.put_round1(&kept, seat, &secret).unwrap();
        assert!(
            store.load_round1(&kept, seat).is_ok(),
            "an un-wiped ceremony must keep its checkpoint for resume"
        );

        // --- wipe-on-success: after success the ceremony's files are gone ---
        let done = CeremonyId::new("ceremony-succeeded").unwrap();
        store.put_round1(&done, seat, &secret).unwrap();
        assert!(store.load_round1(&done, seat).is_ok());
        store.wipe(&done).unwrap();
        assert!(
            store.load_round1(&done, seat).is_err(),
            "wipe-on-success must remove the ceremony's checkpoint files"
        );
        // wipe is idempotent — wiping an already-gone ceremony is a no-op.
        store.wipe(&done).unwrap();

        // The aborted ceremony is untouched by the other's wipe.
        assert!(
            store.load_round1(&kept, seat).is_ok(),
            "wiping one ceremony must not affect another"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// A hostile ceremony id cannot escape the `ceremonies/` subtree.
    #[test]
    fn ceremony_id_rejects_traversal() {
        for bad in ["", "..", "a/b", "../evil", "a\\b", "with space", "dot.dot"] {
            assert!(
                CeremonyId::new(bad).is_err(),
                "ceremony id {bad:?} must be rejected"
            );
        }
        for ok in ["abc", "ceremony-1", "A_B-9"] {
            assert!(CeremonyId::new(ok).is_ok(), "ceremony id {ok:?} must be ok");
        }
    }
}
