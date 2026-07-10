//! Small-n in-process DKG correctness (KEY-01, KEY-02, KEY-05).
//!
//! Fast TDD-speed sizes (3-of-5) per D-01; the real t=501/n=1000 correctness
//! proof is the `#[ignore]` `dkg_1000_correctness` test (KEY-06, D-03).

use bitcoin::KnownHrp;
use frost_secp256k1_tr::keys::EvenY;

use tsig::bridge::address_from_group_key;
use tsig::crypto::{confirm_group_key, run_inprocess_dkg, KeygenError};

/// A 3-of-5 in-process DKG yields one even-Y group key that every seat confirms
/// and that the canonical bridge turns into a P2TR address (KEY-01/02).
#[test]
fn dkg_3_of_5_yields_one_even_y_group_key_feeding_the_bridge() {
    let (packages, group) = run_inprocess_dkg(3, 5).expect("in-process DKG");

    // One KeyPackage per simulated seat.
    assert_eq!(packages.len(), 5, "one KeyPackage per seat");

    // D-11 parity invariant: the group key is normalized to even-Y.
    assert!(
        group.verifying_key().has_even_y(),
        "group verifying key must be even-Y after into_even_y(None)"
    );

    // KEY-05 / KEY-06 (small n): every seat's verifying key equals the group key.
    confirm_group_key(&packages, &group).expect("every seat confirms the group key");

    // KEY-01/02: the group key = Taproot internal key P feeds the ONE bridge and
    // produces a valid key-path P2TR address.
    let addr = address_from_group_key(group.verifying_key(), KnownHrp::Regtest)
        .expect("bridge accepts the even-Y group key");
    assert!(
        addr.to_string().starts_with("bcrt1p"),
        "expected a regtest P2TR (bech32m) address, got {addr}"
    );
}

/// A seat whose key disagrees with the group key makes client-side confirmation
/// fail and abort the ceremony (KEY-05). We simulate a corrupted seat by
/// splicing a KeyPackage from an independent ceremony (its verifying key belongs
/// to a different group key), since KeyPackage internals are private.
#[test]
fn corrupted_seat_fails_confirmation_and_aborts() {
    let (mut packages, group) = run_inprocess_dkg(3, 5).expect("ceremony A");
    let (other, _other_group) = run_inprocess_dkg(3, 5).expect("independent ceremony B");

    // Sanity: the honest set confirms cleanly.
    confirm_group_key(&packages, &group).expect("honest set confirms");

    // Corrupt one seat with a package from the independent ceremony.
    let victim = *packages.keys().next().expect("at least one seat");
    packages.insert(victim, other[&victim].clone());

    match confirm_group_key(&packages, &group) {
        Err(KeygenError::GroupKeyMismatch { seat }) => {
            assert_eq!(seat, victim, "the spliced seat is the reported culprit");
        }
        other => panic!("expected GroupKeyMismatch abort, got {other:?}"),
    }
}
