use std::fmt;

use super::access::{module_index_by_id, module_index_by_identity, visible_from};
use super::{ModuleNamespace, PreparedImports, lookup};
use crate::{ImportViolation, ModuleIdentity, SurfaceImportTarget};
use nocter_declarations::{BuiltinAttachment, ExportedEntity, ProgramBuildError};
use nocter_model::{ModuleId, Symbol};
use nocter_syntax::NodeId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreludeError {
    Rule(ImportViolation),
    UnknownModule(ModuleIdentity),
    InconsistentImport(NodeId),
    Program(ProgramBuildError),
}

impl fmt::Display for PreludeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(violation) => write!(
                formatter,
                "{}: {}",
                violation.rule().code(),
                violation.rule().message()
            ),
            Self::UnknownModule(module) => {
                write!(formatter, "standard prelude module {module:?} is absent")
            }
            Self::InconsistentImport(import) => {
                write!(formatter, "authored import {import:?} has no retained path")
            }
            Self::Program(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PreludeError {}

impl From<ImportViolation> for PreludeError {
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
    pub fn lookup_local(&self, module: ModuleId, name: Symbol) -> Option<ExportedEntity> {
        self.imports.lookup_local(module, name).or_else(|| {
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
}

/// Adds the exact compiler-selected standard prelude as a fallback for every non-standard module.
///
/// The package containing `prelude_module` is treated as the standard-library package for this
/// stage. Its modules receive no fallback. Authored imports of the prelude remain invalid.
///
/// # Errors
///
/// Returns [`PreludeError`] when the selected module is absent or source explicitly imports it.
pub fn apply_standard_prelude<'syntax>(
    mut imports: PreparedImports<'syntax>,
    prelude_module: &ModuleIdentity,
) -> Result<PreparedNamespaces<'syntax>, PreludeError> {
    let (prelude_index, prelude_id, standard_package, attachment_modules) = {
        let reserved = &imports.generics.headers.reserved;
        let prelude_index = module_index_by_identity(reserved, prelude_module)
            .ok_or_else(|| PreludeError::UnknownModule(prelude_module.clone()))?;
        for (index, import) in reserved.imports.iter().enumerate() {
            if matches!(
                import.target(),
                SurfaceImportTarget::Module(target) if target == prelude_module
            ) {
                let path = imports
                    .import_path(index)
                    .ok_or(PreludeError::InconsistentImport(import.node()))?;
                return Err(ImportViolation::compiler_managed_prelude_import(path).into());
            }
        }

        let prelude_id = reserved.module_ids[prelude_index];
        let standard_package = reserved
            .program
            .module_package(prelude_id)
            .ok_or_else(|| PreludeError::UnknownModule(prelude_module.clone()))?;
        let attachment_modules = [
            (BuiltinAttachment::Scalar, "num"),
            (BuiltinAttachment::Str, "str"),
            (BuiltinAttachment::Error, "error"),
            (BuiltinAttachment::Slice, "slice"),
        ]
        .into_iter()
        .filter_map(|(attachment, path)| {
            let identity = ModuleIdentity::new(prelude_module.package().clone(), [path]);
            module_index_by_identity(reserved, &identity)
                .map(|index| (attachment, reserved.module_ids[index]))
        })
        .collect::<Vec<_>>();
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
        .map_err(PreludeError::Program)?;
    for (attachment, module) in attachment_modules {
        imports
            .generics
            .headers
            .reserved
            .program
            .set_builtin_attachment_module(attachment, module)
            .map_err(PreludeError::Program)?;
    }

    let reserved = &imports.generics.headers.reserved;
    let prelude_namespace = &imports.namespaces[prelude_index];
    let mut fallback: Vec<ModuleNamespace> = Vec::with_capacity(reserved.modules.len());
    for (index, module) in reserved.modules.iter().enumerate() {
        if module.package() == prelude_module.package() {
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
