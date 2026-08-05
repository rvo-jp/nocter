use super::super::documents::OpenDocument;
use super::model::{DocumentSnapshot, PackageSnapshot};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(in crate::driver::lsp) enum SnapshotChange {
    Full,
    Paths(HashSet<PathBuf>),
}

impl SnapshotChange {
    pub(in crate::driver::lsp) fn path(path: Option<&Path>) -> Self {
        Self::Paths(path.into_iter().map(canonical_or_owned).collect())
    }

    pub(in crate::driver::lsp) fn paths(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        Self::Paths(paths.into_iter().map(canonical_or_owned).collect())
    }

    fn any_path(&self, predicate: impl Fn(&Path) -> bool) -> bool {
        match self {
            Self::Full => true,
            Self::Paths(paths) => paths.iter().any(|path| predicate(path)),
        }
    }
}

pub(in crate::driver::lsp) fn can_reuse_package(
    package: &PackageSnapshot,
    change: &SnapshotChange,
) -> bool {
    !change.any_path(|path| package.package_files.contains(path))
}

pub(in crate::driver::lsp) fn can_reuse_document(
    previous: &DocumentSnapshot,
    previous_document: &OpenDocument,
    document: &OpenDocument,
    package_root: Option<&Path>,
    package_revision: Option<u64>,
    change: &SnapshotChange,
) -> bool {
    previous_document == document
        && previous.package_root.as_deref() == package_root
        && previous.package_revision == package_revision
        && !change.any_path(|path| previous.analysis.depends_on(path))
}

fn canonical_or_owned(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
