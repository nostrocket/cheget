//! L2 transport — `Transport` trait + in-memory stub (the Nostr swap seam).
//!
//! This is the load-bearing architectural seam every later ceremony phase (3–6)
//! runs against with **zero relay code**: orchestration depends only on the
//! [`Transport`] trait, and Phase 7 swaps in real `FileTransport` /
//! `NostrTransport` impls behind the same trait with no call-site churn (D-08).
//!
//! The seam is deliberately transport-agnostic: **no `nostr-sdk` / relay type
//! leaks** into the trait, the [`Envelope`], or the [`Filter`]. Payloads are
//! opaque bytes so NIP-44 v2 encryption slots in at Phase 7 above the seam.

pub mod envelope;
pub mod inmemory;

pub use envelope::{Envelope, EnvelopeId, MessageClass, Seat};
pub use inmemory::InMemoryTransport;

/// A subscription filter: every `Some` field must match; `None` matches any.
///
/// Mirrors the Nostr filter model (SPEC §7) — select by message class and the
/// `(ceremony/session id, round, seat)` binding tags, plus an optional recipient
/// selector for directed messages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// Match only this message class.
    pub class: Option<MessageClass>,
    /// Match only this ceremony/session id.
    pub ceremony_or_session_id: Option<String>,
    /// Match only this round.
    pub round: Option<u32>,
    /// Match only envelopes sent by this seat.
    pub seat: Option<Seat>,
    /// Recipient selector.
    ///
    /// A *directed* envelope (`recipient: Some(r)`) is delivered **only** to a
    /// subscriber whose filter carries `recipient == Some(r)`. A *broadcast*
    /// envelope (`recipient: None`) is delivered to every matching subscriber
    /// regardless of this field.
    pub recipient: Option<Seat>,
}

impl Filter {
    /// An empty filter that matches every envelope (subject to directed rules).
    pub fn all() -> Self {
        Self::default()
    }

    /// Restrict to a message class (builder-style).
    pub fn class(mut self, class: MessageClass) -> Self {
        self.class = Some(class);
        self
    }

    /// Restrict to a ceremony/session id (builder-style).
    pub fn ceremony(mut self, id: impl Into<String>) -> Self {
        self.ceremony_or_session_id = Some(id.into());
        self
    }

    /// Restrict to a round (builder-style).
    pub fn round(mut self, round: u32) -> Self {
        self.round = Some(round);
        self
    }

    /// Restrict to a sending seat (builder-style).
    pub fn seat(mut self, seat: Seat) -> Self {
        self.seat = Some(seat);
        self
    }

    /// Set the recipient selector — required to receive directed messages
    /// addressed to `recipient` (builder-style).
    pub fn recipient(mut self, recipient: Seat) -> Self {
        self.recipient = Some(recipient);
        self
    }

    /// Whether `envelope` matches this filter.
    ///
    /// A `None` field matches anything. A directed envelope only matches when the
    /// filter explicitly selects its recipient; a broadcast envelope ignores the
    /// filter's `recipient` field.
    pub fn matches(&self, envelope: &Envelope) -> bool {
        if let Some(class) = self.class {
            if class != envelope.class {
                return false;
            }
        }
        if let Some(id) = &self.ceremony_or_session_id {
            if id != &envelope.ceremony_or_session_id {
                return false;
            }
        }
        if let Some(round) = self.round {
            if round != envelope.round {
                return false;
            }
        }
        if let Some(seat) = self.seat {
            if seat != envelope.seat {
                return false;
            }
        }
        match envelope.recipient {
            // Directed: only the addressed recipient's filter receives it.
            Some(recipient) => self.recipient == Some(recipient),
            // Broadcast: delivered to any matching subscriber.
            None => true,
        }
    }
}

/// The transport seam.
///
/// Synchronous by design for the Phase-1 in-process stub; the payload is opaque
/// bytes and no relay concrete appears in the signature, so a later async Nostr
/// impl can live behind the same trait (a thin blocking adapter, or a trait
/// evolution, at Phase 7) without churning ceremony/session call sites.
pub trait Transport {
    /// Publish an envelope. Re-publishing an envelope with an id already seen is
    /// a no-op (dedup). Returns the stable [`EnvelopeId`] it was keyed under.
    fn publish(&self, envelope: Envelope) -> EnvelopeId;

    /// Return every published envelope matching `filter`, deduplicated by id.
    fn subscribe(&self, filter: &Filter) -> Vec<Envelope>;
}
