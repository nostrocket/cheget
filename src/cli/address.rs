//! `address` command — print the group's BIP341 P2TR address (KEY-04).
//!
//! Reads a **public-artifact** file (D-09): a frost `PublicKeyPackage` serialized
//! to its canonical bytes and wrapped in a small `serde_json` envelope carrying a
//! `key_id` and a reserved `epoch` (used from Phase 4). No secret material is ever
//! read or written. The address is derived through the ONE canonical bridge
//! (`bridge::address_from_group_key`), so the CLI and the KEY-03 KAT agree by
//! construction.

use std::path::Path;

use bitcoin::KnownHrp;
use clap::{Args, ValueEnum};
use frost_secp256k1_tr as frost;
use serde::{Deserialize, Serialize};

use super::CliResult;
use crate::bridge::{address_from_group_key, BridgeError};

/// Bitcoin network selector for address rendering (maps to a bech32m HRP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Network {
    /// Mainnet (`bc1p...`).
    Bitcoin,
    /// Testnet / testnet4 (`tb1p...`).
    Testnet,
    /// Signet (`tb1p...`).
    Signet,
    /// Regtest (`bcrt1p...`).
    Regtest,
}

impl Network {
    /// Map to rust-bitcoin's known-HRP for `Address::p2tr`.
    pub fn known_hrp(self) -> KnownHrp {
        match self {
            Network::Bitcoin => KnownHrp::Mainnet,
            // testnet, testnet4 and signet all share the `tb` HRP.
            Network::Testnet | Network::Signet => KnownHrp::Testnets,
            Network::Regtest => KnownHrp::Regtest,
        }
    }

    /// Map to rust-bitcoin's [`bitcoin::Network`] (used to decode addresses for
    /// the display gate).
    pub fn bitcoin_network(self) -> bitcoin::Network {
        match self {
            Network::Bitcoin => bitcoin::Network::Bitcoin,
            Network::Testnet => bitcoin::Network::Testnet,
            Network::Signet => bitcoin::Network::Signet,
            Network::Regtest => bitcoin::Network::Regtest,
        }
    }
}

/// The public-artifact envelope (D-09).
///
/// The `PublicKeyPackage` is stored as hex of its canonical frost `serialize()`
/// bytes so the file is stable and human-inspectable. This carries only PUBLIC
/// data — never a share, nonce, or signing key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubkeyEnvelope {
    /// Stable identifier for this group key.
    pub key_id: String,
    /// Refresh epoch. Reserved for Phase 4; always 0 in Phase 1.
    #[serde(default)]
    pub epoch: u64,
    /// Hex of the frost `PublicKeyPackage` canonical (postcard) serialization.
    pub pubkey_package_hex: String,
}

/// Errors specific to reading/decoding the public-artifact envelope.
#[derive(Debug)]
pub enum EnvelopeError {
    /// The envelope's hex payload was malformed.
    Hex,
    /// The decoded bytes were not a valid `PublicKeyPackage`.
    Package(frost::Error),
    /// The frost package failed to serialize.
    Serialize(frost::Error),
    /// Bridging the group key to an address failed.
    Bridge(BridgeError),
    /// I/O reading the artifact file.
    Io(std::io::Error),
    /// The JSON envelope failed to parse.
    Json(serde_json::Error),
}

impl std::fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvelopeError::Hex => write!(f, "malformed hex in pubkey_package_hex"),
            EnvelopeError::Package(e) => write!(f, "invalid PublicKeyPackage: {e}"),
            EnvelopeError::Serialize(e) => write!(f, "failed to serialize package: {e}"),
            EnvelopeError::Bridge(e) => write!(f, "{e}"),
            EnvelopeError::Io(e) => write!(f, "reading pubkey file: {e}"),
            EnvelopeError::Json(e) => write!(f, "parsing pubkey envelope: {e}"),
        }
    }
}

impl std::error::Error for EnvelopeError {}

impl PubkeyEnvelope {
    /// Wrap a `PublicKeyPackage` (public data only) into an envelope.
    pub fn from_package(
        key_id: impl Into<String>,
        epoch: u64,
        pkg: &frost::keys::PublicKeyPackage,
    ) -> Result<Self, EnvelopeError> {
        let bytes = pkg.serialize().map_err(EnvelopeError::Serialize)?;
        Ok(Self {
            key_id: key_id.into(),
            epoch,
            pubkey_package_hex: hex_encode(&bytes),
        })
    }

    /// Decode the wrapped `PublicKeyPackage`.
    pub fn decode_package(&self) -> Result<frost::keys::PublicKeyPackage, EnvelopeError> {
        let bytes = hex_decode(&self.pubkey_package_hex).ok_or(EnvelopeError::Hex)?;
        frost::keys::PublicKeyPackage::deserialize(&bytes).map_err(EnvelopeError::Package)
    }
}

/// Read a public-artifact file and derive its BIP341 P2TR address for `network`.
pub fn address_from_pubkey_file(
    path: &Path,
    network: Network,
) -> Result<bitcoin::Address, EnvelopeError> {
    let raw = std::fs::read(path).map_err(EnvelopeError::Io)?;
    let envelope: PubkeyEnvelope = serde_json::from_slice(&raw).map_err(EnvelopeError::Json)?;
    let pkg = envelope.decode_package()?;
    address_from_group_key(pkg.verifying_key(), network.known_hrp())
        .map_err(EnvelopeError::Bridge)
}

/// Arguments for the address command.
#[derive(Debug, Args)]
pub struct AddressArgs {
    /// Path to a public-artifact file (serialized `PublicKeyPackage` envelope).
    #[arg(long)]
    pub pubkey: std::path::PathBuf,
    /// Network to render the address for.
    #[arg(long, value_enum, default_value_t = Network::Bitcoin)]
    pub network: Network,
}

/// Handler: print the P2TR address for the given public-artifact file.
pub fn run(args: AddressArgs) -> CliResult {
    let addr = address_from_pubkey_file(&args.pubkey, args.network)?;
    println!("{addr}");
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}
