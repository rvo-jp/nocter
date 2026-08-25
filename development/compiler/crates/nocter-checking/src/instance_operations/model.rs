use std::collections::BTreeMap;

use crate::{CheckedRequirement, GenericArgument};
use nocter_model::{
    AttachmentFamily, CallableId, GenericParameterId, InstanceId, Symbol, TypeId, TypeStore,
};

/// One refinement-normalized instance declaration and its operation members.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedInstanceOperations {
    target: TypeId,
    generic_parameters: Box<[GenericParameterId]>,
    refinements: Box<[GenericArgument]>,
    requirements: Box<[CheckedRequirement]>,
    members: Box<[CallableId]>,
}

impl CheckedInstanceOperations {
    pub(super) fn new(
        target: TypeId,
        generic_parameters: impl Into<Box<[GenericParameterId]>>,
        refinements: impl Into<Box<[GenericArgument]>>,
        requirements: impl Into<Box<[CheckedRequirement]>>,
        members: impl Into<Box<[CallableId]>>,
    ) -> Self {
        Self {
            target,
            generic_parameters: generic_parameters.into(),
            refinements: refinements.into(),
            requirements: requirements.into(),
            members: members.into(),
        }
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
    pub const fn members(&self) -> &[CallableId] {
        &self.members
    }
}

/// Sole normalized lookup authority for instance-owned operations.
#[derive(Debug)]
pub struct InstanceOperationTable {
    entries: BTreeMap<InstanceId, CheckedInstanceOperations>,
    by_family: BTreeMap<AttachmentFamily, Box<[InstanceId]>>,
    method_names_by_family: BTreeMap<AttachmentFamily, Box<[Symbol]>>,
}

impl InstanceOperationTable {
    pub(super) fn new(
        entries: BTreeMap<InstanceId, CheckedInstanceOperations>,
        by_family: BTreeMap<AttachmentFamily, Box<[InstanceId]>>,
        method_names_by_family: BTreeMap<AttachmentFamily, Box<[Symbol]>>,
    ) -> Self {
        Self {
            entries,
            by_family,
            method_names_by_family,
        }
    }

    #[must_use]
    pub const fn entries(&self) -> &BTreeMap<InstanceId, CheckedInstanceOperations> {
        &self.entries
    }

    pub(crate) fn candidates(&self, types: &TypeStore, target: TypeId) -> Option<&[InstanceId]> {
        AttachmentFamily::of(types, target)
            .and_then(|family| self.by_family.get(&family))
            .map(AsRef::as_ref)
    }

    /// Returns the canonical candidate-name universe for one inherent receiver family.
    ///
    /// This is a discovery index only. Visibility, pattern matching, requirements, receiver
    /// capability, and dispatch are still decided by [`super::InstanceOperationSelector`].
    pub(crate) fn method_names(&self, types: &TypeStore, target: TypeId) -> &[Symbol] {
        AttachmentFamily::of(types, target)
            .and_then(|family| self.method_names_by_family.get(&family))
            .map(AsRef::as_ref)
            .unwrap_or_default()
    }
}
