//! Compiler-owned semantic roles attached to validated trusted declarations.

use crate::source::ByteSpan;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AllocatorCapabilityKind {
    Aborting,
    Recoverable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AllocationFailurePolicy {
    Abort,
    Recoverable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AllocationSource {
    CurrentContext,
    Input(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrustedDeclarationRole {
    AllocatorCapability(AllocatorCapabilityKind),
    CurrentAllocationContext,
    AllocationOperation {
        source: AllocationSource,
        failure_policy: AllocationFailurePolicy,
    },
    AllocationMutation {
        target: usize,
        source: AllocationSource,
        fallback_to_current: bool,
    },
    RegionEnter,
    RegionRelease,
    AllocationAbort,
    IndependentFallibleError,
    StaticResult,
    BorrowedProjection {
        source: usize,
    },
    OwnedValueTransfer {
        source: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeCallable {
    pub(crate) declaration: ByteSpan,
    pub(crate) target_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InterpolationRuntime {
    pub(crate) string_type_declaration: ByteSpan,
    pub(crate) constructor: RuntimeCallable,
    pub(crate) format_interface_declaration: ByteSpan,
    pub(crate) format_interface_canonical_name: String,
    pub(crate) format_method_declaration: ByteSpan,
    pub(crate) format_method_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IterationProtocol {
    pub(crate) interface_declaration: ByteSpan,
    /// Qualified identity derived from the validated declaration's owning module.
    pub(crate) interface_canonical_name: String,
    pub(crate) method_declaration: ByteSpan,
    pub(crate) method_name: String,
    pub(crate) associated_type: Option<IterationAssociatedType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IterationAssociatedType {
    pub(crate) declaration: ByteSpan,
    pub(crate) name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IterationRuntime {
    pub(crate) iterator: IterationProtocol,
    pub(crate) exact_size: IterationProtocol,
    pub(crate) readonly_conversion: IterationProtocol,
    pub(crate) owned_conversion: IterationProtocol,
}

impl InterpolationRuntime {
    pub(crate) fn new(
        string_type_declaration: ByteSpan,
        constructor: RuntimeCallable,
        format_interface_declaration: ByteSpan,
        format_interface_canonical_name: String,
        format_method_declaration: ByteSpan,
        format_method_name: String,
    ) -> Self {
        Self {
            string_type_declaration,
            constructor,
            format_interface_declaration,
            format_interface_canonical_name,
            format_method_declaration,
            format_method_name,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TrustedDeclarationFacts {
    roles: HashMap<ByteSpan, TrustedDeclarationRole>,
    interpolation_runtime: Option<InterpolationRuntime>,
    iteration_runtime: Option<IterationRuntime>,
}

impl TrustedDeclarationFacts {
    pub(crate) fn insert(&mut self, declaration: ByteSpan, role: TrustedDeclarationRole) {
        self.roles.insert(declaration, role);
    }

    pub(crate) fn extend(&mut self, other: Self) {
        self.roles.extend(other.roles);
        if other.interpolation_runtime.is_some() {
            self.interpolation_runtime = other.interpolation_runtime;
        }
        if other.iteration_runtime.is_some() {
            self.iteration_runtime = other.iteration_runtime;
        }
    }

    pub(crate) fn role(&self, declaration: ByteSpan) -> Option<TrustedDeclarationRole> {
        self.roles.get(&declaration).copied()
    }

    pub(crate) fn set_interpolation_runtime(&mut self, runtime: InterpolationRuntime) {
        self.interpolation_runtime = Some(runtime);
    }

    pub(crate) fn interpolation_runtime(&self) -> Option<&InterpolationRuntime> {
        self.interpolation_runtime.as_ref()
    }

    pub(crate) fn set_iteration_runtime(&mut self, runtime: IterationRuntime) {
        self.iteration_runtime = Some(runtime);
    }

    pub(crate) fn iteration_runtime(&self) -> Option<&IterationRuntime> {
        self.iteration_runtime.as_ref()
    }
}
