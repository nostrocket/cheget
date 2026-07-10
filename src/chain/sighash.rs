//! BIP341 Taproot **key-spend** sighash helper (SIGN-01 support, STOR-04).
//!
//! This is the single canonical place the whole project computes the message a
//! FROST key-path signature commits to. It is reused verbatim by the coordinator
//! when it builds the `SigningPackage` **and** by every participant's client-side
//! recompute-before-sign gate (SIGN-07, wired in 01-04) — one helper, one result,
//! no divergence.
//!
//! Two invariants are hard-wired here and must never become parameters:
//!
//! * the sighash type is always [`TapSighashType::Default`] (BIP341/BIP340
//!   key-path), never a legacy transaction-wide sighash type, and
//! * the signature commits to the **entire** prevout set via
//!   [`Prevouts::All`] — Taproot signs all spent outputs, so a missing or
//!   reordered prevout silently changes the challenge `e`.
//!
//! Getting either wrong produces a signature that verifies in a naive unit test
//! but is rejected on-chain (see `.planning/research/PITFALLS.md`, Pitfall 2).

use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
use bitcoin::{TapSighash, Transaction, TxOut};

use super::ChainError;

/// Compute the BIP341 key-spend sighash for `input_index` of `tx`.
///
/// `prevouts` MUST contain the [`TxOut`] being spent by **every** input of `tx`,
/// in input order — Taproot commits to the full prevout set. The sighash type is
/// fixed to [`TapSighashType::Default`]; there is deliberately no way for a caller
/// to request any other type.
///
/// STOR-04 / SIGN-01.
pub fn key_spend_sighash(
    tx: &Transaction,
    input_index: usize,
    prevouts: &[TxOut],
) -> Result<TapSighash, ChainError> {
    let mut cache = SighashCache::new(tx);
    cache
        .taproot_key_spend_signature_hash(
            input_index,
            &Prevouts::All(prevouts),
            TapSighashType::Default,
        )
        .map_err(|e| ChainError::Sighash(e.to_string()))
}
