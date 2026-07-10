//! `address` command — print the group's BIP341 P2TR address.
//!
//! Stub for 01-01 Task 1; wired to the canonical bridge + public-artifact file
//! format in 01-01 Task 3 (KEY-04).

use clap::Args;

use super::CliResult;

/// Arguments for the address command.
#[derive(Debug, Args)]
pub struct AddressArgs {
    /// Path to a public-artifact file (serialized `PublicKeyPackage` envelope).
    #[arg(long)]
    pub pubkey: Option<std::path::PathBuf>,
}

/// Handler stub — wired in 01-01 Task 3.
pub fn run(_args: AddressArgs) -> CliResult {
    Err("address is not implemented yet (wired in plan 01-01 Task 3)".into())
}
