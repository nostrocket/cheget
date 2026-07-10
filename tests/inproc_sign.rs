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

mod common;

use std::collections::BTreeMap;
use std::str::FromStr;

use bitcoin::absolute::LockTime;
use bitcoin::hashes::Hash;
use bitcoin::transaction::Version;
use bitcoin::{
    Amount, KnownHrp, Network, OutPoint, Psbt, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    Witness,
};
use frost_secp256k1_tr as frost;
use frost::{Identifier, SigningPackage};

use tsig::bridge::address_from_group_key;
use tsig::crypto::sign::{aggregate, verify_against_q, AggregateError};
use tsig::crypto::{run_inprocess_dkg, EphemeralNonces};
use tsig::session::display::{display_and_ack, DisplayError};
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

#[test]
fn round2_run_signs_and_verifies_against_q_not_p() {
    let (key_packages, group) = run_inprocess_dkg(3, 5).expect("3-of-5 DKG");
    let psbt = group_spending_psbt(&group);
    let transport = InMemoryTransport::new();
    let mut session =
        SigningSession::new("sess-run", &transport, key_packages, group, psbt, 3, Network::Regtest);

    // --yes: the human ack is bypassed for automation, but the blind-sign
    // recompute check inside the gate still runs.
    let signed = session.run(true).expect("full two-round session run");

    // The finalized key-spend witness is the single 64-byte BIP340 signature.
    let witness = signed.input[0].witness.to_vec();
    assert_eq!(witness.len(), 1, "key-spend witness is one element");
    assert_eq!(witness[0].len(), 64, "BIP340 signature is 64 bytes");

    let sig = frost::Signature::deserialize(&witness[0]).expect("valid signature encoding");
    let sighash = session.sighash(0).expect("sighash");

    // SIGN-04: verifies against the tweaked output key Q ...
    verify_against_q(&sig, sighash.as_byte_array(), session.group())
        .expect("aggregate MUST verify against Q");
    // ... and MUST NOT verify against the internal key P.
    assert!(
        session
            .group()
            .verifying_key()
            .verify(sighash.as_byte_array(), &sig)
            .is_err(),
        "aggregate must NOT verify against internal key P (Pitfall 7)"
    );
}

#[test]
fn round2_display_gate_refuses_blind_sign() {
    let (key_packages, group) = run_inprocess_dkg(3, 5).expect("3-of-5 DKG");
    let psbt = group_spending_psbt(&group);
    let transport = InMemoryTransport::new();
    let session =
        SigningSession::new("sess-disp", &transport, key_packages, group, psbt, 3, Network::Regtest);

    let prevouts = session.prevouts().unwrap();
    let tx = session.unsigned_tx().clone();
    let good = session.sighash(0).unwrap();

    // A coordinator hash that matches the PSBT recompute is accepted.
    display_and_ack(&tx, &prevouts, 0, good.as_byte_array(), true, Network::Regtest)
        .expect("a matching sighash acks");

    // A coordinator-supplied hash that disagrees with the PSBT recompute is
    // refused — the seat recomputes from the PSBT and never trusts the sender.
    let tampered = [0xabu8; 32];
    let err = display_and_ack(&tx, &prevouts, 0, &tampered, true, Network::Regtest);
    assert!(
        matches!(err, Err(DisplayError::BlindSignMismatch { input_index: 0 })),
        "a mismatched coordinator sighash must be refused (SIGN-07): {err:?}"
    );
}

#[test]
fn round2_aggregate_surfaces_culprits_on_invalid_share() {
    let (key_packages, group) = run_inprocess_dkg(3, 5).expect("3-of-5 DKG");
    let selected: Vec<Identifier> = key_packages.keys().take(3).copied().collect();
    let mut rng = frost::rand_core::OsRng;

    // Round A (the package we will aggregate).
    let msg_a = [0x11u8; 32];
    let mut comm_a = BTreeMap::new();
    let mut nonce_a = BTreeMap::new();
    for id in &selected {
        let (n, c) = EphemeralNonces::commit(key_packages[id].signing_share(), &mut rng);
        comm_a.insert(*id, c);
        nonce_a.insert(*id, n);
    }
    let pkg_a = SigningPackage::new(comm_a, &msg_a);
    let mut shares_a = BTreeMap::new();
    for (id, n) in nonce_a {
        shares_a.insert(id, n.sign(&pkg_a, &key_packages[&id]).unwrap());
    }

    // Round B: independent nonces / different message → shares invalid under A.
    let msg_b = [0x22u8; 32];
    let mut comm_b = BTreeMap::new();
    let mut nonce_b = BTreeMap::new();
    for id in &selected {
        let (n, c) = EphemeralNonces::commit(key_packages[id].signing_share(), &mut rng);
        comm_b.insert(*id, c);
        nonce_b.insert(*id, n);
    }
    let pkg_b = SigningPackage::new(comm_b, &msg_b);
    let mut shares_b = BTreeMap::new();
    for (id, n) in nonce_b {
        shares_b.insert(id, n.sign(&pkg_b, &key_packages[&id]).unwrap());
    }

    // Splice B's share for one seat into A's shares → that seat is a culprit.
    let bad = selected[0];
    let mut corrupted = shares_a.clone();
    corrupted.insert(bad, shares_b[&bad]);

    match aggregate(&pkg_a, &corrupted, &group) {
        Err(AggregateError::Culprits(culprits)) => {
            assert!(culprits.contains(&bad), "cheater must be flagged: {culprits:?}")
        }
        other => panic!("expected cheater-detection culprits, got {other:?}"),
    }

    // The clean set aggregates and verifies against Q.
    let sig = aggregate(&pkg_a, &shares_a, &group).expect("clean aggregate");
    verify_against_q(&sig, &msg_a, &group).expect("clean signature verifies against Q");
}

#[test]
fn round2_abort_starts_new_session_with_fresh_nonces() {
    let (key_packages, group) = run_inprocess_dkg(3, 5).expect("3-of-5 DKG");
    let psbt = group_spending_psbt(&group);
    let transport = InMemoryTransport::new();

    let s1 = SigningSession::new(
        "sess-a",
        &transport,
        key_packages,
        group,
        psbt,
        3,
        Network::Regtest,
    );
    let sel1 = s1.liveness_select().unwrap();
    let r1a = s1.round1(0, &sel1).unwrap();

    // Simulate a timeout: abort to a NEW session (new id, fresh nonces).
    let s2 = s1.new_session_on_abort("sess-b");
    assert_ne!(s1.id(), s2.id(), "abort MUST start a new session id");
    let sel2 = s2.liveness_select().unwrap();
    let r1b = s2.round1(0, &sel2).unwrap();

    // The same seat commits DIFFERENT commitments across sessions — commitments
    // (and the nonces behind them) are never reused (SIGN-06, Pitfall 1).
    let id = sel1[0];
    let ca = r1a.signing_package().signing_commitment(&id);
    let cb = r1b.signing_package().signing_commitment(&id);
    assert!(ca.is_some() && cb.is_some());
    assert_ne!(ca, cb, "a new session MUST use fresh nonces, never reuse commitments");
}

#[test]
fn inproc_sign_confirmed_regtest_key_spend_small_n() {
    // The PR gate (D-06): a full 3-of-5 in-process signing session produces a
    // CONFIRMED regtest key-spend end-to-end (SIGN-04 crown jewel at small n).
    common::run_confirmed_key_spend(3, 5);
}
