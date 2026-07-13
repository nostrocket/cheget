//! KEY-03 known-answer test harness for the frost->rust-bitcoin bridge.
//!
//! Pins the bridge to the official BIP341/BIP86 published vectors by asserting a
//! HARD-CODED expected P2TR address string (never merely "it runs"), for BOTH an
//! even-Y and an odd-Y-origin key (D-10/D-11). Also proves KEY-04: the
//! `tsig address --pubkey <file>` path reads a public-artifact envelope and prints
//! the same address the KAT pins.

use std::collections::BTreeMap;

use bitcoin::KnownHrp;
use frost_secp256k1_tr as frost;
use frost::keys::EvenY;
use serde::Deserialize;

use tsig::bridge::{address_from_group_key, BridgeError};
use tsig::cli::address::{self, Network, PubkeyEnvelope};

#[derive(Debug, Deserialize)]
struct Fixture {
    vectors: Vec<Vector>,
}

#[derive(Debug, Deserialize)]
struct Vector {
    name: String,
    parity: String,
    compressed_sec1: String,
    #[serde(default)]
    expected_address: Option<String>,
    #[serde(default)]
    expected_reject: Option<String>,
    #[serde(default)]
    expected_address_after_normalize: Option<String>,
}

fn load_fixture() -> Fixture {
    let raw = include_str!("vectors/bip341_keyspend.json");
    serde_json::from_str(raw).expect("fixture parses")
}

fn hex_decode(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "odd hex length");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("valid hex"))
        .collect()
}

fn vk_from_hex(compressed_sec1: &str) -> frost::VerifyingKey {
    let bytes = hex_decode(compressed_sec1);
    frost::VerifyingKey::deserialize(&bytes).expect("valid compressed secp256k1 point")
}

/// Even-Y vector: the bridge yields the EXACT published BIP341/BIP86 address.
#[test]
fn even_y_vector_matches_published_address() {
    let fixture = load_fixture();
    let v = fixture
        .vectors
        .iter()
        .find(|v| v.parity == "even")
        .expect("an even-Y vector exists");

    let vk = vk_from_hex(&v.compressed_sec1);
    assert!(vk.has_even_y(), "{}: expected even-Y", v.name);

    let addr = address_from_group_key(&vk, KnownHrp::Mainnet)
        .expect("even-Y key bridges to an address");
    let expected = v.expected_address.as_ref().expect("even vector has address");
    assert_eq!(&addr.to_string(), expected, "{}", v.name);
}

/// Odd-Y-origin vector: the raw key is REJECTED with `OddY` (defensive parity
/// invariant, D-11), and after even-Y normalization it bridges to the same
/// published address (normalization preserves the x-coordinate).
#[test]
fn odd_y_origin_is_rejected_then_normalizes_to_published_address() {
    let fixture = load_fixture();
    let v = fixture
        .vectors
        .iter()
        .find(|v| v.parity == "odd")
        .expect("an odd-Y-origin vector exists");

    let vk = vk_from_hex(&v.compressed_sec1);
    assert!(!vk.has_even_y(), "{}: expected odd-Y origin", v.name);

    // Defensive rejection — never blindly strip the SEC1 prefix.
    match address_from_group_key(&vk, KnownHrp::Mainnet) {
        Err(BridgeError::OddY) => {}
        other => panic!("{}: expected Err(OddY), got {other:?}", v.name),
    }
    assert_eq!(v.expected_reject.as_deref(), Some("OddY"), "{}", v.name);

    // Normalize-to-even-Y then bridge => same published address.
    let normalized = vk.into_even_y(None);
    assert!(normalized.has_even_y());
    let addr = address_from_group_key(&normalized, KnownHrp::Mainnet)
        .expect("normalized key bridges");
    let expected = v
        .expected_address_after_normalize
        .as_ref()
        .expect("odd vector has post-normalize address");
    assert_eq!(&addr.to_string(), expected, "{}", v.name);
}

/// KEY-04: `tsig address --pubkey <file>` reads a public-artifact envelope
/// carrying a `PublicKeyPackage` and prints the SAME address the KAT pins.
#[test]
fn address_command_reads_pubkey_file_and_prints_vector_address() {
    let fixture = load_fixture();
    let v = fixture
        .vectors
        .iter()
        .find(|v| v.parity == "even")
        .expect("an even-Y vector exists");
    let expected = v.expected_address.as_ref().unwrap();

    // Build a PublicKeyPackage whose group verifying key is the vector's even-Y
    // key (empty verifying-shares map is fine: address derivation only reads the
    // group verifying key).
    let vk = vk_from_hex(&v.compressed_sec1);
    let shares = BTreeMap::new();
    let pkg = frost::keys::PublicKeyPackage::new(shares, vk, Some(51));

    let envelope =
        PubkeyEnvelope::from_package("test-key", 0, &pkg).expect("envelope encodes");

    let dir = std::env::temp_dir().join(format!("tsig-kat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("pubkey.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();

    let addr = address::address_from_pubkey_file(&path, Network::Bitcoin)
        .expect("address command reads the file");
    assert_eq!(&addr.to_string(), expected);

    let _ = std::fs::remove_dir_all(&dir);
}
