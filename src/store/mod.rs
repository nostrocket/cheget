//! L2 at-rest persistence — the store layer boundary.
//!
//! Everything durable in `cheget` (participant shares, the transport identity
//! key, encrypted between-round DKG checkpoints, and the coordinator SQLite
//! database) is created, encrypted, and written through this module. It sits
//! **outside** the pure crypto core: `crypto/` and `bridge/` must never gain a
//! filesystem or encryption dependency, so persistence never enters the
//! auditable trusted computing base — it only ever handles already-produced
//! FROST bytes.
//!
//! Submodules:
//!
//! * [`atomic`] — crash-safe `write_atomic` (temp + fsync + rename + dir fsync)
//!   and a `0700` directory helper (D-07).
//!
//! Secret bytes cross this boundary only inside `age`-encrypted blobs, and any
//! decrypted plaintext is returned in [`zeroize::Zeroizing`] so it is wiped at
//! the caller's point of use (D-06). The store root is resolved by [`StoreRoot`]
//! from a `CHEGET_HOME` override or `~/.cheget`; `CHEGET_HOME` is a *path*
//! override only and is never a passphrase source.

use std::path::{Path, PathBuf};

use frost_secp256k1_tr as frost;

pub mod atomic;
pub mod crypto;
pub mod manifest;
pub mod passphrase;

pub use crypto::{decrypt_secret, encrypt_secret};
pub use manifest::{Manifest, ShareEntry, ShareState};
pub use passphrase::{InCodePassphrase, PassphraseSource};

/// Environment variable that overrides the store root path (testability / CI
/// seam). It is a **path** override only — never a passphrase source (D-01/D-03).
pub const ENV_HOME_OVERRIDE: &str = "CHEGET_HOME";

/// Default store directory name under the user's home directory.
pub const STORE_DIR_NAME: &str = ".cheget";

/// Errors surfaced by the store layer.
///
/// Follows the repo's manual error idiom (`#[derive(Debug)]` + hand-written
/// [`std::fmt::Display`] + empty [`std::error::Error`] impl — see `ChainError`),
/// deliberately **not** `thiserror`, to keep the dependency surface auditable.
/// Some variants are populated by later Phase 2 plans (02-02 identity/manifest,
/// 02-03 checkpoints, 02-04 coordinator SQLite); they are public API on the
/// shared store error type and constructed there.
#[derive(Debug)]
pub enum StoreError {
    /// A filesystem operation failed (open, write, fsync, rename, mkdir).
    Io(std::io::Error),
    /// An `age` encrypt/decrypt operation failed (wrong passphrase, corrupt
    /// ciphertext, unsupported header). Carries a rendered message — the age
    /// error faces are collapsed to a string to keep the variant simple.
    Age(String),
    /// A FROST type failed to (de)serialize a *value* (e.g. a `KeyPackage`).
    Frost(frost::Error),
    /// A FROST type failed to serialize for *persistence* specifically (kept
    /// distinct from [`StoreError::Frost`] so store call sites read clearly).
    Serialize(frost::Error),
    /// A JSON manifest / public-artifact (de)serialization failed.
    Json(serde_json::Error),
    /// A coordinator SQLite operation failed (02-04).
    Sqlite(rusqlite::Error),
    /// The store manifest was malformed or referenced a missing file (02-02).
    Manifest(String),
    /// An on-disk schema/manifest version is newer or unknown to this build.
    Schema(String),
    /// The user's home directory could not be resolved and no `CHEGET_HOME`
    /// override was set.
    NoHomeDir,
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "store io error: {e}"),
            StoreError::Age(m) => write!(f, "store encryption error: {m}"),
            StoreError::Frost(e) => write!(f, "frost (de)serialization error: {e}"),
            StoreError::Serialize(e) => write!(f, "frost persistence serialize error: {e}"),
            StoreError::Json(e) => write!(f, "manifest json error: {e}"),
            StoreError::Sqlite(e) => write!(f, "coordinator sqlite error: {e}"),
            StoreError::Manifest(m) => write!(f, "store manifest error: {m}"),
            StoreError::Schema(m) => write!(f, "store schema error: {m}"),
            StoreError::NoHomeDir => write!(
                f,
                "cannot resolve store root: no home directory and {ENV_HOME_OVERRIDE} unset"
            ),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Json(e)
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sqlite(e)
    }
}

/// The resolved on-disk store root (`~/.cheget` or the `CHEGET_HOME` override).
///
/// Resolution reads the `CHEGET_HOME` environment override first, falling back
/// to the `home` crate's home directory joined with [`STORE_DIR_NAME`]. It does
/// not touch the filesystem until [`StoreRoot::create`] is called.
#[derive(Debug, Clone)]
pub struct StoreRoot {
    path: PathBuf,
}

impl StoreRoot {
    /// Resolve the store root without creating it.
    ///
    /// `CHEGET_HOME`, if set and non-empty, is used verbatim as the store root.
    /// Otherwise the root is `<home>/.cheget`.
    pub fn resolve() -> Result<Self, StoreError> {
        if let Some(override_path) = std::env::var_os(ENV_HOME_OVERRIDE) {
            if !override_path.is_empty() {
                return Ok(Self {
                    path: PathBuf::from(override_path),
                });
            }
        }
        let home = home::home_dir().ok_or(StoreError::NoHomeDir)?;
        Ok(Self {
            path: home.join(STORE_DIR_NAME),
        })
    }

    /// The resolved store root path (may not yet exist on disk).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Create the store root directory with mode `0700` on Unix (idempotent),
    /// returning the resolved path.
    pub fn create(&self) -> Result<&Path, StoreError> {
        atomic::create_dir_secure(&self.path)?;
        Ok(&self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_honors_home_override() {
        // SAFETY: single-threaded test; restores the prior value.
        let prev = std::env::var_os(ENV_HOME_OVERRIDE);
        std::env::set_var(ENV_HOME_OVERRIDE, "/tmp/cheget-override-xyz");
        let root = StoreRoot::resolve().unwrap();
        assert_eq!(root.path(), Path::new("/tmp/cheget-override-xyz"));
        match prev {
            Some(v) => std::env::set_var(ENV_HOME_OVERRIDE, v),
            None => std::env::remove_var(ENV_HOME_OVERRIDE),
        }
    }

    #[test]
    fn display_is_manual_not_thiserror() {
        let e = StoreError::Age("bad passphrase".into());
        assert_eq!(format!("{e}"), "store encryption error: bad passphrase");
    }
}
