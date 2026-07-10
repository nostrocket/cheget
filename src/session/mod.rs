//! L3 signing session — two-round orchestration; owns the nonce lifetime.
//!
//! Placeholder module seam. Filled by plan **01-04**:
//! - liveness poll + 501-of-1000 subset selection (over-provisioned, Pitfall 11),
//! - round1 commit → display-before-sign gate (SIGN-07) → round2 sign → aggregate,
//! - verify the aggregate against the output key `Q` (SIGN-04),
//! - new-session-on-abort semantics; nonces live in RAM only and are never
//!   persisted (SIGN-05). No `resume`/`checkpoint` verb may exist here.
