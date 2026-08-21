use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{CompilationTarget, PackageIdentity};
use nocter_package_state::PackageAcquisitionAuthority;
use nocter_session::{
    NativeTestCompileRequest, NativeTestSessionError, NativeTestTargetOutcome, TestCaseIdentity,
    TestTargetIdentity, compile_native_tests,
};

use crate::failure::command_compilation_failure;
use crate::source::{CommandCompileRoots, discover_command_source};
use crate::{
    CommandCompilationFailure, CommandSourceError, CommandToolchain, DiagnosticFormat,
    PreparedTestCommand, ResolvedProgramInput, stage_temporary_image,
};

/// Stable presentation facts for both successful and failed test commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestCommandPresentation {
    format: DiagnosticFormat,
    target: CompilationTarget,
    root: PathBuf,
}

impl TestCommandPresentation {
    fn new(format: DiagnosticFormat, target: CompilationTarget, root: PathBuf) -> Self {
        Self {
            format,
            target,
            root,
        }
    }

    #[must_use]
    pub const fn format(&self) -> DiagnosticFormat {
        self.format
    }

    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Public outcome vocabulary for one selected test process or target-wide compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestRunOutcome {
    Passed,
    Failed,
    CompileFailed,
    RunnerFailed,
}

impl TestRunOutcome {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::CompileFailed => "compile_failed",
            Self::RunnerFailed => "runner_failed",
        }
    }
}

/// One diagnostic owned by an individual test run rather than the shared source session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestRunDiagnostic {
    code: &'static str,
    message: Box<str>,
}

impl TestRunDiagnostic {
    fn new(code: &'static str, message: impl Into<Box<str>>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }

    #[must_use]
    pub const fn message(&self) -> &str {
        &self.message
    }
}

/// Ordered result for one target-wide failure or one exact source test declaration.
#[derive(Debug)]
pub struct TestRunResult {
    target: TestTargetIdentity,
    test: Option<TestCaseIdentity>,
    outcome: TestRunOutcome,
    exit_code: Option<i32>,
    signal: Option<i32>,
    stdout: Box<[u8]>,
    stderr: Box<[u8]>,
    diagnostics: Box<[TestRunDiagnostic]>,
}

impl TestRunResult {
    #[must_use]
    pub const fn target(&self) -> &TestTargetIdentity {
        &self.target
    }

    #[must_use]
    pub const fn test(&self) -> Option<&TestCaseIdentity> {
        self.test.as_ref()
    }

    #[must_use]
    pub fn test_name(&self) -> Option<&str> {
        self.test.as_ref().map(TestCaseIdentity::name)
    }

    #[must_use]
    pub const fn outcome(&self) -> TestRunOutcome {
        self.outcome
    }

    #[must_use]
    pub const fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    #[must_use]
    pub const fn signal(&self) -> Option<i32> {
        self.signal
    }

    #[must_use]
    pub const fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    #[must_use]
    pub const fn stderr(&self) -> &[u8] {
        &self.stderr
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &[TestRunDiagnostic] {
        &self.diagnostics
    }
}

/// Aggregate counts derived once from the ordered result set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TestSummary {
    passed: usize,
    failed: usize,
}

impl TestSummary {
    #[must_use]
    pub const fn passed(self) -> usize {
        self.passed
    }

    #[must_use]
    pub const fn failed(self) -> usize {
        self.failed
    }
}

/// Complete typed result consumed by both human and JSON presentation.
#[derive(Debug)]
pub struct TestCommandResult {
    package: PackageIdentity,
    presentation: TestCommandPresentation,
    runs: Box<[TestRunResult]>,
    summary: TestSummary,
}

impl TestCommandResult {
    #[must_use]
    pub const fn package(&self) -> &PackageIdentity {
        &self.package
    }

    #[must_use]
    pub const fn presentation(&self) -> &TestCommandPresentation {
        &self.presentation
    }

    #[must_use]
    pub const fn runs(&self) -> &[TestRunResult] {
        &self.runs
    }

    #[must_use]
    pub const fn summary(&self) -> TestSummary {
        self.summary
    }

    #[must_use]
    pub const fn succeeded(&self) -> bool {
        self.summary.failed == 0
    }
}

/// Resolves, compiles, and independently runs one prepared package test selection.
///
/// # Errors
///
/// Returns only failures that prevent construction of an ordered run set. Target-wide backend,
/// staging, launch, process, and cleanup failures are retained as individual failed runs.
pub fn execute_prepared_test<A: PackageAcquisitionAuthority>(
    command: PreparedTestCommand,
    toolchain: &CommandToolchain,
    authority: &mut A,
) -> Result<TestCommandResult, TestCommandExecutionError> {
    let (plan, resolution, format, target) = command.into_parts();
    let toolchain = toolchain.for_requested_target(target);
    let (input, selector, case, working_directory) = plan.into_parts();
    let presentation =
        TestCommandPresentation::new(format, toolchain.target(), input.declaration().into());
    let roots = CommandCompileRoots::for_test_selector(&selector);
    let unit = discover_command_source(
        &ResolvedProgramInput::Package(input),
        resolution,
        &toolchain,
        roots,
        authority,
    )
    .map_err(|error| TestCommandExecutionError::Source {
        presentation: presentation.clone(),
        error: Box::new(error),
    })?;
    let request = NativeTestCompileRequest::new(&unit, selector, case);
    let compiled =
        compile_native_tests(request).map_err(|error| TestCommandExecutionError::Compile {
            presentation: presentation.clone(),
            failure: Box::new(command_compilation_failure(error, unit)),
        })?;
    run_compiled_tests(compiled, presentation, &working_directory)
        .map_err(TestCommandExecutionError::Integrity)
}

fn run_compiled_tests(
    compiled: nocter_session::CompiledNativeTestSet,
    presentation: TestCommandPresentation,
    working_directory: &Path,
) -> Result<TestCommandResult, TestCommandIntegrityError> {
    let (targets, _) = compiled.into_parts();
    let package = targets
        .first()
        .ok_or(TestCommandIntegrityError::EmptyCompilation)?
        .identity()
        .package()
        .clone();
    let mut runs = Vec::new();
    for target in targets {
        let (identity, outcome) = target.into_parts();
        if identity.package() != &package {
            return Err(TestCommandIntegrityError::MultiplePackages);
        }
        match outcome {
            NativeTestTargetOutcome::Compiled(cases) => {
                for case in cases {
                    let (test, image) = case.into_parts();
                    runs.push(run_test_case(
                        identity.clone(),
                        test,
                        &image,
                        working_directory,
                    ));
                }
            }
            NativeTestTargetOutcome::CompileFailed(error) => runs.push(TestRunResult {
                target: identity,
                test: None,
                outcome: TestRunOutcome::CompileFailed,
                exit_code: None,
                signal: None,
                stdout: Box::new([]),
                stderr: Box::new([]),
                diagnostics: Box::new([TestRunDiagnostic::new("E0900", error.to_string())]),
            }),
        }
    }
    let summary = TestSummary {
        passed: runs
            .iter()
            .filter(|run| run.outcome == TestRunOutcome::Passed)
            .count(),
        failed: runs
            .iter()
            .filter(|run| run.outcome != TestRunOutcome::Passed)
            .count(),
    };
    Ok(TestCommandResult {
        package,
        presentation,
        runs: runs.into_boxed_slice(),
        summary,
    })
}

fn run_test_case(
    target: TestTargetIdentity,
    test: TestCaseIdentity,
    image: &nocter_macho::MachOImage,
    working_directory: &Path,
) -> TestRunResult {
    let artifact = match stage_temporary_image(image) {
        Ok(artifact) => artifact,
        Err(error) => return runner_failure(target, test, error.to_string()),
    };
    let executable = artifact.path().to_path_buf();
    let launched = Command::new(&executable)
        .current_dir(working_directory)
        .output();
    let removed = artifact.remove();
    match (launched, removed) {
        (Ok(output), Ok(())) => {
            let exit_code = output.status.code();
            let signal = process_signal(output.status);
            TestRunResult {
                target,
                test: Some(test),
                outcome: if output.status.success() {
                    TestRunOutcome::Passed
                } else {
                    TestRunOutcome::Failed
                },
                exit_code,
                signal,
                stdout: output.stdout.into_boxed_slice(),
                stderr: output.stderr.into_boxed_slice(),
                diagnostics: Box::new([]),
            }
        }
        (Err(source), Ok(())) => runner_failure(
            target,
            test,
            format!("failed to launch {}: {source}", executable.display()),
        ),
        (Ok(output), Err(cleanup)) => TestRunResult {
            target,
            test: Some(test),
            outcome: TestRunOutcome::RunnerFailed,
            exit_code: output.status.code(),
            signal: process_signal(output.status),
            stdout: output.stdout.into_boxed_slice(),
            stderr: output.stderr.into_boxed_slice(),
            diagnostics: Box::new([TestRunDiagnostic::new(
                "E0704",
                format!("temporary executable cleanup failed: {cleanup}"),
            )]),
        },
        (Err(source), Err(cleanup)) => runner_failure(
            target,
            test,
            format!(
                "failed to launch {}: {source}; cleanup also failed: {cleanup}",
                executable.display()
            ),
        ),
    }
}

fn runner_failure(
    target: TestTargetIdentity,
    test: TestCaseIdentity,
    message: impl Into<Box<str>>,
) -> TestRunResult {
    TestRunResult {
        target,
        test: Some(test),
        outcome: TestRunOutcome::RunnerFailed,
        exit_code: None,
        signal: None,
        stdout: Box::new([]),
        stderr: Box::new([]),
        diagnostics: Box::new([TestRunDiagnostic::new("E0704", message)]),
    }
}

#[cfg(unix)]
fn process_signal(status: std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn process_signal(_status: std::process::ExitStatus) -> Option<i32> {
    None
}

#[derive(Debug)]
pub enum TestCommandExecutionError {
    Source {
        presentation: TestCommandPresentation,
        error: Box<CommandSourceError>,
    },
    Compile {
        presentation: TestCommandPresentation,
        failure: Box<CommandCompilationFailure<NativeTestSessionError>>,
    },
    Integrity(TestCommandIntegrityError),
}

impl TestCommandExecutionError {
    #[must_use]
    pub const fn presentation(&self) -> Option<&TestCommandPresentation> {
        match self {
            Self::Source { presentation, .. } | Self::Compile { presentation, .. } => {
                Some(presentation)
            }
            Self::Integrity(_) => None,
        }
    }

    #[must_use]
    pub fn source_diagnostics(&self) -> Option<(&[SourceDiagnostic], &nocter_source::SourceMap)> {
        match self {
            Self::Source { error, .. } => error.source_diagnostics(),
            Self::Compile { failure, .. } => Some((failure.diagnostics(), failure.sources())),
            Self::Integrity(_) => None,
        }
    }

    #[must_use]
    pub fn diagnostic_code(&self) -> Option<&'static str> {
        match self {
            Self::Source { error, .. } => error.diagnostic_code(),
            Self::Compile { failure, .. } if failure.diagnostics().is_empty() => {
                failure.error().diagnostic_code()
            }
            Self::Compile { .. } | Self::Integrity(_) => None,
        }
    }

    #[must_use]
    pub fn is_user_failure(&self) -> bool {
        match self {
            Self::Source { error, .. } => error.is_user_failure(),
            Self::Compile { failure, .. } => failure.error().diagnostic_code().is_some(),
            Self::Integrity(_) => false,
        }
    }
}

impl fmt::Display for TestCommandExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source { error, .. } => write!(formatter, "test input failed: {error}"),
            Self::Compile { failure, .. } => failure.fmt(formatter),
            Self::Integrity(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TestCommandExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source { error, .. } => Some(error),
            Self::Compile { failure, .. } => Some(failure),
            Self::Integrity(error) => Some(error),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestCommandIntegrityError {
    EmptyCompilation,
    MultiplePackages,
}

impl fmt::Display for TestCommandIntegrityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCompilation => formatter.write_str("test compilation returned no target"),
            Self::MultiplePackages => {
                formatter.write_str("test compilation crossed command-root package identity")
            }
        }
    }
}

impl std::error::Error for TestCommandIntegrityError {}
