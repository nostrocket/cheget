//! Headless persist/reload integration test (D-03).
//!
//! Proves the seam the Phase 3 n=100 persist/reload check relies on: with
//! `CHEGET_HOME` pointed at a temp dir and the **in-code** [`InCodePassphrase`]
//! (never the interactive impl, which is `#[cfg(not(test))]` and cannot link
//! here), a full `put_share` -> `load_share` round-trip runs with NO interactive
//! prompt and NO TTY. The public package and the manifest read with no
//! passphrase at all (D-05).
//!
//! This lives in the default `cargo test` suite (not `#[ignore]`d) so headless CI
//! exercises the whole store path on every run.

use cheget::crypto::run_inprocess_dkg;
use cheget::crypto::{Epoch, KeyId};
use cheget::store::{InCodePassphrase, ParticipantStore, ShareState, ShareTag, StoreRoot};

/// A unique scratch store root under the system temp dir.
fn temp_root() -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("cheget-headless-{}-{}", std::process::id(), nanos))
}

#[test]
fn headless_persist_reload_no_prompt() {
    let root_dir = temp_root();

    // CHEGET_HOME is the CI *path* seam — it drives store-root resolution and is
    // never a passphrase source. This test binary has a single test, so the
    // process-global env var is safe to set here.
    std::env::set_var("CHEGET_HOME", &root_dir);
    let root = StoreRoot::resolve().expect("CHEGET_HOME resolves the store root");
    assert_eq!(
        root.path(),
        root_dir.as_path(),
        "CHEGET_HOME must drive the resolved store root"
    );

    // A small in-process DKG yields real KeyPackages + the group package.
    let (shares, group) = run_inprocess_dkg(3, 5).expect("in-process DKG");
    let (&seat, key_package) = shares.iter().next().expect("at least one share");

    // The in-code PassphraseSource (D-03) — headless, no prompt, no TTY.
    let store = ParticipantStore::new(
        root.path(),
        Box::new(InCodePassphrase::new("headless-ci-passphrase")),
    );
    let tag = ShareTag::new(KeyId::active(), Epoch::GENESIS, seat);

    // Persist then reload: byte-equal KeyPackage, driven entirely headlessly.
    store
        .put_share(&tag, key_package, &group, ShareState::Active)
        .expect("put_share headless");
    let loaded = store.load_share(&tag).expect("load_share headless");
    assert_eq!(
        loaded.serialize().unwrap(),
        key_package.serialize().unwrap(),
        "KeyPackage must persist->reload byte-equal with no prompt"
    );

    // The public package reads with NO passphrase at all (D-05).
    let envelope = store
        .load_public_envelope(&KeyId::active(), Epoch::GENESIS)
        .expect("public envelope reads with no unlock");
    assert_eq!(
        envelope.decode_package().unwrap().verifying_key(),
        group.verifying_key(),
        "public envelope must decode to the group verifying key"
    );

    // The manifest reads unlock-free via the static seam (no PassphraseSource).
    let manifest = ParticipantStore::read_manifest(root.path()).expect("manifest reads unlock-free");
    assert!(
        !manifest.shares.is_empty(),
        "manifest indexes the persisted share"
    );

    std::env::remove_var("CHEGET_HOME");
    std::fs::remove_dir_all(&root_dir).ok();
}
