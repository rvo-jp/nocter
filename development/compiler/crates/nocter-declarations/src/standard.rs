use nocter_model::{BuiltinType, ModuleId, PackageId};

/// Compiler-defined meaning assigned to one exact declaration by toolchain discovery.
///
/// Roles are never inferred from source names or module paths. The declaration remains ordinary
/// Nocter source; this identity only authorizes semantics that cannot be expressed by the language
/// itself, such as interpolation construction and ambient allocation propagation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StandardDeclarationRole {
    AbortingAllocator,
    AllocationContext,
    OwnedString,
    InterpolationConstructor,
    InterpolationTextAppender,
    FormatInterface,
    FormatMethod,
    IteratorInterface,
    IteratorItem,
    IteratorNextMethod,
    ExactSizeIteratorInterface,
    ExactSizeIteratorRemainingLenMethod,
}

/// One anonymous structural type surface that standard source declarations may extend.
///
/// Named builtin types derive their authority from their `primitive type` declarations. Only
/// structural types without a declaration require a separate compiler-selected authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StructuralAttachment {
    Slice,
}

impl StructuralAttachment {
    const COUNT: usize = 1;

    const fn index(self) -> usize {
        self as usize
    }
}

/// Exact compiler-selected authority for standard-library-only declarations.
#[derive(Debug)]
pub struct StandardLibrary {
    package: PackageId,
    structural_attachment_modules: [Option<ModuleId>; StructuralAttachment::COUNT],
    builtin_type_modules: [Option<ModuleId>; BuiltinType::COUNT],
}

impl StandardLibrary {
    pub(crate) const fn new(package: PackageId) -> Self {
        Self {
            package,
            structural_attachment_modules: [None; StructuralAttachment::COUNT],
            builtin_type_modules: [None; BuiltinType::COUNT],
        }
    }

    #[must_use]
    pub const fn builtin_type_module(&self, builtin: BuiltinType) -> Option<ModuleId> {
        self.builtin_type_modules[builtin.index()]
    }

    pub(crate) fn set_builtin_type_module(
        &mut self,
        builtin: BuiltinType,
        module: ModuleId,
    ) -> Result<(), ModuleId> {
        let slot = &mut self.builtin_type_modules[builtin.index()];
        match *slot {
            None => {
                *slot = Some(module);
                Ok(())
            }
            Some(existing) if existing == module => Ok(()),
            Some(existing) => Err(existing),
        }
    }

    #[must_use]
    pub const fn package(&self) -> PackageId {
        self.package
    }

    #[must_use]
    pub const fn structural_attachment_module(
        &self,
        attachment: StructuralAttachment,
    ) -> Option<ModuleId> {
        self.structural_attachment_modules[attachment.index()]
    }

    pub(crate) fn set_structural_attachment_module(
        &mut self,
        attachment: StructuralAttachment,
        module: ModuleId,
    ) -> Result<(), ModuleId> {
        let slot = &mut self.structural_attachment_modules[attachment.index()];
        match *slot {
            None => {
                *slot = Some(module);
                Ok(())
            }
            Some(existing) if existing == module => Ok(()),
            Some(existing) => Err(existing),
        }
    }
}
