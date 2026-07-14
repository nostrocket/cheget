//! Store manifest — the plaintext index of a participant's encrypted shares.
//!
//! **D-05:** the manifest indexes ONLY the age-encrypted `KeyPackage` shares.
//! The transport identity key and the per-`(key_id, epoch)` public
//! `PubkeyEnvelope` live at well-known paths (`identity.age`,
//! `pubkey/<key_id>/epoch-<N>.json`) and are deliberately NOT listed here — they
//! are discoverable without the manifest so `address` / `share status` work with
//! no unlock.
//!
//! **D-07:** the manifest is always written LAST, after a share's ciphertext is
//! durably on disk, so it can never point at a half-written share.
//!
//! **Forward-compat (RESEARCH V5):** every manifest carries a `schema_version`; a
//! newer/unknown version is rejected on load rather than silently misparsed — the
//! same role `PubkeyEnvelope`'s `#[serde(default)] epoch` plays for the public
//! artifact. The tag tuple reuses [`crate::crypto::types`] (`KeyId`, `Epoch`,
//! `SeatId`) rather than reinventing stringly-typed keys (D-02).

use serde::{Deserialize, Serialize};

use super::StoreError;
use crate::crypto::types::{Epoch, KeyId, SeatId};

/// The manifest schema version this build writes, and the newest it accepts.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Lifecycle state of a stored share (mirrors ROT-06 / LIFE-03).
///
/// Serialized as the uppercase tokens `ACTIVE` / `STANDBY` / `RETIRED` so the
/// on-disk manifest matches the coordinator roster's `status` vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShareState {
    /// A live share in the current signing set.
    #[serde(rename = "ACTIVE")]
    Active,
    /// A pre-generated standby share held for revocation/rotation.
    #[serde(rename = "STANDBY")]
    Standby,
    /// A superseded share kept only for audit until pruning (Phase 4).
    #[serde(rename = "RETIRED")]
    Retired,
}

/// One entry indexing an age-encrypted share by its `(key_id, epoch, seat)` tag.
///
/// `seat` is the lowercase hex of the frost `Identifier.serialize()` bytes — the
/// same stable identifier the coordinator roster keys on (survives refresh,
/// Pitfall 16). The tag is duplicated inside the encrypted payload (D-02); this
/// entry is the plaintext index over those payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareEntry {
    /// Stable group-key label (e.g. `"active"`, `"standby"`).
    pub key_id: String,
    /// Refresh epoch the share belongs to.
    pub epoch: u64,
    /// Lowercase hex of the frost `Identifier` for this seat.
    pub seat: String,
    /// Lifecycle state of the share.
    pub state: ShareState,
    /// Creation time, unix seconds.
    pub created_at: u64,
}

impl ShareEntry {
    /// Build an entry from the canonical tag newtypes (D-02) plus lifecycle
    /// state and a creation timestamp (unix seconds).
    pub fn new(
        key_id: &KeyId,
        epoch: Epoch,
        seat: &SeatId,
        state: ShareState,
        created_at: u64,
    ) -> Self {
        Self {
            key_id: key_id.as_str().to_string(),
            epoch: epoch.0,
            seat: seat_hex(seat),
            state,
            created_at,
        }
    }

    /// True if this entry carries exactly the given `(key_id, epoch, seat)` tag.
    fn matches(&self, key_id: &KeyId, epoch: Epoch, seat: &SeatId) -> bool {
        self.key_id == key_id.as_str() && self.epoch == epoch.0 && self.seat == seat_hex(seat)
    }
}

/// Lowercase hex of a seat identifier's canonical serialization.
///
/// This is the on-disk/on-manifest spelling of a [`SeatId`]; it matches the
/// coordinator roster's `identifier` column so the two indices agree.
pub fn seat_hex(seat: &SeatId) -> String {
    let bytes = seat.serialize();
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in &bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// The plaintext share index (D-05).
///
/// Holds a `schema_version` for forward-compat and a flat list of
/// [`ShareEntry`]. Multiple epochs coexist in the list at once (ROT-06 steady
/// state ≈ 2) — old epochs are not removed here; active pruning is Phase 4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// On-disk schema version (see [`CURRENT_SCHEMA_VERSION`]).
    pub schema_version: u32,
    /// The indexed encrypted shares.
    #[serde(default)]
    pub shares: Vec<ShareEntry>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new()
    }
}

impl Manifest {
    /// A fresh, empty manifest tagged with the current schema version.
    pub fn new() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            shares: Vec::new(),
        }
    }

    /// Serialize to pretty JSON bytes for on-disk storage.
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, StoreError> {
        Ok(serde_json::to_vec_pretty(self)?)
    }

    /// Parse a manifest, rejecting an unknown/newer `schema_version` (V5).
    ///
    /// A `schema_version` of `0` or greater than [`CURRENT_SCHEMA_VERSION`]
    /// returns [`StoreError::Schema`] rather than a silent misparse.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, StoreError> {
        let manifest: Manifest = serde_json::from_slice(bytes)?;
        if manifest.schema_version == 0 || manifest.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(StoreError::Schema(format!(
                "manifest schema_version {} is unknown to this build \
                 (supports 1..={CURRENT_SCHEMA_VERSION})",
                manifest.schema_version
            )));
        }
        Ok(manifest)
    }

    /// Insert an entry, replacing any existing entry with the same tag so the
    /// index never carries a duplicate `(key_id, epoch, seat)`.
    pub fn add_entry(&mut self, entry: ShareEntry) {
        if let Some(existing) = self
            .shares
            .iter_mut()
            .find(|e| e.key_id == entry.key_id && e.epoch == entry.epoch && e.seat == entry.seat)
        {
            *existing = entry;
        } else {
            self.shares.push(entry);
        }
    }

    /// Find the entry carrying `(key_id, epoch, seat)`, if any.
    pub fn lookup(&self, key_id: &KeyId, epoch: Epoch, seat: &SeatId) -> Option<&ShareEntry> {
        self.shares.iter().find(|e| e.matches(key_id, epoch, seat))
    }

    /// Remove the entry carrying `(key_id, epoch, seat)`; returns `true` if one
    /// was removed.
    pub fn remove(&mut self, key_id: &KeyId, epoch: Epoch, seat: &SeatId) -> bool {
        let before = self.shares.len();
        self.shares.retain(|e| !e.matches(key_id, epoch, seat));
        self.shares.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seat(i: u16) -> SeatId {
        i.try_into().expect("valid frost identifier")
    }

    #[test]
    fn tags() {
        let mut manifest = Manifest::new();
        let entry = ShareEntry::new(
            &KeyId::active(),
            Epoch::GENESIS,
            &seat(7),
            ShareState::Active,
            1_752_000_000,
        );
        manifest.add_entry(entry.clone());

        // Serialize → deserialize preserves (key_id, epoch, seat, state, created_at).
        let bytes = manifest.to_json_bytes().unwrap();
        let back = Manifest::from_json_bytes(&bytes).unwrap();
        assert_eq!(back, manifest, "manifest must roundtrip byte-equal");
        assert_eq!(back.shares[0], entry, "the entry tags must be preserved exactly");

        // A second epoch coexists; adding the same tag replaces, not duplicates.
        manifest.add_entry(ShareEntry::new(
            &KeyId::active(),
            Epoch::GENESIS.next(),
            &seat(7),
            ShareState::Standby,
            1_752_000_100,
        ));
        assert_eq!(manifest.shares.len(), 2, "distinct epochs coexist");
        manifest.add_entry(ShareEntry::new(
            &KeyId::active(),
            Epoch::GENESIS,
            &seat(7),
            ShareState::Retired,
            1_752_000_200,
        ));
        assert_eq!(manifest.shares.len(), 2, "same tag replaces in place");

        // Lookup finds; remove drops.
        assert!(manifest
            .lookup(&KeyId::active(), Epoch::GENESIS, &seat(7))
            .is_some());
        assert!(manifest.remove(&KeyId::active(), Epoch::GENESIS, &seat(7)));
        assert!(manifest
            .lookup(&KeyId::active(), Epoch::GENESIS, &seat(7))
            .is_none());
        assert!(
            !manifest.remove(&KeyId::active(), Epoch::GENESIS, &seat(7)),
            "removing an absent tag returns false"
        );
    }

    #[test]
    fn rejects_unknown_schema_version() {
        // A newer schema_version must be rejected, not silently misparsed (V5).
        let newer = br#"{"schema_version": 9999, "shares": []}"#;
        assert!(matches!(
            Manifest::from_json_bytes(newer),
            Err(StoreError::Schema(_))
        ));

        // schema_version 0 is likewise unknown.
        let zero = br#"{"schema_version": 0, "shares": []}"#;
        assert!(matches!(
            Manifest::from_json_bytes(zero),
            Err(StoreError::Schema(_))
        ));

        // The current version parses.
        let ok = format!(r#"{{"schema_version": {CURRENT_SCHEMA_VERSION}, "shares": []}}"#);
        assert!(Manifest::from_json_bytes(ok.as_bytes()).is_ok());
    }
}
