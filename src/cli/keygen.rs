//! `keygen` command — DKG ceremony (participant join / coordinator run).
//!
//! Stub entry point (D-08). The real in-process DKG (KEY-01/02/05/06) is wired by
//! plan **01-02** against the in-memory `Transport` stub.

use clap::Args;

use super::CliResult;

/// Arguments for the keygen ceremony.
#[derive(Debug, Args)]
pub struct KeygenArgs {
    /// Ceremony identifier.
    #[arg(long)]
    pub ceremony: Option<String>,
    /// Number of seats (n). Defaults to the fixed n=1000 at ceremony time.
    #[arg(long)]
    pub seats: Option<u16>,
    /// Threshold (t). Fixed at 501; exposed for small-n local testing.
    #[arg(long)]
    pub threshold: Option<u16>,
}

/// Handler stub — filled in 01-02.
pub fn run(_args: KeygenArgs) -> CliResult {
    Err("keygen is not implemented yet (wired in plan 01-02)".into())
}
