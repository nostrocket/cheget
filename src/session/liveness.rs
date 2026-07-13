//! Over-provisioned liveness poll + threshold-subset selection (SIGN-02).
//!
//! FROST is **not robust**: a single dropout among an exactly-`t` signing set
//! aborts the whole session (PITFALLS Pitfall 11). The defence is to
//! *over-provision* the liveness poll — poll a margin **above** `t`, then
//! finalize exactly `t` from the seats that actually responded/committed. Never
//! select exactly `t` seats and hope none drop.
//!
//! In-process at Phase 1 there are no real dropouts, but the session/abort
//! semantics must exist now so Phase 7 inherits them at scale (t=51/n=100).

use frost_secp256k1_tr as frost;
use frost::Identifier;

/// Errors from the liveness poll / subset selection.
#[derive(Debug)]
pub enum LivenessError {
    /// Fewer than `t` seats responded to the (over-provisioned) liveness poll,
    /// so no valid signing subset can be finalized — the session must abort and
    /// a new one be started later (Pitfall 11).
    InsufficientLiveSeats {
        /// Threshold `t` required.
        needed: usize,
        /// Number of seats that actually responded.
        got: usize,
    },
}

impl std::fmt::Display for LivenessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LivenessError::InsufficientLiveSeats { needed, got } => write!(
                f,
                "insufficient live seats: need {needed}, only {got} responded to the \
                 liveness poll (abort; start a new session with a wider poll)"
            ),
        }
    }
}

impl std::error::Error for LivenessError {}

/// Number of seats to poll for liveness: `t` plus an over-provision margin,
/// capped at the roster size `n` (Pitfall 11).
///
/// The margin (≈10%, at least one seat) is what absorbs dropouts before the
/// session has to abort. The coordinator polls this many, then
/// [`poll_and_select`] finalizes exactly `t` from those that respond.
pub fn over_provisioned_poll_size(t: usize, n: usize) -> usize {
    let margin = t.div_ceil(10).max(1);
    (t + margin).min(n)
}

/// Finalize **exactly `t`** signing seats from the responders of an
/// over-provisioned liveness poll.
///
/// `responders` are the seats that actually answered the poll (the caller should
/// have polled [`over_provisioned_poll_size`] of them, i.e. more than `t`). If
/// fewer than `t` responded the session cannot proceed and must abort
/// ([`LivenessError::InsufficientLiveSeats`]); otherwise this returns the first
/// `t` responders — never more than `t`, so the signing set is minimal and the
/// remaining responders are spare capacity, not signers.
pub fn poll_and_select(
    responders: &[Identifier],
    t: usize,
) -> Result<Vec<Identifier>, LivenessError> {
    if responders.len() < t {
        return Err(LivenessError::InsufficientLiveSeats {
            needed: t,
            got: responders.len(),
        });
    }
    // Finalize exactly `t` — the surplus responders are spare capacity that a
    // *new* session (on abort) could draw a different subset from.
    Ok(responders.iter().copied().take(t).collect())
}
