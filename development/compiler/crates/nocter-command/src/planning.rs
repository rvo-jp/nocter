use std::fmt;
use std::path::{Path, PathBuf};

use nocter_session::{ExecutableSelector, TestTargetSelector};

use crate::{PackageCommandInput, ResolvedProgramInput, RunProgramArguments};

/// Raw executable choice accepted by the check argument parser.
#[derive(Debug, Default)]
pub struct CheckCommandOptions {
    executable: Option<Box<str>>,
}

impl CheckCommandOptions {
    #[must_use]
    pub fn new(executable: Option<Box<str>>) -> Self {
        Self { executable }
    }

    #[must_use]
    pub fn executable(name: impl Into<Box<str>>) -> Self {
        Self::new(Some(name.into()))
    }
}

/// Closed check selection after package/file input normalization.
#[derive(Debug)]
pub struct CheckCommandPlan {
    input: ResolvedProgramInput,
    executable: Option<Box<str>>,
}

impl CheckCommandPlan {
    /// Applies check selection without inspecting semantic declarations.
    ///
    /// # Errors
    ///
    /// Rejects `--executable` in single-file mode. Package target existence remains a source-graph
    /// selection responsibility.
    pub fn new(
        input: ResolvedProgramInput,
        options: CheckCommandOptions,
    ) -> Result<Self, CommandPlanError> {
        if input.single_file().is_some() && options.executable.is_some() {
            return Err(CommandPlanError::ExecutableWithSingleFile);
        }
        Ok(Self {
            input,
            executable: options.executable,
        })
    }

    #[must_use]
    pub const fn input(&self) -> &ResolvedProgramInput {
        &self.input
    }

    #[must_use]
    pub fn executable(&self) -> Option<&str> {
        self.executable.as_deref()
    }

    pub(crate) fn into_parts(self) -> (ResolvedProgramInput, Option<Box<str>>) {
        (self.input, self.executable)
    }
}

/// Raw executable/output choices accepted by the build argument parser.
#[derive(Debug, Default)]
pub struct BuildCommandOptions {
    executable: Option<Box<str>>,
    output: Option<PathBuf>,
}

impl BuildCommandOptions {
    #[must_use]
    pub fn new(executable: Option<Box<str>>, output: Option<PathBuf>) -> Self {
        Self { executable, output }
    }

    #[must_use]
    pub fn executable(name: impl Into<Box<str>>) -> Self {
        Self::new(Some(name.into()), None)
    }

    #[must_use]
    pub fn output(path: impl Into<PathBuf>) -> Self {
        Self::new(None, Some(path.into()))
    }
}

/// Closed build selection after package/file input normalization.
#[derive(Debug)]
pub struct BuildCommandPlan {
    input: ResolvedProgramInput,
    operation: BuildOperation,
}

impl BuildCommandPlan {
    /// Applies build selection and output rules without inspecting semantic declarations.
    ///
    /// # Errors
    ///
    /// Rejects `--executable` in single-file mode. Target cardinality remains a session decision.
    pub fn new(
        input: ResolvedProgramInput,
        options: BuildCommandOptions,
    ) -> Result<Self, CommandPlanError> {
        let BuildCommandOptions { executable, output } = options;
        let operation = match &input {
            ResolvedProgramInput::Package(package) => match (executable, output) {
                (None, None) => BuildOperation::PackageSet {
                    output_directory: package.root().to_path_buf(),
                },
                (Some(name), None) => BuildOperation::Selected {
                    selector: ExecutableSelector::Named(name),
                    output: SelectedBuildOutput::TargetNameIn(package.root().to_path_buf()),
                },
                (Some(name), Some(output)) => BuildOperation::Selected {
                    selector: ExecutableSelector::Named(name),
                    output: SelectedBuildOutput::Exact(resolve_output(
                        package.invocation_directory(),
                        &output,
                    )),
                },
                (None, Some(output)) => BuildOperation::Selected {
                    selector: ExecutableSelector::Only,
                    output: SelectedBuildOutput::Exact(resolve_output(
                        package.invocation_directory(),
                        &output,
                    )),
                },
            },
            ResolvedProgramInput::SingleFile(source) => {
                if executable.is_some() {
                    return Err(CommandPlanError::ExecutableWithSingleFile);
                }
                let output = match output {
                    Some(output) => resolve_output(source.invocation_directory(), &output),
                    None => source.invocation_directory().join(
                        source
                            .source()
                            .file_stem()
                            .ok_or(CommandPlanError::InvalidSingleFileIdentity)?,
                    ),
                };
                BuildOperation::Selected {
                    selector: ExecutableSelector::Only,
                    output: SelectedBuildOutput::Exact(output),
                }
            }
        };
        Ok(Self { input, operation })
    }

    #[must_use]
    pub const fn input(&self) -> &ResolvedProgramInput {
        &self.input
    }

    #[must_use]
    pub const fn operation(&self) -> &BuildOperation {
        &self.operation
    }

    pub(crate) fn into_parts(self) -> (ResolvedProgramInput, BuildOperation) {
        (self.input, self.operation)
    }
}

#[derive(Debug)]
pub enum BuildOperation {
    PackageSet {
        output_directory: PathBuf,
    },
    Selected {
        selector: ExecutableSelector,
        output: SelectedBuildOutput,
    },
}

#[derive(Debug)]
pub enum SelectedBuildOutput {
    Exact(PathBuf),
    TargetNameIn(PathBuf),
}

/// Raw package test and case choices accepted by the test argument parser.
#[derive(Debug, Default)]
pub struct TestCommandOptions {
    test: Option<Box<str>>,
    case: Option<Box<str>>,
}

impl TestCommandOptions {
    #[must_use]
    pub fn new(test: Option<Box<str>>, case: Option<Box<str>>) -> Self {
        Self { test, case }
    }
}

/// Closed package-only test selection and process working-directory policy.
#[derive(Debug)]
pub struct TestCommandPlan {
    input: PackageCommandInput,
    selector: TestTargetSelector,
    case: Option<Box<str>>,
    working_directory: PathBuf,
}

impl TestCommandPlan {
    /// Closes test selection without inspecting source or semantic declarations.
    ///
    /// # Errors
    ///
    /// Rejects an exact case without the distinct test-target identity required to scope it.
    pub fn new(
        input: PackageCommandInput,
        options: TestCommandOptions,
    ) -> Result<Self, CommandPlanError> {
        let TestCommandOptions { test, case } = options;
        if case.is_some() && test.is_none() {
            return Err(CommandPlanError::CaseWithoutTest);
        }
        let selector = match test {
            Some(name) => TestTargetSelector::Named(name),
            None => TestTargetSelector::All,
        };
        let working_directory = input.root().to_path_buf();
        Ok(Self {
            input,
            selector,
            case,
            working_directory,
        })
    }

    #[must_use]
    pub const fn input(&self) -> &PackageCommandInput {
        &self.input
    }

    #[must_use]
    pub const fn selector(&self) -> &TestTargetSelector {
        &self.selector
    }

    #[must_use]
    pub fn case(&self) -> Option<&str> {
        self.case.as_deref()
    }

    #[must_use]
    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PackageCommandInput,
        TestTargetSelector,
        Option<Box<str>>,
        PathBuf,
    ) {
        (self.input, self.selector, self.case, self.working_directory)
    }
}

/// Raw executable choice accepted by the run argument parser.
#[derive(Debug, Default)]
pub struct RunCommandOptions {
    executable: Option<Box<str>>,
}

impl RunCommandOptions {
    #[must_use]
    pub fn new(executable: Option<Box<str>>) -> Self {
        Self { executable }
    }

    #[must_use]
    pub fn executable(name: impl Into<Box<str>>) -> Self {
        Self::new(Some(name.into()))
    }
}

/// Closed run selection after package/file input normalization.
#[derive(Debug)]
pub struct RunCommandPlan {
    input: ResolvedProgramInput,
    selector: ExecutableSelector,
    working_directory: PathBuf,
    program_arguments: RunProgramArguments,
}

impl RunCommandPlan {
    /// Applies the sole/named executable policy without inspecting semantic declarations.
    ///
    /// # Errors
    ///
    /// Rejects `--executable` in single-file mode. Sole-target validation remains in the session.
    pub fn new(
        input: ResolvedProgramInput,
        options: RunCommandOptions,
        program_arguments: RunProgramArguments,
    ) -> Result<Self, CommandPlanError> {
        let (selector, working_directory) = match (&input, options.executable) {
            (ResolvedProgramInput::Package(package), Some(name)) => (
                ExecutableSelector::Named(name),
                package.root().to_path_buf(),
            ),
            (ResolvedProgramInput::Package(package), None) => {
                (ExecutableSelector::Only, package.root().to_path_buf())
            }
            (ResolvedProgramInput::SingleFile(_), Some(_)) => {
                return Err(CommandPlanError::ExecutableWithSingleFile);
            }
            (ResolvedProgramInput::SingleFile(source), None) => (
                ExecutableSelector::Only,
                source.invocation_directory().to_path_buf(),
            ),
        };
        Ok(Self {
            input,
            selector,
            working_directory,
            program_arguments,
        })
    }

    #[must_use]
    pub const fn input(&self) -> &ResolvedProgramInput {
        &self.input
    }

    #[must_use]
    pub const fn selector(&self) -> &ExecutableSelector {
        &self.selector
    }

    #[must_use]
    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }

    #[cfg(test)]
    pub(crate) const fn program_arguments(&self) -> &RunProgramArguments {
        &self.program_arguments
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ResolvedProgramInput,
        ExecutableSelector,
        PathBuf,
        RunProgramArguments,
    ) {
        (
            self.input,
            self.selector,
            self.working_directory,
            self.program_arguments,
        )
    }
}

fn resolve_output(invocation_directory: &Path, output: &Path) -> PathBuf {
    if output.is_absolute() {
        output.to_path_buf()
    } else {
        invocation_directory.join(output)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandPlanError {
    ExecutableWithSingleFile,
    InvalidSingleFileIdentity,
    CaseWithoutTest,
}

impl fmt::Display for CommandPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExecutableWithSingleFile => {
                formatter.write_str("--executable cannot be used in single-file mode")
            }
            Self::InvalidSingleFileIdentity => {
                formatter.write_str("resolved single-file input has no output stem")
            }
            Self::CaseWithoutTest => formatter.write_str("--case requires --test"),
        }
    }
}

impl std::error::Error for CommandPlanError {}
