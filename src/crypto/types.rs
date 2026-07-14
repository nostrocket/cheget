//! Small tagging newtypes for crypto artifacts — `(key_id, epoch, seat)`.
//!
//! These carry no secret material; they exist so downstream layers (the public
//! artifact envelope now, rotation/refresh in Phase 4) can tag a group key and a
//! seat without stringly-typed confusion. Kept in the **pure** crypto core.

use frost_secp256k1_tr as frost;

/// Stable identifier for a group key within a deployment (e.g. `"active"`,
/// `"standby"`). Purely a label; the cryptographic identity is the
/// `PublicKeyPackage`'s verifying key.
///
/// Constructed through [`KeyId::new`] (or [`TryFrom`]), which validates that the
/// id is a single safe path component: non-empty and only ASCII alphanumerics,
/// `-`, and `_`. Downstream, a `KeyId` is joined directly into on-disk store
/// paths (`shares/<key_id>/…`, `pubkey/<key_id>/…`), so an unvalidated separator
/// or `..` payload would escape the store subtree — an arbitrary-location
/// read/write primitive (path-traversal tampering, T-02-12). Rejecting at
/// construction makes that invalid state non-representable (the inner field is
/// private), mirroring `CeremonyId::new`. Validation is a pure character check
/// and adds no filesystem dependency to the crypto core.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyId(String);

impl KeyId {
    /// The conventional identifier for the primary group key.
    pub fn active() -> Self {
        // `"active"` is a known-valid single path component.
        Self("active".to_string())
    }

    /// Build a key id, rejecting anything that is not a safe single path
    /// component.
    ///
    /// Allowed: a non-empty string of ASCII alphanumerics plus `-` and `_`. Any
    /// path separator (`/`, `\`), `.`/`..`, whitespace, or control/other byte is
    /// rejected with [`KeyIdError`] so a hostile id can never write outside the
    /// store subtree (T-02-12). Mirrors `CeremonyId::new`.
    pub fn new(id: impl Into<String>) -> Result<Self, KeyIdError> {
        let id = id.into();
        let ok = !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !ok {
            return Err(KeyIdError(id));
        }
        Ok(Self(id))
    }

    /// The validated id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for KeyId {
    type Error = KeyIdError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        KeyId::new(s)
    }
}

impl TryFrom<String> for KeyId {
    type Error = KeyIdError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        KeyId::new(s)
    }
}

impl std::fmt::Display for KeyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A [`KeyId`] was rejected because it is not a safe single path component.
///
/// Carries the offending id for diagnostics. Uses the repo's manual
/// `Debug`/`Display`/`Error` idiom (no `thiserror`), keeping the pure crypto core
/// dependency-free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyIdError(String);

impl std::fmt::Display for KeyIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "key_id {:?} is not a safe path component (must be non-empty [A-Za-z0-9_-])",
            self.0
        )
    }
}

impl std::error::Error for KeyIdError {}

/// Refresh epoch. `0` in Phase 1 (a single DKG); advanced by membership rotation
/// in Phase 4. The Taproot address is invariant across epochs (KEY-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Epoch(pub u64);

impl Epoch {
    /// The genesis epoch (the initial DKG output).
    pub const GENESIS: Epoch = Epoch(0);

    /// The next epoch after a refresh.
    pub fn next(self) -> Self {
        Epoch(self.0 + 1)
    }
}

impl std::fmt::Display for Epoch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A FROST participant identifier — one seat in the `(t, n)` group.
pub type SeatId = frost::Identifier;

#[cfg(test)]
mod tests {
    use super::*;

    /// A hostile `KeyId` (traversal / separator / dot payloads) is
    /// non-constructible, so it can never reach a store path join (T-02-12).
    /// Mirrors `checkpoint::tests::ceremony_id_rejects_traversal`.
    #[test]
    fn key_id_rejects_traversal() {
        for bad in [
            "",
            "..",
            "../..",
            "../../../../home/user/.ssh",
            "a/b",
            "../evil",
            "a\\b",
            "with space",
            "dot.dot",
            "/etc",
        ] {
            assert!(KeyId::new(bad).is_err(), "key_id {bad:?} must be rejected");
            assert!(
                KeyId::try_from(bad).is_err(),
                "key_id {bad:?} must be rejected via TryFrom"
            );
            assert!(
                KeyId::try_from(bad.to_string()).is_err(),
                "key_id {bad:?} must be rejected via TryFrom<String>"
            );
        }
        for good in ["active", "standby", "A_B-9", "key-1"] {
            assert!(KeyId::new(good).is_ok(), "key_id {good:?} must be accepted");
        }
        // The conventional primary-key label is a valid single path component.
        assert_eq!(KeyId::active().as_str(), "active");
    }
}
