//! `sign` command — two-round FROST signing (participant join / coordinator run).
//!
//! Stub entry point (D-08). The real signing session (SIGN-01..07: liveness poll,
//! round1 commit, display-before-sign gate, round2 `sign_with_tweak`,
//! `aggregate_with_tweak(.., None)`, verify against `Q`) is wired by plan **01-04**.

use clap::Args;

use super::CliResult;

/// Arguments for a signing session.
#[derive(Debug, Args)]
pub struct SignArgs {
    /// Session identifier (participant join).
    #[arg(long)]
    pub session: Option<String>,
    /// PSBT file to sign (coordinator).
    #[arg(long)]
    pub psbt: Option<std::path::PathBuf>,
    /// Skip the human display-before-sign acknowledgement. Automation/regtest
    /// only — never the interactive default (SIGN-07).
    #[arg(long, default_value_t = false)]
    pub yes: bool,
}

/// Handler stub — filled in 01-04.
pub fn run(_args: SignArgs) -> CliResult {
    Err("sign is not implemented yet (wired in plan 01-04)".into())
}
