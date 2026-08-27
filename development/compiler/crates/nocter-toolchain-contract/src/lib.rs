//! Closed identities shared across compiler discovery and semantic construction.

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
    ProcessAbort,
}

/// One anonymous structural type surface that standard source declarations may extend.
///
/// Named builtin types derive their authority from their `primitive type` declarations. Only
/// structural types without a declaration require a separate compiler-selected authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StructuralAttachment {
    Slice,
}
