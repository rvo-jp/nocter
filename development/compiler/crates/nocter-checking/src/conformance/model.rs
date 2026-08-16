use nocter_declarations::{AssociatedTypeBinding, InterfaceApplication};
use nocter_model::{Arena, CallableId, ConformanceId, InterfaceId, TypeId};

use super::predicate::CheckedRequirement;

/// Callable selected for one interface method under an explicit conformance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodSelection {
    Implementation(CallableId),
    Default(CallableId),
}

/// One interface method and its exact dispatch target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConformanceMethod {
    interface_method: CallableId,
    selection: MethodSelection,
}

impl ConformanceMethod {
    pub(super) const fn new(interface_method: CallableId, selection: MethodSelection) -> Self {
        Self {
            interface_method,
            selection,
        }
    }

    #[must_use]
    pub const fn interface_method(self) -> CallableId {
        self.interface_method
    }

    #[must_use]
    pub const fn selection(self) -> MethodSelection {
        self.selection
    }
}

/// Canonical checked contract for one explicit conformance declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedConformance {
    interface: InterfaceApplication,
    target: TypeId,
    requirements: Box<[CheckedRequirement]>,
    associated_types: Box<[AssociatedTypeBinding]>,
    methods: Box<[ConformanceMethod]>,
}

impl CheckedConformance {
    pub(super) fn new(
        interface: InterfaceApplication,
        target: TypeId,
        requirements: impl Into<Box<[CheckedRequirement]>>,
        associated_types: impl Into<Box<[AssociatedTypeBinding]>>,
        methods: impl Into<Box<[ConformanceMethod]>>,
    ) -> Self {
        Self {
            interface,
            target,
            requirements: requirements.into(),
            associated_types: associated_types.into(),
            methods: methods.into(),
        }
    }

    #[must_use]
    pub const fn interface(&self) -> &InterfaceApplication {
        &self.interface
    }

    #[must_use]
    pub const fn target(&self) -> TypeId {
        self.target
    }

    #[must_use]
    pub const fn requirements(&self) -> &[CheckedRequirement] {
        &self.requirements
    }

    #[must_use]
    pub const fn associated_types(&self) -> &[AssociatedTypeBinding] {
        &self.associated_types
    }

    #[must_use]
    pub const fn methods(&self) -> &[ConformanceMethod] {
        &self.methods
    }

    #[must_use]
    pub fn associated_type(&self, declaration: nocter_model::AssociatedTypeId) -> Option<TypeId> {
        self.associated_types
            .binary_search_by_key(&declaration, |binding| binding.declaration())
            .ok()
            .map(|index| self.associated_types[index].ty())
    }

    #[must_use]
    pub fn method(&self, declaration: CallableId) -> Option<MethodSelection> {
        self.methods
            .binary_search_by_key(&declaration, |method| method.interface_method())
            .ok()
            .map(|index| self.methods[index].selection())
    }
}

/// Program-wide conformance dispatch authority.
#[derive(Debug)]
pub struct ConformanceTable {
    entries: Arena<ConformanceId, CheckedConformance>,
    by_interface: Arena<InterfaceId, Box<[ConformanceId]>>,
}

impl ConformanceTable {
    pub(super) const fn new(
        entries: Arena<ConformanceId, CheckedConformance>,
        by_interface: Arena<InterfaceId, Box<[ConformanceId]>>,
    ) -> Self {
        Self {
            entries,
            by_interface,
        }
    }

    #[must_use]
    pub const fn entries(&self) -> &Arena<ConformanceId, CheckedConformance> {
        &self.entries
    }

    #[must_use]
    pub fn candidates(&self, interface: InterfaceId) -> &[ConformanceId] {
        self.by_interface
            .get(interface)
            .map(AsRef::as_ref)
            .unwrap_or_default()
    }
}
