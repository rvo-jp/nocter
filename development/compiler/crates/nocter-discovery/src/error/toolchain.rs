use std::fmt;

use nocter_compile_input::ModuleIdentity;
use nocter_declarations::{BuiltinAttachment, StandardDeclarationRole};
use nocter_runtime_contract::PrimitiveRole;
use nocter_syntax::NodeKind;

/// A toolchain-profile failure selected before semantic lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolchainDiscoveryError {
    ModuleOutsideStandardPackage(ModuleIdentity),
    DuplicateBuiltinAttachment(BuiltinAttachment),
    DuplicateStandardRole(StandardDeclarationRole),
    DuplicatePrimitiveRole(PrimitiveRole),
    MissingRoleDeclaration {
        role: StandardDeclarationRole,
        module: ModuleIdentity,
        kind: NodeKind,
        name: Box<str>,
    },
    AmbiguousRoleDeclaration {
        role: StandardDeclarationRole,
        module: ModuleIdentity,
        kind: NodeKind,
        name: Box<str>,
    },
    MissingPrimitiveDeclaration {
        role: PrimitiveRole,
        module: ModuleIdentity,
        name: Box<str>,
    },
    AmbiguousPrimitiveDeclaration {
        role: PrimitiveRole,
        module: ModuleIdentity,
        name: Box<str>,
    },
}

impl fmt::Display for ToolchainDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModuleOutsideStandardPackage(module) => write!(
                formatter,
                "toolchain module {module:?} is outside the selected standard package"
            ),
            Self::DuplicateBuiltinAttachment(attachment) => {
                write!(
                    formatter,
                    "toolchain repeats {attachment:?} built-in attachment"
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
            Self::MissingRoleDeclaration {
                role,
                module,
                kind,
                name,
            } => write!(
                formatter,
                "toolchain role {role:?} cannot find {kind:?} {name:?} in {module:?}"
            ),
            Self::AmbiguousRoleDeclaration {
                role,
                module,
                kind,
                name,
            } => write!(
                formatter,
                "toolchain role {role:?} matches multiple {kind:?} declarations named {name:?} in {module:?}"
            ),
            Self::MissingPrimitiveDeclaration { role, module, name } => write!(
                formatter,
                "toolchain primitive {role:?} cannot find primitive {name:?} in {module:?}"
            ),
            Self::AmbiguousPrimitiveDeclaration { role, module, name } => write!(
                formatter,
                "toolchain primitive {role:?} matches multiple primitive declarations named {name:?} in {module:?}"
            ),
        }
    }
}

impl std::error::Error for ToolchainDiscoveryError {}
