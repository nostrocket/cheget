//! `IdentityKeypair` — the transport-only Nostr identity key, structurally
//! separated from every FROST secret (D-12/D-13/D-14).
//!
//! This is the fourth structural control in the spirit of Phase 1's controls
//! (the non-serializable [`crate::crypto::EphemeralNonces`] is the canonical
//! one). The threat it forecloses (T-02-05): if the transport identity could be
//! *derived from* or *reused as* FROST key material — the two secrets both live
//! on secp256k1 — then compromising one would compromise the other, collapsing
//! the transport/threshold isolation the whole design rests on.
//!
//! [`IdentityKeypair`] makes that reuse a **compile-time impossibility**:
//!
//! - it is generated from an **independent** `secp256k1::rand` `OsRng` draw on
//!   the C-lib `secp256k1` curve (a different RNG *and* a different curve crate
//!   from FROST's `k256`), so it is never a function of any FROST value;
//! - it holds its raw secret in [`Zeroizing`] (`secp256k1::SecretKey` is NOT
//!   `ZeroizeOnDrop`, Pitfall 5) and only builds a live `SecretKey` inside a
//!   short scope for the two operations that need one;
//! - it exposes **no** `From`/`TryFrom`/conversion to or from any FROST type
//!   (`KeyPackage`, `SigningShare`, `VerifyingKey`, …). Reusing FROST material as
//!   the identity key is *non-expressible*, exactly as reusing a nonce is.
//!
//! The proof that no FROST↔identity conversion exists lives in
//! `tests/ui/identity_no_frost_conversion.rs` (a `trybuild` compile-fail
//! snapshot) — that is the reviewable structural artifact, deliberately NOT a
//! runtime assertion or a comment here, mirroring `crypto/nonce.rs`.
//!
//! `npub()` derives the NIP-19 `npub` (D-15 prep for the coordinator roster): the
//! 32-byte x-only public key encoded with the **Bech32** checksum (NOT Bech32m,
//! Pitfall 4) under the `npub` HRP.

use std::path::Path;

use bech32::{Bech32, Hrp};
use secp256k1::rand::rngs::OsRng;
use secp256k1::{Keypair, Secp256k1, SecretKey};
use zeroize::Zeroizing;

use super::atomic::{create_dir_secure, write_atomic};
use super::crypto::{decrypt_secret, encrypt_secret};
use super::passphrase::PassphraseSource;
use super::StoreError;

/// The file name of the age-encrypted identity secret under the store root.
pub const IDENTITY_FILE: &str = "identity.age";

/// A transport-only secp256k1 keypair used solely for the Nostr identity.
///
/// The 32-byte secret is kept in [`Zeroizing`] and stored age-encrypted at rest.
/// There is intentionally no constructor or conversion that takes FROST material
/// (D-13) — the only ways to obtain one are [`IdentityKeypair::generate`] (a
/// fresh independent draw) and [`IdentityKeypair::load`] (decrypt a previously
/// generated key).
pub struct IdentityKeypair {
    secret: Zeroizing<[u8; 32]>,
}

impl IdentityKeypair {
    /// Generate a fresh transport identity from an independent OS CSPRNG draw
    /// on the secp256k1 curve (D-12/D-14).
    pub fn generate() -> Self {
        let secp = Secp256k1::new();
        // Independent OsRng draw — never seeded from or shared with FROST.
        let (sk, _pk) = secp.generate_keypair(&mut OsRng);
        Self {
            secret: Zeroizing::new(sk.secret_bytes()),
        }
    }

    /// The NIP-19 `npub` for this identity: Bech32 (NOT Bech32m) over the 32-byte
    /// x-only public key, HRP `npub` (D-15, Pitfall 4).
    ///
    /// Infallible: the secret is a valid scalar by construction ([`generate`]
    /// draws one; [`load`] validates on reload), and a 32-byte payload always
    /// encodes.
    pub fn npub(&self) -> String {
        let secp = Secp256k1::new();
        let sk = SecretKey::from_slice(&*self.secret)
            .expect("identity secret is a valid scalar by construction");
        let (xonly, _parity) = Keypair::from_secret_key(&secp, &sk).x_only_public_key();
        let hrp = Hrp::parse("npub").expect("\"npub\" is a valid bech32 HRP");
        bech32::encode::<Bech32>(hrp, &xonly.serialize())
            .expect("a 32-byte payload always bech32-encodes")
    }

    /// Age-encrypt the raw secret and write it atomically to
    /// `<root>/identity.age` (mode 0600) under the store passphrase (D-02).
    pub fn persist(
        &self,
        root: &Path,
        passphrase: &dyn PassphraseSource,
    ) -> Result<(), StoreError> {
        create_dir_secure(root)?;
        let pass = passphrase.passphrase()?;
        let ciphertext = encrypt_secret(&pass, &*self.secret)?;
        write_atomic(&root.join(IDENTITY_FILE), &ciphertext)?;
        Ok(())
    }

    /// Decrypt and reconstruct the identity from `<root>/identity.age` under the
    /// same store passphrase (D-02).
    ///
    /// Validates that the decrypted material is exactly 32 bytes and a
    /// well-formed secp256k1 scalar, returning [`StoreError::Identity`]
    /// otherwise. The decrypted plaintext lives in [`Zeroizing`] and is wiped
    /// when this call returns (D-06).
    pub fn load(root: &Path, passphrase: &dyn PassphraseSource) -> Result<Self, StoreError> {
        let pass = passphrase.passphrase()?;
        let ciphertext = std::fs::read(root.join(IDENTITY_FILE)).map_err(StoreError::Io)?;
        let plaintext = decrypt_secret(&pass, &ciphertext)?;
        if plaintext.len() != 32 {
            return Err(StoreError::Identity(format!(
                "identity secret is {} bytes, expected 32",
                plaintext.len()
            )));
        }
        let mut secret = Zeroizing::new([0u8; 32]);
        secret.copy_from_slice(&plaintext);
        // Validate it is a well-formed scalar so `npub()` can stay infallible.
        SecretKey::from_slice(&*secret)
            .map_err(|e| StoreError::Identity(format!("not a valid secp256k1 secret: {e}")))?;
        Ok(Self { secret })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::passphrase::InCodePassphrase;
    use bech32::primitives::decode::CheckedHrpstring;
    use std::path::PathBuf;

    /// A unique scratch store root under the system temp dir.
    fn temp_root() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("cheget-id-{}-{}-{}", std::process::id(), nanos, n))
    }

    #[test]
    fn identity_roundtrip_npub() {
        let id = IdentityKeypair::generate();
        let npub = id.npub();

        // Starts with the NIP-19 prefix.
        assert!(
            npub.starts_with("npub1"),
            "npub must start with npub1, got {npub}"
        );

        // Uses the Bech32 checksum, NOT Bech32m (Pitfall 4).
        assert!(
            CheckedHrpstring::new::<bech32::Bech32>(&npub).is_ok(),
            "npub must validate under the Bech32 checksum"
        );
        assert!(
            CheckedHrpstring::new::<bech32::Bech32m>(&npub).is_err(),
            "npub must NOT validate under Bech32m — wrong variant is a rejected npub"
        );

        // Persist → reload under the SAME store passphrase yields the same npub.
        let root = temp_root();
        let source = InCodePassphrase::new("identity-store-passphrase");
        id.persist(&root, &source).unwrap();
        let reloaded = IdentityKeypair::load(&root, &source).unwrap();
        assert_eq!(
            reloaded.npub(),
            npub,
            "npub must be stable across persist/reload"
        );

        // A wrong passphrase cannot reload the identity.
        let wrong = InCodePassphrase::new("a-different-passphrase");
        assert!(
            IdentityKeypair::load(&root, &wrong).is_err(),
            "a wrong passphrase must not decrypt the identity"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}
