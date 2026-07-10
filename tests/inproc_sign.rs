//! In-process FROST signing session — small-`n` PR gate (D-06).
//!
//! Grows across plan 01-04's tasks:
//! * Task 1 — the liveness poll + round-1 `SigningPackage`-from-PSBT-sighash gate
//!   (`round1_*` tests below);
//! * Task 2 — round-2 tweaked signing, aggregate-with-tweak, verify-against-`Q`,
//!   and cheater/timeout abort semantics;
//! * Task 3 — the end-to-end CONFIRMED regtest key-spend (crown jewel).
//!
//! The n=1000 variant lives in `tests/inproc_sign_1000.rs` (`#[ignore]`,
//! nightly); adversarial gates live in `tests/sign_adversarial.rs`.

use std::str::FromStr;

use bitcoin::absolute::LockTime;
use bitcoin::hashes::Hash;
use bitcoin::transaction::Version;
use bitcoin::{
    Amount, KnownHrp, Network, OutPoint, Psbt, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    Witness,
};
use frost_secp256k1_tr as frost;
use frost::Identifier;

use tsig::bridge::address_from_group_key;
use tsig::crypto::run_inprocess_dkg;
use tsig::session::liveness::{over_provisioned_poll_size, poll_and_select, LivenessError};
use tsig::session::SigningSession;
use tsig::transport::InMemoryTransport;

/// Build a self-consistent PSBT that spends a single (synthetic) output paying to
/// the group's own P2TR address, back to the same address minus a fee. Returns
/// the PSBT ready to feed a [`SigningSession`].
///
/// Used by the round-1 gate tests where no live chain is needed — the sighash is
/// a pure function of the transaction and its prevouts.
pub fn group_spending_psbt(group: &frost::keys::PublicKeyPackage) -> Psbt {
    let addr = address_from_group_key(group.verifying_key(), KnownHrp::Regtest)
        .expect("even-Y group key bridges to a regtest P2TR address");
    let spk: ScriptBuf = addr.script_pubkey();

    let funded_value = Amount::from_sat(100_000);
    let fee = Amount::from_sat(10_000);

    // A synthetic prevout paying to the group address.
    let prev_txid =
        Txid::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap();
    let outpoint = OutPoint { txid: prev_txid, vout: 0 };
    let prevout = TxOut { value: funded_value, script_pubkey: spk.clone() };

    let tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut { value: funded_value - fee, script_pubkey: spk }],
    };

    let mut psbt = Psbt::from_unsigned_tx(tx).expect("valid unsigned tx → PSBT");
    psbt.inputs[0].witness_utxo = Some(prevout);
    psbt
}

#[test]
fn round1_over_provisioned_poll_selects_exactly_t() {
    // The poll is over-provisioned (poll a margin above t); selection finalizes
    // exactly t (Pitfall 11).
    assert_eq!(over_provisioned_poll_size(3, 5), 4, "3-of-5 polls one spare");
    assert_eq!(
        over_provisioned_poll_size(501, 1000),
        552,
        "501-of-1000 polls a 51-seat margin"
    );
    assert_eq!(
        over_provisioned_poll_size(501, 510),
        510,
        "poll size is capped at the roster size n"
    );

    let pool: Vec<Identifier> = (1..=5u16).map(|i| i.try_into().unwrap()).collect();
    let selected = poll_and_select(&pool, 3).expect("select t from an over-provisioned pool");
    assert_eq!(selected.len(), 3, "finalize exactly t, never more");

    // Fewer than t responders → abort (never proceed with a short set).
    let err = poll_and_select(&pool[..2], 3);
    assert!(
        matches!(err, Err(LivenessError::InsufficientLiveSeats { needed: 3, got: 2 })),
        "a short poll must abort, not proceed: {err:?}"
    );
}

#[test]
fn round1_builds_signing_package_from_psbt_sighash() {
    let (key_packages, group) = run_inprocess_dkg(3, 5).expect("3-of-5 in-process DKG");
    let psbt = group_spending_psbt(&group);

    let transport = InMemoryTransport::new();
    let session = SigningSession::new(
        "sess-round1",
        &transport,
        key_packages,
        group,
        psbt,
        3,
        Network::Regtest,
    );

    // Liveness poll over the transport finalizes exactly t=3 from the n=5 roster.
    let selected = session.liveness_select().expect("liveness poll selects t seats");
    assert_eq!(selected.len(), 3, "liveness selection finalizes exactly t");

    // Round 1 collects commitments over the transport and binds the signing
    // package's message to the canonical key-spend sighash for the input.
    let r1 = session.round1(0, &selected).expect("round 1 over transport");
    assert_eq!(r1.committed_seats(), 3, "one commitment per selected seat");

    let sighash = session.sighash(0).expect("key-spend sighash");
    assert_eq!(
        r1.signing_package().message(),
        sighash.as_byte_array(),
        "the SigningPackage message MUST equal chain::key_spend_sighash (SIGN-01)"
    );
}
