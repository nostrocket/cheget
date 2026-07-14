// STOR-02 store-side structural guard (complements tests/ui/checkpoint_no_nonce.rs).
//
// The checkpoint store exposes NO generic `persist<T: Serialize>` sink — only the
// concrete `put_round1` / `put_round2` methods typed to the dkg round secret
// packages. A generic persist would let any serializable value (including future
// secret material) be checkpointed, defeating the type-restriction control.
//
// This program must fail to compile (E0599: no method named `persist` found for
// `CheckpointStore`). If it ever compiles, a generic escape hatch has been added.

use cheget::crypto::SeatId;
use cheget::store::{CeremonyId, CheckpointStore};

fn _no_generic_persist(store: &CheckpointStore, cid: &CeremonyId, seat: SeatId) {
    let _ = store.persist(cid, seat, &"any serializable value");
}

fn main() {}
