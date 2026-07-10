//! L0 crypto core — **pure** wrapper over `frost-secp256k1-tr` 3.0.
//!
//! Wired by plan **01-02**:
//! - the non-serializable [`EphemeralNonces`] newtype ([`nonce`]) (SIGN-05),
//! - in-process DKG generic over `(t, n)` + even-Y normalization + client-side
//!   group-key confirmation (`keygen`, added in the same plan) (KEY-01/02/05/06),
//! - `(key_id, epoch, seat)` tagging newtypes (`types`).
//!
//! Two-round tweaked *aggregation* (the coordinator/session side) lands in
//! 01-04; the participant-side signing primitive (`EphemeralNonces::sign`) is
//! here so nonce discipline is enforced from the first line of signing code.
//!
//! This module MUST NOT gain chain/transport/filesystem dependencies — it is
//! part of the small auditable trusted computing base.

pub mod nonce;

pub use nonce::EphemeralNonces;
