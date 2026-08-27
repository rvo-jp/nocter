use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nocter_filesystem::SourceOverlay;

use super::{AnalysisScope, WorkspaceAnalysisError};
use crate::WorkspaceConfiguration;

/// The complete document-to-compilation-scope decision for one workspace-source revision.
///
/// Scope selection is evaluated against one immutable overlay. Package-root probes are shared by
/// every document in the revision, so document order cannot change either the chosen scope or the
/// observed source bytes.
pub(super) struct WorkspaceTopology {
    documents: BTreeSet<PathBuf>,
    selected: BTreeMap<PathBuf, AnalysisScope>,
    failures: BTreeMap<PathBuf, WorkspaceAnalysisError>,
}

impl WorkspaceTopology {
    pub(super) fn build(
        configuration: &WorkspaceConfiguration,
        source_overlay: &SourceOverlay,
        documents: BTreeSet<PathBuf>,
    ) -> Self {
        Self::build_with_probe(
            configuration,
            source_overlay,
            documents,
            nocter_package::has_package_declaration,
        )
    }

    fn build_with_probe(
        configuration: &WorkspaceConfiguration,
        source_overlay: &SourceOverlay,
        documents: BTreeSet<PathBuf>,
        mut probe: impl FnMut(
            &SourceOverlay,
            &Path,
        ) -> Result<bool, nocter_package::PackageRootProbeError>,
    ) -> Self {
        let mut package_roots =
            BTreeMap::<PathBuf, Result<bool, Arc<nocter_package::PackageRootProbeError>>>::new();
        let mut selected = BTreeMap::new();
        let mut failures = BTreeMap::new();
        for document in &documents {
            match select_scope(
                configuration,
                source_overlay,
                document,
                &mut package_roots,
                &mut probe,
            ) {
                Ok(scope) => {
                    selected.insert(document.clone(), scope);
                }
                Err(error) => {
                    failures.insert(document.clone(), error);
                }
            }
        }
        Self {
            documents,
            selected,
            failures,
        }
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        BTreeSet<PathBuf>,
        BTreeMap<PathBuf, AnalysisScope>,
        BTreeMap<PathBuf, WorkspaceAnalysisError>,
    ) {
        (self.documents, self.selected, self.failures)
    }
}

fn select_scope(
    configuration: &WorkspaceConfiguration,
    source_overlay: &SourceOverlay,
    document: &Path,
    package_roots: &mut BTreeMap<PathBuf, Result<bool, Arc<nocter_package::PackageRootProbeError>>>,
    probe: &mut impl FnMut(&SourceOverlay, &Path) -> Result<bool, nocter_package::PackageRootProbeError>,
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
        let root = package_roots
            .entry(directory.to_path_buf())
            .or_insert_with(|| probe(source_overlay, directory).map_err(Arc::new));
        match root {
            Ok(true) => return Ok(AnalysisScope::Package(directory.to_path_buf())),
            Ok(false) => {}
            Err(error) => {
                return Err(WorkspaceAnalysisError::package_root_probe(Arc::clone(
                    error,
                )));
            }
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
    use std::cell::Cell;
    use std::collections::BTreeSet;
    use std::fs;

    use nocter_filesystem::SourceOverlay;

    use super::super::tests::{TemporaryDirectory, configuration};
    use super::WorkspaceTopology;

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
        let probes = Cell::new(0);

        let topology = WorkspaceTopology::build_with_probe(
            &configuration(temporary.path()),
            &SourceOverlay::empty(),
            documents,
            |overlay, directory| {
                probes.set(probes.get() + 1);
                nocter_package::has_package_declaration(overlay, directory)
            },
        );

        let (_, selected, failures) = topology.into_parts();
        assert_eq!(selected.len(), 2);
        assert!(failures.is_empty());
        assert_eq!(probes.get(), 1);
    }
}
