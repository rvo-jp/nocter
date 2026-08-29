use nocter_compiler_computation::{
    CompilerComputation, CompilerComputationError, CompilerDiscoveredUnit, CompilerDiscoveryError,
    CompilerSourceRevision,
};
use nocter_session::{CompileSessionError, CompiledTarget, SemanticAnalysisDomainError};

use crate::failure::command_compilation_failure;
use crate::{CommandCompilationFailure, CommandSourceError};

/// Runs one discovered command unit through an ephemeral instance of the shared query owner.
#[derive(Debug, Default)]
pub(crate) struct CommandCompiler {
    computation: CompilerComputation,
    filesystem_epoch: u64,
}

impl CommandCompiler {
    #[cfg(test)]
    pub(crate) fn statistics(&self) -> nocter_compiler_computation::CompilerComputationStatistics {
        self.computation.statistics()
    }

    fn advance_sources(
        &mut self,
        overlay: &nocter_filesystem::SourceOverlay,
    ) -> Result<CompilerSourceRevision, CompilerComputationError> {
        self.computation
            .advance_sources(overlay, self.filesystem_epoch)
    }

    pub(crate) fn discover(
        &mut self,
        request: nocter_discovery::DiscoveryRequest,
    ) -> Result<CompilerDiscoveredUnit, CommandSourceError> {
        let revision = self
            .advance_sources(request.source_overlay())
            .map_err(CommandSourceError::Computation)?;
        self.computation
            .discover(&revision, request)
            .map_err(|error| match error {
                CompilerDiscoveryError::Computation(error) => {
                    CommandSourceError::Computation(error)
                }
                CompilerDiscoveryError::Discovery(error) => CommandSourceError::Discovery(error),
            })
    }

    pub(crate) fn resolve_standard_package(
        &mut self,
        standard: nocter_package::StandardPackage,
    ) -> Result<nocter_package::ResolvedPackageGraph, CommandSourceError> {
        let overlay = nocter_filesystem::SourceOverlay::empty();
        let revision = self
            .advance_sources(&overlay)
            .map_err(CommandSourceError::Computation)?;
        let roots = nocter_package::PackageRootCatalog::new(overlay);
        let mut source_syntax = self
            .computation
            .source_syntax(&revision)
            .map_err(CommandSourceError::Computation)?;
        nocter_package::resolve_standard_package_with_root_catalog(
            standard,
            roots,
            &mut source_syntax,
        )
        .map_err(CommandSourceError::StandardPackage)
    }

    pub(crate) fn resolve_package_selection(
        &mut self,
        request: nocter_package::PackageResolutionRequest,
    ) -> Result<nocter_package::ResolvedPackageSelection, CommandPackageQueryError> {
        let overlay = nocter_filesystem::SourceOverlay::empty();
        let revision = self
            .advance_sources(&overlay)
            .map_err(CommandPackageQueryError::Computation)?;
        let roots = nocter_package::PackageRootCatalog::new(overlay);
        let mut source_syntax = self
            .computation
            .source_syntax(&revision)
            .map_err(CommandPackageQueryError::Computation)?;
        nocter_package::resolve_package_selection_with_root_catalog(
            request,
            roots,
            &mut source_syntax,
        )
        .map_err(nocter_package::PackageResolutionFailure::into_error)
        .map_err(CommandPackageQueryError::Resolution)
    }

    pub(crate) fn compile(
        &mut self,
        discovered: &CompilerDiscoveredUnit,
    ) -> Result<CompiledTarget, Box<CommandCompilationFailure<CommandAnalysisError>>> {
        let unit = discovered.unit();
        let product = self
            .computation
            .analyze(discovered)
            .map_err(CommandAnalysisError::Computation)
            .map_err(|error| Box::new(command_compilation_failure(error, unit)))?;
        let analyzed = nocter_session::analyze_unit_from_query(&product)
            .map_err(CommandAnalysisError::Session)
            .map_err(|error| Box::new(command_compilation_failure(error, unit)))?;
        analyzed.into_compilation_result().map_err(|failure| {
            let (error, sources, diagnostics) = (*failure).into_parts();
            Box::new(CommandCompilationFailure::from_parts(
                CommandAnalysisError::Compilation(error),
                sources,
                diagnostics,
            ))
        })
    }
}

pub(crate) enum CommandPackageQueryError {
    Computation(CompilerComputationError),
    Resolution(nocter_package::PackageResolutionError),
}

impl nocter_package_state::PackageResolutionDriver for CommandCompiler {
    fn resolve(
        &mut self,
        request: nocter_package::PackageResolutionRequest,
        filesystem_revision: nocter_package_state::PackageFilesystemRevision,
    ) -> Result<
        nocter_package::ResolvedPackageSelection,
        nocter_package_state::PackageResolutionAttemptError,
    > {
        self.filesystem_epoch = filesystem_revision.get();
        let overlay = nocter_filesystem::SourceOverlay::empty();
        let revision = self.advance_sources(&overlay).map_err(|error| {
            nocter_package_state::PackageResolutionAttemptError::Infrastructure(Box::new(error))
        })?;
        let roots = nocter_package::PackageRootCatalog::new(overlay);
        let mut source_syntax = self.computation.source_syntax(&revision).map_err(|error| {
            nocter_package_state::PackageResolutionAttemptError::Infrastructure(Box::new(
                CommandSourceError::Computation(error),
            ))
        })?;
        nocter_package::resolve_package_selection_with_root_catalog(
            request,
            roots,
            &mut source_syntax,
        )
        .map_err(nocter_package::PackageResolutionFailure::into_error)
        .map_err(nocter_package_state::PackageResolutionAttemptError::Domain)
    }
}

/// Failure from the shared computation entry or its sole session consumer.
#[derive(Debug)]
pub enum CommandAnalysisError {
    Computation(CompilerComputationError),
    Session(SemanticAnalysisDomainError),
    Compilation(CompileSessionError),
}

impl CommandAnalysisError {
    #[must_use]
    pub fn source_diagnostics(&self) -> &[nocter_diagnostics::SourceDiagnostic] {
        match self {
            Self::Compilation(error) => error.source_diagnostics(),
            Self::Computation(_) | Self::Session(_) => &[],
        }
    }

    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<&'static str> {
        match self {
            Self::Compilation(error) => error.diagnostic_code(),
            Self::Computation(_) | Self::Session(_) => None,
        }
    }
}

impl std::fmt::Display for CommandAnalysisError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Computation(error) => write!(formatter, "compiler computation failed: {error}"),
            Self::Session(error) => write!(formatter, "session analysis failed: {error}"),
            Self::Compilation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CommandAnalysisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Computation(error) => Some(error),
            Self::Session(error) => Some(error),
            Self::Compilation(error) => Some(error),
        }
    }
}
