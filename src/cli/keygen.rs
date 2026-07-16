//! `keygen` command — in-process FROST DKG ceremony (D-08 simulate-all-seats).
//!
//! Two output paths, selectable together:
//!
//! * `--out <file>` (Phase 1, D-09): runs the whole `(t, n)` DKG in one process
//!   and writes only the **public** `PublicKeyPackage` envelope to disk; the
//!   secret shares live in-process for the run and are never serialized. The
//!   written artifact is exactly what `cheget watcher address --pubkey <file>`
//!   consumes (KEY-04).
//! * `--persist` (Phase 3, D-02/D-03/D-04): runs the same DKG, then persists
//!   every seat through its OWN encrypted [`ParticipantStore`] rooted at
//!   `<base>/seat-NNNN/` — each seat's `KeyPackage` age/scrypt-encrypted plus the
//!   plaintext group package — under a SINGLE prompt-once passphrase. This makes
//!   `keygen` the first command that drives `InteractivePassphrase::for_new_store`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use clap::Args;
use frost_secp256k1_tr as frost;
use frost::keys::{KeyPackage, PublicKeyPackage};
use frost::Identifier;

use super::address::PubkeyEnvelope;
use super::CliResult;
use crate::crypto::run_inprocess_dkg;
use crate::crypto::types::{Epoch, KeyId};
use crate::store::{ParticipantStore, PassphraseSource, ResolvedPassphrase, ShareState, ShareTag};

/// The real acceptance target (D-02): threshold `t` and membership `n`.
const FULL_THRESHOLD: u16 = 51;
const FULL_SEATS: u16 = 100;
/// Small interactive defaults for local use / TDD-speed runs (D-01).
const SMALL_THRESHOLD: u16 = 3;
const SMALL_SEATS: u16 = 5;

/// Arguments for the keygen ceremony.
#[derive(Debug, Args)]
pub struct KeygenArgs {
    /// Ceremony identifier (informational in the in-process Phase 1 flow).
    #[arg(long)]
    pub ceremony: Option<String>,
    /// Number of seats (n). Overrides the size implied by `--full`.
    #[arg(long)]
    pub seats: Option<u16>,
    /// Threshold (t). Overrides the size implied by `--full`.
    #[arg(long)]
    pub threshold: Option<u16>,
    /// Run the real acceptance target t=51, n=100 (D-02). Without this flag a
    /// small 3-of-5 ceremony runs for fast local use; explicit
    /// `--seats`/`--threshold` take precedence over both.
    #[arg(long, default_value_t = false)]
    pub full: bool,
    /// Stable identifier stored in the public-artifact envelope.
    #[arg(long, default_value = "active")]
    pub key_id: String,
    /// Output path for the public `PublicKeyPackage` envelope (D-09). No secret
    /// share is ever written here. Optional when `--persist` is given.
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Persist every seat's encrypted share to its own store root (D-02/D-03).
    /// Prompts once for a new-store passphrase and reuses it for all seats (D-04).
    #[arg(long, default_value_t = false)]
    pub persist: bool,
    /// Store base directory for `--persist` (defaults to `CHEGET_HOME`/`~/.cheget`).
    #[arg(long)]
    pub base: Option<PathBuf>,
}

impl KeygenArgs {
    /// Resolve the effective `(threshold, seats)` from the flags.
    fn resolve_tn(&self) -> (u16, u16) {
        let (default_t, default_n) = if self.full {
            (FULL_THRESHOLD, FULL_SEATS)
        } else {
            (SMALL_THRESHOLD, SMALL_SEATS)
        };
        (
            self.threshold.unwrap_or(default_t),
            self.seats.unwrap_or(default_n),
        )
    }
}

/// Run the in-process DKG and persist every seat to its OWN encrypted store root
/// under `base` (D-02/D-03), driven by an injected passphrase SOURCE.
///
/// This is the ENTIRE non-interactive write glue: run the DKG (binding — not
/// discarding — the secret shares), resolve the passphrase ONCE, then for each
/// seat build `<base>/seat-NNNN/` and persist through [`ParticipantStore::put_share`]
/// (which already writes the plaintext public envelope, the encrypted share, and
/// the manifest-last per D-07 — this fn re-implements none of that).
///
/// It takes a `&dyn PassphraseSource` and NEVER prompts: the interactive prompt
/// stays at the thin CLI edge (pattern-map hazard 3), and tests inject an
/// `InCodePassphrase` in place of the production `ResolvedPassphrase`.
///
/// It is `pub` (not `pub(crate)`) so the integration test in `tests/` — which
/// links `cheget` as an external crate — can drive it directly.
///
/// Returns the in-memory `(shares, group)`. The `run` handler discards the
/// shares (they are already persisted); the small-n test uses them only for the
/// byte-equal reload assertion. No plaintext-share disk path is added.
pub fn persist_dkg_shares(
    base: &Path,
    t: u16,
    n: u16,
    passphrase: &dyn PassphraseSource,
) -> Result<(BTreeMap<Identifier, KeyPackage>, PublicKeyPackage), Box<dyn std::error::Error>> {
    let (shares, group) = run_inprocess_dkg(t, n)?;
    // Resolve the secret ONCE; every per-seat store reuses a clone (D-04).
    let secret = passphrase.passphrase()?;

    for (i, (&seat, key_package)) in shares.iter().enumerate() {
        let root = base.join(format!("seat-{:04}", i + 1));
        let store = ParticipantStore::new(root, Box::new(ResolvedPassphrase::new(secret.clone())));
        let tag = ShareTag::new(KeyId::active(), Epoch::GENESIS, seat);
        store.put_share(&tag, key_package, &group, ShareState::Active)?;
    }

    Ok((shares, group))
}

/// Write the public `PubkeyEnvelope` (plaintext, no secret) to `out` (D-09).
fn write_public_envelope(
    key_id: &str,
    group: &PublicKeyPackage,
    out: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let envelope = PubkeyEnvelope::from_package(key_id.to_string(), 0, group)?;
    let json = serde_json::to_vec_pretty(&envelope)?;
    std::fs::write(out, &json)?;
    Ok(())
}

/// Handler: run the in-process DKG and write the public envelope and/or persist
/// the encrypted per-seat share set.
pub fn run(args: KeygenArgs) -> CliResult {
    let (t, n) = args.resolve_tn();
    if t == 0 || n == 0 || t > n {
        return Err(format!("invalid (t, n): threshold={t}, seats={n} (require 1 <= t <= n)").into());
    }
    if args.out.is_none() && !args.persist {
        return Err("keygen requires at least one of --out <file> or --persist".into());
    }

    if args.persist {
        // Resolve the store base (honors CHEGET_HOME/~/.cheget) and prompt ONCE
        // via the confirm-twice for_new_store path (D-04).
        let base = crate::cli::resolve_root(args.base.clone())?;
        let secret = crate::cli::acquire_store_passphrase(true)?;
        // Delegate the whole write glue; the returned shares are already
        // persisted, so the handler discards them (no new plaintext-share path).
        let (_shares, group) =
            persist_dkg_shares(&base, t, n, &ResolvedPassphrase::new(secret))?;
        if let Some(out) = &args.out {
            write_public_envelope(&args.key_id, &group, out)?;
        }
        println!(
            "keygen complete: DKG t={t} of n={n}; persisted {n} encrypted seat roots under {}",
            base.display()
        );
    } else {
        // --out-only: Phase-1 compatibility — standalone DKG + public envelope.
        let (_shares, group) = run_inprocess_dkg(t, n)?;
        let out = args.out.as_ref().expect("--out is Some when --persist is false");
        write_public_envelope(&args.key_id, &group, out)?;
        println!(
            "keygen complete: DKG t={t} of n={n}; wrote public key package \"{}\" to {}",
            args.key_id,
            out.display()
        );
    }
    Ok(())
}
