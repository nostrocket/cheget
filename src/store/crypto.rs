//! age/scrypt one-shot at-rest encryption for small secret blobs (D-06, D-09).
//!
//! Every secret `cheget` persists — a serialized FROST `KeyPackage`, the
//! transport identity key, an encrypted DKG checkpoint — is a sub-kilobyte blob,
//! so we use age's one-shot [`age::encrypt`] / [`age::decrypt`] with a **scrypt
//! passphrase recipient** rather than streaming. This keeps the plaintext in a
//! single [`zeroize::Zeroizing`] buffer with the shortest possible lifetime.
//!
//! Security invariants:
//! * the scrypt (passphrase) recipient is the **sole** recipient — never mixed
//!   with an x25519 recipient (T-02-01);
//! * the KDF work factor is an explicit deliberate choice, not the library
//!   default (D-09);
//! * [`decrypt_secret`] returns `Zeroizing<Vec<u8>>` so the caller's drop wipes
//!   the plaintext at its point of use (D-06, T-02-02);
//! * age is audited — we never hand-roll scrypt or the AEAD (RESEARCH V6).

use age::secrecy::SecretString;
use zeroize::Zeroizing;

use super::StoreError;

/// scrypt work factor: `N = 2^SCRYPT_LOG_N`.
///
/// D-09: 18 is age's own interactive default (~256 MiB-equivalent CPU cost) and
/// a sound floor for interactive unlock — high enough to be costly to brute
/// force, low enough that a legitimate unlock is a short pause. Never 0 or ≥64
/// (`set_work_factor` panics). Tuned deliberately, not left to the library.
const SCRYPT_LOG_N: u8 = 18;

/// Encrypt `plaintext` to `passphrase` via age's scrypt recipient, one-shot.
///
/// The work factor is set **before** encryption so it is baked into the header.
pub fn encrypt_secret(
    passphrase: &SecretString,
    plaintext: &[u8],
) -> Result<Vec<u8>, StoreError> {
    let _ = (passphrase, plaintext, SCRYPT_LOG_N);
    unimplemented!("GREEN: age::scrypt::Recipient + age::encrypt")
}

/// Decrypt `ciphertext` with `passphrase`, returning the plaintext wrapped in
/// [`Zeroizing`] so it is wiped when the caller drops it (D-06).
///
/// A wrong passphrase (or tampered ciphertext) returns `Err(StoreError::Age)`
/// with **no** partial plaintext — age verifies the payload MAC before yielding
/// any bytes.
pub fn decrypt_secret(
    passphrase: &SecretString,
    ciphertext: &[u8],
) -> Result<Zeroizing<Vec<u8>>, StoreError> {
    let _ = (passphrase, ciphertext);
    unimplemented!("GREEN: age::scrypt::Identity + age::decrypt → Zeroizing")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pp(s: &str) -> SecretString {
        SecretString::from(s.to_string())
    }

    #[test]
    fn age_roundtrip() {
        let secret = b"a serialized frost KeyPackage would live here";
        let ciphertext = encrypt_secret(&pp("correct horse battery staple"), secret).unwrap();
        // Ciphertext must not contain the plaintext verbatim.
        assert_ne!(ciphertext.as_slice(), secret.as_slice());
        let plaintext = decrypt_secret(&pp("correct horse battery staple"), &ciphertext).unwrap();
        assert_eq!(plaintext.as_slice(), secret.as_slice());
    }

    #[test]
    fn wrong_passphrase_fails() {
        let secret = b"top secret share";
        let ciphertext = encrypt_secret(&pp("right-passphrase"), secret).unwrap();
        let result = decrypt_secret(&pp("WRONG-passphrase"), &ciphertext);
        // Err with the Age face, and never any recovered plaintext.
        assert!(
            matches!(result, Err(StoreError::Age(_))),
            "wrong passphrase must yield StoreError::Age, got {result:?}"
        );
    }
}
