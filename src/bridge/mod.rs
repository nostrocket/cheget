//! L0.5 key bridge — **pure** frost→rust-bitcoin seam.
//!
//! The single place where FROST key bytes cross into rust-bitcoin key types.
//! The canonical conversion functions live in [`taproot`] (added by plan 01-02
//! Task 2). This module MUST stay I/O-free.
