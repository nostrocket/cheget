//! `sign` command — two-round FROST signing over the in-memory `Transport` stub.
//!
//! Phase 1 has **no shares on disk** (D-09: only the public `PublicKeyPackage` is
//! ever persisted), so this command runs a simulate-all-seats DKG in-process
//! (D-08), derives the group address, and drives a [`SigningSession`] over the
//! provided PSBT — the same crown-jewel pipeline the integration tests confirm on
//! regtest, minus the chain. The PSBT must spend the group address the command
//! prints; real signing of an externally-funded PSBT needs persisted shares,
//! which land in Phase 2.
//!
//! The display-before-sign gate (SIGN-07) always recomputes the sighash from the
//! PSBT; `--yes` bypasses only the interactive human ack (automation/regtest).

use std::io::Write;
use std::path::PathBuf;

use bitcoin::Psbt;
use clap::Args;

use super::CliResult;
use crate::bridge::address_from_group_key;
use crate::cli::address::Network;
use crate::crypto::run_inprocess_dkg;
use crate::session::SigningSession;
use crate::transport::InMemoryTransport;

/// The real acceptance target (D-02): threshold `t` and membership `n`.
const FULL_THRESHOLD: u16 = 501;
const FULL_SEATS: u16 = 1000;
/// Small interactive defaults for local use / fast runs (D-01).
const SMALL_THRESHOLD: u16 = 3;
const SMALL_SEATS: u16 = 5;

/// Arguments for a signing session.
#[derive(Debug, Args)]
pub struct SignArgs {
    /// Session identifier (informational in the in-process Phase 1 flow).
    #[arg(long)]
    pub session: Option<String>,
    /// PSBT file to sign (raw consensus bytes). Required to run a session.
    #[arg(long)]
    pub psbt: Option<PathBuf>,
    /// Key label (informational; the active group key).
    #[arg(long, default_value = "active")]
    pub key: String,
    /// Number of seats (n) for the in-process simulated DKG. Overrides `--full`.
    #[arg(long)]
    pub seats: Option<u16>,
    /// Threshold (t) for the in-process simulated DKG. Overrides `--full`.
    #[arg(long)]
    pub threshold: Option<u16>,
    /// Simulate the real acceptance target t=501, n=1000 (D-02). Slow.
    #[arg(long, default_value_t = false)]
    pub full: bool,
    /// Network for rendering the group address.
    #[arg(long, value_enum, default_value_t = Network::Regtest)]
    pub network: Network,
    /// Skip the human display-before-sign acknowledgement. Automation/regtest
    /// only — never the interactive default (SIGN-07).
    #[arg(long, default_value_t = false)]
    pub yes: bool,
}

impl SignArgs {
    /// Resolve the effective `(threshold, seats)` for the simulated DKG.
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

/// Read a PSBT from a file holding its raw consensus (binary) serialization.
fn read_psbt(path: &PathBuf) -> Result<Psbt, Box<dyn std::error::Error>> {
    let raw = std::fs::read(path)?;
    Ok(Psbt::deserialize(&raw)?)
}

/// Prompt the operator to authorize the spend (interactive display gate).
fn prompt_ack() -> bool {
    eprint!("Type 'yes' to authorize signing this spend: ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).is_ok()
        && matches!(line.trim(), "yes" | "y" | "YES" | "Y")
}

/// Handler: run the in-process signing session over the `Transport` stub.
pub fn run(args: SignArgs) -> CliResult {
    let psbt_path = args.psbt.as_ref().ok_or(
        "sign requires --psbt <file> (the transaction to sign); the coordinator distributes \
         the PSBT, never a precomputed sighash (SIGN-07)",
    )?;
    let psbt = read_psbt(psbt_path)?;

    let (t, n) = args.resolve_tn();
    if t == 0 || n == 0 || t > n {
        return Err(format!("invalid (t, n): threshold={t}, seats={n} (require 1 <= t <= n)").into());
    }

    // Phase 1: simulate all seats in-process (D-08). Shares live only in this
    // process and drop at the end of the run; nothing secret is persisted (D-09).
    let (key_packages, group) = run_inprocess_dkg(t, n)?;

    let addr = address_from_group_key(group.verifying_key(), args.network.known_hrp())?;
    println!("group address (t={t} of n={n}, key \"{}\"): {addr}", args.key);
    println!(
        "note: Phase 1 simulates all seats in-process; the PSBT must spend this address \
         (persisted shares arrive in Phase 2)."
    );

    let transport = InMemoryTransport::new();
    let btc_network = args.network.bitcoin_network();
    let mut session = SigningSession::new(
        args.session.clone().unwrap_or_else(|| "cli-session".to_string()),
        &transport,
        key_packages,
        group,
        psbt,
        t as usize,
        btc_network,
    );

    // Display-before-sign: show the spend and require an ack unless --yes.
    let summary = session.preview()?;
    if !args.yes {
        eprintln!("Spend to authorize:\n{summary}");
        if !prompt_ack() {
            return Err("signing not authorized (no acknowledgement)".into());
        }
    }

    // The gate still recomputes the sighash from the PSBT even with the ack given.
    let signed = session.run(true)?;
    let txid = signed.compute_txid();
    let raw_hex = bitcoin::consensus::encode::serialize_hex(&signed);
    println!("signed key-spend txid: {txid}");
    println!("raw tx: {raw_hex}");
    Ok(())
}
