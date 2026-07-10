//! Display-before-sign gate (SIGN-07, Pattern 4, Pitfall 8) — never trust the
//! coordinator.
//!
//! A malicious coordinator can hand a signer a sighash that does not correspond
//! to the transaction the signer believes it is authorizing. The only defence is
//! that each signer **recomputes the sighash itself, from the PSBT**, and refuses
//! to sign if a human-readable summary disagrees. The coordinator therefore sends
//! the PSBT, never a precomputed hash.
//!
//! [`display_and_ack`] is the gate the session runs before every round-2 share:
//!
//! 1. recompute the key-spend sighash from the transaction + prevouts via the ONE
//!    canonical [`key_spend_sighash`] helper (the same code the coordinator used);
//! 2. refuse ([`DisplayError::BlindSignMismatch`]) if it disagrees with the hash
//!    the coordinator put in the `SigningPackage`;
//! 3. render the outputs / amounts / fee, and require an explicit ack — bypassed
//!    only by `--yes`, which is for automation/regtest and is loudly flagged.

use bitcoin::{Address, Amount, Network, Transaction, TxOut};

use crate::chain::{key_spend_sighash, ChainError};

/// Errors from the display-before-sign gate.
#[derive(Debug)]
pub enum DisplayError {
    /// The sighash recomputed from the PSBT disagrees with the coordinator's —
    /// a blind-sign attempt; the signer refuses (SIGN-07).
    BlindSignMismatch {
        /// The input whose recomputed sighash disagreed.
        input_index: usize,
    },
    /// No `--yes` was given and no interactive ack was obtained, so the signer
    /// declined to sign. Automation must pass `--yes`; interactive callers prompt.
    AckRequired,
    /// Recomputing the key-spend sighash failed.
    Sighash(ChainError),
}

impl std::fmt::Display for DisplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayError::BlindSignMismatch { input_index } => write!(
                f,
                "refusing to sign input {input_index}: the sighash recomputed from the PSBT \
                 does not match the coordinator's (blind-sign attempt, SIGN-07)"
            ),
            DisplayError::AckRequired => write!(
                f,
                "signing declined: no acknowledgement (pass --yes for automation, or confirm \
                 interactively)"
            ),
            DisplayError::Sighash(e) => write!(f, "recomputing key-spend sighash: {e}"),
        }
    }
}

impl std::error::Error for DisplayError {}

/// One output line in a human-readable spend summary.
#[derive(Debug, Clone)]
pub struct OutputLine {
    /// The decoded destination address, if the script is a standard address.
    pub address: Option<String>,
    /// The output amount.
    pub value: Amount,
}

/// A human-readable summary of what a transaction spends.
#[derive(Debug, Clone)]
pub struct SpendSummary {
    /// Total value of the inputs (sum of the prevouts).
    pub input_total: Amount,
    /// The outputs (destination + amount).
    pub outputs: Vec<OutputLine>,
    /// The fee (`input_total − Σ output value`).
    pub fee: Amount,
}

impl std::fmt::Display for SpendSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "  inputs total: {}", self.input_total)?;
        for (i, out) in self.outputs.iter().enumerate() {
            let dest = out.address.as_deref().unwrap_or("<non-standard script>");
            writeln!(f, "  output[{i}]: {} → {}", out.value, dest)?;
        }
        write!(f, "  fee: {}", self.fee)
    }
}

/// Summarize what `tx` spends, given the full prevout set (Taproot signs all
/// prevouts). Amounts and the fee are derived from the prevouts and outputs.
pub fn summarize(tx: &Transaction, prevouts: &[TxOut], network: Network) -> SpendSummary {
    let input_total: Amount = prevouts.iter().map(|p| p.value).sum();
    let output_total: Amount = tx.output.iter().map(|o| o.value).sum();
    // Saturating: a malformed tx with outputs exceeding inputs shows a zero fee
    // rather than panicking — the signer still sees the (implausible) amounts.
    let fee = input_total.checked_sub(output_total).unwrap_or(Amount::ZERO);

    let outputs = tx
        .output
        .iter()
        .map(|o| OutputLine {
            address: Address::from_script(&o.script_pubkey, network)
                .ok()
                .map(|a| a.to_string()),
            value: o.value,
        })
        .collect();

    SpendSummary { input_total, outputs, fee }
}

/// The display-before-sign gate for one input (SIGN-07).
///
/// Recomputes the key-spend sighash from `tx` + `prevouts`, refuses with
/// [`DisplayError::BlindSignMismatch`] if it differs from `coordinator_message`,
/// renders the summary, and returns it after the ack. With `yes = false` the
/// gate returns [`DisplayError::AckRequired`] (the CLI layer prompts
/// interactively before calling with an effective ack); `yes = true` bypasses
/// only the human ack — the blind-sign recompute check always runs first.
pub fn display_and_ack(
    tx: &Transaction,
    prevouts: &[TxOut],
    input_index: usize,
    coordinator_message: &[u8],
    yes: bool,
    network: Network,
) -> Result<SpendSummary, DisplayError> {
    use bitcoin::hashes::Hash;

    // 1. Recompute the sighash INDEPENDENTLY from the PSBT — never trust the
    //    coordinator-supplied value.
    let recomputed =
        key_spend_sighash(tx, input_index, prevouts).map_err(DisplayError::Sighash)?;
    if recomputed.as_byte_array().as_slice() != coordinator_message {
        return Err(DisplayError::BlindSignMismatch { input_index });
    }

    // 2. Render the human-readable summary.
    let summary = summarize(tx, prevouts, network);
    eprintln!("Review this spend before signing (input {input_index}):");
    eprintln!("{summary}");

    // 3. Require an ack unless bypassed for automation.
    if !yes {
        return Err(DisplayError::AckRequired);
    }
    eprintln!("  [--yes] acknowledgement bypassed (automation/regtest)");
    Ok(summary)
}
