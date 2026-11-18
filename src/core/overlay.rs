//! Files that live in memory instead of on disk.
//!
//! The live preview uses this for diagrams that only exist in the browser
//! session (imported from the user's disk or created from scratch): a `ref`
//! to such a file resolves exactly like a reference to a file on disk, and an
//! in-memory file shadows an on-disk file with the same path.
//!
//! The overlay is part of the plugin interface: a plugin may need to read
//! other files (the activity diagram resolves `ref` links), and the caller
//! decides where those files come from.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// An in-memory file system overlay.
#[derive(Debug, Clone, Default)]
pub struct Overlay {
    files: BTreeMap<PathBuf, String>,
}

impl Overlay {
    /// An empty overlay.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a file, replacing any previous content for the same path.
    pub fn insert(&mut self, path: impl AsRef<Path>, contents: impl Into<String>) {
        self.files
            .insert(normalize_path(path.as_ref()), contents.into());
    }

    /// The content of a file, if it is in the overlay.
    pub fn get(&self, path: &Path) -> Option<&str> {
        self.files.get(&normalize_path(path)).map(String::as_str)
    }

    /// Number of files in the overlay.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether the overlay holds no file at all.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// Resolve `.` and `..` in a path without touching the filesystem.
///
/// Unlike [`std::fs::canonicalize`] this never fails and never follows
/// symbolic links, so it also works for files that do not exist yet — which
/// is exactly the case for buffers that only exist in the browser session.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_found_by_their_normalized_path() {
        let mut overlay = Overlay::new();
        overlay.insert("/tmp/a/../b.diag", "hello");
        assert_eq!(overlay.get(Path::new("/tmp/b.diag")), Some("hello"));
        assert_eq!(overlay.get(Path::new("/tmp/a/b.diag")), None);
        assert_eq!(overlay.len(), 1);
    }

    #[test]
    fn an_empty_overlay_has_no_files() {
        let overlay = Overlay::new();
        assert!(overlay.is_empty());
        assert_eq!(overlay.len(), 0);
    }

    #[test]
    fn paths_are_normalized_without_touching_the_filesystem() {
        assert_eq!(
            normalize_path(Path::new("a/./b/../c.diag")),
            PathBuf::from("a/c.diag")
        );
        assert_eq!(
            normalize_path(Path::new("/a/b.diag")),
            PathBuf::from("/a/b.diag")
        );
    }
}
