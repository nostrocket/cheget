//! L0 crypto core — **pure** wrapper over `frost-secp256k1-tr` 3.0.
//!
//! Wired by plan **01-02**:
//! - the non-serializable [`EphemeralNonces`] newtype ([`nonce`]) (SIGN-05),
//! - in-process DKG generic over `(t, n)` + even-Y normalization + client-side
//!   group-key confirmation (`keygen`, added in the same plan) (KEY-01/02/05/06),
//! - `(key_id, epoch, seat)` tagging newtypes (`types`).
//!
//! Two-round tweaked *aggregation* (the coordinator side) lives in [`sign`]
//! (added in 01-04): `aggregate_with_tweak(.., None)` + verify-against-`Q`, the
//! only aggregation path exposed to app code. The participant-side signing
//! primitive (`EphemeralNonces::sign`) enforces nonce discipline from the first
//! line of signing code.
//!
//! This module MUST NOT gain chain/transport/filesystem dependencies — it is
//! part of the small auditable trusted computing base.

pub mod keygen;
pub mod nonce;
pub mod sign;
pub mod types;

pub use keygen::{
    confirm_group_key, run_inprocess_dkg, run_inprocess_dkg_with_rng, KeygenError,
};
pub use nonce::EphemeralNonces;
pub use sign::{aggregate, signature_bytes, verify_against_q, AggregateError};
pub use types::{Epoch, KeyId, SeatId};
