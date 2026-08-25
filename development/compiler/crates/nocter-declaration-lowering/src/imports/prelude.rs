use std::fmt;

use super::access::{module_index_by_id, module_index_by_identity, visible_from};
use super::{ModuleNamespace, NamespaceBinding, PreparedImports, lookup};
use crate::{ImportViolation, ModuleIdentity, PackageIdentity, ToolchainInput};
use nocter_declarations::{
    ExportedEntity, FallbackEntry, ModuleNamespace as SemanticModuleNamespace, NamespaceEntry,
    ProgramBuildError, StandardDeclaration, StandardDeclarationRole, StructuralAttachment,
};
use nocter_model::{BuiltinType, ModuleId, PackageId, Symbol};
use nocter_syntax::{NodeId, SyntaxToken};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolchainError {
    MissingProfile,
    Rule(ImportViolation),
    UnknownStandardPackage(PackageIdentity),
    UnknownModule(ModuleIdentity),
    PreludeOutsideStandardPackage,
    StructuralAttachmentOutsideStandardPackage(StructuralAttachment),
    DuplicateStructuralAttachment(StructuralAttachment),
    MissingBuiltinType(BuiltinType),
    DuplicateBuiltinType(BuiltinType),
    MissingStandardDeclaration(StandardDeclarationRole),
    DuplicateStandardDeclaration(StandardDeclarationRole),
    InvalidStandardDeclaration(StandardDeclarationRole),
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
            Self::StructuralAttachmentOutsideStandardPackage(attachment) => {
                write!(
                    formatter,
                    "{attachment:?} structural attachment is outside the selected standard package"
                )
            }
            Self::DuplicateStructuralAttachment(attachment) => {
                write!(
                    formatter,
                    "toolchain profile repeats {attachment:?} attachment"
                )
            }
            Self::MissingBuiltinType(builtin) => {
                write!(
                    formatter,
                    "toolchain built-in type {builtin:?} lost its declaration"
                )
            }
            Self::DuplicateBuiltinType(builtin) => {
                write!(formatter, "toolchain repeats {builtin:?} built-in type")
            }
            Self::MissingStandardDeclaration(role) => {
                write!(
                    formatter,
                    "toolchain standard role {role:?} lost its declaration"
                )
            }
            Self::DuplicateStandardDeclaration(role) => {
                write!(formatter, "toolchain repeats standard role {role:?}")
            }
            Self::InvalidStandardDeclaration(role) => {
                write!(
                    formatter,
                    "toolchain standard role {role:?} selects an invalid declaration"
                )
            }
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

/// Authored namespaces plus compiler-selected universal and standard-prelude fallback entries.
#[derive(Debug)]
pub struct PreparedNamespaces<'syntax> {
    pub(crate) imports: PreparedImports<'syntax>,
    fallback: Box<[ModuleNamespace]>,
}

impl PreparedNamespaces<'_> {
    #[must_use]
    pub const fn imports(&self) -> &PreparedImports<'_> {
        &self.imports
    }

    #[must_use]
    pub fn lookup_local(
        &self,
        source: crate::SurfaceSourceId,
        name: Symbol,
    ) -> Option<ExportedEntity> {
        self.imports.lookup_local(source, name).or_else(|| {
            let module = self
                .imports
                .generics
                .headers
                .reserved
                .module_for_source(source)?;
            let index = module_index_by_id(&self.imports.generics.headers.reserved, module)?;
            lookup(&self.fallback[index], name).map(|binding| binding.entity)
        })
    }

    #[must_use]
    pub fn lookup_export(
        &self,
        from: ModuleId,
        module: ModuleId,
        name: Symbol,
    ) -> Option<ExportedEntity> {
        self.imports.lookup_export(from, module, name)
    }

    /// Freezes the already-selected authored and fallback namespace layers into the declaration
    /// program. Later semantic stages consume that authority instead of rebuilding lookup tables.
    pub(crate) fn define_program_namespaces(&mut self) -> Result<(), ProgramBuildError> {
        for (index, namespace) in self.imports.source_namespaces.iter().enumerate() {
            let source = self
                .imports
                .generics
                .headers
                .reserved
                .sources
                .get(index)
                .ok_or(ProgramBuildError::UnknownModule)?;
            let module = self
                .imports
                .generics
                .headers
                .reserved
                .module_for_source(crate::SurfaceSourceId::from_index(index))
                .ok_or(ProgramBuildError::UnknownModule)?;
            let module_index = module_index_by_id(&self.imports.generics.headers.reserved, module)
                .ok_or(ProgramBuildError::UnknownModule)?;
            let authored = namespace
                .iter()
                .map(|(name, binding)| (*name, binding.entity))
                .collect::<Vec<_>>();
            let fallback = self.fallback[module_index]
                .iter()
                .filter(|(name, _)| {
                    namespace
                        .binary_search_by_key(name, |(candidate, _)| *candidate)
                        .is_err()
                })
                .map(|(name, binding)| (*name, binding.entity));
            let source_id = source.syntax().source();
            let directly_visible = self
                .imports
                .generics
                .headers
                .reserved
                .source_visibilities
                .iter()
                .copied()
                .filter(|see| see.source().index() == index)
                .filter_map(|see| {
                    self.imports
                        .generics
                        .headers
                        .reserved
                        .sources
                        .get(see.target().index())
                        .map(|target| target.syntax().source())
                });
            self.imports
                .generics
                .headers
                .reserved
                .source_index
                .define_source_namespace(source_id, authored, fallback);
            self.imports
                .generics
                .headers
                .reserved
                .source_index
                .define_source_access(source_id, directly_visible);
        }
        let namespaces = self
            .imports
            .namespaces
            .iter()
            .zip(self.fallback.iter())
            .map(|(authored, fallback)| {
                SemanticModuleNamespace::new(
                    authored.iter().map(|(name, binding)| {
                        NamespaceEntry::new(*name, binding.entity, binding.visibility)
                    }),
                    fallback
                        .iter()
                        .map(|(name, binding)| FallbackEntry::new(*name, binding.entity)),
                )
                .map_err(|error| ProgramBuildError::DuplicateModuleNamespaceName(error.name()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let modules = self.imports.generics.headers.reserved.module_ids.clone();
        if modules.len() != namespaces.len() {
            return Err(ProgramBuildError::UnknownModule);
        }
        for (module, namespace) in modules.into_iter().zip(namespaces) {
            self.imports
                .generics
                .headers
                .reserved
                .program
                .define_module_namespace(module, namespace)?;
        }
        Ok(())
    }
}

/// Applies the exact compiler-selected standard package, built-in surfaces, and prelude fallback.
///
/// Standard ownership comes from `toolchain.standard_package`; it is never inferred from the
/// prelude path. Standard modules receive no fallback. Authored imports of the prelude remain
/// invalid.
///
/// # Errors
///
/// Returns [`ToolchainError`] when an exact selected identity is absent, inconsistent, duplicated,
/// or explicitly imported where the compiler owns the edge.
pub fn apply_toolchain_profile<'syntax>(
    mut imports: PreparedImports<'syntax>,
    toolchain: &ToolchainInput,
) -> Result<PreparedNamespaces<'syntax>, ToolchainError> {
    let resolved = resolve_toolchain_profile(&imports, toolchain)?;
    install_toolchain_profile(&mut imports, &resolved)?;
    let fallback = compose_fallback(&imports, toolchain, &resolved);
    Ok(PreparedNamespaces { imports, fallback })
}

struct ResolvedBuiltinType {
    builtin: BuiltinType,
    module: ModuleId,
    symbol: Symbol,
    declaration: SyntaxToken,
}

struct ResolvedToolchainProfile {
    prelude_index: usize,
    prelude: ModuleId,
    standard_package: PackageId,
    structural_attachments: Vec<(StructuralAttachment, ModuleId)>,
    builtin_types: Vec<ResolvedBuiltinType>,
    standard_declarations: Vec<(StandardDeclarationRole, StandardDeclaration)>,
}

fn resolve_toolchain_profile(
    imports: &PreparedImports<'_>,
    toolchain: &ToolchainInput,
) -> Result<ResolvedToolchainProfile, ToolchainError> {
    let reserved = &imports.generics.headers.reserved;
    if toolchain.prelude().package() != toolchain.standard_package() {
        return Err(ToolchainError::PreludeOutsideStandardPackage);
    }
    let prelude_index = module_index_by_identity(reserved, toolchain.prelude())
        .ok_or_else(|| ToolchainError::UnknownModule(toolchain.prelude().clone()))?;
    for (index, import) in reserved.imports.iter().enumerate() {
        if import.target() == toolchain.prelude() {
            let path = imports
                .import_path(index)
                .ok_or(ToolchainError::InconsistentImport(import.node()))?;
            return Err(ImportViolation::compiler_managed_prelude_import(path).into());
        }
    }
    let package_index = reserved
        .packages
        .iter()
        .position(|package| package.identity() == toolchain.standard_package())
        .ok_or_else(|| {
            ToolchainError::UnknownStandardPackage(toolchain.standard_package().clone())
        })?;
    Ok(ResolvedToolchainProfile {
        prelude_index,
        prelude: reserved.module_ids[prelude_index],
        standard_package: reserved.package_ids[package_index],
        structural_attachments: resolve_structural_attachments(imports, toolchain)?,
        builtin_types: resolve_builtin_types(imports, toolchain)?,
        standard_declarations: resolve_standard_declarations(imports, toolchain)?,
    })
}

fn resolve_standard_declarations(
    imports: &PreparedImports<'_>,
    toolchain: &ToolchainInput,
) -> Result<Vec<(StandardDeclarationRole, StandardDeclaration)>, ToolchainError> {
    let reserved = &imports.generics.headers.reserved;
    let mut resolved = Vec::with_capacity(toolchain.standard_roles().len());
    for input in toolchain.standard_roles() {
        if resolved.iter().any(|(role, _)| *role == input.role()) {
            return Err(ToolchainError::DuplicateStandardDeclaration(input.role()));
        }
        let (index, _) = reserved
            .declarations
            .iter()
            .enumerate()
            .find(|(_, declaration)| declaration.name() == Some(input.declaration()))
            .ok_or(ToolchainError::MissingStandardDeclaration(input.role()))?;
        let declaration = match reserved.entities[index] {
            Some(crate::ReservedEntity::BuiltinType(id)) => StandardDeclaration::BuiltinType(id),
            Some(crate::ReservedEntity::NominalType(id)) => StandardDeclaration::NominalType(id),
            Some(crate::ReservedEntity::Interface(id)) => StandardDeclaration::Interface(id),
            Some(crate::ReservedEntity::AssociatedType(id)) => {
                StandardDeclaration::AssociatedType(id)
            }
            Some(crate::ReservedEntity::Callable(id)) => StandardDeclaration::Callable(id),
            Some(_) | None => {
                return Err(ToolchainError::InvalidStandardDeclaration(input.role()));
            }
        };
        resolved.push((input.role(), declaration));
    }
    Ok(resolved)
}

fn resolve_structural_attachments(
    imports: &PreparedImports<'_>,
    toolchain: &ToolchainInput,
) -> Result<Vec<(StructuralAttachment, ModuleId)>, ToolchainError> {
    let reserved = &imports.generics.headers.reserved;
    let mut resolved = Vec::with_capacity(toolchain.structural_attachments().len());
    for input in toolchain.structural_attachments() {
        if input.module().package() != toolchain.standard_package() {
            return Err(ToolchainError::StructuralAttachmentOutsideStandardPackage(
                input.attachment(),
            ));
        }
        if resolved
            .iter()
            .any(|(attachment, _)| *attachment == input.attachment())
        {
            return Err(ToolchainError::DuplicateStructuralAttachment(
                input.attachment(),
            ));
        }
        let index = module_index_by_identity(reserved, input.module())
            .ok_or_else(|| ToolchainError::UnknownModule(input.module().clone()))?;
        resolved.push((input.attachment(), reserved.module_ids[index]));
    }
    Ok(resolved)
}

fn resolve_builtin_types(
    imports: &PreparedImports<'_>,
    toolchain: &ToolchainInput,
) -> Result<Vec<ResolvedBuiltinType>, ToolchainError> {
    let reserved = &imports.generics.headers.reserved;
    let mut resolved = Vec::<ResolvedBuiltinType>::with_capacity(toolchain.builtin_types().len());
    for input in toolchain.builtin_types() {
        if resolved
            .iter()
            .any(|candidate| candidate.builtin == input.builtin())
        {
            return Err(ToolchainError::DuplicateBuiltinType(input.builtin()));
        }
        let (index, declaration) = reserved
            .declarations
            .iter()
            .copied()
            .enumerate()
            .find(|(_, declaration)| declaration.name() == Some(input.declaration()))
            .ok_or(ToolchainError::MissingBuiltinType(input.builtin()))?;
        if reserved.entities[index] != Some(crate::ReservedEntity::BuiltinType(input.builtin())) {
            return Err(ToolchainError::MissingBuiltinType(input.builtin()));
        }
        resolved.push(ResolvedBuiltinType {
            builtin: input.builtin(),
            module: reserved
                .module_for_source(declaration.source())
                .ok_or(ToolchainError::MissingBuiltinType(input.builtin()))?,
            symbol: reserved
                .symbols()
                .get(input.builtin().spelling())
                .ok_or(ToolchainError::MissingBuiltinType(input.builtin()))?,
            declaration: input.declaration(),
        });
    }
    Ok(resolved)
}

fn install_toolchain_profile(
    imports: &mut PreparedImports<'_>,
    resolved: &ResolvedToolchainProfile,
) -> Result<(), ToolchainError> {
    imports
        .generics
        .headers
        .reserved
        .program
        .set_standard_package(resolved.standard_package)
        .map_err(ToolchainError::Program)?;
    for &(attachment, module) in &resolved.structural_attachments {
        imports
            .generics
            .headers
            .reserved
            .program
            .set_structural_attachment_module(attachment, module)
            .map_err(ToolchainError::Program)?;
    }
    for builtin in &resolved.builtin_types {
        imports
            .generics
            .headers
            .reserved
            .program
            .set_builtin_type_module(builtin.builtin, builtin.module)
            .map_err(ToolchainError::Program)?;
    }
    for &(role, declaration) in &resolved.standard_declarations {
        imports
            .generics
            .headers
            .reserved
            .program
            .set_standard_declaration(role, declaration)
            .map_err(ToolchainError::Program)?;
    }
    Ok(())
}

fn compose_fallback(
    imports: &PreparedImports<'_>,
    toolchain: &ToolchainInput,
    resolved: &ResolvedToolchainProfile,
) -> Box<[ModuleNamespace]> {
    let reserved = &imports.generics.headers.reserved;
    let prelude_namespace = &imports.namespaces[resolved.prelude_index];
    let mut fallback: Vec<ModuleNamespace> = Vec::with_capacity(reserved.modules.len());
    for (index, module) in reserved.modules.iter().enumerate() {
        let recipient = reserved.module_ids[index];
        let mut visible = resolved
            .builtin_types
            .iter()
            .map(|builtin| {
                (
                    builtin.symbol,
                    NamespaceBinding {
                        entity: ExportedEntity::BuiltinType(builtin.builtin),
                        visibility: nocter_declarations::Visibility::Public,
                        origin: nocter_source_index::SyntaxOrigin::Token(builtin.declaration),
                    },
                )
            })
            .collect::<Vec<_>>();
        if module.package() != toolchain.standard_package() {
            let prelude = prelude_namespace
                .iter()
                .filter(|(name, _)| {
                    visible
                        .iter()
                        .all(|(builtin_name, _)| *builtin_name != *name)
                })
                .filter(|(_, binding)| {
                    visible_from(reserved, binding.visibility, recipient, resolved.prelude)
                })
                .copied()
                .collect::<Vec<_>>();
            visible.extend(prelude);
        }
        visible.sort_unstable_by_key(|(name, _)| *name);
        let visible = visible.into_boxed_slice();
        fallback.push(visible);
    }
    fallback.into_boxed_slice()
}
