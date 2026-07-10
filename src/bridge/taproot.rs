//! The ONE canonical frost→rust-bitcoin key seam (KEY-03, KEY-04).
//!
//! This module is the single place in the crate where FROST key bytes cross into
//! rust-bitcoin key types. Three convention systems collide here — frost
//! serialization (33-byte compressed SEC1), BIP340 x-only/parity, and the BIP341
//! taproot tweak — and a silent error yields an unspendable or wrong-key
//! signature. It is therefore:
//!
//! - **the only** caller of [`bitcoin::XOnlyPublicKey::from_slice`] in the crate
//!   (enforced by the KEY-03 test harness), and
//! - **defensive about parity** (D-11): a non-even-Y group key is rejected with
//!   [`BridgeError::OddY`] rather than having its SEC1 prefix blindly stripped.
//!
//! Pinned end-to-end by `tests/bridge_roundtrip.rs` against the official
//! BIP341/BIP86 known-answer vectors for both an even-Y and an odd-Y-origin key.
//!
//! The module is **pure**: no chain, transport, or filesystem imports.

use bitcoin::secp256k1::Secp256k1;
use bitcoin::taproot::TapNodeHash;
use bitcoin::{Address, KnownHrp, XOnlyPublicKey};
use frost_secp256k1_tr as frost;
use frost::keys::{EvenY, Tweak};

/// Errors from the frost→rust-bitcoin key bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// The group verifying key does not have an even Y coordinate. The crypto
    /// core normalizes to even-Y after DKG (`into_even_y`); reaching the bridge
    /// with an odd-Y key is a contract violation, not something to paper over by
    /// stripping the SEC1 prefix (D-11).
    OddY,
    /// The serialized verifying key was not the expected 33-byte compressed form.
    Len(usize),
    /// The 32-byte x-only slice was not a valid secp256k1 x-only public key.
    Secp(bitcoin::secp256k1::Error),
    /// The frost `VerifyingKey` failed to serialize.
    Serialize,
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::OddY => write!(
                f,
                "group verifying key is not even-Y (parity invariant violated; \
                 normalize with into_even_y before bridging)"
            ),
            BridgeError::Len(n) => {
                write!(f, "serialized verifying key was {n} bytes, expected 33")
            }
            BridgeError::Secp(e) => write!(f, "invalid x-only public key: {e}"),
            BridgeError::Serialize => write!(f, "failed to serialize verifying key"),
        }
    }
}

impl std::error::Error for BridgeError {}

/// Convert a FROST group verifying key (the Taproot **internal** key `P`) into
/// the BIP341 key-path P2TR address for `hrp`'s network.
///
/// With `merkle_root = None`, `Address::p2tr` derives the BIP86 key-only output
/// key `Q = P + H_taproot(P)·G` and encodes the address that commits to it — so
/// the on-chain output is indistinguishable from single-sig.
///
/// Enforces the even-Y parity invariant (D-11): returns [`BridgeError::OddY`] if
/// `vk` is not even-Y, rather than blindly stripping the compressed-SEC1 prefix.
///
/// This function contains the crate's ONLY call to
/// [`XOnlyPublicKey::from_slice`].
pub fn address_from_group_key(
    vk: &frost::VerifyingKey,
    hrp: KnownHrp,
) -> Result<Address, BridgeError> {
    // D-11: defensive parity assertion — do NOT unconditionally strip the prefix.
    if !vk.has_even_y() {
        return Err(BridgeError::OddY);
    }

    // frost's `serialize()` returns the 33-byte compressed SEC1 encoding; with an
    // even-Y key the leading byte is 0x02 and the trailing 32 bytes are x-only.
    let sec1 = vk.serialize().map_err(|_| BridgeError::Serialize)?;
    if sec1.len() != 33 {
        return Err(BridgeError::Len(sec1.len()));
    }
    debug_assert_eq!(sec1[0], 0x02, "even-Y compressed SEC1 must start with 0x02");

    let xonly = XOnlyPublicKey::from_slice(&sec1[1..]).map_err(BridgeError::Secp)?;
    let secp = Secp256k1::verification_only();
    // merkle_root = None ⇒ BIP86 key-only output Q.
    Ok(Address::p2tr(&secp, xonly, None::<TapNodeHash>, hrp))
}

/// Derive the tweaked **output** key `Q` from the group public-key package.
///
/// Signatures are verified against `Q` (the key the address commits to), never
/// against the internal key `P` — verifying against `P` passes a naive unit test
/// but fails on-chain (Pitfall 7). Used by the signing session (01-04).
///
/// The package is normalized to even-Y first, then tweaked with `merkle_root =
/// None` (BIP86 key-only), mirroring `aggregate_with_tweak(.., None)`.
pub fn output_key_q(pubkeys: &frost::keys::PublicKeyPackage) -> frost::VerifyingKey {
    let tweaked = pubkeys.clone().into_even_y(None).tweak(None::<&[u8]>);
    *tweaked.verifying_key()
}
