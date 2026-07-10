//! `keygen` command — in-process FROST DKG ceremony (D-08 simulate-all-seats).
//!
//! Phase 1 runs the whole `(t, n)` DKG in one process (no transport) and writes
//! only the **public** `PublicKeyPackage` envelope to disk (D-09); the secret
//! shares live in this process for the duration of the run and are never
//! serialized. The written artifact is exactly what `tsig watcher address
//! --pubkey <file>` consumes (KEY-04).

use std::path::PathBuf;

use clap::Args;

use super::address::PubkeyEnvelope;
use super::CliResult;
use crate::crypto::run_inprocess_dkg;

/// The real acceptance target (D-02): threshold `t` and membership `n`.
const FULL_THRESHOLD: u16 = 501;
const FULL_SEATS: u16 = 1000;
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
    /// Run the real acceptance target t=501, n=1000 (D-02). Without this flag a
    /// small 3-of-5 ceremony runs for fast local use; explicit
    /// `--seats`/`--threshold` take precedence over both.
    #[arg(long, default_value_t = false)]
    pub full: bool,
    /// Stable identifier stored in the public-artifact envelope.
    #[arg(long, default_value = "active")]
    pub key_id: String,
    /// Output path for the public `PublicKeyPackage` envelope (D-09). No secret
    /// share is ever written.
    #[arg(long)]
    pub out: PathBuf,
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

/// Handler: run the in-process DKG and write the public key package (D-09).
pub fn run(args: KeygenArgs) -> CliResult {
    let (t, n) = args.resolve_tn();
    if t == 0 || n == 0 || t > n {
        return Err(format!("invalid (t, n): threshold={t}, seats={n} (require 1 <= t <= n)").into());
    }

    // Simulate all seats in-process. The returned shares are dropped at the end
    // of this function — only the public package is persisted (D-09).
    let (_shares, group) = run_inprocess_dkg(t, n)?;

    let envelope = PubkeyEnvelope::from_package(args.key_id.clone(), 0, &group)?;
    let json = serde_json::to_vec_pretty(&envelope)?;
    std::fs::write(&args.out, &json)?;

    println!(
        "keygen complete: DKG t={t} of n={n}; wrote public key package \"{}\" to {}",
        args.key_id,
        args.out.display()
    );
    Ok(())
}
