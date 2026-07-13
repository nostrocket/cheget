//! `cheget` binary entry point — persona dispatch only.
//!
//! All work lives in the library (`cheget::*`); `main` merely parses the clap
//! persona tree and routes to a handler, mapping errors to a nonzero exit code.

use std::process::ExitCode;

use clap::Parser;
use cheget::cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("cheget: error: {err}");
            ExitCode::FAILURE
        }
    }
}
