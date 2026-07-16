//! `sign` command — two-round FROST signing over the in-memory `Transport` stub.
//!
//! Two sources of key material, selected by `--persist`, feeding one identical
//! signing pipeline:
//!
//! * **`--persist` (Phase 3, D-05):** load `t` of the per-seat `seat-NNNN` store
//!   roots written by `keygen --persist` — each root's sole active `KeyPackage`
//!   decrypted under a single prompt-once unlock — plus the plaintext group
//!   `PublicKeyPackage`, then drive the [`SigningSession`] over the provided PSBT.
//!   This is the store→sign half of KEY-06: the confirmed key-spend is produced
//!   from PERSISTED shares.
//! * **default (Phase-1 compatibility, D-08):** no shares on disk — run a
//!   simulate-all-seats DKG in-process, derive the group address, and drive the
//!   same session. The PSBT must spend the address the command prints.
//!
//! Only the SOURCE of `key_packages`/`group` changes between modes. The
//! display-before-sign gate (SIGN-07) always recomputes the sighash from the
//! PSBT; `--yes` bypasses only the interactive human ack (automation/regtest).
//! Signing nonces remain in-memory only (SIGN-05) — no persistence is added here.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use bitcoin::Psbt;
use clap::Args;
use frost_secp256k1_tr as frost;
use frost::keys::{KeyPackage, PublicKeyPackage};
use frost::Identifier;

use super::CliResult;
use crate::bridge::address_from_group_key;
use crate::cli::address::Network;
use crate::crypto::run_inprocess_dkg;
use crate::crypto::types::{Epoch, KeyId};
use crate::session::SigningSession;
use crate::store::{ParticipantStore, ResolvedPassphrase};
use crate::transport::InMemoryTransport;

/// The real acceptance target (D-02): threshold `t` and membership `n`.
const FULL_THRESHOLD: u16 = 51;
const FULL_SEATS: u16 = 100;
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
    /// Simulate the real acceptance target t=51, n=100 (D-02). Slow.
    #[arg(long, default_value_t = false)]
    pub full: bool,
    /// Network for rendering the group address.
    #[arg(long, value_enum, default_value_t = Network::Regtest)]
    pub network: Network,
    /// Load `t` persisted seat roots from the store instead of running a fresh
    /// in-process DKG (D-05). Prompts once (no-echo) to unlock the shares.
    #[arg(long, default_value_t = false)]
    pub persist: bool,
    /// Store base directory for `--persist` (defaults to `CHEGET_HOME`/`~/.cheget`).
    #[arg(long)]
    pub base: Option<PathBuf>,
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

/// Load `t` persisted seat roots from `base` and assemble the signing inputs —
/// the ENTIRE non-interactive read glue for `sign --persist` (D-05).
///
/// Discovers `seat-*` roots under `base`, sorts them for a deterministic order,
/// errors clearly if fewer than `t` exist, then takes the first `t` (in
/// production `t = 51`, so first-`t` is the D-05 first-51; the [`SigningSession`]
/// still selects `t` via its own liveness poll). The unlock secret is resolved
/// ONCE from the injected source and a [`ResolvedPassphrase`] clone drives each
/// per-seat [`ParticipantStore`]; each root yields its sole active
/// `(seat, KeyPackage)` via [`ParticipantStore::load_only_active`]. The group
/// [`PublicKeyPackage`] is read from the first selected root WITHOUT any unlock
/// (the public envelope needs no passphrase).
///
/// Takes a passphrase SOURCE and NEVER prompts — the interactive prompt stays at
/// the CLI edge (pattern-map hazard 3) — and is `pub` (not `pub(crate)`) so the
/// 03-02 integration test can drive it with an `InCodePassphrase` (the lib target
/// links `tests/` as an external crate).
pub fn load_persisted_shares(
    base: &Path,
    t: u16,
    passphrase: &dyn crate::store::PassphraseSource,
) -> Result<(BTreeMap<Identifier, KeyPackage>, PublicKeyPackage), Box<dyn std::error::Error>> {
    // Discover seat-* roots under base and sort for a deterministic first-t.
    let mut roots: Vec<PathBuf> = std::fs::read_dir(base)
        .map_err(|e| format!("cannot read store base {}: {e}", base.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("seat-"))
                .unwrap_or(false)
        })
        .collect();
    roots.sort();

    if roots.len() < t as usize {
        return Err(format!(
            "insufficient persisted seat roots under {}: found {}, need t={t}",
            base.display(),
            roots.len()
        )
        .into());
    }
    let selected = &roots[..t as usize];

    // Resolve the unlock secret ONCE; each per-seat store reuses a clone (D-04).
    let secret = passphrase.passphrase()?;

    let mut key_packages: BTreeMap<Identifier, KeyPackage> = BTreeMap::new();
    for root in selected {
        let store = ParticipantStore::new(
            root.clone(),
            Box::new(ResolvedPassphrase::new(secret.clone())),
        );
        let (seat, key_package) = store.load_only_active()?;
        key_packages.insert(seat, key_package);
    }

    // The group package is public — read it with NO unlock from the first root.
    let group_store = ParticipantStore::new(
        selected[0].clone(),
        Box::new(ResolvedPassphrase::new(secret.clone())),
    );
    let group = group_store
        .load_public_envelope(&KeyId::active(), Epoch::GENESIS)?
        .decode_package()?;

    Ok((key_packages, group))
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

    // The SOURCE of the key material differs by mode; the signing session, its
    // in-memory-only nonces (SIGN-05), and the display gate (SIGN-07) are
    // identical either way.
    let (key_packages, group) = if args.persist {
        // Load t of the persisted seat roots (D-05). Resolve the base and prompt
        // ONCE (no-echo `for_unlock`) at the thin CLI edge, then delegate the
        // whole discover→select-t→assemble read glue to load_persisted_shares.
        let base = crate::cli::resolve_root(args.base.clone())?;
        let secret = crate::cli::acquire_store_passphrase(false)?;
        load_persisted_shares(&base, t, &ResolvedPassphrase::new(secret))?
    } else {
        // Phase-1 compatibility: no shares on disk — simulate all seats
        // in-process (D-08). Shares live only in this process and drop at run end.
        run_inprocess_dkg(t, n)?
    };

    let addr = address_from_group_key(group.verifying_key(), args.network.known_hrp())?;
    println!("group address (t={t} of n={n}, key \"{}\"): {addr}", args.key);
    if !args.persist {
        println!(
            "note: no --persist: simulating all seats in-process; the PSBT must spend this \
             address. Use --persist to sign from stored shares."
        );
    }

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
