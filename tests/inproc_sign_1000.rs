//! Full-scale in-process signing proof — t=501 / n=1000 (D-02), the REAL
//! acceptance target for the crown-jewel key-spend.
//!
//! `#[ignore]` by default: the in-process n=1000 DKG is a multi-CPU-hour job
//! (round-3 share verification dominates, see 01-02), so this is a nightly /
//! on-demand gate, not the PR gate. It runs the SAME end-to-end pipeline as the
//! small-`n` PR test (`tests/inproc_sign.rs`) — DKG → address → fund → sign →
//! aggregate-with-tweak → verify against `Q` → finalize → broadcast → CONFIRM on
//! the auto-spawned regtest node — just at full scale.
//!
//! The `(t, n)` scale is overridable via `TSIG_SIGN_T` / `TSIG_SIGN_N` (default
//! 501 / 1000) so an operator can capture intermediate data points that complete
//! within a tighter time budget; the default remains the mandated 501/1000.
//!
//! Run it with:
//! ```text
//! cargo test --release --test inproc_sign_1000 -- --ignored --nocapture
//! ```

mod common;

fn scale() -> (u16, u16) {
    let t = std::env::var("TSIG_SIGN_T")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(501u16);
    let n = std::env::var("TSIG_SIGN_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000u16);
    (t, n)
}

#[test]
#[ignore = "nightly/on-demand: full-scale n=1000 in-process DKG is multi-CPU-hour (D-02/D-06)"]
fn inproc_sign_confirmed_regtest_key_spend_501_of_1000() {
    let (t, n) = scale();
    eprintln!("running full-scale confirmed key-spend at t={t} of n={n} (D-02)");
    common::run_confirmed_key_spend(t, n);
}
