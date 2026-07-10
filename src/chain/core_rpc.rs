//! Bitcoin Core JSON-RPC [`ChainBackend`] over `bitcoincore-rpc` 0.19 (STOR-04).
//!
//! This is the default backend and the one that fronts the confirmed key-spend
//! path (D-07): it imports the watch-only `tr()` descriptor, lists the group
//! address's UTXOs, estimates fee, broadcasts, and reports confirmation depth
//! against a real (regtest, in tests) node.

use bitcoin::{Address, FeeRate, OutPoint, Transaction, Txid, XOnlyPublicKey};
use bitcoincore_rpc::json::{ImportDescriptors, Timestamp};
use bitcoincore_rpc::{Auth, Client, RpcApi};

use super::{ChainBackend, ChainError, Utxo};

/// A [`ChainBackend`] backed by a Bitcoin Core node over JSON-RPC.
pub struct CoreRpcBackend {
    client: Client,
}

impl CoreRpcBackend {
    /// Connect to a Core node at `url` (typically a wallet endpoint such as
    /// `http://127.0.0.1:PORT/wallet/NAME`) with the given `auth`.
    pub fn new(url: &str, auth: Auth) -> Result<Self, ChainError> {
        let client = Client::new(url, auth).map_err(|e| ChainError::Rpc(e.to_string()))?;
        Ok(Self { client })
    }

    /// Wrap an already-constructed RPC client.
    pub fn from_client(client: Client) -> Self {
        Self { client }
    }

    /// Escape hatch to the underlying RPC client for regtest test plumbing
    /// (funding / raw-tx signing helpers that are not part of the production
    /// [`ChainBackend`] surface). Not used on any production code path.
    pub fn rpc(&self) -> &Client {
        &self.client
    }
}

impl ChainBackend for CoreRpcBackend {
    fn import_tr_descriptor(&self, internal_key: XOnlyPublicKey) -> Result<(), ChainError> {
        // BIP86 key-path descriptor: tr(<x-only internal key>), merkle root None.
        let descriptor = format!("tr({internal_key})");
        // Core requires the descriptor checksum; ask the node to compute it.
        let info = self
            .client
            .get_descriptor_info(&descriptor)
            .map_err(|e| ChainError::Rpc(e.to_string()))?;
        let checksummed = match info.checksum {
            Some(c) => format!("{descriptor}#{c}"),
            None => descriptor,
        };
        let req = ImportDescriptors {
            descriptor: checksummed,
            timestamp: Timestamp::Now,
            // Watch-only: not an active (address-deriving) descriptor, no privkeys.
            active: Some(false),
            range: None,
            next_index: None,
            internal: None,
            label: None,
        };
        let results = self
            .client
            .import_descriptors(req)
            .map_err(|e| ChainError::Rpc(e.to_string()))?;
        if results.iter().all(|r| r.success) {
            Ok(())
        } else {
            Err(ChainError::Descriptor(format!(
                "importdescriptors reported failure: {results:?}"
            )))
        }
    }

    fn list_utxos(&self, address: &Address) -> Result<Vec<Utxo>, ChainError> {
        let entries = self
            .client
            // minconf 0 so freshly-mined coinbase to the watched address is visible.
            .list_unspent(Some(0), None, Some(&[address]), Some(true), None)
            .map_err(|e| ChainError::Rpc(e.to_string()))?;
        Ok(entries
            .into_iter()
            .map(|e| Utxo {
                outpoint: OutPoint { txid: e.txid, vout: e.vout },
                value: e.amount,
                script_pubkey: e.script_pub_key,
                confirmations: e.confirmations,
            })
            .collect())
    }

    fn estimate_fee(&self, target: u16) -> Result<Option<FeeRate>, ChainError> {
        let res = self
            .client
            .estimate_smart_fee(target, None)
            .map_err(|e| ChainError::Rpc(e.to_string()))?;
        match res.fee_rate {
            // Core reports BTC/kvB; convert to sat/vB for a rust-bitcoin FeeRate.
            Some(btc_per_kvb) => Ok(FeeRate::from_sat_per_vb(btc_per_kvb.to_sat() / 1000)),
            // Regtest / insufficient data: let the caller fall back to a floor.
            None => Ok(None),
        }
    }

    fn broadcast(&self, tx: &Transaction) -> Result<Txid, ChainError> {
        self.client
            .send_raw_transaction(tx)
            .map_err(|e| ChainError::Rpc(e.to_string()))
    }

    fn confirmation_depth(&self, txid: &Txid) -> Result<u32, ChainError> {
        let tx = self
            .client
            .get_transaction(txid, Some(true))
            .map_err(|e| ChainError::Rpc(e.to_string()))?;
        // A negative value means conflicted/abandoned; treat as 0 depth.
        Ok(tx.info.confirmations.max(0) as u32)
    }
}
