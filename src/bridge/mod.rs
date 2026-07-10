//! L0.5 key bridge — **pure** frost→rust-bitcoin seam.
//!
//! The single place where FROST key bytes cross into rust-bitcoin key types
//! (KEY-03, KEY-04). All conversions live in [`taproot`], which is the sole
//! caller of the x-only `from_slice` constructor. This module stays I/O-free.

pub mod taproot;

pub use taproot::{address_from_group_key, output_key_q, BridgeError};
