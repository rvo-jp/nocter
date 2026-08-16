use std::collections::BTreeMap;

use nocter_model::{Arena, CallableId, GenericParameterId, InstanceId, TypeId, TypeStore};

use crate::type_relations::InherentTypeFamily;
use crate::{CheckedRequirement, GenericArgument};

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
    entries: Arena<InstanceId, CheckedInstanceOperations>,
    by_family: BTreeMap<InherentTypeFamily, Box<[InstanceId]>>,
}

impl InstanceOperationTable {
    pub(super) fn new(
        entries: Arena<InstanceId, CheckedInstanceOperations>,
        by_family: BTreeMap<InherentTypeFamily, Box<[InstanceId]>>,
    ) -> Self {
        Self { entries, by_family }
    }

    #[must_use]
    pub const fn entries(&self) -> &Arena<InstanceId, CheckedInstanceOperations> {
        &self.entries
    }

    pub(crate) fn candidates(&self, types: &TypeStore, target: TypeId) -> Option<&[InstanceId]> {
        InherentTypeFamily::of(types, target)
            .and_then(|family| self.by_family.get(&family))
            .map(AsRef::as_ref)
    }
}
