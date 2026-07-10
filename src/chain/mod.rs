//! L2 chain access — `ChainBackend` trait + backend implementations.
//!
//! Placeholder module seam. Filled by plan **01-03**:
//! - a synchronous `trait ChainBackend` (UTXO listing, fee estimation, broadcast,
//!   BIP341 key-spend sighash helper, watch-only `tr()` descriptor import),
//! - a Bitcoin Core JSON-RPC backend (fronts the confirmed key-spend path, D-07),
//! - an Esplora backend for trait-conformance coverage (STOR-04).
