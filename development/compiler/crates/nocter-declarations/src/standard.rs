use nocter_model::{ModuleId, PackageId};

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
    FormatInterface,
    FormatMethod,
    IteratorInterface,
    IteratorItem,
    IteratorNextMethod,
    ExactSizeIteratorInterface,
    ExactSizeIteratorRemainingLenMethod,
}

/// One compiler-owned built-in surface that source declarations may extend.
///
/// These semantic roles are distinct from module path spellings. Compilation setup resolves each
/// role to an exact module identity once, before declaration validation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuiltinAttachment {
    Scalar,
    Str,
    Error,
    Slice,
}

impl BuiltinAttachment {
    const COUNT: usize = 4;

    const fn index(self) -> usize {
        self as usize
    }
}

/// Exact compiler-selected authority for standard-library-only declarations.
#[derive(Debug)]
pub struct StandardLibrary {
    package: PackageId,
    attachment_modules: [Option<ModuleId>; BuiltinAttachment::COUNT],
}

impl StandardLibrary {
    pub(crate) const fn new(package: PackageId) -> Self {
        Self {
            package,
            attachment_modules: [None; BuiltinAttachment::COUNT],
        }
    }

    #[must_use]
    pub const fn package(&self) -> PackageId {
        self.package
    }

    #[must_use]
    pub const fn attachment_module(&self, attachment: BuiltinAttachment) -> Option<ModuleId> {
        self.attachment_modules[attachment.index()]
    }

    pub(crate) fn set_attachment_module(
        &mut self,
        attachment: BuiltinAttachment,
        module: ModuleId,
    ) -> Result<(), ModuleId> {
        let slot = &mut self.attachment_modules[attachment.index()];
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
