use nocter_declarations::{AssociatedTypeBinding, InterfaceApplication, ProvenanceOrigin};
use std::collections::BTreeMap;

use nocter_model::{
    Arena, CallableId, GenericParameterId, InterfaceId, InterfaceImplementationId, Symbol, TypeId,
};

use super::predicate::CheckedRequirement;
use crate::GenericArgument;

pub(super) type InterfaceImplementationInputCorrespondence =
    Box<[(ProvenanceOrigin, ProvenanceOrigin)]>;

/// Callable selected for one interface method under an explicit interface implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodSelection {
    Implementation(CallableId),
    Default(CallableId),
}

/// One interface method and its exact dispatch target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceImplementationMethod {
    interface_method: CallableId,
    selection: MethodSelection,
    input_correspondence: InterfaceImplementationInputCorrespondence,
}

impl InterfaceImplementationMethod {
    pub(super) fn new(
        interface_method: CallableId,
        selection: MethodSelection,
        input_correspondence: InterfaceImplementationInputCorrespondence,
    ) -> Self {
        Self {
            interface_method,
            selection,
            input_correspondence,
        }
    }

    #[must_use]
    pub const fn interface_method(&self) -> CallableId {
        self.interface_method
    }

    #[must_use]
    pub const fn selection(&self) -> MethodSelection {
        self.selection
    }

    /// Maps an interface input identity to the selected body's corresponding input.
    ///
    /// Signature compatibility owns this correspondence. Later analyses must consume it rather
    /// than reconstructing parameter positions from declarations.
    pub(crate) fn selected_input(&self, input: ProvenanceOrigin) -> Option<ProvenanceOrigin> {
        self.input_correspondence
            .iter()
            .find_map(|(interface, selected)| (*interface == input).then_some(*selected))
    }
}

/// Canonical checked contract for one explicit interface implementation declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedInterfaceImplementation {
    interface: InterfaceApplication,
    target: TypeId,
    generic_parameters: Box<[GenericParameterId]>,
    refinements: Box<[GenericArgument]>,
    requirements: Box<[CheckedRequirement]>,
    associated_types: Box<[AssociatedTypeBinding]>,
    methods: Box<[InterfaceImplementationMethod]>,
}

impl CheckedInterfaceImplementation {
    pub(super) fn new(
        interface: InterfaceApplication,
        target: TypeId,
        generic_parameters: impl Into<Box<[GenericParameterId]>>,
        refinements: impl Into<Box<[GenericArgument]>>,
        requirements: impl Into<Box<[CheckedRequirement]>>,
        associated_types: impl Into<Box<[AssociatedTypeBinding]>>,
        methods: impl Into<Box<[InterfaceImplementationMethod]>>,
    ) -> Self {
        Self {
            interface,
            target,
            generic_parameters: generic_parameters.into(),
            refinements: refinements.into(),
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
    pub const fn generic_parameters(&self) -> &[GenericParameterId] {
        &self.generic_parameters
    }

    #[must_use]
    pub const fn refinements(&self) -> &[GenericArgument] {
        &self.refinements
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
    pub const fn methods(&self) -> &[InterfaceImplementationMethod] {
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
            .binary_search_by_key(
                &declaration,
                InterfaceImplementationMethod::interface_method,
            )
            .ok()
            .map(|index| self.methods[index].selection())
    }
}

/// Program-wide interface implementation dispatch authority.
#[derive(Debug)]
pub struct InterfaceImplementationTable {
    entries: BTreeMap<InterfaceImplementationId, CheckedInterfaceImplementation>,
    by_interface: Arena<InterfaceId, Box<[InterfaceImplementationId]>>,
    interfaces_by_method: BTreeMap<Symbol, Box<[InterfaceId]>>,
}

impl InterfaceImplementationTable {
    pub(super) const fn new(
        entries: BTreeMap<InterfaceImplementationId, CheckedInterfaceImplementation>,
        by_interface: Arena<InterfaceId, Box<[InterfaceImplementationId]>>,
        interfaces_by_method: BTreeMap<Symbol, Box<[InterfaceId]>>,
    ) -> Self {
        Self {
            entries,
            by_interface,
            interfaces_by_method,
        }
    }

    #[must_use]
    pub const fn entries(
        &self,
    ) -> &BTreeMap<InterfaceImplementationId, CheckedInterfaceImplementation> {
        &self.entries
    }

    #[must_use]
    pub fn candidates(&self, interface: InterfaceId) -> &[InterfaceImplementationId] {
        self.by_interface
            .get(interface)
            .map(AsRef::as_ref)
            .unwrap_or_default()
    }

    /// Returns interfaces declaring one method name in canonical declaration identity order.
    #[must_use]
    pub fn method_interfaces(&self, name: Symbol) -> &[InterfaceId] {
        self.interfaces_by_method
            .get(&name)
            .map(AsRef::as_ref)
            .unwrap_or_default()
    }

    /// Returns every interface-method name known to the interface implementation authority.
    ///
    /// Consumers must still run ordinary method selection for the receiver and lexical proof
    /// environment. This index prevents tooling from rediscovering interface members by scanning
    /// declarations.
    pub(crate) fn method_names(&self) -> impl Iterator<Item = Symbol> + '_ {
        self.interfaces_by_method.keys().copied()
    }
}
