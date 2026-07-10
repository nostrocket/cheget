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

use clap::{Parser, Subcommand};

/// Result type for CLI handlers.
pub type CliResult = Result<(), Box<dyn std::error::Error>>;

/// `tsig` — 501-of-1000 FROST Taproot signing CLI.
#[derive(Debug, Parser)]
#[command(name = "tsig", version, about, long_about = None)]
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
}

/// Coordinator subcommands.
#[derive(Debug, Subcommand)]
pub enum CoordinatorCmd {
    /// Run a keygen ceremony as coordinator. (Wired in 01-02.)
    Keygen(keygen::KeygenArgs),
    /// Run a signing session from a PSBT. (Wired in 01-04.)
    Sign(sign::SignArgs),
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
            },
            Persona::Coordinator(cmd) => match cmd {
                CoordinatorCmd::Keygen(args) => keygen::run(args),
                CoordinatorCmd::Sign(args) => sign::run(args),
            },
            Persona::Watcher(cmd) => match cmd {
                WatcherCmd::Address(args) => address::run(args),
            },
        }
    }
}
