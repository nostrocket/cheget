//! Transport seam behavior — the in-memory stub (SIGN-02 support, D-08).
//!
//! Proves the four behaviors the later ceremony/signing phases rely on:
//! publish→subscribe delivery, filter exclusion, id-based dedup, and directed
//! (recipient-scoped) delivery. This stub is what 01-04's signing session
//! publishes commitments / packages / shares over.

use cheget::transport::{Envelope, Filter, InMemoryTransport, MessageClass, Seat, Transport};

fn commitments(seat: u16, round: u32, payload: &[u8]) -> Envelope {
    Envelope::broadcast(
        MessageClass::Commitments,
        "session-1",
        round,
        Seat(seat),
        payload.to_vec(),
    )
}

#[test]
fn publish_then_matching_subscribe_returns_envelope() {
    let t = InMemoryTransport::new();
    let env = commitments(7, 1, b"nonce-commitment");
    t.publish(env.clone());

    let got = t.subscribe(&Filter::all().class(MessageClass::Commitments));
    assert_eq!(got, vec![env]);
}

#[test]
fn filter_excludes_non_matching_class_round_and_seat() {
    let t = InMemoryTransport::new();
    let env = commitments(7, 1, b"x");
    t.publish(env);

    // Wrong class.
    assert!(t
        .subscribe(&Filter::all().class(MessageClass::SignatureShare))
        .is_empty());
    // Wrong round.
    assert!(t.subscribe(&Filter::all().round(2)).is_empty());
    // Wrong sender seat.
    assert!(t.subscribe(&Filter::all().seat(Seat(8))).is_empty());
    // Wrong ceremony/session id.
    assert!(t.subscribe(&Filter::all().ceremony("session-2")).is_empty());
}

#[test]
fn duplicate_id_publish_yields_single_delivery() {
    let t = InMemoryTransport::new();
    let env = commitments(7, 1, b"same-bytes");
    // Two envelopes with identical fields share a content-derived id.
    let id_a = t.publish(env.clone());
    let id_b = t.publish(env.clone());
    assert_eq!(id_a, id_b, "identical envelopes must share an id");

    let got = t.subscribe(&Filter::all());
    assert_eq!(got.len(), 1, "duplicate id must dedup to one delivery");
    assert_eq!(got[0], env);
}

#[test]
fn directed_envelope_reaches_only_matching_recipient_filter() {
    let t = InMemoryTransport::new();
    let directed = Envelope::directed(
        MessageClass::Round2Bundle,
        "ceremony-1",
        2,
        Seat(1),
        Seat(42),
        b"share-for-42".to_vec(),
    );
    t.publish(directed.clone());

    // The addressed recipient receives it.
    let to_42 = t.subscribe(&Filter::all().recipient(Seat(42)));
    assert_eq!(to_42, vec![directed.clone()]);

    // A different recipient does not.
    assert!(t.subscribe(&Filter::all().recipient(Seat(43))).is_empty());

    // A broadcast-style filter (no recipient) does not receive a directed msg.
    assert!(t.subscribe(&Filter::all()).is_empty());
}
