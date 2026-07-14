//! The passphrase seam (D-01/D-02/D-03) — how the single store passphrase is
//! acquired, with the production path locked to an interactive no-echo prompt.
//!
//! Load-bearing rule: the **production** [`PassphraseSource`] is interactive
//! only. No environment variable and no CLI flag for the passphrase ever ships
//! (D-01/D-03, RESEARCH Pitfall 7) — mirroring how the transport seam refuses to
//! leak a `nostr-sdk` type, this seam refuses to admit a non-interactive
//! passphrase in production. `CHEGET_HOME` overrides only the store *path*, never
//! the passphrase.
//!
//! One store passphrase unlocks the identity key **and** every share (D-02); the
//! trait therefore carries no per-key notion — a single `passphrase()` call.
//!
//! Two implementations sit behind the trait:
//! * [`InteractivePassphrase`] — production, no-echo, gated out of test builds so
//!   a test can never block on or link a terminal prompt;
//! * [`InCodePassphrase`] — a fixed passphrase for headless CI/tests (D-03).

use age::secrecy::SecretString;

use super::StoreError;

/// Source of the one store passphrase (D-02).
///
/// Production callers construct an [`InteractivePassphrase`]; tests construct an
/// [`InCodePassphrase`]. There is deliberately no env/flag-backed impl.
pub trait PassphraseSource {
    /// Acquire the store passphrase.
    ///
    /// Returns `Err` if an interactive prompt fails or a create-time
    /// confirmation does not match. (The trait is fallible so the interactive
    /// impl can surface IO / mismatch errors; the in-code impl is infallible.)
    fn passphrase(&self) -> Result<SecretString, StoreError>;
}

/// In-code passphrase for headless CI/tests only (D-03). Never used in
/// production — the production path is [`InteractivePassphrase`].
pub struct InCodePassphrase(SecretString);

impl InCodePassphrase {
    /// Build a fixed-passphrase source from a plain string (test fixture).
    pub fn new(passphrase: impl Into<String>) -> Self {
        Self(SecretString::from(passphrase.into()))
    }
}

impl PassphraseSource for InCodePassphrase {
    fn passphrase(&self) -> Result<SecretString, StoreError> {
        Ok(self.0.clone())
    }
}

/// Interactive, no-echo passphrase prompt (production, D-01/D-04).
///
/// Reads directly into a [`SecretString`] via `rpassword` so the passphrase is
/// never echoed and never lands in a plain `String`. On new-store creation it
/// prompts twice and requires a match, and prints the unrecoverability warning
/// (D-04). Gated `#[cfg(not(test))]` so no test build links a terminal prompt.
#[cfg(not(test))]
pub struct InteractivePassphrase {
    /// When true, prompt twice + require a match (new-store creation, D-04).
    confirm: bool,
}

#[cfg(not(test))]
impl InteractivePassphrase {
    /// Prompt once to unlock an existing store.
    pub fn for_unlock() -> Self {
        Self { confirm: false }
    }

    /// Prompt twice (confirm) to create a new store, warning that a lost
    /// passphrase is unrecoverable (D-04).
    pub fn for_new_store() -> Self {
        Self { confirm: true }
    }
}

#[cfg(not(test))]
impl PassphraseSource for InteractivePassphrase {
    fn passphrase(&self) -> Result<SecretString, StoreError> {
        if !self.confirm {
            let entered = rpassword::prompt_password("Store passphrase: ")?;
            return Ok(SecretString::from(entered));
        }

        // New-store creation: warn, then confirm-twice-and-match (D-04).
        eprintln!(
            "WARNING: this passphrase encrypts your identity key and every share.\n\
             A lost passphrase makes them unrecoverable — there is no reset."
        );
        let first = rpassword::prompt_password("New store passphrase: ")?;
        let second = rpassword::prompt_password("Confirm store passphrase: ")?;
        if first != second {
            return Err(StoreError::Age("passphrases did not match".into()));
        }
        Ok(SecretString::from(first))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::crypto::{decrypt_secret, encrypt_secret};

    #[test]
    fn in_code_source_drives_roundtrip_headlessly() {
        let source = InCodePassphrase::new("ci-store-passphrase");
        let secret = b"headless share bytes";
        // The in-code source yields a passphrase with no prompt, and it drives
        // the crypto round-trip end to end.
        let ciphertext = encrypt_secret(&source.passphrase().unwrap(), secret).unwrap();
        let plaintext = decrypt_secret(&source.passphrase().unwrap(), &ciphertext).unwrap();
        assert_eq!(plaintext.as_slice(), secret.as_slice());
    }
}
