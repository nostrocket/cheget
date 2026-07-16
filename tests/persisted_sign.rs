//! Persisted-share confirmed regtest key-spend (KEY-06 read half, D-05/D-06).
//!
//! Proves the crown-jewel confirmed key-spend is produced BY the sign path's
//! real read glue — `cheget::cli::sign::load_persisted_shares` — from shares the
//! 03-01 writer (`cheget::cli::keygen::persist_dkg_shares`) actually persisted to
//! disk, NOT from a fresh in-process DKG and NOT from a re-implemented load loop.
//!
//! The fixture injects an `InCodePassphrase` in place of the CLI's production
//! `ResolvedPassphrase` (the interactive prompt edge is unreachable from tests —
//! pattern-map hazard 3 — and both helpers take a passphrase SOURCE precisely so
//! this substitution works), writes the per-seat roots with the real writer, then
//! drives the read helper into `common::run_confirmed_key_spend_from_shares`.
//!
//! The default test is a small-n (t=3, n=5) PR gate. A `#[ignore]`d full-scale
//! functional smoke (t=51, n=100, overridable via `CHEGET_PERSIST_T` /
//! `CHEGET_PERSIST_N`) confirms the same at the real target on demand — a
//! one-time functional check, NOT a measurement (D-06): no numbers are recorded.

mod common;

use cheget::cli::keygen::persist_dkg_shares;
use cheget::cli::sign::load_persisted_shares;
use cheget::store::InCodePassphrase;

/// Read a `u16` from an environment variable, falling back to `default`.
fn env_u16(key: &str, default: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// A unique scratch store base under the system temp dir (no tempfile dep).
fn temp_base() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "cheget-persist-sign-{}-{}-{}",
        std::process::id(),
        nanos,
        n
    ))
}

/// Persist `n` seat roots with the real writer, load the first `t` through the
/// REAL read glue, and drive the confirmed regtest key-spend from those
/// PERSISTED shares (D-05).
fn persisted_confirmed_key_spend(t: u16, n: u16) {
    let base = temp_base();
    let pass = InCodePassphrase::new("persisted-sign-test-pass");

    // Set up the fixture with the 03-01 writer (do NOT hand-replicate the loop).
    persist_dkg_shares(&base, t, n, &pass).expect("persist per-seat seat roots");

    // Produce the key material by DRIVING THE READ HELPER: discover → sort →
    // take-first-t → load_only_active → assemble is what yields the shares (D-05,
    // hazard 3: inject InCodePassphrase in place of the CLI ResolvedPassphrase).
    let (key_packages, group) =
        load_persisted_shares(&base, t, &pass).expect("load t persisted seat roots");
    assert_eq!(
        key_packages.len(),
        t as usize,
        "load_persisted_shares assembles exactly t shares"
    );

    // The confirmed key-spend is produced BY the persisted-load glue, distinct
    // from inproc_sign's fresh DKG and from any re-implemented load loop.
    common::run_confirmed_key_spend_from_shares(key_packages, group, t);

    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn persisted_sign_confirmed_regtest_key_spend_small_n() {
    // PR gate (D-06): persist 5 seat roots, load the first 3 via the real read
    // glue, and confirm a 3-of-5 regtest key-spend from PERSISTED shares.
    persisted_confirmed_key_spend(3, 5);
}

#[test]
#[ignore = "full-scale functional smoke: persist+load 100 seat roots (scrypt log_n=18 is costly); run on demand with --ignored"]
fn persisted_sign_confirmed_regtest_key_spend_full_100() {
    // On-demand functional confirmation at the real target (D-05/D-06). NOT a
    // measurement — commit no numbers, add no MEASUREMENTS.md.
    let t = env_u16("CHEGET_PERSIST_T", 51);
    let n = env_u16("CHEGET_PERSIST_N", 100);
    eprintln!("running full-scale persisted-share confirmed key-spend at t={t} of n={n} (D-05)");
    persisted_confirmed_key_spend(t, n);
}
