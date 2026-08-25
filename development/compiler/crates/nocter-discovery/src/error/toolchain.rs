use std::fmt;

use nocter_compile_input::ModuleIdentity;
use nocter_declarations::{StandardDeclarationRole, StructuralAttachment};
use nocter_model::BuiltinType;
use nocter_runtime_contract::PrimitiveRole;
use nocter_syntax::NodeKind;

/// A toolchain-profile failure selected before semantic lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolchainDiscoveryError {
    ModuleOutsideStandardPackage(ModuleIdentity),
    DuplicateStructuralAttachment(StructuralAttachment),
    DuplicateStandardRole(StandardDeclarationRole),
    DuplicatePrimitiveRole(PrimitiveRole),
    DuplicateBuiltinType(BuiltinType),
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
    MissingBuiltinTypeDeclaration {
        builtin: BuiltinType,
        module: ModuleIdentity,
        name: Box<str>,
    },
    AmbiguousBuiltinTypeDeclaration {
        builtin: BuiltinType,
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
            Self::MissingBuiltinTypeDeclaration {
                builtin,
                module,
                name,
            } => write!(
                formatter,
                "toolchain built-in type {builtin:?} cannot find primitive type {name:?} in {module:?}"
            ),
            Self::AmbiguousBuiltinTypeDeclaration {
                builtin,
                module,
                name,
            } => write!(
                formatter,
                "toolchain built-in type {builtin:?} matches multiple primitive type declarations named {name:?} in {module:?}"
            ),
        }
    }
}

impl std::error::Error for ToolchainDiscoveryError {}
