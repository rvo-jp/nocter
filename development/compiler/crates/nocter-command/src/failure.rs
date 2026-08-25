use nocter_diagnostics::SourceDiagnostic;
use nocter_discovery::DiscoveredUnit;
use nocter_source::SourceMap;

use crate::{BuildCommandError, BuildSetCommandError, RunCommandError};
use nocter_native_session::NativeTestSessionError;
use nocter_session::CompileSessionError;

pub(crate) trait CommandDiagnosticError {
    fn source_diagnostics(&self) -> &[SourceDiagnostic];
}

impl CommandDiagnosticError for BuildCommandError {
    fn source_diagnostics(&self) -> &[SourceDiagnostic] {
        BuildCommandError::source_diagnostics(self)
    }
}

impl CommandDiagnosticError for BuildSetCommandError {
    fn source_diagnostics(&self) -> &[SourceDiagnostic] {
        BuildSetCommandError::source_diagnostics(self)
    }
}

impl CommandDiagnosticError for RunCommandError {
    fn source_diagnostics(&self) -> &[SourceDiagnostic] {
        RunCommandError::source_diagnostics(self)
    }
}

impl CommandDiagnosticError for CompileSessionError {
    fn source_diagnostics(&self) -> &[SourceDiagnostic] {
        CompileSessionError::source_diagnostics(self)
    }
}

impl CommandDiagnosticError for NativeTestSessionError {
    fn source_diagnostics(&self) -> &[SourceDiagnostic] {
        NativeTestSessionError::source_diagnostics(self)
    }
}

/// One failed command compilation plus the immutable source snapshot needed to present it.
///
/// The compiler phase selects source diagnostics before this boundary. This value only preserves
/// those envelopes and their exact invocation sources while the typed failure continues through
/// command orchestration.
#[derive(Debug)]
pub struct CommandCompilationFailure<E> {
    error: E,
    sources: SourceMap,
    diagnostics: Box<[SourceDiagnostic]>,
}

pub(crate) fn command_compilation_failure<E: CommandDiagnosticError>(
    error: E,
    unit: DiscoveredUnit,
) -> CommandCompilationFailure<E> {
    let mut diagnostics = unit.syntax_diagnostics().into_vec();
    diagnostics.extend_from_slice(error.source_diagnostics());
    CommandCompilationFailure {
        error,
        sources: unit.into_sources(),
        diagnostics: diagnostics.into_boxed_slice(),
    }
}

impl<E> CommandCompilationFailure<E> {
    #[must_use]
    pub const fn error(&self) -> &E {
        &self.error
    }

    #[must_use]
    pub const fn sources(&self) -> &SourceMap {
        &self.sources
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &[SourceDiagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn into_parts(self) -> (E, SourceMap, Box<[SourceDiagnostic]>) {
        (self.error, self.sources, self.diagnostics)
    }
}

impl<E: std::fmt::Display> std::fmt::Display for CommandCompilationFailure<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CommandCompilationFailure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
