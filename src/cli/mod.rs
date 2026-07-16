//! L4 CLI — clap persona tree (participant / coordinator / watcher).
//!
//! Real entry points (D-08): commands parse and dispatch to handlers. In Phase 1
//! the ceremony commands run against the in-memory `Transport` stub (wired by
//! later plans); `keygen` and `sign` are explicit `unimplemented` stubs here and
//! are filled by 01-02 / 01-04. `address` is wired in 01-01 Task 3.
//!
//! CLI does no work itself — it only routes to the library.

pub mod address;
pub mod keygen;
pub mod sign;

use clap::{Args, Parser, Subcommand};

use crate::coordinator::CoordinatorStore;
use crate::store::{ParticipantStore, StoreError, StoreRoot};

/// Result type for CLI handlers.
pub type CliResult = Result<(), Box<dyn std::error::Error>>;

/// `cheget` — 51-of-100 FROST Taproot signing CLI.
#[derive(Debug, Parser)]
#[command(name = "cheget", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub persona: Persona,
}

/// Top-level persona selection. Each persona exposes only the commands that
/// persona is expected to run (SPEC §5).
#[derive(Debug, Subcommand)]
pub enum Persona {
    /// Participant persona: hold a share, join ceremonies, sign.
    #[command(subcommand)]
    Participant(ParticipantCmd),
    /// Coordinator persona: drive ceremonies and signing sessions.
    #[command(subcommand)]
    Coordinator(CoordinatorCmd),
    /// Watcher persona: read-only address/policy inspection.
    #[command(subcommand)]
    Watcher(WatcherCmd),
}

/// Participant subcommands.
#[derive(Debug, Subcommand)]
pub enum ParticipantCmd {
    /// Join a keygen (DKG) ceremony. (Wired in 01-02.)
    Keygen(keygen::KeygenArgs),
    /// Join a signing session (round1 commit + round2 sign). (Wired in 01-04.)
    Sign(sign::SignArgs),
    /// List held shares from the store — reads the plaintext manifest with NO
    /// unlock and no `--pubkey` file (D-05); never prompts for a passphrase.
    ShareStatus(ShareStatusArgs),
}

/// Coordinator subcommands.
#[derive(Debug, Subcommand)]
pub enum CoordinatorCmd {
    /// Run a keygen ceremony as coordinator. (Wired in 01-02.)
    Keygen(keygen::KeygenArgs),
    /// Run a signing session from a PSBT. (Wired in 01-04.)
    Sign(sign::SignArgs),
    /// List the roster from the coordinator's public SQLite store (STOR-03).
    Roster(RosterArgs),
}

/// Arguments for `participant share-status`.
#[derive(Debug, Args)]
pub struct ShareStatusArgs {
    /// Optional store root override (defaults to `CHEGET_HOME` or `~/.cheget`).
    #[arg(long)]
    pub home: Option<std::path::PathBuf>,
}

/// Arguments for `coordinator roster`.
#[derive(Debug, Args)]
pub struct RosterArgs {
    /// Group-key label to list (`active` | `standby`).
    #[arg(long, default_value = "active")]
    pub key_id: String,
    /// Optional store root override (defaults to `CHEGET_HOME` or `~/.cheget`).
    #[arg(long)]
    pub home: Option<std::path::PathBuf>,
}

/// Resolve the store root, honoring an explicit `--home` override over the
/// `CHEGET_HOME`/`~/.cheget` resolution.
///
/// `pub(crate)` so the keygen writer (03-01) and the sign reader (03-02) share
/// one root-resolution path.
pub(crate) fn resolve_root(
    home: Option<std::path::PathBuf>,
) -> Result<std::path::PathBuf, StoreError> {
    match home {
        Some(path) => Ok(path),
        None => Ok(StoreRoot::resolve()?.path().to_path_buf()),
    }
}

/// Acquire the single store passphrase at the thin CLI edge (D-04).
///
/// This is the ONLY place the interactive prompt is constructed; the persist /
/// load loops take a [`crate::store::PassphraseSource`] so they never touch a
/// terminal (pattern-map hazard 3). `confirm` selects the create-time
/// confirm-twice `for_new_store` path vs. the single-prompt `for_unlock` path.
#[cfg(not(test))]
pub(crate) fn acquire_store_passphrase(
    confirm: bool,
) -> Result<age::secrecy::SecretString, StoreError> {
    use crate::store::InteractivePassphrase;
    use crate::store::PassphraseSource;
    let source = if confirm {
        InteractivePassphrase::for_new_store()
    } else {
        InteractivePassphrase::for_unlock()
    };
    source.passphrase()
}

/// Test build: `InteractivePassphrase` is `#[cfg(not(test))]`, so return a fixed
/// passphrase to keep the lib compiling under `cargo test` without linking a
/// terminal prompt (pattern-map hazard 3).
#[cfg(test)]
pub(crate) fn acquire_store_passphrase(
    _confirm: bool,
) -> Result<age::secrecy::SecretString, StoreError> {
    Ok(age::secrecy::SecretString::from(
        "test-store-passphrase".to_string(),
    ))
}

/// Handler: list held shares from the store with no unlock (D-05).
fn run_share_status(args: ShareStatusArgs) -> CliResult {
    let root = resolve_root(args.home)?;
    let manifest = ParticipantStore::read_manifest(&root)?;
    if manifest.shares.is_empty() {
        println!("no shares held");
        return Ok(());
    }
    println!("KEY_ID\tEPOCH\tSEAT\tSTATE");
    for entry in &manifest.shares {
        println!(
            "{}\t{}\t{}\t{:?}",
            entry.key_id, entry.epoch, entry.seat, entry.state
        );
    }
    Ok(())
}

/// Handler: list the coordinator roster from the public SQLite store.
fn run_roster(args: RosterArgs) -> CliResult {
    let root = resolve_root(args.home)?;
    let db_path = CoordinatorStore::default_db_path(&root);
    let store = CoordinatorStore::open(&db_path)?;
    let roster = store.list_roster(&args.key_id)?;
    if roster.is_empty() {
        println!("roster empty for key_id={}", args.key_id);
        return Ok(());
    }
    println!("IDENTIFIER\tSEAT\tNPUB\tSTATUS\tJOIN\tLEAVE");
    for r in &roster {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            r.identifier,
            r.seat_index.map(|s| s.to_string()).unwrap_or_default(),
            r.npub,
            r.status,
            r.join_epoch,
            r.leave_epoch.map(|e| e.to_string()).unwrap_or_default(),
        );
    }
    Ok(())
}

/// Watcher subcommands.
#[derive(Debug, Subcommand)]
pub enum WatcherCmd {
    /// Print the group's BIP341 P2TR address from a public-key-package file.
    Address(address::AddressArgs),
}

impl Cli {
    /// Dispatch the parsed command to its handler.
    pub fn run(self) -> CliResult {
        match self.persona {
            Persona::Participant(cmd) => match cmd {
                ParticipantCmd::Keygen(args) => keygen::run(args),
                ParticipantCmd::Sign(args) => sign::run(args),
                ParticipantCmd::ShareStatus(args) => run_share_status(args),
            },
            Persona::Coordinator(cmd) => match cmd {
                CoordinatorCmd::Keygen(args) => keygen::run(args),
                CoordinatorCmd::Sign(args) => sign::run(args),
                CoordinatorCmd::Roster(args) => run_roster(args),
            },
            Persona::Watcher(cmd) => match cmd {
                WatcherCmd::Address(args) => address::run(args),
            },
        }
    }
}
