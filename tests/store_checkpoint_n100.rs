//! STOR-02 at-scale persist/reload harness — BUILT here (Phase 2, plan 02-03),
//! RUN at the full 51/100 set in Phase 3 (Phase 3 acceptance criterion 3).
//!
//! This proves the durable path holds for an entire share set plus the encrypted
//! between-round DKG checkpoints: every seat's `KeyPackage` and a real
//! round-1/round-2 `SecretPackage` persist → reload byte-faithfully through the
//! participant + checkpoint stores under one store passphrase.
//!
//! It is `#[ignore]`d so it is EXCLUDED from the default `cargo test` suite — the
//! at-rest crypto is deliberately slow (age/scrypt at `log_n = 18` is ~1s per
//! encrypt/decrypt), so a full 51/100 run performs ~200 scrypt operations and
//! takes minutes even in `--release`. Phase 3 runs it on demand:
//!
//! ```text
//! cargo test --release --test store_checkpoint_n100 persist_reload_100 -- --ignored --nocapture
//! ```
//!
//! Scale defaults to the mandated acceptance target (t=51, n=100, D-02) so Phase 3
//! needs no edit — just run it. Both are overridable via `CHEGET_N100_T` /
//! `CHEGET_N100_N` for a faster local smoke run (the stores are generic over the
//! share set, D-01), mirroring the `CHEGET_DKG_T` / `CHEGET_DKG_N` seam in
//! `tests/dkg_100_correctness.rs`.

use std::collections::BTreeMap;

use frost_secp256k1_tr as frost;
use frost::keys::dkg;

use cheget::crypto::types::{Epoch, KeyId, SeatId};
use cheget::crypto::run_inprocess_dkg;
use cheget::store::{
    CeremonyId, CheckpointStore, InCodePassphrase, ParticipantStore, ShareState, ShareTag,
};

/// Read a `u16` from an environment variable, falling back to `default`.
fn env_u16(key: &str, default: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// A unique scratch store root under the system temp dir (no tempfile dep).
fn temp_root() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("cheget-n100-{}-{}-{}", std::process::id(), nanos, n))
}

/// Persist a full share set + a real between-round checkpoint pair, then reload
/// everything and assert byte-equality (STOR-02 durability at scale).
#[test]
#[ignore = "at-scale persist/reload — slow (age/scrypt log_n=18); run in Phase 3 with --release --ignored"]
fn persist_reload_100() {
    // Defaults to the acceptance target t=51, n=100 (D-02); overridable for a
    // faster local smoke run. Phase 3 runs the default full set.
    let n: u16 = env_u16("CHEGET_N100_N", 100);
    let t: u16 = env_u16("CHEGET_N100_T", 51);
    assert!(
        t >= 1 && t <= n,
        "require 1 <= t ({t}) <= n ({n})"
    );

    let root = temp_root();
    let passphrase = "n100-store-passphrase";

    // ---- Full share set through the participant store ----
    let (shares, group) = run_inprocess_dkg(t, n).expect("in-process DKG");
    assert_eq!(shares.len(), n as usize, "DKG must yield n shares");

    let store = ParticipantStore::new(
        root.clone(),
        Box::new(InCodePassphrase::new(passphrase)),
    );

    // Persist every seat's KeyPackage (encrypted) + the group public envelope.
    for (&seat, key_package) in &shares {
        let tag = ShareTag::new(KeyId::active(), Epoch::GENESIS, seat);
        store
            .put_share(&tag, key_package, &group, ShareState::Active)
            .expect("put_share");
    }

    // Reload every seat's KeyPackage and assert it is byte-equal to the original.
    for (&seat, key_package) in &shares {
        let tag = ShareTag::new(KeyId::active(), Epoch::GENESIS, seat);
        let loaded = store.load_share(&tag).expect("load_share");
        assert_eq!(
            loaded.serialize().unwrap(),
            key_package.serialize().unwrap(),
            "seat {seat:?} KeyPackage must persist→reload byte-equal"
        );
    }

    // ---- Between-round DKG checkpoints through the checkpoint store ----
    // A real part1 → checkpoint round1 → reload → part2 → checkpoint round2 →
    // reload for one seat, proving the encrypted between-round path holds. (Only
    // one seat's round secrets are exercised here; the full ceremony's
    // correctness is covered by dkg_100_correctness / run_inprocess_dkg.)
    let checkpoints = CheckpointStore::new(
        root.clone(),
        Box::new(InCodePassphrase::new(passphrase)),
    );
    let cid = CeremonyId::new("n100-ceremony").unwrap();
    // OsRng is a zero-sized Copy source of OS entropy — pass it by value per seat.
    let rng = frost::rand_core::OsRng;

    let mut r1_secret: BTreeMap<SeatId, dkg::round1::SecretPackage> = BTreeMap::new();
    let mut r1_pkgs: BTreeMap<SeatId, dkg::round1::Package> = BTreeMap::new();
    for i in 1..=n {
        let id: SeatId = i.try_into().expect("nonzero seat id");
        let (secret, pkg) = dkg::part1(id, n, t, rng).expect("part1");
        r1_secret.insert(id, secret);
        r1_pkgs.insert(id, pkg);
    }

    let seat: SeatId = 1u16.try_into().unwrap();
    let orig1 = r1_secret.remove(&seat).unwrap();
    checkpoints.put_round1(&cid, seat, &orig1).expect("put_round1");
    let loaded1 = checkpoints.load_round1(&cid, seat).expect("load_round1");
    assert_eq!(
        loaded1.serialize().unwrap(),
        orig1.serialize().unwrap(),
        "round-1 SecretPackage must persist→reload byte-equal"
    );

    let mut others = r1_pkgs.clone();
    others.remove(&seat);
    let (secret2, _sent) = dkg::part2(loaded1, &others).expect("part2 from reloaded round-1");
    checkpoints.put_round2(&cid, seat, &secret2).expect("put_round2");
    let loaded2 = checkpoints.load_round2(&cid, seat).expect("load_round2");
    assert_eq!(
        loaded2.serialize().unwrap(),
        secret2.serialize().unwrap(),
        "round-2 SecretPackage must persist→reload byte-equal"
    );

    // Wipe-on-success removes the ceremony's checkpoints (D-10).
    checkpoints.wipe(&cid).expect("wipe");
    assert!(
        checkpoints.load_round1(&cid, seat).is_err(),
        "wipe-on-success must remove the ceremony's checkpoint files"
    );

    std::fs::remove_dir_all(&root).ok();
}
