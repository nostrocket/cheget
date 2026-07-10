//! L2 chain access — the `ChainBackend` trait + backend implementations (STOR-04).
//!
//! All side-effecting chain access sits behind [`ChainBackend`] so the crypto core
//! (`crypto/`, `bridge/`) stays pure and the confirmed key-spend can run hermetically
//! against an auto-spawned regtest node (D-05). The trait speaks only rust-bitcoin
//! 0.32 types — no backend-specific type ever leaks through it — so the coordinator,
//! the watcher (Phase 5 sweep/watch), and the tests all depend on the trait, never a
//! concrete client.
//!
//! Two backends are provided:
//!
//! * [`CoreRpcBackend`] over `bitcoincore-rpc` — fronts the confirmed key-spend path
//!   (native regtest mining), the default backend (D-07).
//! * [`EsploraBackend`] over `esplora-client` — built to the **same** trait and
//!   covered by conformance tests, but not in the n=1000 confirm path (D-07).
//!
//! The BIP341 key-spend sighash helper lives in [`sighash`]; it is the one message a
//! FROST key-path signature commits to (SIGN-01 support).

pub mod core_rpc;
pub mod esplora;
pub mod sighash;

pub use core_rpc::CoreRpcBackend;
pub use esplora::EsploraBackend;
pub use sighash::key_spend_sighash;

use bitcoin::{Address, FeeRate, OutPoint, ScriptBuf, Transaction, Txid, XOnlyPublicKey};

/// Errors surfaced by a [`ChainBackend`] or the sighash helper.
#[derive(Debug)]
pub enum ChainError {
    /// A Bitcoin Core JSON-RPC call failed.
    Rpc(String),
    /// An Esplora HTTP call failed.
    Esplora(String),
    /// Computing the key-spend sighash failed.
    Sighash(String),
    /// A watch-only descriptor could not be built or imported.
    Descriptor(String),
    /// The backend does not support this operation (e.g. descriptor import on an
    /// address-indexed Esplora endpoint, which watches every address implicitly).
    Unsupported(&'static str),
}

impl std::fmt::Display for ChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainError::Rpc(m) => write!(f, "bitcoin core rpc error: {m}"),
            ChainError::Esplora(m) => write!(f, "esplora error: {m}"),
            ChainError::Sighash(m) => write!(f, "key-spend sighash error: {m}"),
            ChainError::Descriptor(m) => write!(f, "descriptor error: {m}"),
            ChainError::Unsupported(op) => write!(f, "operation unsupported by this backend: {op}"),
        }
    }
}

impl std::error::Error for ChainError {}

/// One unspent output as seen by a backend, in rust-bitcoin types only.
#[derive(Debug, Clone)]
pub struct Utxo {
    /// The outpoint (`txid:vout`) locking this coin.
    pub outpoint: OutPoint,
    /// The output value.
    pub value: bitcoin::Amount,
    /// The output's `scriptPubKey`.
    pub script_pubkey: ScriptBuf,
    /// Confirmation depth (0 = unconfirmed / mempool).
    pub confirmations: u32,
}

/// Synchronous chain access seam (STOR-04).
///
/// Sync by design (RESEARCH Open Q2): the Core RPC backend fronts the confirm path
/// and is a blocking JSON-RPC client, and the signing session is a straight-line
/// orchestration — no async runtime is pulled into the trusted computing base.
pub trait ChainBackend {
    /// Import a watch-only `tr(<internal-key>)` descriptor (BIP86 key-path, merkle
    /// root `None`) so the backend will report UTXOs paying to the group address.
    ///
    /// Address-indexed backends (Esplora) watch every address implicitly and treat
    /// this as a no-op.
    fn import_tr_descriptor(&self, internal_key: XOnlyPublicKey) -> Result<(), ChainError>;

    /// List the unspent outputs paying to `address`.
    fn list_utxos(&self, address: &Address) -> Result<Vec<Utxo>, ChainError>;

    /// Estimate the fee rate for confirmation within `target` blocks.
    ///
    /// Returns `Ok(None)` when the backend has insufficient data to estimate (e.g.
    /// a fresh regtest node), so callers can fall back to a configured floor.
    fn estimate_fee(&self, target: u16) -> Result<Option<FeeRate>, ChainError>;

    /// Broadcast a fully-signed transaction, returning its txid.
    fn broadcast(&self, tx: &Transaction) -> Result<Txid, ChainError>;

    /// Confirmation depth of `txid` (0 = unconfirmed / unknown).
    fn confirmation_depth(&self, txid: &Txid) -> Result<u32, ChainError>;
}
