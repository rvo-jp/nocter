use nocter_compiler_computation::{
    CompilerComputation, CompilerComputationError, CompilerDiscoveredUnit, CompilerDiscoveryError,
    CompilerSourceRevision, ValueAccessAnalysis,
};
use nocter_diagnostics::DiagnosticCode;
use nocter_session::{
    AnalyzedUnit, CompileSessionFailure, CompiledTarget, SemanticAnalysisDomainError,
    ValueAccessEntryError,
};
use nocter_syntax::ParseGoal;

use crate::failure::command_compilation_failure;
use crate::{CommandCompilationFailure, CommandSourceError};

/// Runs one discovered command unit through an ephemeral instance of the shared query owner.
#[derive(Debug)]
pub(crate) struct CommandCompiler {
    computation: CompilerComputation,
    filesystem_epoch: u64,
    source_goal: ParseGoal,
}

impl CommandCompiler {
    pub(crate) fn new(context: &crate::CommandPackageContext) -> Self {
        Self::for_source_goal(context.source_goal())
    }

    fn for_source_goal(source_goal: ParseGoal) -> Self {
        Self {
            computation: CompilerComputation::new(),
            filesystem_epoch: 0,
            source_goal,
        }
    }

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
        let roots = nocter_package::PackageRootCatalog::for_goal(overlay, self.source_goal);
        let mut source_syntax = self
            .computation
            .source_syntax(&revision)
            .map_err(CommandSourceError::Computation)?;
        nocter_package::resolve_standard_package_with_root_catalog(
            standard,
            roots,
            revision.source_identity_domain(),
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
        let roots = nocter_package::PackageRootCatalog::for_goal(overlay, self.source_goal);
        let mut source_syntax = self
            .computation
            .source_syntax(&revision)
            .map_err(CommandPackageQueryError::Computation)?;
        nocter_package::resolve_package_selection_with_root_catalog(
            request,
            roots,
            revision.source_identity_domain(),
            &mut source_syntax,
        )
        .map_err(nocter_package::PackageResolutionFailure::into_error)
        .map_err(CommandPackageQueryError::Resolution)
    }

    pub(crate) fn compile(
        &mut self,
        discovered: &CompilerDiscoveredUnit,
    ) -> Result<CompiledTarget, Box<CommandCompilationFailure<CommandAnalysisError>>> {
        released_target(self.analyze(discovered)?)
    }

    pub(crate) fn check(
        &mut self,
        discovered: &CompilerDiscoveredUnit,
    ) -> Result<CheckedCommandUnit, Box<CommandCompilationFailure<CommandAnalysisError>>> {
        let analyzed = self.analyze(discovered)?;
        if matches!(
            analyzed.value_access_analysis(),
            Some(ValueAccessAnalysis::Accepted(_))
        ) {
            Ok(CheckedCommandUnit::ValueAccess(analyzed))
        } else {
            released_target(analyzed).map(|target| CheckedCommandUnit::Released(Box::new(target)))
        }
    }

    fn analyze(
        &mut self,
        discovered: &CompilerDiscoveredUnit,
    ) -> Result<AnalyzedUnit, Box<CommandCompilationFailure<CommandAnalysisError>>> {
        let unit = discovered.unit();
        let product = self
            .computation
            .analyze(discovered)
            .map_err(CommandAnalysisError::Computation)
            .map_err(|error| Box::new(command_compilation_failure(error, unit)))?;
        nocter_session::analyze_unit_from_query(&product)
            .map_err(CommandAnalysisError::Session)
            .map_err(|error| Box::new(command_compilation_failure(error, unit)))
    }
}

#[cfg(test)]
impl Default for CommandCompiler {
    fn default() -> Self {
        Self::for_source_goal(ParseGoal::SourceFile)
    }
}

pub(crate) enum CheckedCommandUnit {
    Released(Box<CompiledTarget>),
    ValueAccess(AnalyzedUnit),
}

fn released_target(
    analyzed: AnalyzedUnit,
) -> Result<CompiledTarget, Box<CommandCompilationFailure<CommandAnalysisError>>> {
    analyzed.into_compilation_result().map_err(|failure| {
        let (error, sources, diagnostics) = (*failure).into_parts();
        Box::new(CommandCompilationFailure::from_parts(
            CommandAnalysisError::Compilation(error),
            sources,
            diagnostics,
        ))
    })
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
        let roots = nocter_package::PackageRootCatalog::for_goal(overlay, self.source_goal);
        let mut source_syntax = self.computation.source_syntax(&revision).map_err(|error| {
            nocter_package_state::PackageResolutionAttemptError::Infrastructure(Box::new(
                CommandSourceError::Computation(error),
            ))
        })?;
        nocter_package::resolve_package_selection_with_root_catalog(
            request,
            roots,
            revision.source_identity_domain(),
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
    Compilation(CompileSessionFailure),
    ValueAccessTarget(ValueAccessEntryError),
}

impl CommandAnalysisError {
    #[must_use]
    pub fn source_diagnostics(&self) -> &[nocter_diagnostics::SourceDiagnostic] {
        match self {
            Self::Compilation(error) => error.source_diagnostics(),
            Self::Computation(_) | Self::Session(_) | Self::ValueAccessTarget(_) => &[],
        }
    }

    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<DiagnosticCode> {
        match self {
            Self::Compilation(error) => error.diagnostic_code(),
            Self::ValueAccessTarget(error) => error.diagnostic_code(),
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
            Self::ValueAccessTarget(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CommandAnalysisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Computation(error) => Some(error),
            Self::Session(error) => Some(error),
            Self::Compilation(error) => Some(error),
            Self::ValueAccessTarget(error) => Some(error),
        }
    }
}
