use std::collections::BTreeSet;
use std::path::PathBuf;

use super::AnalysisScope;

/// Complete compiler demand for one workspace analysis scope.
///
/// A package generation is authoritative for every currently selected source in that package, so
/// no physical source may act as a representative compiler root.
pub(super) enum ScopeCompilationInput {
    Package {
        root: PathBuf,
        requested_sources: Box<[PathBuf]>,
    },
    ToolchainStandard,
    SingleFile(PathBuf),
}

impl ScopeCompilationInput {
    pub(super) fn new(
        scope: &AnalysisScope,
        requested_sources: impl IntoIterator<Item = PathBuf>,
    ) -> Self {
        match scope {
            AnalysisScope::Package(root) => Self::Package {
                root: root.clone(),
                requested_sources: requested_sources
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
            AnalysisScope::ToolchainStandard(_) => Self::ToolchainStandard,
            AnalysisScope::SingleFile(source) => Self::SingleFile(source.clone()),
        }
    }
}
