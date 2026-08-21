use std::fmt;
use std::path::{Path, PathBuf};

use nocter_session::ExecutableSelector;

use crate::ResolvedProgramInput;

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

    pub(crate) fn into_parts(self) -> (ResolvedProgramInput, ExecutableSelector, PathBuf) {
        (self.input, self.selector, self.working_directory)
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
        }
    }
}

impl std::error::Error for CommandPlanError {}
