//! Full-scale in-process signing proof — t=51 / n=100 (D-02), the REAL
//! acceptance target for the crown-jewel key-spend.
//!
//! Runs by default (no `#[ignore]`): it spins up a regtest node and runs a full
//! n=100 in-process DKG before the confirmed key-spend. Release is strongly
//! recommended — a debug build makes the O(n²) DKG materially slower. It
//! runs the SAME end-to-end pipeline as the small-`n` PR test
//! (`tests/inproc_sign.rs`) — DKG → address → fund → sign →
//! aggregate-with-tweak → verify against `Q` → finalize → broadcast → CONFIRM on
//! the auto-spawned regtest node — just at full scale.
//!
//! The `(t, n)` scale is overridable via `CHEGET_SIGN_T` / `CHEGET_SIGN_N` (default
//! 51 / 100) so an operator can capture intermediate data points that complete
//! within a tighter time budget; the default remains the mandated 51/100.
//!
//! Run it with:
//! ```text
//! cargo test --release --test inproc_sign_100 -- --nocapture
//! ```

mod common;

fn scale() -> (u16, u16) {
    let t = std::env::var("CHEGET_SIGN_T")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(51u16);
    let n = std::env::var("CHEGET_SIGN_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100u16);
    (t, n)
}

#[test]
fn inproc_sign_confirmed_regtest_key_spend_51_of_100() {
    let (t, n) = scale();
    eprintln!("running full-scale confirmed key-spend at t={t} of n={n} (D-02)");
    common::run_confirmed_key_spend(t, n);
}
