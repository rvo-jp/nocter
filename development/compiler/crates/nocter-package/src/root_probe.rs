use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use nocter_filesystem::SourceOverlay;
use nocter_source::{SourceError, SourceMap, SourceName};
use nocter_syntax::{NodeKind, ParseGoal, SyntaxElement, parse};

/// Inspects whether a directory's `index.nct` declares a package.
///
/// A physical `index.nct` alone declares a module. Only a top-level `#package` directive promotes
/// that module to a package root. The probe intentionally does not decode the complete package
/// record; malformed package data is diagnosed later by package loading rather than silently
/// changing physical ownership.
///
/// # Errors
///
/// Returns the exact source-selection or source-decoding failure encountered while inspecting an
/// existing `index.nct`. A missing index is reported as `Ok(false)`.
pub fn has_package_declaration(
    source_overlay: &SourceOverlay,
    directory: &Path,
) -> Result<bool, PackageRootProbeError> {
    let path = directory.join("index.nct");
    if !source_overlay
        .is_file(&path)
        .map_err(|source| PackageRootProbeError::Filesystem {
            path: path.clone(),
            source,
        })?
    {
        return Ok(false);
    }
    let bytes = source_overlay
        .read(&path)
        .map_err(|source| PackageRootProbeError::Filesystem {
            path: path.clone(),
            source,
        })?;
    let mut sources = SourceMap::new();
    let source_id = sources
        .add_bytes(SourceName::new(path.to_string_lossy().as_ref()), &bytes)
        .map_err(|source| PackageRootProbeError::Source {
            path: path.clone(),
            source,
        })?;
    let source = sources
        .get(source_id)
        .ok_or_else(|| PackageRootProbeError::MissingSource(path.clone()))?;
    let syntax = parse(source, ParseGoal::SourceFile);
    Ok(syntax.children(syntax.root_id()).iter().any(|element| {
        let SyntaxElement::Node(node) = element else {
            return false;
        };
        syntax
            .node(*node)
            .is_some_and(|node| node.kind() == NodeKind::PackageDirective)
            && syntax.children(*node).iter().any(|element| {
                let SyntaxElement::Token(token) = element else {
                    return false;
                };
                source.text_at(token.range()) == Some("package")
            })
    }))
}

#[derive(Debug)]
pub enum PackageRootProbeError {
    Filesystem { path: PathBuf, source: io::Error },
    Source { path: PathBuf, source: SourceError },
    MissingSource(PathBuf),
}

impl fmt::Display for PackageRootProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filesystem { path, source } => {
                write!(formatter, "could not inspect {}: {source}", path.display())
            }
            Self::Source { path, source } => {
                write!(formatter, "could not decode {}: {source}", path.display())
            }
            Self::MissingSource(path) => {
                write!(formatter, "package probe lost source {}", path.display())
            }
        }
    }
}

impl std::error::Error for PackageRootProbeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Filesystem { source, .. } => Some(source),
            Self::Source { source, .. } => Some(source),
            Self::MissingSource(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn distinguishes_package_modules_from_child_modules() {
        let root =
            std::env::temp_dir().join(format!("nocter-package-root-probe-{}", std::process::id()));
        let child = root.join("child");
        fs::create_dir_all(&child).unwrap();
        fs::write(
            root.join("index.nct"),
            "#package: { name: \"app\", version: \"0.1.0\", }\n",
        )
        .unwrap();
        fs::write(child.join("index.nct"), "pub struct Child\n").unwrap();

        let overlay = SourceOverlay::empty();
        assert!(has_package_declaration(&overlay, &root).unwrap());
        assert!(!has_package_declaration(&overlay, &child).unwrap());

        fs::remove_dir_all(root).unwrap();
    }
}
