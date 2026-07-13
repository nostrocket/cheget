//! Auto-spawned regtest fixture + broadcast/confirm smoke test (D-05).
//!
//! Proves the `corepc-node` auto-spawn + [`cheget::chain::CoreRpcBackend`] can
//! fund, watch, broadcast, and confirm — the chain half of the eventual n=100
//! confirmed key-spend (the FROST signing half lands in 01-04, which reuses
//! `spawn_regtest` from `tests/common`). No `bitcoind` need be installed; no
//! secret key material of the FROST group is involved.

mod common;

// Re-exported so downstream integration tests (and 01-04's e2e) can depend on the
// fixture from this well-known module path.
pub use common::{spawn_regtest, RegtestFixture};

use std::collections::HashMap;
use std::str::FromStr;

use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::{Address, Amount, KnownHrp};
use bitcoincore_rpc::RpcApi;
use cheget::chain::ChainBackend;

/// A deterministic, valid regtest P2TR (BIP86 key-path) address plus the x-only
/// internal key it commits to — the same key we hand to `import_tr_descriptor`
/// so the imported `tr()` descriptor and this address resolve to one scriptPubKey.
fn watch_only_p2tr() -> (Address, bitcoin::XOnlyPublicKey) {
    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(&[0x11u8; 32]).expect("valid secret key");
    let (xonly, _parity) = sk.public_key(&secp).x_only_public_key();
    let addr = Address::p2tr(&secp, xonly, None, KnownHrp::Regtest);
    (addr, xonly)
}

#[test]
fn regtest_fund_watch_broadcast_confirm_smoke() {
    let fx = spawn_regtest();

    // 1. Fund the default wallet: mine 101 blocks so one coinbase matures.
    let miner = fx.mine(101);

    // 2. Import the watch-only tr() descriptor into the watch-only wallet, then
    //    prove the address is watched by funding it from the default wallet and
    //    listing its UTXO through the backend.
    let (watch_addr, internal_key) = watch_only_p2tr();
    fx.backend
        .import_tr_descriptor(internal_key)
        .expect("import watch-only tr() descriptor");

    fx.funding
        .send_to_address(&watch_addr, Amount::from_btc(1.0).unwrap(), None, None, None, None, None, None)
        .expect("send 1 BTC to the watched address");
    fx.funding.generate_to_address(1, &miner).expect("confirm the funding tx");

    let utxos = fx.backend.list_utxos(&watch_addr).expect("list watched UTXOs");
    assert!(!utxos.is_empty(), "backend must report the watched UTXO");
    assert!(
        utxos.iter().any(|u| u.value == Amount::from_btc(1.0).unwrap()),
        "the 1 BTC output must be visible to the backend: {utxos:?}"
    );
    assert!(
        utxos.iter().all(|u| u.confirmations >= 1),
        "the funding output must be confirmed"
    );

    // 3. estimate_fee must not error (regtest typically has insufficient data → None).
    let _ = fx.backend.estimate_fee(6).expect("estimate_fee must not error on regtest");

    // 4. Broadcast a wallet-signed tx THROUGH the backend, then confirm it. The tx
    //    pays to the watched address so the watch-only wallet can report its depth.
    let mut outs: HashMap<String, Amount> = HashMap::new();
    outs.insert(watch_addr.to_string(), Amount::from_btc(0.5).unwrap());
    let raw = fx
        .funding
        .create_raw_transaction_hex(&[], &outs, None, None)
        .expect("createrawtransaction");
    let funded = fx
        .funding
        .fund_raw_transaction(raw, None, None)
        .expect("fundrawtransaction")
        .transaction()
        .expect("decode funded tx");
    let signed = fx
        .funding
        .sign_raw_transaction_with_wallet(&funded, None, None)
        .expect("signrawtransactionwithwallet")
        .transaction()
        .expect("decode signed tx");

    let txid = fx.backend.broadcast(&signed).expect("broadcast via ChainBackend");
    assert_eq!(txid, signed.compute_txid(), "broadcast returns the tx's own id");

    fx.funding.generate_to_address(6, &miner).expect("mine 6 confirmations");
    let depth = fx.backend.confirmation_depth(&txid).expect("confirmation depth");
    assert!(depth >= 6, "expected >= 6 confirmations, got {depth}");

    // Sanity: the txid round-trips as a valid txid string.
    assert_eq!(bitcoin::Txid::from_str(&txid.to_string()).unwrap(), txid);
}
