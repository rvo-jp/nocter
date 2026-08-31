use std::fmt;

use nocter_diagnostics::DiagnosticCode;
use nocter_native_session::NativeImageSetCompileRequest;
use nocter_package_state::PackageAcquisitionAuthority;
use nocter_session::ExecutableCompileRequest;

use crate::compiler::CommandCompiler;
use crate::failure::command_compilation_failure;
use crate::source::{CommandCompileRoots, discover_command_source};
use crate::{
    BuildCommandError, BuildOperation, BuildSetCommandError, BuiltExecutable, BuiltExecutableSet,
    CommandAnalysisError, CommandCompilationFailure, CommandSourceError, CommandToolchain,
    ExecutedProgram, PreparedBuildCommand, PreparedRunCommand, RunCommandError, build_executables,
    build_selected_executable, run_executable,
};

/// Complete successful output of one prepared build command.
#[derive(Debug)]
pub enum BuildCommandResult {
    Selected(BuiltExecutable),
    PackageSet(BuiltExecutableSet),
}

/// Resolves package/source inputs, discovers one compile unit, compiles it, and publishes the
/// output selected by a prepared build command.
///
/// # Errors
///
/// Returns the exact source, selected-executable, or package-set boundary that failed.
pub fn execute_prepared_build<A: PackageAcquisitionAuthority>(
    command: PreparedBuildCommand,
    toolchain: &CommandToolchain,
    authority: &mut A,
) -> Result<BuildCommandResult, BuildCommandExecutionError> {
    let (plan, resolution, target) = command.into_parts();
    let toolchain = toolchain.for_requested_target(target);
    let (input, operation) = plan.into_parts();
    let compile_roots = match &operation {
        BuildOperation::PackageSet { .. } => CommandCompileRoots::AllExecutables,
        BuildOperation::Selected { selector, .. } => CommandCompileRoots::for_selector(selector),
    };
    let mut compiler = CommandCompiler::default();
    let unit = discover_command_source(
        &input,
        resolution,
        &toolchain,
        compile_roots,
        authority,
        &mut compiler,
    )
    .map_err(BuildCommandExecutionError::Source)?;
    let target = compiler
        .compile(&unit)
        .map_err(BuildCommandExecutionError::Analysis)?;
    match operation {
        BuildOperation::PackageSet { output_directory } => {
            match build_executables(NativeImageSetCompileRequest::all(target), output_directory) {
                Ok(built) => Ok(BuildCommandResult::PackageSet(built)),
                Err(error) => Err(BuildCommandExecutionError::PackageSet(Box::new(
                    command_compilation_failure(error, unit.unit()),
                ))),
            }
        }
        BuildOperation::Selected { selector, output } => {
            match build_selected_executable(ExecutableCompileRequest::new(target, selector), output)
            {
                Ok(built) => Ok(BuildCommandResult::Selected(built)),
                Err(error) => Err(BuildCommandExecutionError::Selected(Box::new(
                    command_compilation_failure(error, unit.unit()),
                ))),
            }
        }
    }
}

/// Resolves package/source inputs, discovers one compile unit, compiles the selected executable,
/// and runs it under the prepared working-directory policy.
///
/// # Errors
///
/// Returns the exact source, compile, artifact, launch, or cleanup boundary that failed.
pub fn execute_prepared_run<A: PackageAcquisitionAuthority>(
    command: PreparedRunCommand,
    toolchain: &CommandToolchain,
    authority: &mut A,
) -> Result<ExecutedProgram, RunCommandExecutionError> {
    let (plan, resolution, target) = command.into_parts();
    let toolchain = toolchain.for_requested_target(target);
    let (input, selector, working_directory) = plan.into_parts();
    let mut compiler = CommandCompiler::default();
    let unit = discover_command_source(
        &input,
        resolution,
        &toolchain,
        CommandCompileRoots::for_selector(&selector),
        authority,
        &mut compiler,
    )
    .map_err(RunCommandExecutionError::Source)?;
    let target = compiler
        .compile(&unit)
        .map_err(RunCommandExecutionError::Analysis)?;
    match run_executable(
        ExecutableCompileRequest::new(target, selector),
        working_directory,
    ) {
        Ok(executed) => Ok(executed),
        Err(error) => Err(RunCommandExecutionError::Run(Box::new(
            command_compilation_failure(error, unit.unit()),
        ))),
    }
}

#[derive(Debug)]
pub enum BuildCommandExecutionError {
    Source(CommandSourceError),
    Analysis(Box<CommandCompilationFailure<CommandAnalysisError>>),
    Selected(Box<CommandCompilationFailure<BuildCommandError>>),
    PackageSet(Box<CommandCompilationFailure<BuildSetCommandError>>),
}

impl BuildCommandExecutionError {
    #[must_use]
    pub fn source_diagnostics(
        &self,
    ) -> Option<(
        &[nocter_diagnostics::SourceDiagnostic],
        &nocter_source::SourceMap,
    )> {
        match self {
            Self::Source(error) => error.source_diagnostics(),
            Self::Analysis(failure) => Some((failure.diagnostics(), failure.sources())),
            Self::Selected(failure) => Some((failure.diagnostics(), failure.sources())),
            Self::PackageSet(failure) => Some((failure.diagnostics(), failure.sources())),
        }
    }

    #[must_use]
    pub fn diagnostic_code(&self) -> Option<DiagnosticCode> {
        match self {
            Self::Source(error) => error.diagnostic_code(),
            Self::Analysis(failure) if failure.diagnostics().is_empty() => {
                failure.error().diagnostic_code()
            }
            Self::Selected(failure) if failure.diagnostics().is_empty() => {
                failure.error().diagnostic_code()
            }
            Self::PackageSet(failure) if failure.diagnostics().is_empty() => {
                failure.error().diagnostic_code()
            }
            Self::Analysis(_) | Self::Selected(_) | Self::PackageSet(_) => None,
        }
    }

    #[must_use]
    pub fn is_user_failure(&self) -> bool {
        match self {
            Self::Source(error) => error.is_user_failure(),
            Self::Analysis(failure) => failure.error().diagnostic_code().is_some(),
            Self::Selected(failure) => failure.error().diagnostic_code().is_some(),
            Self::PackageSet(failure) => failure.error().diagnostic_code().is_some(),
        }
    }
}

impl fmt::Display for BuildCommandExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "build input failed: {error}"),
            Self::Analysis(error) => error.fmt(formatter),
            Self::Selected(error) => error.fmt(formatter),
            Self::PackageSet(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BuildCommandExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Analysis(error) => Some(error),
            Self::Selected(error) => Some(error),
            Self::PackageSet(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub enum RunCommandExecutionError {
    Source(CommandSourceError),
    Analysis(Box<CommandCompilationFailure<CommandAnalysisError>>),
    Run(Box<CommandCompilationFailure<RunCommandError>>),
}

impl RunCommandExecutionError {
    #[must_use]
    pub fn source_diagnostics(
        &self,
    ) -> Option<(
        &[nocter_diagnostics::SourceDiagnostic],
        &nocter_source::SourceMap,
    )> {
        match self {
            Self::Source(error) => error.source_diagnostics(),
            Self::Analysis(failure) => Some((failure.diagnostics(), failure.sources())),
            Self::Run(failure) => Some((failure.diagnostics(), failure.sources())),
        }
    }

    #[must_use]
    pub fn diagnostic_code(&self) -> Option<DiagnosticCode> {
        match self {
            Self::Source(error) => error.diagnostic_code(),
            Self::Analysis(failure) if failure.diagnostics().is_empty() => {
                failure.error().diagnostic_code()
            }
            Self::Run(failure) if failure.diagnostics().is_empty() => {
                failure.error().diagnostic_code()
            }
            Self::Analysis(_) | Self::Run(_) => None,
        }
    }

    #[must_use]
    pub fn is_user_failure(&self) -> bool {
        match self {
            Self::Source(error) => error.is_user_failure(),
            Self::Analysis(failure) => failure.error().diagnostic_code().is_some(),
            Self::Run(failure) => failure.error().diagnostic_code().is_some(),
        }
    }
}

impl fmt::Display for RunCommandExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "run input failed: {error}"),
            Self::Analysis(error) => error.fmt(formatter),
            Self::Run(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RunCommandExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Analysis(error) => Some(error),
            Self::Run(error) => Some(error),
        }
    }
}
