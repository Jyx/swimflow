//! Shared helpers for the integration tests.
//!
//! `TempDir` removes its directory on drop, so a test run leaves no
//! `swimflow-*` folders behind in the system temp directory.

use std::ops::Deref;
use std::path::Path;

pub struct TempDir(tempfile::TempDir);

impl TempDir {
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
