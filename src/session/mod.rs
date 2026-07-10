//! L3 signing session — two-round FROST orchestration; owns the nonce lifetime.
//!
//! The [`SigningSession`] wires together the four seams proven by the earlier
//! waves — the bridge (01-01), the crypto core + non-serializable nonces
//! (01-02), the chain sighash helper (01-03) and the `Transport` stub (01-05) —
//! into the crown-jewel flow: liveness poll → round-1 commitments →
//! display-before-sign gate → round-2 tweaked signing → tweaked aggregation →
//! verify against the output key `Q` → finalize the PSBT (SIGN-01..07).
//!
//! Two coordinator-untrusted gates are structural here:
//!
//! * **display-before-sign (SIGN-07):** each seat recomputes the sighash from the
//!   PSBT it was handed (never a coordinator-supplied hash) and refuses to sign
//!   if a displayed summary disagrees — see [`display`].
//! * **verify-against-Q (SIGN-04):** the aggregated signature is verified against
//!   the tweaked *output* key `Q`, never the internal key `P` — see
//!   [`crate::crypto::sign`].
//!
//! Nonce discipline (SIGN-05/06): the per-seat [`EphemeralNonces`] live only in
//! this session's memory, are never serialized, and are consumed **by value** in
//! round 2. On any abort/timeout the session is spent; a *new* session (new id,
//! fresh nonces) must be started — commitments are never reused. There is
//! deliberately no `resume`/`checkpoint` verb.

pub mod liveness;

use std::collections::BTreeMap;

use bitcoin::hashes::Hash;
use bitcoin::{Network, Psbt, Transaction, TxOut};
use frost_secp256k1_tr as frost;
use frost::keys::{KeyPackage, PublicKeyPackage};
use frost::round1::SigningCommitments;
use frost::{Identifier, SigningPackage};

use crate::chain::{key_spend_sighash, ChainError};
use crate::crypto::EphemeralNonces;
use crate::transport::{Envelope, Filter, MessageClass, Seat, Transport};

use liveness::{poll_and_select, LivenessError};

/// The coordinator's transport seat. Seat `0` is never a valid FROST identifier
/// (identifiers are the roster indices `1..=n`), so it cannot collide with a
/// signer seat — directed round-2 shares are addressed here.
#[allow(dead_code)]
const COORDINATOR_SEAT: Seat = Seat(0);

/// Errors surfaced by the signing session.
#[derive(Debug)]
pub enum SessionError {
    /// The liveness poll could not finalize a `t`-subset.
    Liveness(LivenessError),
    /// A chain-layer error (sighash computation).
    Chain(ChainError),
    /// A `frost` primitive error.
    Frost(frost::Error),
    /// The PSBT input carried no `witness_utxo`, so its prevout is unknown and the
    /// key-spend sighash cannot be computed.
    NoWitnessUtxo {
        /// The offending input index.
        input_index: usize,
    },
    /// A selected identifier is not in this session's key-package roster.
    UnknownSeat(Identifier),
    /// The session has already produced (or aborted) its signatures. Nonces are
    /// consumed and must never be reused — start a NEW session (SIGN-06).
    Spent,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::Liveness(e) => write!(f, "liveness: {e}"),
            SessionError::Chain(e) => write!(f, "chain: {e}"),
            SessionError::Frost(e) => write!(f, "frost: {e}"),
            SessionError::NoWitnessUtxo { input_index } => {
                write!(f, "PSBT input {input_index} has no witness_utxo (unknown prevout)")
            }
            SessionError::UnknownSeat(id) => write!(f, "selected seat {id:?} not in roster"),
            SessionError::Spent => write!(
                f,
                "session is spent (nonces consumed); start a NEW session — never reuse commitments"
            ),
        }
    }
}

impl std::error::Error for SessionError {}

/// Round-1 state: the coordinator's [`SigningPackage`] for one input plus the
/// per-seat [`EphemeralNonces`] held in memory.
///
/// The nonces are **move-only** and consumed by `SigningSession::round2`; this
/// struct therefore cannot be cloned or persisted, which is exactly the SIGN-05
/// nonce-lifetime discipline surfaced at the session boundary.
pub struct Round1 {
    pub(crate) signing_package: SigningPackage,
    pub(crate) nonces: BTreeMap<Identifier, EphemeralNonces>,
}

impl Round1 {
    /// The coordinator's signing package (commitments + the message = sighash).
    pub fn signing_package(&self) -> &SigningPackage {
        &self.signing_package
    }

    /// Number of seats that committed in round 1 (must equal `t`).
    pub fn committed_seats(&self) -> usize {
        self.nonces.len()
    }
}

/// A two-round FROST signing session over a [`Transport`] stub.
///
/// Generic over the transport so the Phase-1 in-memory stub and the Phase-7
/// `FileTransport`/`NostrTransport` both drive it with no call-site churn.
pub struct SigningSession<'a, T: Transport> {
    id: String,
    transport: &'a T,
    pub(crate) key_packages: BTreeMap<Identifier, KeyPackage>,
    pub(crate) group: PublicKeyPackage,
    psbt: Psbt,
    t: usize,
    pub(crate) network: Network,
    /// Reverse map: transport `Seat` → FROST `Identifier` (roster indices).
    id_of_seat: BTreeMap<Seat, Identifier>,
    /// Forward map: FROST `Identifier` → transport `Seat`.
    seat_of_id: BTreeMap<Identifier, Seat>,
    /// Whether this session has produced/aborted (nonces consumed) — a spent
    /// session must never be reused (SIGN-06).
    pub(crate) spent: bool,
}

impl<'a, T: Transport> SigningSession<'a, T> {
    /// Start a signing session bound to a fresh session `id`.
    ///
    /// `key_packages` are the DKG seat shares (simulate-all-seats in Phase 1),
    /// `group` the group public-key package, `psbt` the transaction to sign
    /// (the coordinator distributes the PSBT, never a precomputed sighash — the
    /// SIGN-07 recompute gate depends on this), and `t` the signing threshold.
    pub fn new(
        id: impl Into<String>,
        transport: &'a T,
        key_packages: BTreeMap<Identifier, KeyPackage>,
        group: PublicKeyPackage,
        psbt: Psbt,
        t: usize,
        network: Network,
    ) -> Self {
        // Assign each roster identifier a stable transport seat. BTreeMap iterates
        // identifiers in ascending order, matching the DKG's `1..=n` assignment,
        // so seat `i` maps to identifier `i` — but the session only relies on the
        // mapping being a bijection, not on its exact numbering.
        let mut id_of_seat = BTreeMap::new();
        let mut seat_of_id = BTreeMap::new();
        for (i, id) in key_packages.keys().enumerate() {
            let seat = Seat((i + 1) as u16);
            id_of_seat.insert(seat, *id);
            seat_of_id.insert(*id, seat);
        }

        Self {
            id: id.into(),
            transport,
            key_packages,
            group,
            psbt,
            t,
            network,
            id_of_seat,
            seat_of_id,
            spent: false,
        }
    }

    /// This session's id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The unsigned transaction carried by the PSBT.
    pub fn unsigned_tx(&self) -> &Transaction {
        &self.psbt.unsigned_tx
    }

    /// The full prevout set (one `TxOut` per input, in input order), taken from
    /// each PSBT input's `witness_utxo`. Taproot signs **all** prevouts.
    pub fn prevouts(&self) -> Result<Vec<TxOut>, SessionError> {
        self.psbt
            .inputs
            .iter()
            .enumerate()
            .map(|(i, input)| {
                input
                    .witness_utxo
                    .clone()
                    .ok_or(SessionError::NoWitnessUtxo { input_index: i })
            })
            .collect()
    }

    /// The BIP341 key-spend sighash for `input_index`, via the one canonical
    /// [`key_spend_sighash`] helper (SIGN-01). This is the message the coordinator
    /// puts in the `SigningPackage` **and** the value each seat recomputes in the
    /// display gate — one helper, no divergence.
    pub fn sighash(&self, input_index: usize) -> Result<bitcoin::TapSighash, SessionError> {
        let prevouts = self.prevouts()?;
        key_spend_sighash(&self.psbt.unsigned_tx, input_index, &prevouts)
            .map_err(SessionError::Chain)
    }

    /// A transport topic id scoped to `(session, input)` so envelopes for
    /// different inputs (and different sessions) never collide in the stub store.
    pub(crate) fn topic(&self, input_index: usize) -> String {
        format!("{}#{}", self.id, input_index)
    }

    /// Run the over-provisioned liveness poll over the transport and finalize a
    /// `t`-seat signing subset (SIGN-02, Pitfall 11).
    ///
    /// Every roster seat publishes a liveness ack (a `SessionControl` envelope);
    /// the coordinator collects the responders and [`poll_and_select`]s exactly
    /// `t`. In-process every seat responds, so the pool is over-provisioned by
    /// construction (`n > t`); the *selection* still takes only `t`.
    pub fn liveness_select(&self) -> Result<Vec<Identifier>, SessionError> {
        for (seat, _id) in &self.id_of_seat {
            let env = Envelope::broadcast(
                MessageClass::SessionControl,
                &self.id,
                0,
                *seat,
                b"live".to_vec(),
            );
            self.transport.publish(env);
        }

        let responders_env = self.transport.subscribe(
            &Filter::all()
                .class(MessageClass::SessionControl)
                .ceremony(&self.id),
        );
        let mut responders: Vec<Identifier> = responders_env
            .iter()
            .filter_map(|e| self.id_of_seat.get(&e.seat).copied())
            .collect();
        responders.sort();
        responders.dedup();

        poll_and_select(&responders, self.t).map_err(SessionError::Liveness)
    }

    /// Round 1: the `selected` seats each commit fresh [`EphemeralNonces`] and
    /// publish their [`SigningCommitments`] over the transport; the coordinator
    /// collects them and builds the [`SigningPackage`] whose message is the
    /// key-spend sighash for `input_index` (SIGN-01, SIGN-02).
    ///
    /// The returned [`Round1`] holds the nonces in memory (never serialized); it
    /// is consumed by `SigningSession::round2`.
    pub fn round1(
        &self,
        input_index: usize,
        selected: &[Identifier],
    ) -> Result<Round1, SessionError> {
        let sighash = self.sighash(input_index)?;
        let topic = self.topic(input_index);
        let mut rng = frost::rand_core::OsRng;

        let mut nonces: BTreeMap<Identifier, EphemeralNonces> = BTreeMap::new();
        for id in selected {
            let kp = self.key_packages.get(id).ok_or(SessionError::UnknownSeat(*id))?;
            let seat = *self.seat_of_id.get(id).ok_or(SessionError::UnknownSeat(*id))?;
            let (nonce, commitments) = EphemeralNonces::commit(kp.signing_share(), &mut rng);
            let payload = commitments.serialize().map_err(SessionError::Frost)?;
            self.transport.publish(Envelope::broadcast(
                MessageClass::Commitments,
                &topic,
                1,
                seat,
                payload,
            ));
            nonces.insert(*id, nonce);
        }

        // The coordinator re-reads the commitments from the transport (not from
        // its own loop) so the data path is the real publish→subscribe seam.
        let collected = self.transport.subscribe(
            &Filter::all()
                .class(MessageClass::Commitments)
                .ceremony(&topic)
                .round(1),
        );
        let mut commitments: BTreeMap<Identifier, SigningCommitments> = BTreeMap::new();
        for env in collected {
            if let Some(id) = self.id_of_seat.get(&env.seat) {
                if selected.contains(id) {
                    let c =
                        SigningCommitments::deserialize(&env.payload).map_err(SessionError::Frost)?;
                    commitments.insert(*id, c);
                }
            }
        }

        let signing_package = SigningPackage::new(commitments, sighash.as_byte_array());
        Ok(Round1 { signing_package, nonces })
    }
}
