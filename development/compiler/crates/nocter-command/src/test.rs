use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use nocter_diagnostics::SourceDiagnostic;
use nocter_discovery::DiscoveryFailure;
use nocter_model::{CompilationTarget, PackageIdentity};
use nocter_native_session::{
    NativeTestCompileRequest, NativeTestSessionError, NativeTestTargetOutcome, TestCaseIdentity,
    compile_native_tests,
};
use nocter_package_state::PackageAcquisitionAuthority;
use nocter_source::SourceMap;

use crate::failure::command_compilation_failure;
use crate::source::discover_command_tests;
use crate::{
    CommandCompilationFailure, CommandSourceError, CommandToolchain, DiagnosticFormat,
    PreparedTestCommand, stage_temporary_image,
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

/// Resolver-stable package identity and authored name of one command-selected test target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestRunTarget {
    package: PackageIdentity,
    name: Box<str>,
}

impl TestRunTarget {
    fn new(package: PackageIdentity, name: Box<str>) -> Self {
        Self { package, name }
    }

    #[must_use]
    pub const fn package(&self) -> &PackageIdentity {
        &self.package
    }

    #[must_use]
    pub const fn name(&self) -> &str {
        &self.name
    }
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
    target: TestRunTarget,
    test: Option<TestCaseIdentity>,
    outcome: TestRunOutcome,
    exit_code: Option<i32>,
    signal: Option<i32>,
    stdout: Box<[u8]>,
    stderr: Box<[u8]>,
    diagnostics: Box<[TestRunDiagnostic]>,
    source_diagnostics: Box<[SourceDiagnostic]>,
    sources: Option<SourceMap>,
}

impl TestRunResult {
    #[must_use]
    pub const fn target(&self) -> &TestRunTarget {
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

    #[must_use]
    pub const fn source_diagnostics(&self) -> &[SourceDiagnostic] {
        &self.source_diagnostics
    }

    #[must_use]
    pub const fn sources(&self) -> Option<&SourceMap> {
        self.sources.as_ref()
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
    let sources = discover_command_tests(&input, resolution, &toolchain, &selector, authority)
        .map_err(|error| TestCommandExecutionError::Source {
            presentation: presentation.clone(),
            error: Box::new(error),
        })?;
    let mut package = None;
    let mut runs = Vec::new();
    for source in sources {
        let (source_package, target_name, discovery) = source.into_parts();
        if let Some(package) = &package {
            if package != &source_package {
                return Err(TestCommandExecutionError::Integrity(
                    TestCommandIntegrityError::MultiplePackages,
                ));
            }
        } else {
            package = Some(source_package.clone());
        }
        let target = TestRunTarget::new(source_package, target_name);
        let unit = match discovery {
            Ok(unit) => unit,
            Err(failure) => {
                let wrapped = CommandSourceError::Discovery(failure);
                let diagnostic_code = wrapped.diagnostic_code().unwrap_or("E0900");
                let CommandSourceError::Discovery(failure) = wrapped else {
                    unreachable!("constructed discovery error retains its variant")
                };
                runs.push(discovery_failure(target, &failure, diagnostic_code));
                continue;
            }
        };
        let request = NativeTestCompileRequest::new(
            &unit,
            nocter_session::TestTargetSelector::Named(target.name().into()),
            case.clone(),
        );
        match compile_native_tests(request) {
            Ok(compiled) => runs.extend(
                run_compiled_target(compiled, &target, &working_directory)
                    .map_err(TestCommandExecutionError::Integrity)?,
            ),
            Err(error @ NativeTestSessionError::Selection(_)) => {
                return Err(TestCommandExecutionError::Compile {
                    presentation: presentation.clone(),
                    failure: Box::new(command_compilation_failure(error, unit)),
                });
            }
            Err(error) => {
                let code = error.diagnostic_code().unwrap_or("E0900");
                let message = error.to_string();
                let (_, sources, diagnostics) =
                    command_compilation_failure(error, unit).into_parts();
                runs.push(compile_failure(target, code, message, diagnostics, sources));
            }
        }
    }
    let package = package.ok_or(TestCommandExecutionError::Integrity(
        TestCommandIntegrityError::EmptyCompilation,
    ))?;
    Ok(finish_test_result(package, presentation, runs))
}

fn run_compiled_target(
    compiled: nocter_native_session::CompiledNativeTestSet,
    target: &TestRunTarget,
    working_directory: &Path,
) -> Result<Vec<TestRunResult>, TestCommandIntegrityError> {
    let (targets, _) = compiled.into_parts();
    if targets.len() != 1 {
        return Err(TestCommandIntegrityError::UnexpectedTargetCount);
    }
    let mut runs = Vec::new();
    for compilation in targets {
        let (identity, outcome) = compilation.into_parts();
        if identity.package() != target.package() || identity.name() != target.name() {
            return Err(TestCommandIntegrityError::MismatchedTarget);
        }
        match outcome {
            NativeTestTargetOutcome::Compiled(cases) => {
                for case in cases {
                    let (test, image) = case.into_parts();
                    runs.push(run_test_case(
                        target.clone(),
                        test,
                        &image,
                        working_directory,
                    ));
                }
            }
            NativeTestTargetOutcome::CompileFailed(error) => runs.push(TestRunResult {
                target: target.clone(),
                test: None,
                outcome: TestRunOutcome::CompileFailed,
                exit_code: None,
                signal: None,
                stdout: Box::new([]),
                stderr: Box::new([]),
                diagnostics: Box::new([TestRunDiagnostic::new("E0900", error.to_string())]),
                source_diagnostics: Box::new([]),
                sources: None,
            }),
        }
    }
    Ok(runs)
}

fn discovery_failure(
    target: TestRunTarget,
    failure: &DiscoveryFailure,
    code: &'static str,
) -> TestRunResult {
    let message = failure.to_string();
    let source_diagnostics = failure.diagnostics().to_vec().into_boxed_slice();
    let sources = failure.sources().clone();
    let diagnostics: Box<[TestRunDiagnostic]> = if source_diagnostics.is_empty() {
        vec![TestRunDiagnostic::new(code, message)].into_boxed_slice()
    } else {
        Box::new([])
    };
    TestRunResult {
        target,
        test: None,
        outcome: TestRunOutcome::CompileFailed,
        exit_code: None,
        signal: None,
        stdout: Box::new([]),
        stderr: Box::new([]),
        diagnostics,
        source_diagnostics,
        sources: Some(sources),
    }
}

fn compile_failure(
    target: TestRunTarget,
    code: &'static str,
    message: String,
    source_diagnostics: Box<[SourceDiagnostic]>,
    sources: SourceMap,
) -> TestRunResult {
    let diagnostics: Box<[TestRunDiagnostic]> = if source_diagnostics.is_empty() {
        vec![TestRunDiagnostic::new(code, message)].into_boxed_slice()
    } else {
        Box::new([])
    };
    TestRunResult {
        target,
        test: None,
        outcome: TestRunOutcome::CompileFailed,
        exit_code: None,
        signal: None,
        stdout: Box::new([]),
        stderr: Box::new([]),
        diagnostics,
        source_diagnostics,
        sources: Some(sources),
    }
}

fn finish_test_result(
    package: PackageIdentity,
    presentation: TestCommandPresentation,
    runs: Vec<TestRunResult>,
) -> TestCommandResult {
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
    TestCommandResult {
        package,
        presentation,
        runs: runs.into_boxed_slice(),
        summary,
    }
}

fn run_test_case(
    target: TestRunTarget,
    test: TestCaseIdentity,
    image: &nocter_native_session::NativeImage,
    working_directory: &Path,
) -> TestRunResult {
    let artifact = match stage_temporary_image(image.bytes()) {
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
                source_diagnostics: Box::new([]),
                sources: None,
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
            source_diagnostics: Box::new([]),
            sources: None,
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
    target: TestRunTarget,
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
        source_diagnostics: Box::new([]),
        sources: None,
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
    UnexpectedTargetCount,
    MismatchedTarget,
}

impl fmt::Display for TestCommandIntegrityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCompilation => formatter.write_str("test compilation returned no target"),
            Self::MultiplePackages => {
                formatter.write_str("test compilation crossed command-root package identity")
            }
            Self::UnexpectedTargetCount => {
                formatter.write_str("one test discovery did not produce exactly one target")
            }
            Self::MismatchedTarget => {
                formatter.write_str("compiled test target does not match command selection")
            }
        }
    }
}

impl std::error::Error for TestCommandIntegrityError {}
