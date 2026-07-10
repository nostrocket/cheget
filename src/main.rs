//! `tsig` binary entry point — persona dispatch only.
//!
//! All work lives in the library (`tsig::*`); `main` merely parses the clap
//! persona tree and routes to a handler, mapping errors to a nonzero exit code.

use std::process::ExitCode;

use clap::Parser;
use tsig::cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("tsig: error: {err}");
            ExitCode::FAILURE
        }
    }
}
