//! L0 crypto core — **pure** wrapper over `frost-secp256k1-tr` 3.0.
//!
//! Placeholder module seam. Filled by plan **01-02**:
//! - in-process DKG (`keys::dkg::part1/2/3`) + even-Y normalization (KEY-01/02),
//! - two-round tweaked signing (`round1::commit`, `round2::sign_with_tweak`,
//!   `aggregate_with_tweak(.., None)`) (SIGN-02/03),
//! - the non-serializable `EphemeralNonces` newtype (SIGN-05).
//!
//! This module MUST NOT gain chain/transport/filesystem dependencies — it is part
//! of the small auditable trusted computing base.
