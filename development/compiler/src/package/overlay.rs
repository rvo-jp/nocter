//! Read-only source overlays for editor-owned package files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub(crate) struct PackageSourceOverlay {
    sources: HashMap<PathBuf, String>,
}

impl PackageSourceOverlay {
    pub(crate) fn insert(&mut self, path: &Path, text: String) {
        self.sources.insert(canonical_or_owned(path), text);
    }

    pub(super) fn source(&self, path: &Path) -> Option<&str> {
        self.sources
            .get(&canonical_or_owned(path))
            .map(String::as_str)
    }
}

fn canonical_or_owned(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
