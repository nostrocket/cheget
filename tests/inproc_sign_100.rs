//! Full-scale in-process signing proof — t=51 / n=100 (D-02), the REAL
//! acceptance target for the crown-jewel key-spend.
//!
//! `#[ignore]` by default: it spins up a regtest node and runs a full n=100
//! in-process DKG before the confirmed key-spend, so it is kept off the unit-test
//! PR gate (see Task 5 for the measured wall-clock and the ignore decision). It
//! runs the SAME end-to-end pipeline as the small-`n` PR test
//! (`tests/inproc_sign.rs`) — DKG → address → fund → sign →
//! aggregate-with-tweak → verify against `Q` → finalize → broadcast → CONFIRM on
//! the auto-spawned regtest node — just at full scale.
//!
//! The `(t, n)` scale is overridable via `TSIG_SIGN_T` / `TSIG_SIGN_N` (default
//! 51 / 100) so an operator can capture intermediate data points that complete
//! within a tighter time budget; the default remains the mandated 51/100.
//!
//! Run it with:
//! ```text
//! cargo test --release --test inproc_sign_100 -- --ignored --nocapture
//! ```

mod common;

fn scale() -> (u16, u16) {
    let t = std::env::var("TSIG_SIGN_T")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(51u16);
    let n = std::env::var("TSIG_SIGN_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100u16);
    (t, n)
}

#[test]
#[ignore = "on-demand: full-scale n=100 in-process DKG + confirmed regtest key-spend (D-02/D-06)"]
fn inproc_sign_confirmed_regtest_key_spend_51_of_100() {
    let (t, n) = scale();
    eprintln!("running full-scale confirmed key-spend at t={t} of n={n} (D-02)");
    common::run_confirmed_key_spend(t, n);
}
