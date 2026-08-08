//! Physical source layout rules shared by packages, imports, and tooling.

use std::path::{Component, Path, PathBuf};

pub(crate) const MODULE_ROOT_SOURCE_NAME: &str = "index.nct";

pub(crate) fn is_module_root_source(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == MODULE_ROOT_SOURCE_NAME)
}

/// Returns the logical module path represented by a source path relative to a
/// package or standard-library root.
///
/// `index.nct` represents its containing directory. Other `.nct` files are
/// implementation sources and therefore represent that same directory, not a
/// child module named after the file.
pub(crate) fn logical_module_path(relative_source: &Path) -> Option<PathBuf> {
    if relative_source
        .extension()
        .is_none_or(|extension| extension != "nct")
    {
        return None;
    }
    Some(
        relative_source
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf(),
    )
}

/// Normalizes `.` and `..` without requiring the path to exist.
///
/// Import resolution uses this only to match editor overlays against candidate
/// paths. Filesystem-backed security checks still use canonical paths.
pub(crate) fn normalize_lexical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                match normalized.components().next_back() {
                    Some(Component::Normal(_)) => {
                        normalized.pop();
                    }
                    Some(Component::ParentDir) | None if !path.has_root() => {
                        normalized.push(component.as_os_str());
                    }
                    Some(Component::Prefix(_) | Component::RootDir | Component::CurDir)
                    | Some(Component::ParentDir)
                    | None => {}
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

/// Canonicalizes the longest existing prefix and preserves a missing suffix.
///
/// Editor buffers may describe files that have not been saved yet. Their
/// parent directory still supplies the same symlink-resolved identity as a
/// filesystem-backed sibling.
pub(crate) fn canonicalize_with_missing_suffix(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut missing = Vec::new();
    loop {
        if let Ok(canonical) = existing.canonicalize() {
            return missing
                .iter()
                .rev()
                .fold(canonical, |path, segment| path.join(segment));
        }
        let Some(name) = existing.file_name() else {
            return normalize_lexical_path(path);
        };
        missing.push(name.to_os_string());
        let Some(parent) = existing.parent() else {
            return normalize_lexical_path(path);
        };
        existing = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_lexical_path;
    use std::path::Path;

    #[test]
    fn lexical_normalization_preserves_leading_parent_components() {
        assert_eq!(
            normalize_lexical_path(Path::new("../../src/./parser/../lexer.nct")),
            Path::new("../../src/lexer.nct")
        );
    }

    #[test]
    fn lexical_normalization_cannot_pop_an_absolute_root() {
        assert_eq!(
            normalize_lexical_path(Path::new("/work/../src/../../index.nct")),
            Path::new("/index.nct")
        );
    }
}
