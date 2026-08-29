use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use nocter_filesystem::SourceOverlay;
use nocter_syntax::SourceSyntaxProvider;

use super::{AnalysisScope, WorkspaceAnalysisError};
use crate::WorkspaceConfiguration;

/// The complete document-to-compilation-scope decision for one workspace-source revision.
///
/// Scope selection is evaluated against one immutable overlay. Package-root probes are shared by
/// every document in the revision, so document order cannot change either the chosen scope or the
/// observed source bytes.
pub(super) struct WorkspaceTopology {
    selections: BTreeMap<PathBuf, DocumentScopeSelection>,
    package_roots: nocter_package::PackageRootCatalog,
}

pub(super) enum DocumentScopeSelection {
    Selected(AnalysisScope),
    Rejected(WorkspaceAnalysisError),
}

impl WorkspaceTopology {
    pub(super) fn build_with_source_syntax(
        configuration: &WorkspaceConfiguration,
        source_overlay: &SourceOverlay,
        documents: BTreeSet<PathBuf>,
        source_syntax: &mut dyn SourceSyntaxProvider,
    ) -> Self {
        let mut package_roots =
            nocter_package::PackageRootCatalogBuilder::new(source_overlay.clone());
        let selections = documents
            .into_iter()
            .map(|document| {
                let selection =
                    match select_scope(configuration, &document, &mut package_roots, source_syntax)
                    {
                        Ok(scope) => DocumentScopeSelection::Selected(scope),
                        Err(error) => DocumentScopeSelection::Rejected(error),
                    };
                (document, selection)
            })
            .collect();
        Self {
            selections,
            package_roots: package_roots.finish(),
        }
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        BTreeMap<PathBuf, DocumentScopeSelection>,
        nocter_package::PackageRootCatalog,
    ) {
        (self.selections, self.package_roots)
    }
}

fn select_scope(
    configuration: &WorkspaceConfiguration,
    document: &Path,
    package_roots: &mut nocter_package::PackageRootCatalogBuilder,
    source_syntax: &mut dyn SourceSyntaxProvider,
) -> Result<AnalysisScope, WorkspaceAnalysisError> {
    if document
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("nct")
    {
        return Err(WorkspaceAnalysisError::unsupported_source(
            document.to_path_buf(),
        ));
    }
    let standard_root = configuration.toolchain().standard().root();
    if document.starts_with(standard_root) {
        return Ok(AnalysisScope::ToolchainStandard(
            standard_root.to_path_buf(),
        ));
    }
    let workspace = configuration
        .root_for_document(document)
        .ok_or_else(|| WorkspaceAnalysisError::outside_workspace(document.to_path_buf()))?;
    let mut directory = document
        .parent()
        .ok_or_else(|| WorkspaceAnalysisError::outside_workspace(document.to_path_buf()))?;
    loop {
        match package_roots.has_package_declaration(directory, source_syntax) {
            Ok(true) => return Ok(AnalysisScope::Package(directory.to_path_buf())),
            Ok(false) => {}
            Err(error) => return Err(WorkspaceAnalysisError::package_root_probe(error)),
        }
        if directory == workspace {
            break;
        }
        let Some(parent) = directory.parent() else {
            break;
        };
        directory = parent;
    }
    Ok(AnalysisScope::SingleFile(document.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;

    use nocter_filesystem::SourceOverlay;

    use super::super::tests::{TemporaryDirectory, configuration};
    use super::{DocumentScopeSelection, WorkspaceTopology};

    #[test]
    fn probes_each_candidate_directory_once_for_the_whole_revision() {
        let temporary = TemporaryDirectory::new();
        fs::write(
            temporary.path().join("index.nct"),
            "#package: { name: \"app\", version: \"0.1.0\", }\n",
        )
        .unwrap();
        let source_a = temporary.path().join("a.nct");
        let source_b = temporary.path().join("b.nct");
        fs::write(&source_a, "func a(): void {}\n").unwrap();
        fs::write(&source_b, "func b(): void {}\n").unwrap();
        let documents = BTreeSet::from([
            fs::canonicalize(source_a).unwrap(),
            fs::canonicalize(source_b).unwrap(),
        ]);
        let topology = WorkspaceTopology::build_with_source_syntax(
            &configuration(temporary.path()),
            &SourceOverlay::empty(),
            documents,
            &mut nocter_syntax::DirectSourceSyntax,
        );

        let (selections, package_roots) = topology.into_parts();
        assert_eq!(selections.len(), 2);
        assert!(
            selections
                .values()
                .all(|selection| matches!(selection, DocumentScopeSelection::Selected(_)))
        );
        assert_eq!(package_roots.len(), 1);
    }
}
