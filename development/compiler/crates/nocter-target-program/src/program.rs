use std::collections::BTreeSet;
use std::fmt;

use nocter_checking::CheckedProgram;
use nocter_model::{CompilationTarget, PackageId, PackageTargetId};

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
    /// Validates and freezes one checked program against one immutable toolchain snapshot.
    ///
    /// # Errors
    ///
    /// Returns a typed boundary failure when target capability, standard-package authority,
    /// primitive contracts, or package-target identities are incomplete.
    pub fn build(
        checked: CheckedProgram,
        toolchain: ToolchainSnapshot,
    ) -> Result<Self, TargetProgramError> {
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
        validate_package_targets(&checked)?;
        validate_primitive_registry(graph, checked.types(), &toolchain)
            .map_err(TargetProgramError::PrimitiveRegistry)?;
        Ok(Self { checked, toolchain })
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

fn validate_package_targets(checked: &CheckedProgram) -> Result<(), TargetProgramError> {
    let graph = checked.graph();
    let mut names = BTreeSet::new();
    let mut orders = BTreeSet::new();
    for (id, target) in graph.package_targets().iter() {
        let valid = graph.packages().get(target.package()).is_some()
            && graph
                .modules()
                .get(target.module())
                .is_some_and(|module| module.package() == target.package())
            && graph.symbols().spelling(target.name()).is_some()
            && names.insert((target.package(), target.kind(), target.name()))
            && orders.insert((target.package(), target.declaration_order()));
        if !valid {
            return Err(TargetProgramError::InvalidPackageTarget(id));
        }
    }
    Ok(())
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
    InvalidPackageTarget(PackageTargetId),
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
            Self::InvalidPackageTarget(_) => {
                formatter.write_str("checked program contains an invalid package target")
            }
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
            | Self::StandardPackageMismatch { .. }
            | Self::InvalidPackageTarget(_) => None,
        }
    }
}

#[cfg(test)]
mod tests;
