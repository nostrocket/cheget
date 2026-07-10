//! Esplora HTTP [`ChainBackend`] over `esplora-client` 0.13 (blocking API), STOR-04.
//!
//! Built to the **same** [`ChainBackend`] trait as [`super::core_rpc::CoreRpcBackend`]
//! and covered by conformance tests, but per D-07 it is **not** in the n=1000 confirm
//! path — Bitcoin Core fronts the confirmed key-spend. Esplora is address-indexed, so
//! there is nothing to "import": the descriptor-import method is a documented no-op.

use std::collections::HashMap;

use bitcoin::{Address, FeeRate, OutPoint, Transaction, Txid, XOnlyPublicKey};
use esplora_client::{BlockingClient, Builder};

use super::{ChainBackend, ChainError, Utxo};

/// A [`ChainBackend`] backed by an Esplora HTTP endpoint (blocking client).
pub struct EsploraBackend {
    client: BlockingClient,
}

impl EsploraBackend {
    /// Build a backend for the Esplora server at `base_url` (including any API
    /// prefix the server expects, e.g. `https://blockstream.info/api`).
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Builder::new(base_url).build_blocking(),
        }
    }
}

/// Pick the fee estimate for the requested `target`: the coarsest available
/// bucket whose confirmation target does not exceed `target`, falling back to the
/// smallest-target bucket when none qualifies.
fn pick_fee(estimates: &HashMap<u16, FeeRate>, target: u16) -> Option<FeeRate> {
    estimates
        .iter()
        .filter(|(k, _)| **k <= target)
        .max_by_key(|(k, _)| **k)
        .or_else(|| estimates.iter().min_by_key(|(k, _)| **k))
        .map(|(_, v)| *v)
}

impl ChainBackend for EsploraBackend {
    fn import_tr_descriptor(&self, _internal_key: XOnlyPublicKey) -> Result<(), ChainError> {
        // Esplora indexes every address; there is no wallet to import a watch-only
        // descriptor into. Watching is implicit — this is intentionally a no-op.
        Ok(())
    }

    fn list_utxos(&self, address: &Address) -> Result<Vec<Utxo>, ChainError> {
        let tip = self
            .client
            .get_height()
            .map_err(|e| ChainError::Esplora(e.to_string()))?;
        let utxos = self
            .client
            .get_address_utxos(address)
            .map_err(|e| ChainError::Esplora(e.to_string()))?;
        let script_pubkey = address.script_pubkey();
        Ok(utxos
            .into_iter()
            .map(|u| {
                let confirmations = match (u.status.confirmed, u.status.block_height) {
                    (true, Some(h)) => tip.saturating_sub(h) + 1,
                    _ => 0,
                };
                Utxo {
                    outpoint: OutPoint { txid: u.txid, vout: u.vout },
                    value: u.value,
                    script_pubkey: script_pubkey.clone(),
                    confirmations,
                }
            })
            .collect())
    }

    fn estimate_fee(&self, target: u16) -> Result<Option<FeeRate>, ChainError> {
        let estimates = self
            .client
            .get_fee_estimates()
            .map_err(|e| ChainError::Esplora(e.to_string()))?;
        Ok(pick_fee(&estimates, target))
    }

    fn broadcast(&self, tx: &Transaction) -> Result<Txid, ChainError> {
        self.client
            .broadcast(tx)
            .map_err(|e| ChainError::Esplora(e.to_string()))
    }

    fn confirmation_depth(&self, txid: &Txid) -> Result<u32, ChainError> {
        let status = self
            .client
            .get_tx_status(txid)
            .map_err(|e| ChainError::Esplora(e.to_string()))?;
        match (status.confirmed, status.block_height) {
            (true, Some(h)) => {
                let tip = self
                    .client
                    .get_height()
                    .map_err(|e| ChainError::Esplora(e.to_string()))?;
                Ok(tip.saturating_sub(h) + 1)
            }
            _ => Ok(0),
        }
    }
}
