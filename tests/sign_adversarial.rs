//! Adversarial gates for the signing session (SIGN-06, SIGN-07).
//!
//! Two runtime attacks the coordinator-untrusted design must defeat:
//!
//! * **(a) malicious coordinator / blind-sign (SIGN-07):** the coordinator hands
//!   a sighash that does not correspond to the PSBT the participant is asked to
//!   sign. The participant recomputes the sighash from the PSBT itself and
//!   refuses. `--yes` bypasses only the human ack, NOT this recompute check.
//! * **(b) nonce reuse (SIGN-05/06 runtime complement):** attempting to reuse the
//!   same round-1 `SigningCommitments`/nonces across two different sighashes is
//!   impossible — a spent session refuses to run again, and a fresh session
//!   produces brand-new commitments. This complements the 01-02 compile-fail
//!   proof (the nonce type is non-`Serialize` and consumed by value).

use std::str::FromStr;

use bitcoin::absolute::LockTime;
use bitcoin::hashes::Hash;
use bitcoin::transaction::Version;
use bitcoin::{
    Amount, KnownHrp, Network, OutPoint, Psbt, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    Witness,
};
use frost_secp256k1_tr as frost;

use tsig::bridge::address_from_group_key;
use tsig::crypto::run_inprocess_dkg;
use tsig::session::display::{display_and_ack, DisplayError};
use tsig::session::{SessionError, SigningSession};
use tsig::transport::InMemoryTransport;

/// A self-consistent PSBT spending a synthetic output paying to the group
/// address back to itself minus a fee — no live chain needed (the sighash is a
/// pure function of the tx + prevouts). Local to this lean, regtest-free binary.
fn group_spending_psbt(group: &frost::keys::PublicKeyPackage) -> Psbt {
    let addr = address_from_group_key(group.verifying_key(), KnownHrp::Regtest)
        .expect("even-Y group key bridges to a regtest P2TR address");
    let spk: ScriptBuf = addr.script_pubkey();
    let funded = Amount::from_sat(100_000);
    let fee = Amount::from_sat(10_000);
    let outpoint = OutPoint {
        txid: Txid::from_str(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap(),
        vout: 0,
    };
    let tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut { value: funded - fee, script_pubkey: spk.clone() }],
    };
    let mut psbt = Psbt::from_unsigned_tx(tx).expect("valid unsigned tx → PSBT");
    psbt.inputs[0].witness_utxo = Some(TxOut { value: funded, script_pubkey: spk });
    psbt
}

#[test]
fn malicious_coordinator_sighash_is_refused_even_with_yes() {
    let (key_packages, group) = run_inprocess_dkg(3, 5).expect("3-of-5 DKG");
    let psbt = group_spending_psbt(&group);
    let transport = InMemoryTransport::new();
    let session = SigningSession::new(
        "adv-blindsign",
        &transport,
        key_packages,
        group,
        psbt,
        3,
        Network::Regtest,
    );

    let prevouts = session.prevouts().unwrap();
    let tx = session.unsigned_tx().clone();
    let honest = session.sighash(0).unwrap();

    // The honest sighash (what the PSBT actually commits to) is accepted.
    display_and_ack(&tx, &prevouts, 0, honest.as_byte_array(), true, Network::Regtest)
        .expect("the PSBT's own sighash acks");

    // A malicious coordinator swaps in a DIFFERENT sighash. Even with --yes (which
    // only bypasses the human ack), the participant's PSBT recompute catches it.
    let malicious = {
        let mut m = honest.as_byte_array().to_vec();
        m[0] ^= 0xff; // any hash that isn't the PSBT's
        m
    };
    let err = display_and_ack(&tx, &prevouts, 0, &malicious, true, Network::Regtest);
    assert!(
        matches!(err, Err(DisplayError::BlindSignMismatch { input_index: 0 })),
        "a coordinator-supplied sighash != PSBT recompute MUST be refused (SIGN-07): {err:?}"
    );
}

#[test]
fn nonce_reuse_is_rejected_a_spent_session_cannot_run_again() {
    let (key_packages, group) = run_inprocess_dkg(3, 5).expect("3-of-5 DKG");
    let psbt = group_spending_psbt(&group);
    let transport = InMemoryTransport::new();
    let mut session = SigningSession::new(
        "adv-noncereuse",
        &transport,
        key_packages,
        group,
        psbt,
        3,
        Network::Regtest,
    );

    // First run consumes the round-1 nonces to produce the signature.
    session.run(true).expect("first run signs");

    // Any attempt to run the SAME session again is rejected before a single new
    // signature share could be emitted — the nonces are spent (SIGN-06).
    let reuse = session.run(true);
    assert!(
        matches!(reuse, Err(SessionError::Spent)),
        "a spent session must refuse to run again (never reuse commitments): {reuse:?}"
    );
}

#[test]
fn abort_yields_fresh_commitments_never_the_reused_set() {
    let (key_packages, group) = run_inprocess_dkg(3, 5).expect("3-of-5 DKG");
    let psbt = group_spending_psbt(&group);
    let transport = InMemoryTransport::new();

    let s1 = SigningSession::new(
        "adv-abort-a",
        &transport,
        key_packages,
        group,
        psbt,
        3,
        Network::Regtest,
    );
    let sel1 = s1.liveness_select().unwrap();
    let r1a = s1.round1(0, &sel1).unwrap();

    // Simulated timeout → abort to a NEW session; its round-1 commitments for the
    // same seat differ (fresh nonces), so the old set is provably not reused.
    let s2 = s1.new_session_on_abort("adv-abort-b");
    let sel2 = s2.liveness_select().unwrap();
    let r1b = s2.round1(0, &sel2).unwrap();

    let id = sel1[0];
    assert_ne!(
        r1a.signing_package().signing_commitment(&id),
        r1b.signing_package().signing_commitment(&id),
        "a post-abort session MUST use fresh commitments (SIGN-06, Pitfall 1/11)"
    );
}
