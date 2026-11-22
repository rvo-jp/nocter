//! Phase-owned common diagnostics; metadata/integrity failures gain no authored location.
use super::{Failure, ModuleSourcesFailure};
use crate::{
    CompileUnitInput, ImportDiagnostic, LoweringError, SurfaceDiagnostic, SurfaceError,
    ToolchainError, TopologyDiagnostic,
};
use nocter_diagnostics::SourceDiagnostic;

impl Failure {
    /// Common diagnostics already selected by declaration checking. Empty means no public
    /// source diagnostic was established; consumers must retain the typed cause.
    #[must_use]
    pub fn source_diagnostics(&self) -> Vec<&SourceDiagnostic> {
        match self {
            Self::Syntax(diagnostics) => diagnostics.iter().collect(),
            Self::Headers(diagnostics) => diagnostics
                .iter()
                .map(super::HeaderDiagnostic::source)
                .collect(),
            Self::Primitive(diagnostic) => vec![diagnostic.source()],
            Self::SourceRole(diagnostic) => vec![diagnostic.source()],
            Self::Visibility(diagnostic) => vec![diagnostic.source()],
            Self::Import(diagnostic) => vec![diagnostic.source()],
            Self::Namespace(diagnostic) => vec![diagnostic.source()],
            Self::WrongParseGoal
            | Self::InconsistentSyntax
            | Self::ChangedHeader
            | Self::RequiredValues(_)
            | Self::MissingValueEvidence
            | Self::ModuleInputRequired => Vec::new(),
        }
    }
}

impl ModuleSourcesFailure {
    /// Projects authored topology/surface rules through their existing diagnostic owners.
    /// `None` means an authored diagnostic could not bind to this exact input. An empty result
    /// supplies no public diagnostic; the original toolchain/metadata/integrity cause remains.
    #[must_use]
    pub fn project_diagnostics(
        &self,
        input: &CompileUnitInput<'_>,
    ) -> Option<Box<[SourceDiagnostic]>> {
        let one = |diagnostic: &SourceDiagnostic| vec![diagnostic.clone()].into_boxed_slice();
        match self {
            Self::Access(failure) => {
                Some(failure.source_diagnostics().into_iter().cloned().collect())
            }
            Self::Contract(diagnostic) => Some(one(diagnostic.source())),
            Self::Surface(SurfaceError::Topology(LoweringError::Rule(violation))) => {
                TopologyDiagnostic::project(violation, input)
                    .map(|diagnostic| one(diagnostic.source()))
            }
            Self::Surface(error) if crate::surface_diagnostic::classify(error).is_some() => {
                SurfaceDiagnostic::project(error.clone(), input)
                    .ok()
                    .map(|diagnostic| one(diagnostic.source()))
            }
            Self::Toolchain(ToolchainError::Rule(violation)) => {
                ImportDiagnostic::project(*violation, input)
                    .ok()
                    .map(|diagnostic| one(diagnostic.source()))
            }
            Self::Surface(_)
            | Self::PrimitiveAuthority(_)
            | Self::Toolchain(_)
            | Self::InconsistentContract(_) => Some(Box::new([])),
        }
    }
}
