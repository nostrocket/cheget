//! KEY-06 / D-03: full n=100 in-process DKG correctness proof + O(n²)
//! timing/memory instrumentation.
//!
//! Runs by default (no `#[ignore]`); release is strongly recommended because the
//! ceremony is O(n²) and a debug build is materially slower. The *measurement
//! itself* is the point — pass/fail is feasibility, not a fixed threshold (D-03).
//!
//! Run it (release strongly recommended — the ceremony is O(n²): each of n
//! seats processes ~n peer packages, and each round-3 verification is O(t)):
//!
//! ```text
//! cargo test --release --test dkg_100_correctness -- --nocapture
//! ```
//!
//! The DKG loop is inlined here (rather than calling `run_inprocess_dkg`) so we
//! can time `part1` / `part2` / `part3` separately. The correctness invariant it
//! asserts — all 100 `KeyPackage`s verify to ONE group `PublicKeyPackage` — is
//! identical to what `run_inprocess_dkg` enforces (crypto/keygen.rs). Rounds 2
//! and 3 are parallelized across seats (the per-seat crypto is independent and
//! deterministic; only round 1 needs the shared RNG and stays sequential), so
//! the reported figures are parallel wall-clock on this host.

use std::collections::BTreeMap;
use std::thread;
use std::time::Instant;

use frost_secp256k1_tr as frost;
use frost::keys::dkg;
use frost::keys::{EvenY, KeyPackage, PublicKeyPackage};
use frost::Identifier;

#[test]
fn dkg_100_all_shares_verify_to_one_group_key() {
    // Defaults to the mandated acceptance target t=51, n=100 (D-02). The scale
    // is overridable via CHEGET_DKG_T / CHEGET_DKG_N so the same instrumented loop
    // can capture O(n^2) scaling data points at smaller, faster-completing sizes
    // (the DKG code is generic over (t, n) per D-01). At n=100 the full run is
    // an on-demand job (see Task 5 for the measured wall-clock), not the PR gate.
    let max_signers: u16 = env_u16("CHEGET_DKG_N", 100);
    let min_signers: u16 = env_u16("CHEGET_DKG_T", 51);
    assert!(
        min_signers >= 1 && min_signers <= max_signers,
        "require 1 <= t ({min_signers}) <= n ({max_signers})"
    );
    let mut rng = frost::rand_core::OsRng;

    let workers = thread::available_parallelism().map(|n| n.get()).unwrap_or(1);

    // ---- Round 1 (sequential — consumes the shared RNG) ----
    let t1 = Instant::now();
    let mut r1_secret: Vec<(Identifier, dkg::round1::SecretPackage)> = Vec::new();
    let mut r1_pkgs: BTreeMap<Identifier, dkg::round1::Package> = BTreeMap::new();
    for i in 1..=max_signers {
        let id: Identifier = i.try_into().expect("nonzero seat id");
        let (secret, pkg) = dkg::part1(id, max_signers, min_signers, &mut rng).expect("part1");
        r1_secret.push((id, secret));
        r1_pkgs.insert(id, pkg);
    }
    let d1 = t1.elapsed();

    // ---- Round 2 (parallel across seats) ----
    let t2 = Instant::now();
    let r1_pkgs_ref = &r1_pkgs;
    let mut r2_buckets: Vec<Vec<(Identifier, dkg::round1::SecretPackage)>> =
        (0..workers).map(|_| Vec::new()).collect();
    for (k, item) in r1_secret.into_iter().enumerate() {
        r2_buckets[k % workers].push(item);
    }
    #[allow(clippy::type_complexity)]
    let mut r2_results: Vec<(
        Identifier,
        dkg::round2::SecretPackage,
        BTreeMap<Identifier, dkg::round2::Package>,
    )> = Vec::new();
    thread::scope(|s| {
        let handles: Vec<_> = r2_buckets
            .into_iter()
            .map(|bucket| {
                s.spawn(move || {
                    // Clone the round-1 package map ONCE per worker, then present
                    // "all others" to each seat by removing/re-inserting its own
                    // entry — avoids the O(n^2 * t) memory blow-up of rebuilding
                    // a 99-entry commitment map per seat.
                    let mut pool = (*r1_pkgs_ref).clone();
                    let mut out = Vec::with_capacity(bucket.len());
                    for (id, secret) in bucket {
                        let self_pkg = pool.remove(&id).expect("seat present in pool");
                        let (secret2, sent) = dkg::part2(secret, &pool).expect("part2");
                        pool.insert(id, self_pkg);
                        out.push((id, secret2, sent));
                    }
                    out
                })
            })
            .collect();
        for h in handles {
            r2_results.extend(h.join().expect("round-2 worker"));
        }
    });
    let d2 = t2.elapsed();

    // Fan the round-2 packages out by recipient.
    let mut r2_by_recipient: BTreeMap<Identifier, BTreeMap<Identifier, dkg::round2::Package>> =
        BTreeMap::new();
    let mut r2_secret: Vec<(Identifier, dkg::round2::SecretPackage)> =
        Vec::with_capacity(r2_results.len());
    for (id, secret2, sent) in r2_results {
        for (recipient, pkg) in sent {
            r2_by_recipient.entry(recipient).or_default().insert(id, pkg);
        }
        r2_secret.push((id, secret2));
    }

    // ---- Round 3 (parallel across seats) ----
    let t3 = Instant::now();
    let r2_by_recipient_ref = &r2_by_recipient;
    let mut r3_buckets: Vec<Vec<(Identifier, dkg::round2::SecretPackage)>> =
        (0..workers).map(|_| Vec::new()).collect();
    for (k, item) in r2_secret.into_iter().enumerate() {
        r3_buckets[k % workers].push(item);
    }
    let mut kp_results: Vec<(Identifier, KeyPackage, PublicKeyPackage)> = Vec::new();
    thread::scope(|s| {
        let handles: Vec<_> = r3_buckets
            .into_iter()
            .map(|bucket| {
                s.spawn(move || {
                    // Same remove/re-insert trick as round 2: clone the round-1
                    // package map once per worker rather than per seat.
                    let mut pool = (*r1_pkgs_ref).clone();
                    let mut out = Vec::with_capacity(bucket.len());
                    for (id, secret2) in &bucket {
                        let self_pkg = pool.remove(id).expect("seat present in pool");
                        let r2_for_me = &r2_by_recipient_ref[id];
                        let (kp, pubkeys) =
                            dkg::part3(secret2, &pool, r2_for_me).expect("part3");
                        pool.insert(*id, self_pkg);
                        out.push((*id, kp.into_even_y(None), pubkeys.into_even_y(None)));
                    }
                    out
                })
            })
            .collect();
        for h in handles {
            kp_results.extend(h.join().expect("round-3 worker"));
        }
    });
    let d3 = t3.elapsed();

    // Memory measured here, at the peak-holding point (round-1/round-2 package
    // maps + all 100 derived KeyPackages still live), before anything drops.
    let rss_mib = resident_set_mib();

    // ---- Correctness: all 100 KeyPackages verify to ONE group key (KEY-06) ----
    let mut group: Option<PublicKeyPackage> = None;
    let mut verified = 0usize;
    for (id, kp, pubkeys) in &kp_results {
        assert!(
            pubkeys.verifying_key().has_even_y(),
            "seat {id:?} group key must be even-Y"
        );
        match &group {
            Some(g) => assert_eq!(
                g.verifying_key(),
                pubkeys.verifying_key(),
                "KEY-06: seat {id:?} disagrees on the group verifying key"
            ),
            None => group = Some(pubkeys.clone()),
        }
        assert_eq!(
            kp.verifying_key(),
            group.as_ref().expect("group set").verifying_key(),
            "KEY-06: seat {id:?} KeyPackage verifying key != group key"
        );
        verified += 1;
    }

    assert_eq!(
        verified, max_signers as usize,
        "all {max_signers} seats must verify to one group key"
    );

    // The O(n^2) measurement IS the deliverable (D-03).
    println!("KEY-06 in-process DKG (t={min_signers}, n={max_signers}): {verified} KeyPackages verify to ONE group key");
    println!(
        "  timing (wall-clock, {workers} workers)  part1={d1:?}  part2={d2:?}  part3={d3:?}  total={:?}",
        d1 + d2 + d3
    );
    println!("  memory  resident set at peak (post part3): {rss_mib:.1} MiB");
    println!(
        "  shape   O(n^2): each of {max_signers} seats processes {} peer packages; round-3 verify is O(t={min_signers})",
        max_signers - 1
    );
}

/// Read a `u16` from an environment variable, falling back to `default`.
fn env_u16(key: &str, default: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// Best-effort, dependency-free resident-set-size probe (KiB→MiB) via `ps`.
///
/// Reports the process RSS at the moment of the call — invoked at the
/// peak-holding point above — rather than a kernel high-water mark, which is
/// sufficient for the feasibility measurement D-03 asks for. Returns `0.0` if
/// `ps` is unavailable.
fn resident_set_mib() -> f64 {
    let pid = std::process::id().to_string();
    match std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
    {
        Ok(out) => {
            let kib: f64 = String::from_utf8_lossy(&out.stdout)
                .trim()
                .parse()
                .unwrap_or(0.0);
            kib / 1024.0
        }
        Err(_) => 0.0,
    }
}
