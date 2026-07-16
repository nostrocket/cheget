//! Small-n writer-correctness test for the `keygen --persist` write topology
//! (KEY-06 write half, D-02/D-03/D-04).
//!
//! This DRIVES THE HANDLER'S OWN GLUE — `cheget::cli::keygen::persist_dkg_shares`
//! — rather than re-implementing the DKG + per-seat persist loop. It injects an
//! `InCodePassphrase` in place of the CLI's production `ResolvedPassphrase`
//! (the interactive prompt edge is unreachable from tests — pattern-map hazard 3
//! — and the helper takes a passphrase SOURCE precisely so this substitution
//! works), then reopens each `<base>/seat-NNNN/` root the helper created and
//! asserts:
//!   1. each persisted `KeyPackage` reloads byte-equal to the DKG output;
//!   2. the plaintext public envelope decodes to the group verifying key;
//!   3. each per-seat manifest holds exactly one `Active` entry.
//!
//! Defaults to t=2, n=3 for a fast scrypt cost (age/scrypt log_n=18 is ~1s per
//! op); overridable via `CHEGET_KEYGEN_T` / `CHEGET_KEYGEN_N` mirroring the other
//! test seams. The at-scale n=100 durability check lives in
//! `tests/store_checkpoint_n100.rs::persist_reload_100` (kept `#[ignore]`d).

use cheget::cli::keygen::persist_dkg_shares;
use cheget::crypto::types::{Epoch, KeyId};
use cheget::store::{InCodePassphrase, ParticipantStore, ShareState, ShareTag};

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
    std::env::temp_dir().join(format!("cheget-keygen-{}-{}-{}", std::process::id(), nanos, n))
}

#[test]
fn persist_dkg_shares_writes_reloadable_per_seat_roots() -> Result<(), Box<dyn std::error::Error>> {
    let n: u16 = env_u16("CHEGET_KEYGEN_N", 3);
    let t: u16 = env_u16("CHEGET_KEYGEN_T", 2);
    assert!(t >= 1 && t <= n, "require 1 <= t ({t}) <= n ({n})");

    let base = temp_base();
    let passphrase = "test-pass";

    // Drive the handler's real write glue (not a re-implemented loop).
    let (shares, group) =
        persist_dkg_shares(&base, t, n, &InCodePassphrase::new(passphrase))?;
    assert_eq!(shares.len(), n as usize, "DKG must yield n shares");

    // The helper enumerates `shares` (a BTreeMap, stable order) 1-based into
    // `seat-NNNN`; walk the same enumeration to map each root to its share.
    for (i, (&seat, key_package)) in shares.iter().enumerate() {
        let root = base.join(format!("seat-{:04}", i + 1));
        let store =
            ParticipantStore::new(root.clone(), Box::new(InCodePassphrase::new(passphrase)));

        // (1) Encrypted share reloads byte-equal to the DKG output.
        let tag = ShareTag::new(KeyId::active(), Epoch::GENESIS, seat);
        let loaded = store.load_share(&tag)?;
        assert_eq!(
            loaded.serialize().unwrap(),
            key_package.serialize().unwrap(),
            "seat {seat:?} KeyPackage must persist->reload byte-equal"
        );

        // (2) Plaintext public envelope decodes to the group verifying key.
        let envelope = store.load_public_envelope(&KeyId::active(), Epoch::GENESIS)?;
        let decoded = envelope.decode_package()?;
        assert_eq!(
            decoded.verifying_key(),
            group.verifying_key(),
            "seat {seat:?} public envelope must decode to the group verifying key"
        );

        // (3) Per-seat manifest holds exactly one Active entry.
        let manifest = ParticipantStore::read_manifest(&root)?;
        assert_eq!(
            manifest.shares.len(),
            1,
            "seat {seat:?} manifest must hold exactly one entry"
        );
        assert!(
            matches!(manifest.shares[0].state, ShareState::Active),
            "seat {seat:?} manifest entry must be Active"
        );
    }

    std::fs::remove_dir_all(&base).ok();
    Ok(())
}
