use std::fmt;

use nocter_compile_input::ToolchainInput;
use nocter_declarations::ProgramBuildError;
use nocter_model::{BuiltinType, PackageIdentity};
use nocter_runtime_contract::PrimitiveRole;
use nocter_syntax::NodeId;
use nocter_toolchain_contract::{StandardDeclarationRole, StructuralAttachment};

use crate::{
    DeclarationSurface, ImportViolation, ModuleIdentity, SurfaceDeclaration, SurfaceDeclarationId,
    SurfaceDeclarationKind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolchainError {
    MissingProfile,
    Rule(ImportViolation),
    UnknownStandardPackage(PackageIdentity),
    UnknownModule(ModuleIdentity),
    PreludeOutsideStandardPackage,
    StructuralAttachmentOutsideStandardPackage(StructuralAttachment),
    DeclarationModuleOutsideStandardPackage(ModuleIdentity),
    DuplicateStructuralAttachment(StructuralAttachment),
    MissingBuiltinType(BuiltinType),
    DuplicateBuiltinType(BuiltinType),
    MissingStandardDeclaration(StandardDeclarationRole),
    DuplicateStandardDeclaration(StandardDeclarationRole),
    InvalidStandardDeclaration(StandardDeclarationRole),
    MissingPrimitiveDeclaration(PrimitiveRole),
    DuplicatePrimitiveDeclaration(PrimitiveRole),
    InconsistentImport(NodeId),
    Program(ProgramBuildError),
}

impl fmt::Display for ToolchainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingProfile => formatter.write_str("compile input has no toolchain profile"),
            Self::Rule(violation) => write!(
                formatter,
                "{}: {}",
                violation.rule().code(),
                violation.rule().message()
            ),
            Self::UnknownModule(module) => {
                write!(formatter, "toolchain module {module:?} is absent")
            }
            Self::UnknownStandardPackage(package) => {
                write!(formatter, "standard package {package:?} is absent")
            }
            Self::PreludeOutsideStandardPackage => {
                formatter.write_str("toolchain prelude is outside the selected standard package")
            }
            Self::StructuralAttachmentOutsideStandardPackage(attachment) => write!(
                formatter,
                "{attachment:?} structural attachment is outside the selected standard package"
            ),
            Self::DeclarationModuleOutsideStandardPackage(module) => write!(
                formatter,
                "toolchain declaration module {module:?} is outside the selected standard package"
            ),
            Self::DuplicateStructuralAttachment(attachment) => write!(
                formatter,
                "toolchain profile repeats {attachment:?} attachment"
            ),
            Self::MissingBuiltinType(builtin) => write!(
                formatter,
                "toolchain built-in type {builtin:?} lost its declaration"
            ),
            Self::DuplicateBuiltinType(builtin) => {
                write!(formatter, "toolchain repeats {builtin:?} built-in type")
            }
            Self::MissingStandardDeclaration(role) => write!(
                formatter,
                "toolchain standard role {role:?} lost its declaration"
            ),
            Self::DuplicateStandardDeclaration(role) => {
                write!(formatter, "toolchain repeats standard role {role:?}")
            }
            Self::InvalidStandardDeclaration(role) => write!(
                formatter,
                "toolchain standard role {role:?} selects an invalid declaration"
            ),
            Self::MissingPrimitiveDeclaration(role) => {
                write!(
                    formatter,
                    "toolchain primitive role {role:?} has no declaration"
                )
            }
            Self::DuplicatePrimitiveDeclaration(role) => write!(
                formatter,
                "toolchain primitive role {role:?} has multiple declarations"
            ),
            Self::InconsistentImport(import) => {
                write!(formatter, "authored import {import:?} has no retained path")
            }
            Self::Program(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ToolchainError {}

impl From<ImportViolation> for ToolchainError {
    fn from(violation: ImportViolation) -> Self {
        Self::Rule(violation)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ResolvedBuiltinType {
    builtin: BuiltinType,
    declaration: SurfaceDeclarationId,
}

impl ResolvedBuiltinType {
    pub(crate) const fn builtin(self) -> BuiltinType {
        self.builtin
    }

    pub(crate) const fn declaration(self) -> SurfaceDeclarationId {
        self.declaration
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ResolvedStandardRole {
    role: StandardDeclarationRole,
    declaration: SurfaceDeclarationId,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ResolvedPrimitiveRole {
    role: PrimitiveRole,
    declaration: SurfaceDeclarationId,
}

impl ResolvedPrimitiveRole {
    pub(crate) const fn role(self) -> PrimitiveRole {
        self.role
    }

    pub(crate) const fn declaration(self) -> SurfaceDeclarationId {
        self.declaration
    }
}

impl ResolvedStandardRole {
    pub(crate) const fn role(self) -> StandardDeclarationRole {
        self.role
    }

    pub(crate) const fn declaration(self) -> SurfaceDeclarationId {
        self.declaration
    }
}

/// Toolchain declarations selected once from the target-filtered declaration surface.
#[derive(Debug)]
pub(crate) struct ResolvedToolchainInput {
    standard_package: PackageIdentity,
    prelude: ModuleIdentity,
    structural_attachments: Box<[(StructuralAttachment, ModuleIdentity)]>,
    standard_roles: Box<[ResolvedStandardRole]>,
    primitive_roles: Box<[ResolvedPrimitiveRole]>,
    builtin_types: Box<[ResolvedBuiltinType]>,
}

impl ResolvedToolchainInput {
    pub(crate) const fn standard_package(&self) -> &PackageIdentity {
        &self.standard_package
    }

    pub(crate) const fn prelude(&self) -> &ModuleIdentity {
        &self.prelude
    }

    pub(crate) fn structural_attachments(&self) -> &[(StructuralAttachment, ModuleIdentity)] {
        &self.structural_attachments
    }

    pub(crate) fn standard_roles(&self) -> &[ResolvedStandardRole] {
        &self.standard_roles
    }

    pub(crate) fn primitive_roles(&self) -> &[ResolvedPrimitiveRole] {
        &self.primitive_roles
    }

    pub(crate) fn builtin_types(&self) -> &[ResolvedBuiltinType] {
        &self.builtin_types
    }
}

pub(crate) fn resolve_toolchain_surface(
    surface: &DeclarationSurface<'_>,
    input: &ToolchainInput,
) -> Result<ResolvedToolchainInput, ToolchainError> {
    for module in input
        .standard_roles()
        .iter()
        .map(nocter_compile_input::StandardRoleLocator::module)
        .chain(
            input
                .primitive_roles()
                .iter()
                .map(nocter_compile_input::PrimitiveRoleLocator::module),
        )
        .chain(
            input
                .builtin_types()
                .iter()
                .map(nocter_compile_input::BuiltinTypeLocator::module),
        )
    {
        if module.package() != input.standard_package() {
            return Err(ToolchainError::DeclarationModuleOutsideStandardPackage(
                module.clone(),
            ));
        }
    }
    let standard_roles = input
        .standard_roles()
        .iter()
        .map(|locator| {
            let matches =
                matching_declarations(surface, locator.module(), locator.name(), |item| {
                    item.visibility().is_some()
                        && declaration_node_kind(surface, item) == Some(locator.kind())
                });
            match matches.as_slice() {
                [declaration] => Ok(ResolvedStandardRole {
                    role: locator.role(),
                    declaration: *declaration,
                }),
                [] => Err(ToolchainError::MissingStandardDeclaration(locator.role())),
                _ => Err(ToolchainError::DuplicateStandardDeclaration(locator.role())),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let builtin_types = input
        .builtin_types()
        .iter()
        .map(|locator| {
            let matches =
                matching_declarations(surface, locator.module(), locator.name(), |item| {
                    item.visibility().is_some()
                        && item.kind() == SurfaceDeclarationKind::PrimitiveType
                });
            match matches.as_slice() {
                [declaration] => Ok(ResolvedBuiltinType {
                    builtin: locator.builtin(),
                    declaration: *declaration,
                }),
                [] => Err(ToolchainError::MissingBuiltinType(locator.builtin())),
                _ => Err(ToolchainError::DuplicateBuiltinType(locator.builtin())),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let primitive_roles = input
        .primitive_roles()
        .iter()
        .map(|locator| {
            let matches =
                matching_declarations(surface, locator.module(), locator.name(), |item| {
                    item.kind() == SurfaceDeclarationKind::PrimitiveFunction
                });
            match matches.as_slice() {
                [declaration] => Ok(ResolvedPrimitiveRole {
                    role: locator.role(),
                    declaration: *declaration,
                }),
                [] => Err(ToolchainError::MissingPrimitiveDeclaration(locator.role())),
                _ => Err(ToolchainError::DuplicatePrimitiveDeclaration(
                    locator.role(),
                )),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ResolvedToolchainInput {
        standard_package: input.standard_package().clone(),
        prelude: input.prelude().clone(),
        structural_attachments: input
            .structural_attachments()
            .iter()
            .map(|attachment| (attachment.attachment(), attachment.module().clone()))
            .collect(),
        standard_roles: standard_roles.into_boxed_slice(),
        primitive_roles: primitive_roles.into_boxed_slice(),
        builtin_types: builtin_types.into_boxed_slice(),
    })
}

fn matching_declarations(
    surface: &DeclarationSurface<'_>,
    module: &ModuleIdentity,
    name: &str,
    shape: impl Fn(SurfaceDeclaration) -> bool,
) -> Vec<SurfaceDeclarationId> {
    surface
        .declarations()
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, declaration)| shape(*declaration))
        .filter(|(_, declaration)| {
            surface
                .sources()
                .get(declaration.source().index())
                .is_some_and(|source| source.module() == module)
        })
        .filter(|(_, declaration)| {
            declaration.name().is_some_and(|token| {
                surface
                    .source_map()
                    .get(token.source())
                    .and_then(|source| source.text_at(token.range()))
                    == Some(name)
            })
        })
        .map(|(index, _)| SurfaceDeclarationId::from_index(index))
        .collect()
}

fn declaration_node_kind(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Option<nocter_syntax::NodeKind> {
    surface
        .sources()
        .get(declaration.source().index())?
        .syntax()
        .node(declaration.node())
        .map(nocter_syntax::SyntaxNode::kind)
}

#[cfg(test)]
mod tests {
    use nocter_compile_input::{
        BuiltinTypeLocator, CompileUnitInput, ModuleInput, ModuleSourceInput, ModuleSourceKind,
        PackageInput, PackageMode, ToolchainInput,
    };
    use nocter_model::{BuiltinType, CompilationTarget, PackageIdentity};
    use nocter_source::{SourceMap, SourceName};
    use nocter_syntax::{ParseGoal, parse};

    use super::resolve_toolchain_surface;
    use crate::{ModuleIdentity, collect_declaration_surface};

    #[test]
    fn target_filtered_surface_is_the_sole_toolchain_declaration_authority() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("/std/index.nct"),
                b"#target: \"arm64-darwin\"\n\
                  pub primitive type i32\n\
                  #target: \"x64-linux\"\n\
                  pub primitive type i32\n",
            )
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        assert!(!tree.has_errors(), "{:#?}", tree.diagnostics());

        let package = PackageIdentity::new("toolchain:std");
        let root = ModuleIdentity::new(package.clone(), Vec::<&str>::new());
        let toolchain = ToolchainInput::new(package.clone(), root.clone(), Vec::new(), Vec::new())
            .with_builtin_types(vec![BuiltinTypeLocator::new(
                BuiltinType::I32,
                root.clone(),
                "i32",
            )]);
        let input = CompileUnitInput::new(
            CompilationTarget::Arm64Darwin,
            &sources,
            vec![PackageInput::new(package, "std", PackageMode::Declared)],
            vec![ModuleInput::new(
                root,
                vec![ModuleSourceInput::new(
                    "/std/index.nct",
                    ModuleSourceKind::Root,
                    &tree,
                )],
            )],
            Vec::new(),
        )
        .with_toolchain(toolchain.clone());

        let surface = collect_declaration_surface(&input).unwrap();
        assert_eq!(surface.declarations().len(), 1);
        let resolved = resolve_toolchain_surface(&surface, &toolchain).unwrap();
        assert_eq!(resolved.builtin_types().len(), 1);
        assert_eq!(resolved.builtin_types()[0].builtin(), BuiltinType::I32);
    }
}
