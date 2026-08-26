use nocter_model::{
    AssociatedTypeId, BorrowCapability, CallableContract, CallableId, GenericParameterId,
    InstanceId, InterfaceId, NominalTypeId, TypeAliasId, TypeId,
};

use crate::{AssociatedTypeBinding, InterfaceApplication};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExpansionCapability {
    Readonly,
    ReadWrite,
    Owned,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RequirementOwner {
    NominalType(NominalTypeId),
    TypeAlias(TypeAliasId),
    Interface(InterfaceId),
    Callable(CallableId),
    Instance(InstanceId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RequirementSubject {
    GenericParameter(GenericParameterId),
    AssociatedType(AssociatedTypeId),
    InterfaceSelf(InterfaceId),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum RequirementKind {
    Interface {
        subject: RequirementSubject,
        application: InterfaceApplication,
        associated_types: Box<[AssociatedTypeBinding]>,
    },
    Callable {
        subject: GenericParameterId,
        contract: CallableContract,
    },
    Copy(GenericParameterId),
    Equality {
        operand: GenericParameterId,
    },
    Ordering {
        operand: GenericParameterId,
    },
    Index {
        capability: BorrowCapability,
        container: GenericParameterId,
        index: TypeId,
        result: TypeId,
    },
    Coercion {
        source: TypeId,
        target: TypeId,
    },
    Expansion {
        capability: ExpansionCapability,
        source: GenericParameterId,
        result: TypeId,
    },
    BinderRefinement {
        parameter: GenericParameterId,
        replacement: TypeId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Requirement {
    owner: RequirementOwner,
    kind: RequirementKind,
}

impl Requirement {
    #[must_use]
    pub const fn new(owner: RequirementOwner, kind: RequirementKind) -> Self {
        Self { owner, kind }
    }

    #[must_use]
    pub const fn owner(&self) -> RequirementOwner {
        self.owner
    }

    #[must_use]
    pub const fn kind(&self) -> &RequirementKind {
        &self.kind
    }
}
