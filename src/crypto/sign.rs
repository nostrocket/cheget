//! Coordinator-side tweaked aggregation + verify-against-`Q` (SIGN-03, SIGN-04, SIGN-06).
//!
//! This is the ONLY aggregation path exposed to application code, and it is
//! **tweaked-only**: it wires [`frost::aggregate_with_tweak`] with
//! `merkle_root = None` (BIP86 key-only) and verifies the result against the
//! tweaked **output** key `Q`. The untweaked [`frost::aggregate`] and a
//! `Some(merkle_root)` variant are deliberately never surfaced — mixing tweaked
//! and untweaked paths, or verifying against the internal key `P`, silently
//! yields an invalid or wrong-key signature that "passes" a naive unit test but
//! is rejected on-chain (PITFALLS Pitfall 7).
//!
//! FROST 3.0 runs cheater detection by default, so a bad share makes aggregation
//! return the identifiable culprits ([`frost::Error::culprits`]); this module
//! surfaces them as [`AggregateError::Culprits`] (SIGN-06).

use std::collections::BTreeMap;

use frost_secp256k1_tr as frost;
use frost::keys::PublicKeyPackage;
use frost::round2::SignatureShare;
use frost::{Identifier, Signature, SigningPackage};

use crate::bridge::output_key_q;

/// Errors from tweaked aggregation / verification.
#[derive(Debug)]
pub enum AggregateError {
    /// Cheater detection identified one or more misbehaving signers (SIGN-06).
    /// The session must abort and start anew, excluding these seats.
    Culprits(Vec<Identifier>),
    /// A `frost` aggregation error with no identifiable culprit.
    Frost(frost::Error),
    /// The aggregated signature did not verify against the output key `Q`.
    VerifyAgainstQ,
    /// The aggregated signature was not the expected 64-byte BIP340 encoding.
    Length(usize),
}

impl std::fmt::Display for AggregateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AggregateError::Culprits(c) => {
                write!(f, "aggregation failed; cheater-detection culprits: {c:?}")
            }
            AggregateError::Frost(e) => write!(f, "frost aggregation error: {e}"),
            AggregateError::VerifyAgainstQ => {
                write!(f, "aggregated signature did not verify against the output key Q")
            }
            AggregateError::Length(n) => {
                write!(f, "aggregated signature was {n} bytes, expected 64 (BIP340)")
            }
        }
    }
}

impl std::error::Error for AggregateError {}

/// Aggregate signature shares into a single BIP340 signature using the BIP-341
/// Taproot tweak (`merkle_root = None`, BIP86 key-only).
///
/// On a bad share, returns [`AggregateError::Culprits`] carrying the seats that
/// cheater detection flagged (SIGN-06).
pub fn aggregate(
    signing_package: &SigningPackage,
    shares: &BTreeMap<Identifier, SignatureShare>,
    pubkeys: &PublicKeyPackage,
) -> Result<Signature, AggregateError> {
    frost::aggregate_with_tweak(signing_package, shares, pubkeys, None::<&[u8]>).map_err(|e| {
        let culprits = e.culprits();
        if culprits.is_empty() {
            AggregateError::Frost(e)
        } else {
            AggregateError::Culprits(culprits)
        }
    })
}

/// Verify `sig` over `message` against the tweaked **output** key `Q`
/// (`bridge::output_key_q`), never the internal key `P` (SIGN-04, Pitfall 7).
pub fn verify_against_q(
    sig: &Signature,
    message: &[u8],
    pubkeys: &PublicKeyPackage,
) -> Result<(), AggregateError> {
    let q = output_key_q(pubkeys);
    q.verify(message, sig).map_err(|_| AggregateError::VerifyAgainstQ)
}

/// Serialize an aggregated signature to the 64-byte BIP340 encoding used as the
/// Taproot key-spend witness.
pub fn signature_bytes(sig: &Signature) -> Result<[u8; 64], AggregateError> {
    let bytes = sig.serialize().map_err(AggregateError::Frost)?;
    let len = bytes.len();
    bytes.try_into().map_err(|_| AggregateError::Length(len))
}
