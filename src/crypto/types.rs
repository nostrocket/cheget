//! Small tagging newtypes for crypto artifacts — `(key_id, epoch, seat)`.
//!
//! These carry no secret material; they exist so downstream layers (the public
//! artifact envelope now, rotation/refresh in Phase 4) can tag a group key and a
//! seat without stringly-typed confusion. Kept in the **pure** crypto core.

use frost_secp256k1_tr as frost;

/// Stable identifier for a group key within a deployment (e.g. `"active"`,
/// `"standby"`). Purely a label; the cryptographic identity is the
/// `PublicKeyPackage`'s verifying key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyId(pub String);

impl KeyId {
    /// The conventional identifier for the primary group key.
    pub fn active() -> Self {
        Self("active".to_string())
    }
}

impl From<&str> for KeyId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for KeyId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Display for KeyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

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
