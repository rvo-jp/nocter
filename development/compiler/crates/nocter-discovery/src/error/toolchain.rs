use std::fmt;

use nocter_compile_input::ModuleIdentity;
use nocter_model::BuiltinType;
use nocter_runtime_contract::PrimitiveRole;
use nocter_toolchain_contract::{StandardDeclarationRole, StructuralAttachment};

/// A structural toolchain-profile failure detectable without interpreting declarations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolchainDiscoveryError {
    ModuleOutsideStandardPackage(ModuleIdentity),
    DuplicateStructuralAttachment(StructuralAttachment),
    DuplicateStandardRole(StandardDeclarationRole),
    DuplicatePrimitiveRole(PrimitiveRole),
    DuplicateBuiltinType(BuiltinType),
}

impl fmt::Display for ToolchainDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModuleOutsideStandardPackage(module) => write!(
                formatter,
                "toolchain module {module:?} is outside the selected standard package"
            ),
            Self::DuplicateStructuralAttachment(attachment) => {
                write!(
                    formatter,
                    "toolchain repeats {attachment:?} structural attachment"
                )
            }
            Self::DuplicateStandardRole(role) => {
                write!(
                    formatter,
                    "toolchain repeats {role:?} standard semantic role"
                )
            }
            Self::DuplicatePrimitiveRole(role) => {
                write!(formatter, "toolchain repeats {role:?} primitive role")
            }
            Self::DuplicateBuiltinType(builtin) => {
                write!(formatter, "toolchain repeats {builtin:?} built-in type")
            }
        }
    }
}

impl std::error::Error for ToolchainDiscoveryError {}
