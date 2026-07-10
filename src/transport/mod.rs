//! L2 transport — `Transport` trait + in-memory stub (the Nostr swap seam).
//!
//! Placeholder module seam. Filled by plan **01-05**:
//! - an `Envelope { class, ceremony/session id, round, seat, recipient, payload }`
//!   with an opaque byte payload (so NIP-44 encryption slots in at Phase 7),
//! - a synchronous `publish` / `subscribe(filter)` trait,
//! - an in-memory stub that simulates all seats in one process (D-08).
//!
//! No `nostr-sdk` types may leak into this trait.
