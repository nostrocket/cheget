//! In-memory / in-process [`Transport`] stub — "simulate all seats in one
//! process" (D-08).
//!
//! This is the concrete backing store every Phase 1–6 ceremony/signing flow runs
//! against with zero relay code. It is a synchronous `BTreeMap<EnvelopeId,
//! Envelope>` (per RESEARCH Alternatives — simplest correct choice now, keeping
//! the trait shaped for the later async Nostr swap). Keying by the stable
//! content-derived [`EnvelopeId`] gives id-based dedup for free and a
//! deterministic iteration order.

use std::collections::BTreeMap;
use std::sync::Mutex;

use super::{Envelope, EnvelopeId, Filter, Transport};

/// A single-process message board shared by every simulated seat.
///
/// Cloneable handles are **not** shared — construct one `InMemoryTransport` and
/// pass `&`-references to each seat's orchestration so they publish/subscribe
/// against the same store.
#[derive(Debug, Default)]
pub struct InMemoryTransport {
    // Interior mutability so `publish` takes `&self` (matching the trait and the
    // future shared-relay-client shape). `Mutex` keeps it usable if a later test
    // simulates seats across threads.
    store: Mutex<BTreeMap<EnvelopeId, Envelope>>,
}

impl InMemoryTransport {
    /// Create an empty transport.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of distinct (deduplicated) envelopes currently held.
    pub fn len(&self) -> usize {
        self.store.lock().expect("transport store poisoned").len()
    }

    /// Whether the store holds no envelopes.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Transport for InMemoryTransport {
    fn publish(&self, envelope: Envelope) -> EnvelopeId {
        let id = envelope.id();
        // `entry(..).or_insert` ignores a re-publish of an id already present,
        // giving id-based dedup (a replayed/duplicate envelope is a no-op).
        self.store
            .lock()
            .expect("transport store poisoned")
            .entry(id)
            .or_insert(envelope);
        id
    }

    fn subscribe(&self, filter: &Filter) -> Vec<Envelope> {
        self.store
            .lock()
            .expect("transport store poisoned")
            .values()
            .filter(|env| filter.matches(env))
            .cloned()
            .collect()
    }
}
