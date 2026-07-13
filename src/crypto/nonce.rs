//! Non-serializable signing-nonce newtype — the single highest-severity
//! structural control in `tsig` (SIGN-05).
//!
//! Reusing or persisting a FROST signing nonce across two different sighashes
//! gives an adversary (including a malicious coordinator who observes every
//! partial) two linear equations in the same unknowns, and the signer's
//! long-lived share falls out. 51 extracted shares reconstruct the group key
//! forever. This is a *key-extraction* bug class, not a mere robustness issue
//! (PITFALLS Pitfall 1).
//!
//! [`EphemeralNonces`] makes that failure mode a **compile-time impossibility**:
//!
//! - it wraps [`frost::round1::SigningNonces`] in a move-only newtype that
//!   implements NEITHER `Serialize` NOR `Deserialize`, and derives no `Clone`
//!   that could let a nonce outlive the single round it was committed for;
//! - it holds its inner state in [`zeroize::Zeroizing`] so the nonce is wiped on
//!   drop (FROST 3.0 already makes `SigningNonces: ZeroizeOnDrop`; this is the
//!   same discipline, made explicit at the type boundary);
//! - it is created in round 1 by [`EphemeralNonces::commit`] and consumed **by
//!   value** in round 2 by [`EphemeralNonces::sign`], so the nonce is dropped
//!   the instant the signature share is produced and cannot be signed with
//!   twice.
//!
//! The proof that this type cannot be persisted lives in
//! `tests/ui/nonce_no_serialize.rs` (a `trybuild` compile-fail snapshot) — it is
//! the reviewable artifact, deliberately NOT a comment or assertion in this
//! module.

use frost_secp256k1_tr as frost;
use frost::keys::{KeyPackage, SigningShare};
use frost::rand_core::{CryptoRng, RngCore};
use frost::round1::{SigningCommitments, SigningNonces};
use frost::round2::SignatureShare;
use frost::{Error, SigningPackage};
use zeroize::Zeroizing;

/// Move-only, non-serializable wrapper over FROST round-1 signing nonces.
///
/// Deliberately implements no serialization trait and no `Clone`: a nonce must
/// never cross a persistence boundary and must never outlive the single signing
/// round it was committed for (SIGN-05).
pub struct EphemeralNonces(Zeroizing<SigningNonces>);

impl EphemeralNonces {
    /// Round 1: generate fresh signing nonces and their public commitments by
    /// wrapping [`frost::round1::commit`].
    ///
    /// The returned [`SigningCommitments`] are public and are what a participant
    /// sends to the coordinator; the [`EphemeralNonces`] stay in memory only and
    /// are never serialized.
    pub fn commit<R: RngCore + CryptoRng>(
        share: &SigningShare,
        rng: &mut R,
    ) -> (Self, SigningCommitments) {
        let (nonces, commitments) = frost::round1::commit(share, rng);
        (Self(Zeroizing::new(nonces)), commitments)
    }

    /// Round 2: consume the nonces **by value** and produce a BIP-341 tweaked
    /// signature share via [`frost::round2::sign_with_tweak`] with
    /// `merkle_root = None` (BIP86 key-only spend).
    ///
    /// Taking `self` by value is the load-bearing invariant: the nonce is
    /// dropped (and zeroized) the moment the share is produced, so it is
    /// structurally impossible to reuse it against a second sighash.
    pub fn sign(
        self,
        signing_package: &SigningPackage,
        key_package: &KeyPackage,
    ) -> Result<SignatureShare, Error> {
        frost::round2::sign_with_tweak(signing_package, &self.0, key_package, None)
    }
}
