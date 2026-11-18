//! Test-only scratch directories.
//!
//! Tests used to build PID-named directories under the system temp dir and
//! never remove them, which littered `/tmp` with thousands of empty
//! `swimflow-*` folders. `TempDir` wraps [`tempfile::TempDir`] and derefs to
//! [`Path`], so existing `&dir` / `dir.join(..)` call sites keep working while
//! the directory is removed when the test ends (including on panic).

use std::ops::Deref;
use std::path::Path;

pub struct TempDir(tempfile::TempDir);

impl TempDir {
    /// Create an empty scratch directory whose name starts with `prefix`.
    ///
    /// The directory lives in the *canonical* system temp dir: on macOS the
    /// temp dir is `/var/...`, a symlink to `/private/var/...`, and code that
    /// canonicalizes paths (the live-preview server) would otherwise see a
    /// different prefix than the test.
    pub fn new(prefix: &str) -> Self {
        let base = std::env::temp_dir();
        let base = std::fs::canonicalize(&base).unwrap_or(base);
        let dir = tempfile::Builder::new()
            .prefix(prefix)
            .tempdir_in(base)
            .expect("create a temporary test directory");
        Self(dir)
    }
}

impl Deref for TempDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        self.0.path()
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        self.0.path()
    }
}
