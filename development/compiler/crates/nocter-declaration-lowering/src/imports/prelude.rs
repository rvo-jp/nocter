use std::fmt;

use super::access::{module_index_by_id, module_index_by_identity, visible_from};
use super::{ModuleNamespace, PreparedImports, lookup};
use crate::{ImportViolation, ModuleIdentity, PackageIdentity, ToolchainInput};
use nocter_declarations::{
    BuiltinAttachment, ExportedEntity, FallbackEntry, ModuleNamespace as SemanticModuleNamespace,
    NamespaceEntry, ProgramBuildError,
};
use nocter_model::{ModuleId, Symbol};
use nocter_syntax::NodeId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolchainError {
    MissingProfile,
    Rule(ImportViolation),
    UnknownStandardPackage(PackageIdentity),
    UnknownModule(ModuleIdentity),
    PreludeOutsideStandardPackage,
    AttachmentOutsideStandardPackage(BuiltinAttachment),
    DuplicateAttachment(BuiltinAttachment),
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
            Self::AttachmentOutsideStandardPackage(attachment) => {
                write!(
                    formatter,
                    "{attachment:?} built-in attachment is outside the selected standard package"
                )
            }
            Self::DuplicateAttachment(attachment) => {
                write!(
                    formatter,
                    "toolchain profile repeats {attachment:?} attachment"
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

/// Authored namespaces plus compiler-managed standard-prelude fallback entries.
#[derive(Debug)]
pub struct PreparedNamespaces<'syntax> {
    pub(crate) imports: PreparedImports<'syntax>,
    prelude: Box<[ModuleNamespace]>,
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
            lookup(&self.prelude[index], name).map(|binding| binding.entity)
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

    /// Freezes the already-selected authored and prelude namespace layers into the declaration
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
            let fallback = self.prelude[module_index]
                .iter()
                .filter(|(name, _)| {
                    namespace
                        .binary_search_by_key(name, |(candidate, _)| *candidate)
                        .is_err()
                })
                .map(|(name, binding)| (*name, binding.entity));
            self.imports
                .generics
                .headers
                .reserved
                .source_index
                .define_source_namespace(source.syntax().source(), authored, fallback);
        }
        let namespaces = self
            .imports
            .namespaces
            .iter()
            .zip(self.prelude.iter())
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
    let (prelude_index, prelude_id, standard_package, attachment_modules) = {
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

        let prelude_id = reserved.module_ids[prelude_index];
        let standard_package_index = reserved
            .packages
            .iter()
            .position(|package| package.identity() == toolchain.standard_package())
            .ok_or_else(|| {
                ToolchainError::UnknownStandardPackage(toolchain.standard_package().clone())
            })?;
        let standard_package = reserved.package_ids[standard_package_index];
        let mut attachment_modules = Vec::with_capacity(toolchain.builtin_attachments().len());
        for input in toolchain.builtin_attachments() {
            if input.module().package() != toolchain.standard_package() {
                return Err(ToolchainError::AttachmentOutsideStandardPackage(
                    input.attachment(),
                ));
            }
            if attachment_modules
                .iter()
                .any(|(attachment, _)| *attachment == input.attachment())
            {
                return Err(ToolchainError::DuplicateAttachment(input.attachment()));
            }
            let index = module_index_by_identity(reserved, input.module())
                .ok_or_else(|| ToolchainError::UnknownModule(input.module().clone()))?;
            attachment_modules.push((input.attachment(), reserved.module_ids[index]));
        }
        (
            prelude_index,
            prelude_id,
            standard_package,
            attachment_modules,
        )
    };
    imports
        .generics
        .headers
        .reserved
        .program
        .set_standard_package(standard_package)
        .map_err(ToolchainError::Program)?;
    for (attachment, module) in attachment_modules {
        imports
            .generics
            .headers
            .reserved
            .program
            .set_builtin_attachment_module(attachment, module)
            .map_err(ToolchainError::Program)?;
    }

    let reserved = &imports.generics.headers.reserved;
    let prelude_namespace = &imports.namespaces[prelude_index];
    let mut fallback: Vec<ModuleNamespace> = Vec::with_capacity(reserved.modules.len());
    for (index, module) in reserved.modules.iter().enumerate() {
        if module.package() == toolchain.standard_package() {
            fallback.push(Box::new([]));
            continue;
        }
        let recipient = reserved.module_ids[index];
        let visible = prelude_namespace
            .iter()
            .filter(|(_, binding)| {
                visible_from(reserved, binding.visibility, recipient, prelude_id)
            })
            .copied()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        fallback.push(visible);
    }

    Ok(PreparedNamespaces {
        imports,
        prelude: fallback.into_boxed_slice(),
    })
}
