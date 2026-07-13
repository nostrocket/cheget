//! `ChainBackend` trait-conformance tests for both backends (STOR-04, D-07).
//!
//! * `CoreRpcBackend` is exercised against the hermetic auto-spawned regtest node
//!   (it fronts the confirm path).
//! * `EsploraBackend` is exercised against an in-process mock Esplora HTTP server
//!   so the test stays hermetic (D-07: Esplora is conformance-covered only, never
//!   in the n=100 confirm path — no electrs/regtest-esplora stack is stood up).
//!
//! Both backends are driven through the SAME trait surface via
//! [`assert_query_surface`], proving one contract, two implementations.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::str::FromStr;
use std::thread;

use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::{Address, KnownHrp, Transaction, Txid, XOnlyPublicKey};
use cheget::chain::{ChainBackend, EsploraBackend};

/// A fixed, valid txid the mock Esplora server pretends to have broadcast/confirmed.
const MOCK_TXID: &str = "1111111111111111111111111111111111111111111111111111111111111111";
/// A fixed, valid block hash used in mock confirmation status payloads.
const MOCK_BLOCKHASH: &str = "2222222222222222222222222222222222222222222222222222222222222222";

/// Build a deterministic, valid P2TR address + its x-only internal key.
fn sample_p2tr() -> (Address, XOnlyPublicKey) {
    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(&[0x22u8; 32]).expect("valid secret key");
    let (xonly, _parity) = sk.public_key(&secp).x_only_public_key();
    let addr = Address::p2tr(&secp, xonly, None, KnownHrp::Regtest);
    (addr, xonly)
}

/// The shared read/import contract every `ChainBackend` must satisfy without
/// funding: import a watch-only descriptor, list UTXOs, and estimate a fee, all
/// without erroring.
fn assert_query_surface<B: ChainBackend>(backend: &B, addr: &Address, key: XOnlyPublicKey) {
    backend
        .import_tr_descriptor(key)
        .expect("import_tr_descriptor must succeed");
    let _utxos = backend.list_utxos(addr).expect("list_utxos must not error");
    let _fee = backend.estimate_fee(6).expect("estimate_fee must not error");
}

// ---------------------------------------------------------------------------
// Core RPC backend — against the hermetic regtest node.
// ---------------------------------------------------------------------------

#[test]
fn core_rpc_backend_conforms() {
    let fx = common::spawn_regtest();
    let (addr, key) = sample_p2tr();
    // Core backend satisfies the query surface (list is empty on a fresh node).
    assert_query_surface(&fx.backend, &addr, key);
    let utxos = fx.backend.list_utxos(&addr).expect("list_utxos");
    assert!(utxos.is_empty(), "no funds sent to this address yet");
    // Confirmation depth of an unknown txid is reported without panicking.
    // (get_transaction errors for a wallet-unknown txid; that is a surfaced RPC
    // error, not a panic — so we only assert the query surface above for Core.)
}

// ---------------------------------------------------------------------------
// Esplora backend — against an in-process mock HTTP server.
// ---------------------------------------------------------------------------

/// A minimal single-purpose Esplora HTTP mock covering exactly the endpoints the
/// `EsploraBackend` hits. Serves canned HTTP/1.1 responses; one request per
/// accepted connection.
struct MockEsplora {
    port: u16,
}

impl MockEsplora {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock esplora");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());

                // Parse the request line: "<METHOD> <PATH> HTTP/1.1".
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).is_err() {
                    continue;
                }
                let mut parts = request_line.split_whitespace();
                let method = parts.next().unwrap_or("").to_string();
                let path = parts.next().unwrap_or("").to_string();

                // Drain headers; capture Content-Length to consume any POST body.
                let mut content_length = 0usize;
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).is_err() {
                        break;
                    }
                    if header == "\r\n" || header.is_empty() {
                        break;
                    }
                    if let Some(v) = header.to_ascii_lowercase().strip_prefix("content-length:") {
                        content_length = v.trim().parse().unwrap_or(0);
                    }
                }
                if content_length > 0 {
                    let mut body = vec![0u8; content_length];
                    let _ = reader.read_exact(&mut body);
                }

                let body = route(&method, &path);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        MockEsplora { port }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

/// Map an Esplora request to a canned response body.
fn route(method: &str, path: &str) -> String {
    if path == "/blocks/tip/height" {
        return "150".to_string();
    }
    if path == "/fee-estimates" {
        return r#"{"1":20.5,"6":10.0,"144":1.0}"#.to_string();
    }
    if method == "POST" && path == "/tx" {
        // Esplora returns the broadcast txid as plain text.
        return MOCK_TXID.to_string();
    }
    if path.ends_with("/utxo") {
        return format!(
            r#"[{{"txid":"{MOCK_TXID}","vout":0,"status":{{"confirmed":true,"block_height":100,"block_hash":"{MOCK_BLOCKHASH}","block_time":1700000000}},"value":100000}}]"#
        );
    }
    if path.ends_with("/status") {
        return format!(
            r#"{{"confirmed":true,"block_height":100,"block_hash":"{MOCK_BLOCKHASH}","block_time":1700000000}}"#
        );
    }
    // Unknown path: empty body (the client will surface a decode error if it hits one).
    String::new()
}

#[test]
fn esplora_backend_conforms() {
    let server = MockEsplora::start();
    let backend = EsploraBackend::new(&server.url());
    let (addr, key) = sample_p2tr();

    // Same trait surface as the Core backend.
    assert_query_surface(&backend, &addr, key);

    // list_utxos parses the mock UTXO with a derived confirmation depth (tip 150,
    // confirmed at height 100 → depth 51).
    let utxos = backend.list_utxos(&addr).expect("list_utxos");
    assert_eq!(utxos.len(), 1);
    assert_eq!(utxos[0].value, bitcoin::Amount::from_sat(100_000));
    assert_eq!(utxos[0].confirmations, 51);

    // estimate_fee returns the closest bucket <= target (target 6 → the "6" bucket).
    let fee = backend.estimate_fee(6).expect("estimate_fee").expect("some fee");
    assert!(fee > bitcoin::FeeRate::ZERO);

    // broadcast round-trips a tx through the mock and returns the mock txid.
    let dummy = Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![],
        output: vec![],
    };
    let txid = backend.broadcast(&dummy).expect("broadcast");
    assert_eq!(txid, Txid::from_str(MOCK_TXID).unwrap());

    // confirmation_depth uses /status + tip height (150 - 100 + 1 = 51).
    let depth = backend.confirmation_depth(&txid).expect("confirmation_depth");
    assert_eq!(depth, 51);
}
