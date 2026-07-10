//! Shared regtest test fixture (D-05).
//!
//! `spawn_regtest()` auto-downloads (via the `corepc-node` `28_0`+`download`
//! features) and spawns a throwaway regtest `bitcoind` on a temp datadir with a
//! free port, then wires:
//!
//! * a `tsig` [`CoreRpcBackend`] to a **dedicated watch-only wallet** (private
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

use bitcoincore_rpc::{Auth, Client, RpcApi};
use corepc_node::Node;
use tsig::chain::CoreRpcBackend;

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
