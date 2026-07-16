//! Shared regtest test fixture (D-05).
//!
//! `spawn_regtest()` auto-downloads (via the `corepc-node` `28_0`+`download`
//! features) and spawns a throwaway regtest `bitcoind` on a temp datadir with a
//! free port, then wires:
//!
//! * a `cheget` [`CoreRpcBackend`] to a **dedicated watch-only wallet** (private
//!   keys disabled) — the production surface: the group's `tr()` descriptor is
//!   watch-only, and Core refuses to import a keyless descriptor into a wallet
//!   that holds private keys; and
//! * a plain `bitcoincore-rpc` client to the node's key-holding `default` wallet
//!   ([`RegtestFixture::funding`]) — test-only plumbing to fund, build, and sign
//!   the transactions the backend then broadcasts.
//!
//! The node is killed when the returned [`RegtestFixture`] is dropped. No
//! system-installed `bitcoind` is required and no secret key material of the
//! FROST group is involved.
//!
//! Reused by `tests/regtest_fixture.rs` (broadcast/confirm smoke) and
//! `tests/chain_backend_conformance.rs` (Core trait conformance), and by 01-04's
//! end-to-end confirmed key-spend.
#![allow(dead_code)]

use std::collections::BTreeMap;

use bitcoin::absolute::LockTime;
use bitcoin::transaction::Version;
use bitcoin::{
    Amount, KnownHrp, Network, Psbt, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use corepc_node::Node;
use frost_secp256k1_tr as frost;
use frost::keys::{KeyPackage, PublicKeyPackage};
use frost::Identifier;
use cheget::bridge::{address_from_group_key, internal_key_xonly};
use cheget::chain::{ChainBackend, CoreRpcBackend};
use cheget::crypto::run_inprocess_dkg;
use cheget::session::SigningSession;
use cheget::transport::InMemoryTransport;

/// A live regtest node plus the two clients a chain test needs.
pub struct RegtestFixture {
    /// The regtest node. Killed on drop — keep it alive for the test's duration.
    pub node: Node,
    /// A Core RPC backend pointed at a watch-only wallet (the production surface).
    pub backend: CoreRpcBackend,
    /// A key-holding client on the `default` wallet for test funding/signing.
    pub funding: Client,
}

impl RegtestFixture {
    /// Mine `blocks` to a fresh `default`-wallet address (matures coinbase / adds
    /// confirmations) and return that miner address.
    pub fn mine(&self, blocks: u64) -> bitcoin::Address {
        let addr = self
            .funding
            .get_new_address(None, None)
            .expect("getnewaddress")
            .assume_checked();
        self.funding
            .generate_to_address(blocks, &addr)
            .expect("generatetoaddress");
        addr
    }
}

/// Auto-spawn a hermetic regtest node with a watch-only backend + funding client.
///
/// Panics with a clear message if the node cannot be spawned (e.g. the pinned
/// Core binary was not downloaded at build time).
pub fn spawn_regtest() -> RegtestFixture {
    let node = Node::from_downloaded()
        .expect("auto-download + spawn regtest bitcoind (corepc-node 28_0+download)");

    let socket = node.params.rpc_socket;
    let cookie = node.params.cookie_file.clone();

    // corepc-node creates/loads a key-holding wallet named "default".
    let default_url = format!("http://{socket}/wallet/default");
    let funding = Client::new(&default_url, Auth::CookieFile(cookie.clone()))
        .expect("connect to the default (funding) wallet");

    // Create a dedicated watch-only wallet (private keys disabled, blank) for the
    // group's tr() descriptor — the production watch-only pattern.
    funding
        .create_wallet("watch", Some(true), Some(true), None, None)
        .expect("create watch-only wallet");
    let watch_url = format!("http://{socket}/wallet/watch");
    let backend = CoreRpcBackend::new(&watch_url, Auth::CookieFile(cookie))
        .expect("connect the ChainBackend to the watch-only wallet");

    RegtestFixture { node, backend, funding }
}

/// End-to-end CONFIRMED regtest key-spend at `t`-of-`n` — the crown-jewel proof
/// (SIGN-04, D-02/D-05/D-06). Shared by the small-`n` PR gate
/// (`tests/inproc_sign.rs`) and the `#[ignore]` n=100 on-demand gate
/// (`tests/inproc_sign_100.rs`).
///
/// Runs the whole pipeline against an auto-spawned regtest node: in-process DKG →
/// bridge to a P2TR address → import the watch-only `tr()` descriptor → fund it →
/// build a spending PSBT → drive a `SigningSession` (`--yes`) over the in-memory
/// `Transport` stub → aggregate-with-tweak → verify against `Q` → finalize →
/// broadcast → mine → assert the spend is confirmed. Panics on any failure so the
/// caller is a one-line `#[test]`.
pub fn run_confirmed_key_spend(t: u16, n: u16) {
    // In-process DKG (simulate all seats, D-08) → group key (even-Y), then the
    // shared chain-proof body. Existing fresh-DKG callers keep this entry point.
    let (key_packages, group) = run_inprocess_dkg(t, n).expect("in-process DKG");
    run_confirmed_key_spend_from_shares(key_packages, group, t);
}

/// The chain-proof body of [`run_confirmed_key_spend`], taking PRE-LOADED shares.
///
/// Spins up a regtest node and runs the whole pipeline from the address bridge
/// through broadcast + confirm, but sources `key_packages`/`group` from the
/// caller instead of a fresh DKG — so 03-02's persisted-share test can prove the
/// confirmed key-spend is produced BY the store→load glue over PERSISTED shares
/// (D-05), and existing fresh-DKG callers delegate here after their own DKG.
/// Panics on any failure so the caller is a one-line `#[test]`.
pub fn run_confirmed_key_spend_from_shares(
    key_packages: BTreeMap<Identifier, KeyPackage>,
    group: PublicKeyPackage,
    t: u16,
) {
    let fx = spawn_regtest();
    let miner = fx.mine(101); // mature a coinbase so the funding wallet has coins

    // 1. Bridge to the group's P2TR address and import the watch-only descriptor
    //    (the descriptor's key is the INTERNAL key P, not the output key Q).
    let addr = address_from_group_key(group.verifying_key(), KnownHrp::Regtest)
        .expect("bridge group key → regtest P2TR address");
    let internal_key = internal_key_xonly(group.verifying_key()).expect("internal x-only key");
    fx.backend
        .import_tr_descriptor(internal_key)
        .expect("import watch-only tr() descriptor");

    // 2. Fund the group address and confirm it.
    let funded = Amount::from_btc(1.0).unwrap();
    fx.funding
        .send_to_address(&addr, funded, None, None, None, None, None, None)
        .expect("fund the group address");
    fx.funding.generate_to_address(1, &miner).expect("confirm funding");

    let utxo = fx
        .backend
        .list_utxos(&addr)
        .expect("list group UTXOs")
        .into_iter()
        .find(|u| u.value == funded)
        .expect("the 1 BTC group UTXO is visible to the watch-only backend");

    // 3. Build the spending PSBT: spend the group UTXO back to the group address
    //    minus a fee. The coordinator distributes the PSBT (never a sighash).
    let fee = Amount::from_sat(10_000);
    let tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: utxo.outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut { value: funded - fee, script_pubkey: addr.script_pubkey() }],
    };
    let mut psbt = Psbt::from_unsigned_tx(tx).expect("unsigned tx → PSBT");
    psbt.inputs[0].witness_utxo =
        Some(TxOut { value: utxo.value, script_pubkey: utxo.script_pubkey.clone() });

    // 4. Drive the signing session over the in-memory Transport stub.
    let transport = InMemoryTransport::new();
    let mut session = SigningSession::new(
        "e2e-key-spend",
        &transport,
        key_packages,
        group,
        psbt,
        t as usize,
        Network::Regtest,
    );
    let signed = session.run(true).expect("two-round session → verified key-spend");

    // 5. Broadcast the finalized key-spend and mine it to confirmation.
    let txid = fx.backend.broadcast(&signed).expect("broadcast the key-spend");
    assert_eq!(txid, signed.compute_txid(), "broadcast returns the tx's own id");
    fx.funding.generate_to_address(6, &miner).expect("mine 6 confirmations");

    let depth = fx.backend.confirmation_depth(&txid).expect("confirmation depth");
    assert!(
        depth >= 6,
        "the {t}-of-N key-spend must be CONFIRMED on regtest (got depth {depth})"
    );
}
