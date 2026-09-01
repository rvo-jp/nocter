use std::fmt;

use nocter_checking::CheckedProgram;
use nocter_model::{CompilationTarget, PackageId};

use crate::primitive_contracts::validate_primitive_registry;
use crate::{PrimitiveRegistryValidationError, ToolchainSnapshot};

/// The complete selected-target success boundary shared by check, build, and run.
///
/// Construction consumes the checked program. A caller therefore cannot accidentally continue
/// from source checking while bypassing target capability and primitive validation.
#[derive(Debug)]
pub struct TargetProgram {
    checked: CheckedProgram,
    toolchain: ToolchainSnapshot,
}

impl TargetProgram {
    /// Validates and freezes a closed checked program.
    ///
    /// # Errors
    ///
    /// Returns the target-program error together with the still-valid checked semantic program.
    pub fn build_retaining_checked(
        checked: CheckedProgram,
        toolchain: ToolchainSnapshot,
    ) -> Result<Self, Box<TargetProgramFailure>> {
        if let Err(error) = validate_target_program(&checked, &toolchain) {
            return Err(Box::new(TargetProgramFailure { error, checked }));
        }
        Ok(Self { checked, toolchain })
    }

    /// Validates and freezes a checked program without retaining a rejected input.
    ///
    /// # Errors
    ///
    /// Returns the selected-target contract failure.
    pub fn build(
        checked: CheckedProgram,
        toolchain: ToolchainSnapshot,
    ) -> Result<Self, TargetProgramError> {
        Self::build_retaining_checked(checked, toolchain).map_err(|failure| failure.error)
    }

    #[must_use]
    pub const fn checked(&self) -> &CheckedProgram {
        &self.checked
    }

    #[must_use]
    pub const fn toolchain(&self) -> &ToolchainSnapshot {
        &self.toolchain
    }

    #[must_use]
    pub fn into_parts(self) -> (CheckedProgram, ToolchainSnapshot) {
        (self.checked, self.toolchain)
    }
}

fn validate_target_program(
    checked: &CheckedProgram,
    toolchain: &ToolchainSnapshot,
) -> Result<(), TargetProgramError> {
    let graph = checked.graph();
    if graph.target() != toolchain.target() {
        return Err(TargetProgramError::TargetMismatch {
            checked: graph.target(),
            toolchain: toolchain.target(),
        });
    }
    let Some(standard_package) = graph.standard_package() else {
        return Err(TargetProgramError::MissingStandardPackage);
    };
    if standard_package != toolchain.standard_package() {
        return Err(TargetProgramError::StandardPackageMismatch {
            checked: standard_package,
            toolchain: toolchain.standard_package(),
        });
    }
    validate_primitive_registry(graph, checked.types(), toolchain)
        .map_err(TargetProgramError::PrimitiveRegistry)?;
    Ok(())
}

/// A target-program rejection that preserves its completed checked semantic input.
#[derive(Debug)]
pub struct TargetProgramFailure {
    error: TargetProgramError,
    checked: CheckedProgram,
}

impl TargetProgramFailure {
    #[must_use]
    pub fn into_parts(self) -> (TargetProgramError, CheckedProgram) {
        (self.error, self.checked)
    }
}

/// Failure to cross the selected-target buildability boundary.
#[derive(Debug)]
pub enum TargetProgramError {
    TargetMismatch {
        checked: CompilationTarget,
        toolchain: CompilationTarget,
    },
    MissingStandardPackage,
    StandardPackageMismatch {
        checked: PackageId,
        toolchain: PackageId,
    },
    PrimitiveRegistry(PrimitiveRegistryValidationError),
}

impl fmt::Display for TargetProgramError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetMismatch { checked, toolchain } => write!(
                formatter,
                "checked target {checked} does not match toolchain target {toolchain}"
            ),
            Self::MissingStandardPackage => {
                formatter.write_str("checked program has no compiler-selected standard package")
            }
            Self::StandardPackageMismatch { .. } => formatter.write_str(
                "checked standard package does not match the toolchain standard package",
            ),
            Self::PrimitiveRegistry(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TargetProgramError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PrimitiveRegistry(error) => Some(error),
            Self::TargetMismatch { .. }
            | Self::MissingStandardPackage
            | Self::StandardPackageMismatch { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests;
