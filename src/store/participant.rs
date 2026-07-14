//! `ParticipantStore` — the participant's durable home for shares + public key
//! (STOR-01).
//!
//! Two paths, deliberately asymmetric:
//!
//! * **Secret path (D-05/D-06/D-07):** a per-`(key_id, epoch, seat)`
//!   `KeyPackage`, serialized → wrapped in [`Zeroizing`] → age-encrypted →
//!   written atomically to `shares/<key_id>/epoch-<N>/seat-<hex>.age` at mode
//!   `0600`, with the plaintext manifest updated **last** so it never indexes a
//!   half-written share. On load the decrypted bytes live in a [`Zeroizing`]
//!   buffer that drops the instant the operation ends (decrypt-use-drop, D-06).
//!
//! * **Public path (D-05, continues Phase 1 D-09):** the group
//!   `PublicKeyPackage` is written in **plaintext** under
//!   `pubkey/<key_id>/epoch-<N>.json` reusing the existing
//!   [`crate::cli::address::PubkeyEnvelope`] format, so `address` / `share
//!   status` derive from the store with **no passphrase** — one canonical
//!   address-derivation path.
//!
//! Multiple epochs coexist (ROT-06 steady state ≈ 2): new epoch directories are
//! written alongside old ones and old epochs are never deleted here — active
//! pruning is Phase 4.

use std::path::{Path, PathBuf};

use frost_secp256k1_tr as frost;
use frost::keys::{KeyPackage, PublicKeyPackage};
use zeroize::Zeroizing;

use super::atomic::{create_dir_secure, write_atomic};
use super::crypto::{decrypt_secret, encrypt_secret};
use super::manifest::{seat_hex, Manifest, ShareEntry, ShareState};
use super::passphrase::PassphraseSource;
use super::StoreError;
use crate::cli::address::{EnvelopeError, PubkeyEnvelope};
use crate::crypto::types::{Epoch, KeyId, SeatId};

/// The `(key_id, epoch, seat)` coordinates of one stored share (D-02).
///
/// Reuses the canonical crypto tag newtypes rather than reinventing a key.
#[derive(Debug, Clone)]
pub struct ShareTag {
    /// The group-key label this share belongs to.
    pub key_id: KeyId,
    /// The refresh epoch.
    pub epoch: Epoch,
    /// The seat identifier within the `(t, n)` group.
    pub seat: SeatId,
}

impl ShareTag {
    /// Convenience constructor for a `(key_id, epoch, seat)` tag.
    pub fn new(key_id: KeyId, epoch: Epoch, seat: SeatId) -> Self {
        Self {
            key_id,
            epoch,
            seat,
        }
    }
}

/// The participant secret store rooted at a resolved store directory, driven by
/// a single [`PassphraseSource`] (D-02/D-03 — one passphrase unlocks every share).
pub struct ParticipantStore {
    root: PathBuf,
    passphrase: Box<dyn PassphraseSource>,
}

impl ParticipantStore {
    /// Open a store at `root`, acquiring the passphrase through `passphrase`.
    ///
    /// Does not touch the filesystem until a `put_*`/`load_*` call runs.
    pub fn new(root: impl Into<PathBuf>, passphrase: Box<dyn PassphraseSource>) -> Self {
        Self {
            root: root.into(),
            passphrase,
        }
    }

    /// Persist a share (encrypted) and its group public envelope (plaintext).
    ///
    /// Write order enforces D-07: the public envelope and encrypted share land
    /// first (each atomically), then the manifest is updated **last** so it can
    /// never point at a half-written share.
    pub fn put_share(
        &self,
        tag: &ShareTag,
        key_package: &KeyPackage,
        group: &PublicKeyPackage,
        state: ShareState,
    ) -> Result<(), StoreError> {
        let passphrase = self.passphrase.passphrase()?;

        // Public path (plaintext, no secret) — safe to (over)write anytime.
        self.put_public_envelope(&tag.key_id, tag.epoch, group)?;

        // Secret path: serialize → Zeroizing → encrypt → atomic write (0600).
        let share_path = self.share_path(tag);
        create_dir_secure(parent_of(&share_path)?)?;
        let plaintext =
            Zeroizing::new(key_package.serialize().map_err(StoreError::Serialize)?);
        let ciphertext = encrypt_secret(&passphrase, &plaintext)?;
        write_atomic(&share_path, &ciphertext)?;

        // Manifest LAST (D-07): only now does the index reference the share.
        let mut manifest = self.load_manifest()?;
        manifest.add_entry(ShareEntry::new(
            &tag.key_id,
            tag.epoch,
            &tag.seat,
            state,
            now_unix(),
        ));
        create_dir_secure(&self.root)?;
        write_atomic(&self.manifest_path(), &manifest.to_json_bytes()?)?;
        Ok(())
    }

    /// Load and decrypt a share, returning the `KeyPackage`.
    ///
    /// The decrypted bytes live in a [`Zeroizing`] buffer scoped to this call and
    /// are wiped when it returns (D-06). A wrong passphrase yields
    /// [`StoreError::Age`] with no recovered plaintext.
    pub fn load_share(&self, tag: &ShareTag) -> Result<KeyPackage, StoreError> {
        let passphrase = self.passphrase.passphrase()?;
        let ciphertext = std::fs::read(self.share_path(tag)).map_err(StoreError::Io)?;
        // `plaintext` is Zeroizing<Vec<u8>>: dropped (and wiped) at fn end.
        let plaintext = decrypt_secret(&passphrase, &ciphertext)?;
        KeyPackage::deserialize(&plaintext).map_err(StoreError::Frost)
    }

    /// Write the plaintext public `PubkeyEnvelope` for `(key_id, epoch)` (D-05).
    ///
    /// Reuses [`PubkeyEnvelope::from_package`] so the store's public artifact is
    /// byte-identical to the Phase 1 `keygen` output — one address-derivation
    /// path.
    pub fn put_public_envelope(
        &self,
        key_id: &KeyId,
        epoch: Epoch,
        group: &PublicKeyPackage,
    ) -> Result<(), StoreError> {
        let envelope = PubkeyEnvelope::from_package(key_id.0.clone(), epoch.0, group)
            .map_err(map_envelope_err)?;
        let json = serde_json::to_vec_pretty(&envelope)?;
        let path = self.public_path(key_id, epoch);
        create_dir_secure(parent_of(&path)?)?;
        write_atomic(&path, &json)?;
        Ok(())
    }

    /// Read the plaintext public envelope for `(key_id, epoch)` — **no unlock**.
    ///
    /// This never touches the passphrase source; the public package is
    /// intentionally readable with no secret so `address` / `share status` work
    /// on a locked store.
    pub fn load_public_envelope(
        &self,
        key_id: &KeyId,
        epoch: Epoch,
    ) -> Result<PubkeyEnvelope, StoreError> {
        let bytes = std::fs::read(self.public_path(key_id, epoch)).map_err(StoreError::Io)?;
        let envelope: PubkeyEnvelope = serde_json::from_slice(&bytes)?;
        Ok(envelope)
    }

    /// Load the manifest, treating an absent file as an empty manifest.
    pub fn load_manifest(&self) -> Result<Manifest, StoreError> {
        match std::fs::read(self.manifest_path()) {
            Ok(bytes) => Manifest::from_json_bytes(&bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Manifest::new()),
            Err(e) => Err(StoreError::Io(e)),
        }
    }

    /// Read the plaintext share manifest at `root` with **no passphrase** (D-05).
    ///
    /// This is the unlock-free read path `share status` uses: it never
    /// constructs a [`PassphraseSource`], so listing held shares can never
    /// prompt or touch a secret. An absent manifest reads as empty.
    pub fn read_manifest(root: &Path) -> Result<Manifest, StoreError> {
        match std::fs::read(root.join("manifest.json")) {
            Ok(bytes) => Manifest::from_json_bytes(&bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Manifest::new()),
            Err(e) => Err(StoreError::Io(e)),
        }
    }

    fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.json")
    }

    fn share_path(&self, tag: &ShareTag) -> PathBuf {
        self.root
            .join("shares")
            .join(&tag.key_id.0)
            .join(format!("epoch-{}", tag.epoch.0))
            .join(format!("seat-{}.age", seat_hex(&tag.seat)))
    }

    fn public_path(&self, key_id: &KeyId, epoch: Epoch) -> PathBuf {
        self.root
            .join("pubkey")
            .join(&key_id.0)
            .join(format!("epoch-{}.json", epoch.0))
    }
}

/// Map a public-envelope error into the store error surface, preserving the
/// frost serialize error face where possible.
fn map_envelope_err(e: EnvelopeError) -> StoreError {
    match e {
        EnvelopeError::Serialize(fe) => StoreError::Serialize(fe),
        EnvelopeError::Package(fe) => StoreError::Frost(fe),
        EnvelopeError::Json(je) => StoreError::Json(je),
        EnvelopeError::Io(io) => StoreError::Io(io),
        other => StoreError::Manifest(other.to_string()),
    }
}

/// The parent directory of `path`, or an `InvalidInput` I/O error.
fn parent_of(path: &Path) -> Result<&Path, StoreError> {
    path.parent().ok_or_else(|| {
        StoreError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "store path has no parent directory",
        ))
    })
}

/// Current unix time in whole seconds (0 on a pre-epoch clock).
fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::run_inprocess_dkg;
    use crate::store::passphrase::InCodePassphrase;

    /// A unique scratch store root under the system temp dir.
    fn temp_root() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("cheget-store-{}-{}-{}", std::process::id(), nanos, n))
    }

    #[test]
    fn share_roundtrip() {
        // A small in-process DKG gives real KeyPackages + the group package.
        let (shares, group) = run_inprocess_dkg(3, 5).unwrap();
        let (&seat_id, key_package) = shares.iter().next().unwrap();

        let root = temp_root();
        let store = ParticipantStore::new(
            root.clone(),
            Box::new(InCodePassphrase::new("test-store-passphrase")),
        );
        let tag = ShareTag::new(KeyId::active(), Epoch::GENESIS, seat_id);

        store
            .put_share(&tag, key_package, &group, ShareState::Active)
            .unwrap();

        // Reload under the same passphrase is byte-equal to the original.
        let loaded = store.load_share(&tag).unwrap();
        assert_eq!(
            loaded.serialize().unwrap(),
            key_package.serialize().unwrap(),
            "KeyPackage must persist→reload byte-equal"
        );

        // Manifest indexes the share after the write (D-07 ordering).
        let manifest = store.load_manifest().unwrap();
        assert!(manifest
            .lookup(&KeyId::active(), Epoch::GENESIS, &seat_id)
            .is_some());

        // The public envelope reads with NO unlock and decodes to the group key.
        // A store constructed with the WRONG passphrase still reads the public
        // path (it never touches the passphrase) but cannot decrypt the share.
        let locked = ParticipantStore::new(
            root.clone(),
            Box::new(InCodePassphrase::new("a-completely-different-passphrase")),
        );
        let envelope = locked
            .load_public_envelope(&KeyId::active(), Epoch::GENESIS)
            .expect("public envelope must read with no unlock");
        let decoded = envelope.decode_package().unwrap();
        assert_eq!(
            decoded.verifying_key(),
            group.verifying_key(),
            "public envelope must decode to the group verifying key"
        );
        assert!(
            locked.load_share(&tag).is_err(),
            "a wrong passphrase must not decrypt the share"
        );

        // A second epoch coexists alongside the first (no deletion of old).
        let tag2 = ShareTag::new(KeyId::active(), Epoch::GENESIS.next(), seat_id);
        store
            .put_share(&tag2, key_package, &group, ShareState::Standby)
            .unwrap();
        assert!(store.load_share(&tag).is_ok(), "epoch 0 share still present");
        assert!(store.load_share(&tag2).is_ok(), "epoch 1 share present");

        std::fs::remove_dir_all(&root).ok();
    }
}
