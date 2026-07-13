//! Opaque-payload signed-envelope model — the transport-agnostic message unit.
//!
//! An [`Envelope`] is what every ceremony/session message becomes on the wire,
//! shaped to fit the future Nostr event model (SPEC §7) **without** leaking any
//! `nostr-sdk` type into orchestration (D-08):
//!
//! - [`MessageClass`] mirrors the per-class custom event *kinds* (`ceremony-open`,
//!   `round1-package`, `round2-bundle`, `commitments`, `signature-share`,
//!   `confirmation`, `session-control`);
//! - the ceremony/session id, `round`, and `seat` mirror the `["cer",..]`,
//!   `["round",..]`, `["seat",..]` binding tags that give idempotent resumption;
//! - `recipient` mirrors the `["p", <npub>]` directed tag (`None` == broadcast);
//! - `payload` is **opaque bytes** so NIP-44 v2 payload encryption slots in at
//!   Phase 7 without touching any call site.
//!
//! The stable [`Envelope::id`] seeds the Phase-7 event-`id` dedup / replay defense
//! (Pitfall 20): the in-memory stub dedups on it today.

/// A seat in the `(t, n)` group, addressed by its integer index.
///
/// Transport addresses seats by a plain integer so the seam stays free of both
/// FROST and Nostr types; the session layer maps `frost::Identifier` ↔ `Seat`
/// (and, at Phase 7, `Seat` ↔ roster npub). `u16` comfortably covers `n = 100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seat(pub u16);

impl std::fmt::Display for Seat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A stable, content-derived envelope identity.
///
/// Two envelopes with identical fields share an id, so re-publishing the same
/// message is a no-op (dedup). This is the in-process seed of the Phase-7 Nostr
/// event-`id` dedup: there the id is a hash of the signed event; here it is a
/// deterministic hash of the envelope contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnvelopeId(pub u64);

impl std::fmt::Display for EnvelopeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// The message class — one variant per future Nostr custom event kind (SPEC §7).
///
/// Kept transport-agnostic: these are protocol message categories, not event
/// kinds; Phase 7 maps each to a concrete `Kind::Custom(..)` behind the trait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MessageClass {
    /// A coordinator opens a ceremony (roster hash, parameters).
    CeremonyOpen,
    /// DKG round-1 package (public commitments), broadcast.
    Round1Package,
    /// DKG round-2 bundle (per-recipient shares), directed + confidential.
    Round2Bundle,
    /// Signing round-1 nonce commitments, broadcast.
    Commitments,
    /// Signing round-2 signature share, directed to the coordinator.
    SignatureShare,
    /// A participant confirms the group verifying key (KEY-05).
    Confirmation,
    /// Out-of-band session control (liveness, abort, sweep notices).
    SessionControl,
}

impl MessageClass {
    /// A stable discriminant used in the content-derived envelope id.
    fn tag(self) -> u8 {
        match self {
            MessageClass::CeremonyOpen => 1,
            MessageClass::Round1Package => 2,
            MessageClass::Round2Bundle => 3,
            MessageClass::Commitments => 4,
            MessageClass::SignatureShare => 5,
            MessageClass::Confirmation => 6,
            MessageClass::SessionControl => 7,
        }
    }
}

/// A signed-envelope message: transport-agnostic, opaque payload.
///
/// The authenticity model is deferred: on Nostr the event's BIP340 signature *is*
/// the envelope signature (SPEC §7). Phase 1's in-process stub carries no
/// signature — the seam only needs to move opaque bytes between seats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// Which protocol message class this is (future per-class event kind).
    pub class: MessageClass,
    /// The ceremony or signing-session identifier this message binds to.
    pub ceremony_or_session_id: String,
    /// The protocol round (`0` where a class is round-agnostic).
    pub round: u32,
    /// The sending seat.
    pub seat: Seat,
    /// The intended recipient seat for a *directed* message; `None` == broadcast.
    pub recipient: Option<Seat>,
    /// **Opaque** payload bytes — encryption/serialization live above the seam.
    pub payload: Vec<u8>,
}

impl Envelope {
    /// Construct a broadcast envelope (delivered to every matching subscriber).
    pub fn broadcast(
        class: MessageClass,
        ceremony_or_session_id: impl Into<String>,
        round: u32,
        seat: Seat,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            class,
            ceremony_or_session_id: ceremony_or_session_id.into(),
            round,
            seat,
            recipient: None,
            payload,
        }
    }

    /// Construct a directed envelope (delivered only to `recipient`).
    pub fn directed(
        class: MessageClass,
        ceremony_or_session_id: impl Into<String>,
        round: u32,
        seat: Seat,
        recipient: Seat,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            class,
            ceremony_or_session_id: ceremony_or_session_id.into(),
            round,
            seat,
            recipient: Some(recipient),
            payload,
        }
    }

    /// Stable, deterministic id derived from every field (FNV-1a, 64-bit).
    ///
    /// Deterministic across runs and machines (unlike a `DefaultHasher`), so it
    /// is a faithful in-process stand-in for the Nostr event id used to dedup.
    pub fn id(&self) -> EnvelopeId {
        // FNV-1a over a length-prefixed canonical encoding of the fields.
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut h = OFFSET;
        let mut mix = |bytes: &[u8]| {
            for &b in bytes {
                h ^= b as u64;
                h = h.wrapping_mul(PRIME);
            }
        };

        mix(&[self.class.tag()]);
        let id_bytes = self.ceremony_or_session_id.as_bytes();
        mix(&(id_bytes.len() as u64).to_le_bytes());
        mix(id_bytes);
        mix(&self.round.to_le_bytes());
        mix(&self.seat.0.to_le_bytes());
        match self.recipient {
            Some(r) => {
                mix(&[1u8]);
                mix(&r.0.to_le_bytes());
            }
            None => mix(&[0u8]),
        }
        mix(&(self.payload.len() as u64).to_le_bytes());
        mix(&self.payload);

        EnvelopeId(h)
    }
}
