use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use nocter_session::{ExecutableIdentity, NativeImageEntry};

/// Filesystem-independent output assignment for one complete native image set.
#[derive(Debug)]
pub struct BuildOutputPlan {
    outputs: Box<[PlannedOutput]>,
}

impl BuildOutputPlan {
    /// Assigns each executable to its authored name directly below `package_root`.
    ///
    /// # Errors
    ///
    /// Rejects names that are not one ordinary filename and any two targets that resolve to the
    /// same output path. No filesystem state is inspected or changed.
    pub fn for_package(
        entries: &[NativeImageEntry],
        package_root: impl AsRef<Path>,
    ) -> Result<Self, OutputPlanError> {
        let package_root = package_root.as_ref();
        let mut by_path = BTreeMap::<PathBuf, ExecutableIdentity>::new();
        let mut outputs = Vec::with_capacity(entries.len());
        for entry in entries {
            let identity = entry.identity();
            let planned = Self::for_selected(identity, package_root)?;
            let path = planned.path.clone();
            if let Some(first) = by_path.insert(path.clone(), identity.clone()) {
                return Err(OutputPlanError::DuplicateOutput {
                    path,
                    first,
                    second: identity.clone(),
                });
            }
            outputs.push(planned);
        }
        Ok(Self {
            outputs: outputs.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn outputs(&self) -> &[PlannedOutput] {
        &self.outputs
    }

    /// Assigns one selected executable to its authored name below `output_directory`.
    ///
    /// # Errors
    ///
    /// Rejects an authored name that is not one ordinary filename.
    pub fn for_selected(
        identity: &ExecutableIdentity,
        output_directory: impl AsRef<Path>,
    ) -> Result<PlannedOutput, OutputPlanError> {
        if !is_filename(identity.name()) {
            return Err(OutputPlanError::InvalidExecutableName(identity.clone()));
        }
        Ok(PlannedOutput {
            identity: identity.clone(),
            path: output_directory.as_ref().join(identity.name()),
        })
    }
}

/// One exact target-to-path assignment.
#[derive(Debug)]
pub struct PlannedOutput {
    identity: ExecutableIdentity,
    path: PathBuf,
}

impl PlannedOutput {
    #[must_use]
    pub const fn identity(&self) -> &ExecutableIdentity {
        &self.identity
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn is_filename(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

#[derive(Debug)]
pub enum OutputPlanError {
    InvalidExecutableName(ExecutableIdentity),
    DuplicateOutput {
        path: PathBuf,
        first: ExecutableIdentity,
        second: ExecutableIdentity,
    },
}

impl fmt::Display for OutputPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidExecutableName(executable) => write!(
                formatter,
                "executable name {:?} in package {} is not one output filename",
                executable.name(),
                executable.package().as_str()
            ),
            Self::DuplicateOutput {
                path,
                first,
                second,
            } => write!(
                formatter,
                "executables {} ({}) and {} ({}) both select output {}",
                first.name(),
                first.package().as_str(),
                second.name(),
                second.package().as_str(),
                path.display()
            ),
        }
    }
}

impl std::error::Error for OutputPlanError {}
