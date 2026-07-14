// STOR-02 store-side structural guard (complements tests/ui/nonce_no_serialize.rs).
//
// The checkpoint store's persist methods are typed to the CONCRETE dkg round
// secret packages only. A signing nonce (`EphemeralNonces`) is not a dkg round
// secret and is not even serializable, so it can never be handed to a checkpoint
// method. This program must fail to compile (E0308: expected a
// `&dkg::round1::SecretPackage` / `&dkg::round2::SecretPackage`, found an
// `&EphemeralNonces`). If it ever compiles, a signing nonce has become an
// expressible checkpoint input — the highest-severity key-extraction bug class.

use cheget::crypto::{EphemeralNonces, SeatId};
use cheget::store::{CeremonyId, CheckpointStore};

fn _round1_rejects_nonce(
    store: &CheckpointStore,
    cid: &CeremonyId,
    seat: SeatId,
    nonce: &EphemeralNonces,
) {
    let _ = store.put_round1(cid, seat, nonce);
}

fn _round2_rejects_nonce(
    store: &CheckpointStore,
    cid: &CeremonyId,
    seat: SeatId,
    nonce: &EphemeralNonces,
) {
    let _ = store.put_round2(cid, seat, nonce);
}

fn main() {}
