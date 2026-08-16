use nocter_model::{
    AssociatedTypeId, BorrowCapability, CallableContract, CallableId, ConformanceId,
    GenericParameterId, InstanceId, InterfaceId, NominalTypeId, TypeAliasId, TypeId,
};

use crate::InterfaceApplication;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExpansionCapability {
    Readonly,
    ReadWrite,
    Owned,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum StructuralCapability {
    Interface(InterfaceApplication),
    Callable(CallableContract),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RequirementOwner {
    NominalType(NominalTypeId),
    TypeAlias(TypeAliasId),
    Interface(InterfaceId),
    Callable(CallableId),
    Instance(InstanceId),
    Conformance(ConformanceId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RequirementSubject {
    GenericParameter(GenericParameterId),
    AssociatedType(AssociatedTypeId),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum RequirementKind {
    Capability {
        subject: RequirementSubject,
        capability: StructuralCapability,
    },
    Copy(GenericParameterId),
    TypeEquality {
        left: TypeId,
        right: TypeId,
    },
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
