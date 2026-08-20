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

/// Compiler-defined meaning assigned to one exact bodyless standard callable.
///
/// The role is target-independent semantic identity. A toolchain maps every role to an exact
/// source declaration before lowering; target validation later proves that declaration's complete
/// contract and availability for the selected target.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PrimitiveRole {
    NewError,
    CurrentAllocatorState,
    CurrentAllocatorKind,
    AllocationAbort,
    PointerAddress,
    PointerFromReference,
    PointerFromReadWriteReference,
    PointerFromAddress,
    PointeeSize,
    PointeeAlignment,
    CopyStringToPointer,
    CopyPointerToPointer,
    StoreByteToPointer,
    StoreValueToPointer,
    DropValueAtPointer,
    TakeValueAtPointer,
    StringFromRawParts,
    ByteSliceFromRawParts,
    MutableByteSliceFromRawParts,
    ValueSliceFromRawParts,
    MutableValueSliceFromRawParts,
    BytesFromString,
    StringSubviewUnchecked,
    SliceLength,
    SlicePointerAddress,
    StringLength,
    StringPointerAddress,
    ProcessExit,
    ProcessArgumentCount,
    ProcessArgument,
    ProcessEnvironmentCount,
    ProcessEnvironmentName,
    ProcessEnvironmentValue,
    Syscall0,
    Syscall1,
    Syscall2,
    Syscall3,
    Syscall4,
    Syscall5,
    Syscall6,
    Trap,
    Unreachable,
}

impl PrimitiveRole {
    pub const ALL: &'static [Self] = &[
        Self::NewError,
        Self::CurrentAllocatorState,
        Self::CurrentAllocatorKind,
        Self::AllocationAbort,
        Self::PointerAddress,
        Self::PointerFromReference,
        Self::PointerFromReadWriteReference,
        Self::PointerFromAddress,
        Self::PointeeSize,
        Self::PointeeAlignment,
        Self::CopyStringToPointer,
        Self::CopyPointerToPointer,
        Self::StoreByteToPointer,
        Self::StoreValueToPointer,
        Self::DropValueAtPointer,
        Self::TakeValueAtPointer,
        Self::StringFromRawParts,
        Self::ByteSliceFromRawParts,
        Self::MutableByteSliceFromRawParts,
        Self::ValueSliceFromRawParts,
        Self::MutableValueSliceFromRawParts,
        Self::BytesFromString,
        Self::StringSubviewUnchecked,
        Self::SliceLength,
        Self::SlicePointerAddress,
        Self::StringLength,
        Self::StringPointerAddress,
        Self::ProcessExit,
        Self::ProcessArgumentCount,
        Self::ProcessArgument,
        Self::ProcessEnvironmentCount,
        Self::ProcessEnvironmentName,
        Self::ProcessEnvironmentValue,
        Self::Syscall0,
        Self::Syscall1,
        Self::Syscall2,
        Self::Syscall3,
        Self::Syscall4,
        Self::Syscall5,
        Self::Syscall6,
        Self::Trap,
        Self::Unreachable,
    ];
}

/// One compiler-owned built-in surface that source declarations may extend.
///
/// These semantic roles are distinct from module path spellings. Compilation setup resolves each
/// role to an exact module identity once, before declaration validation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
