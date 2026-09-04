use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nocter_filesystem::SourceOverlay;
use nocter_source::{SourceError, SourceMap, SourceName};
use nocter_syntax::{
    NodeKind, ParseGoal, ParsedSyntax, SourceSyntaxError, SourceSyntaxProvider, SyntaxElement,
    SyntaxTree,
};

/// Immutable package-root facts selected from one exact source overlay.
///
/// A catalog carries the exact bytes that justified every cached fact. Package loading can assign
/// those bytes its semantic source identity without reopening an `index.nct`, while discovery can
/// reuse the same root decisions when validating traversal.
#[derive(Clone, Debug)]
pub struct PackageRootCatalog {
    source_overlay: SourceOverlay,
    roots: BTreeMap<PathBuf, PackageRootProbe>,
}

impl PackageRootCatalog {
    #[must_use]
    pub fn new(source_overlay: SourceOverlay) -> Self {
        Self {
            source_overlay,
            roots: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn into_builder(self) -> PackageRootCatalogBuilder {
        PackageRootCatalogBuilder { catalog: self }
    }

    #[must_use]
    pub const fn source_overlay(&self) -> &SourceOverlay {
        &self.source_overlay
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.roots.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }
}

/// Construction authority for one package-root catalog.
///
/// Each canonical directory is inspected at most once. Successful and failed probes are retained,
/// so document order and repeated boundary checks cannot change which source bytes were observed.
#[derive(Debug)]
pub struct PackageRootCatalogBuilder {
    catalog: PackageRootCatalog,
}

impl PackageRootCatalogBuilder {
    #[must_use]
    pub fn new(source_overlay: SourceOverlay) -> Self {
        Self {
            catalog: PackageRootCatalog::new(source_overlay),
        }
    }

    /// Reports whether a canonical directory's `index.nct` declares a package.
    ///
    /// # Errors
    ///
    /// Returns the exact retained source-selection or source-decoding failure. A missing index is
    /// reported as `Ok(false)`.
    pub fn has_package_declaration(
        &mut self,
        directory: &Path,
        source_syntax: &mut dyn SourceSyntaxProvider,
    ) -> Result<bool, Arc<PackageRootProbeError>> {
        Ok(self
            .probe(directory, source_syntax)?
            .is_some_and(|root| root.is_package))
    }

    #[must_use]
    pub fn finish(self) -> PackageRootCatalog {
        self.catalog
    }

    pub(crate) fn snapshot(&self) -> PackageRootCatalog {
        self.catalog.clone()
    }

    pub(crate) fn root_source_with_source_syntax(
        &mut self,
        directory: &Path,
        source_syntax: &mut dyn SourceSyntaxProvider,
    ) -> Result<Option<PackageRootSource>, Arc<PackageRootProbeError>> {
        self.probe(directory, source_syntax)
    }

    #[must_use]
    pub const fn source_overlay(&self) -> &SourceOverlay {
        self.catalog.source_overlay()
    }

    fn probe(
        &mut self,
        directory: &Path,
        source_syntax: &mut dyn SourceSyntaxProvider,
    ) -> Result<Option<PackageRootSource>, Arc<PackageRootProbeError>> {
        if let Some(probe) = self.catalog.roots.get(directory) {
            return probe.result();
        }
        let result = self.read_root(directory, source_syntax);
        let probe = match result {
            Ok(root) => PackageRootProbe::Resolved(root),
            Err(error) => PackageRootProbe::Failed(Arc::new(error)),
        };
        let result = probe.result();
        self.catalog.roots.insert(directory.to_path_buf(), probe);
        result
    }

    fn read_root(
        &mut self,
        directory: &Path,
        source_syntax: &mut dyn SourceSyntaxProvider,
    ) -> Result<Option<PackageRootSource>, PackageRootProbeError> {
        let requested_path = directory.join("index.nct");
        let Some(observed) = self
            .catalog
            .source_overlay
            .observe_file(&requested_path)
            .map_err(|source| PackageRootProbeError::Filesystem {
                path: requested_path.clone(),
                source,
            })?
        else {
            return Ok(None);
        };
        let path = observed.canonical_path().to_path_buf();
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new(path.to_string_lossy().as_ref()),
                observed.bytes(),
            )
            .map_err(|source| PackageRootProbeError::Source {
                path: path.clone(),
                source,
            })?;
        let source_file = sources
            .get(source)
            .ok_or_else(|| PackageRootProbeError::MissingSource(path.clone()))?;
        let syntax = source_syntax
            .parsed_syntax(source_file, ParseGoal::SourceFile)
            .map_err(|source| PackageRootProbeError::SourceSyntax {
                path: path.clone(),
                source,
            })?;
        let tree = syntax
            .bind(source_file)
            .ok_or_else(|| PackageRootProbeError::MissingSource(path.clone()))?;
        let is_package = has_package_directive(source_file, &tree);
        Ok(Some(PackageRootSource {
            path,
            bytes: observed.bytes().into(),
            is_package,
            syntax,
        }))
    }
}

#[derive(Clone, Debug)]
enum PackageRootProbe {
    Resolved(Option<PackageRootSource>),
    Failed(Arc<PackageRootProbeError>),
}

impl PackageRootProbe {
    fn result(&self) -> Result<Option<PackageRootSource>, Arc<PackageRootProbeError>> {
        match self {
            Self::Resolved(root) => Ok(root.clone()),
            Self::Failed(error) => Err(Arc::clone(error)),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PackageRootSource {
    path: PathBuf,
    bytes: Arc<[u8]>,
    is_package: bool,
    syntax: Arc<ParsedSyntax>,
}

impl PackageRootSource {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn syntax(&self) -> &ParsedSyntax {
        &self.syntax
    }
}

fn has_package_directive(source: &nocter_source::SourceFile, syntax: &SyntaxTree) -> bool {
    syntax.children(syntax.root_id()).iter().any(|element| {
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
    })
}

#[derive(Debug)]
pub enum PackageRootProbeError {
    Filesystem {
        path: PathBuf,
        source: io::Error,
    },
    Source {
        path: PathBuf,
        source: SourceError,
    },
    MissingSource(PathBuf),
    SourceSyntax {
        path: PathBuf,
        source: SourceSyntaxError,
    },
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
            Self::SourceSyntax { path, source } => {
                write!(formatter, "could not parse {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for PackageRootProbeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Filesystem { source, .. } => Some(source),
            Self::Source { source, .. } => Some(source),
            Self::SourceSyntax { source, .. } => Some(source),
            Self::MissingSource(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn retains_one_parsed_source_for_repeated_root_queries() {
        let root =
            std::env::temp_dir().join(format!("nocter-package-root-probe-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("index.nct"),
            "#package: { name: \"app\", version: \"0.1.0\", }\n",
        )
        .unwrap();

        let root = fs::canonicalize(root).unwrap();
        let mut catalog = PackageRootCatalogBuilder::new(SourceOverlay::empty());
        let mut syntax = nocter_syntax::DirectSourceSyntax;
        assert!(catalog.has_package_declaration(&root, &mut syntax).unwrap());
        assert!(catalog.has_package_declaration(&root, &mut syntax).unwrap());
        assert_eq!(catalog.finish().len(), 1);

        fs::remove_dir_all(root).unwrap();
    }
}
