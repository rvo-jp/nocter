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

impl StandardDeclarationRole {
    /// Returns the stable compiler-contract name of this role.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AbortingAllocator => "aborting_allocator",
            Self::AllocationContext => "allocation_context",
            Self::OwnedString => "owned_string",
            Self::InterpolationConstructor => "interpolation_constructor",
            Self::InterpolationTextAppender => "interpolation_text_appender",
            Self::FormatInterface => "format_interface",
            Self::FormatMethod => "format_method",
            Self::IteratorInterface => "iterator_interface",
            Self::IteratorItem => "iterator_item",
            Self::IteratorNextMethod => "iterator_next_method",
            Self::ExactSizeIteratorInterface => "exact_size_iterator_interface",
            Self::ExactSizeIteratorRemainingLenMethod => "exact_size_iterator_remaining_len_method",
            Self::ProcessAbort => "process_abort",
        }
    }
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
    /// Returns the stable compiler-contract name of this structural surface.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Slice => "slice",
        }
    }
}
