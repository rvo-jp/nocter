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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum InterpolationInputKind {
    Str,
    String,
    I32,
    U8,
    Usize,
    Bool,
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
    formatters: HashMap<InterpolationInputKind, RuntimeCallable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IterationProtocol {
    pub(crate) interface_declaration: ByteSpan,
    /// Qualified identity derived from the validated declaration's owning module.
    pub(crate) interface_canonical_name: String,
    pub(crate) method_declaration: ByteSpan,
    pub(crate) method_name: String,
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
        formatters: HashMap<InterpolationInputKind, RuntimeCallable>,
    ) -> Self {
        Self {
            string_type_declaration,
            constructor,
            formatters,
        }
    }

    pub(crate) fn formatter(&self, kind: InterpolationInputKind) -> Option<&RuntimeCallable> {
        self.formatters.get(&kind)
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
