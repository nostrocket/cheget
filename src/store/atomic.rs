//! Crash-safe atomic file write + restrictive directory/file permissions (D-07).
//!
//! A share or checkpoint must never be observed half-written: a crash mid-write
//! that truncated a *live* share would silently destroy key material that Phase 4
//! rotation depends on. The discipline (RESEARCH Pattern 3) is:
//!
//! 1. write the bytes to a uniquely-named temp file **in the same directory**
//!    (a cross-directory rename is not atomic), created with `create_new` so it
//!    can never clobber an existing file, and `0600` on Unix;
//! 2. `sync_all()` the file so its contents are durable;
//! 3. `fs::rename` the temp over the final path — atomic on a POSIX filesystem;
//! 4. `sync_all()` the **parent directory** so the rename itself survives a crash
//!    (the commonly-omitted durability step, RESEARCH Pitfall 2).
//!
//! The existing final file is only ever replaced by a fully-written temp; it is
//! never opened for truncation, so an interrupted write leaves the old content
//! intact. All permission code is gated `#[cfg(unix)]`; on Windows perms are
//! best-effort and the durability ordering still holds.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use super::StoreError;

/// A process-unique suffix for temp file names, so concurrent writers in the
/// same directory never collide on the `create_new` temp.
fn unique_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{}-{}", std::process::id(), nanos, n)
}

/// Create `path` (and any missing parents) as a directory with mode `0700` on
/// Unix, so only the owner can traverse the store. On non-Unix the mode is
/// best-effort (std applies platform defaults). Idempotent: succeeds if the
/// directory already exists.
pub fn create_dir_secure(path: &Path) -> Result<(), StoreError> {
    if path.is_dir() {
        return Ok(());
    }
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    Ok(())
}

/// Atomically write `bytes` to `final_path`, creating a `0600` file on Unix.
///
/// See the module docs for the temp → fsync → rename → dir-fsync sequence. The
/// final path is replaced atomically; a failure before the rename leaves any
/// existing final file untouched and never produces a truncated/partial file.
pub fn write_atomic(final_path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let dir = final_path.parent().ok_or_else(|| {
        StoreError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "atomic write target has no parent directory",
        ))
    })?;
    let tmp = dir.join(format!(".{}.tmp", unique_suffix()));

    // Scope the file handle so it is closed before the rename.
    let write_result = (|| -> io::Result<()> {
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?; // fsync the file contents
        Ok(())
    })();

    if let Err(e) = write_result {
        // Best-effort cleanup of the temp; propagate the original error.
        let _ = fs::remove_file(&tmp);
        return Err(StoreError::Io(e));
    }

    if let Err(e) = fs::rename(&tmp, final_path) {
        let _ = fs::remove_file(&tmp);
        return Err(StoreError::Io(e));
    }

    // fsync the DIRECTORY so the rename (directory entry) is durable across a
    // crash. Opening a directory read-only and sync_all-ing it is the POSIX way.
    File::open(dir)?.sync_all()?;
    Ok(())
}

#[cfg(unix)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    /// A unique scratch directory under the system temp dir (avoids adding a
    /// tempfile dev-dependency). Created 0700 via the helper under test.
    fn scratch_dir() -> PathBuf {
        let base = std::env::temp_dir().join(format!("cheget-atomic-{}", unique_suffix()));
        create_dir_secure(&base).unwrap();
        base
    }

    fn mode_of(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn perms() {
        let dir = scratch_dir();
        // created dir is 0700
        assert_eq!(mode_of(&dir), 0o700, "store dir must be owner-only 0700");

        let file = dir.join("share.age");
        write_atomic(&file, b"ciphertext").unwrap();
        // created file is 0600
        assert_eq!(mode_of(&file), 0o600, "share file must be owner-only 0600");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn atomic_no_partial() {
        let dir = scratch_dir();
        let final_path = dir.join("share");

        // A stale, unrelated .tmp sibling must never be promoted to the final.
        let stale = dir.join(".stale-leftover.tmp");
        fs::write(&stale, b"garbage").unwrap();

        // First write, then overwrite with a larger blob.
        write_atomic(&final_path, b"AAAA").unwrap();
        assert_eq!(fs::read(&final_path).unwrap(), b"AAAA");
        write_atomic(&final_path, b"BBBBBBBB").unwrap();
        assert_eq!(fs::read(&final_path).unwrap(), b"BBBBBBBB");

        // The stale tmp is untouched and was never renamed into place.
        assert!(stale.exists(), "unrelated .tmp must be left alone");
        assert_eq!(fs::read(&stale).unwrap(), b"garbage");

        // No temp file created by write_atomic lingers (its own temp was renamed).
        let tmp_leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name();
                let n = n.to_string_lossy();
                n.ends_with(".tmp") && n != ".stale-leftover.tmp"
            })
            .collect();
        assert!(tmp_leftovers.is_empty(), "write_atomic left a temp behind");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn interrupted_write_does_not_truncate_existing() {
        let dir = scratch_dir();
        let final_path = dir.join("share");
        write_atomic(&final_path, b"LIVE").unwrap();

        // Make the directory read+execute only (no write): temp creation must
        // fail, so write_atomic errors BEFORE any rename — the live file stays.
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).unwrap();
        let res = write_atomic(&final_path, b"CLOBBERED");
        // Restore write perm so the assertion + cleanup can proceed.
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();

        assert!(res.is_err(), "write into a non-writable dir must fail");
        assert_eq!(
            fs::read(&final_path).unwrap(),
            b"LIVE",
            "existing share must never be truncated by a failed write"
        );

        fs::remove_dir_all(&dir).ok();
    }
}
