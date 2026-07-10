//! `tsig` — 501-of-1000 FROST Taproot signing CLI.
//!
//! A single binary that lets a fixed-threshold (t=501, n=1000) group jointly
//! control one Bitcoin Taproot address via FROST threshold Schnorr signatures
//! (RFC 9591, secp256k1, BIP340/341 key-path spend).
//!
//! ## Module map (layered bottom-up; see `.planning/.../01-RESEARCH.md`)
//!
//! - [`bridge`] — L0.5, **pure**: the ONE canonical frost→rust-bitcoin key seam
//!   (`VerifyingKey` → x-only → `XOnlyPublicKey` → BIP341 P2TR + output key `Q`).
//! - [`crypto`] — L0, **pure**: frost wrapper (DKG, tweaked signing, nonce type).
//! - [`chain`] — L2: `ChainBackend` trait + Core RPC / Esplora impls (side-effecting).
//! - [`transport`] — L2: `Transport` trait + in-memory stub (the Nostr swap seam).
//! - [`session`] — L3: two-round signing session; owns nonce lifetime (RAM only).
//! - [`cli`] — L4: clap persona tree (participant / coordinator / watcher).
//!
//! `bridge` and `crypto` MUST stay I/O-free so the auditable/reproducible trusted
//! computing base stays small.

pub mod bridge;
pub mod chain;
pub mod cli;
pub mod crypto;
pub mod session;
pub mod transport;
