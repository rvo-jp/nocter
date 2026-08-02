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
    RegionEnter,
    RegionRelease,
    AllocationAbort,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TrustedDeclarationFacts {
    roles: HashMap<ByteSpan, TrustedDeclarationRole>,
}

impl TrustedDeclarationFacts {
    pub(crate) fn insert(&mut self, declaration: ByteSpan, role: TrustedDeclarationRole) {
        self.roles.insert(declaration, role);
    }

    pub(crate) fn extend(&mut self, other: Self) {
        self.roles.extend(other.roles);
    }

    pub(crate) fn role(&self, declaration: ByteSpan) -> Option<TrustedDeclarationRole> {
        self.roles.get(&declaration).copied()
    }
}
