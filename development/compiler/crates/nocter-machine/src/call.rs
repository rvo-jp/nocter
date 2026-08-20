use nocter_model::TypeId;
use nocter_target_program::PrimitiveRole;

use crate::{
    MachineAddressId, MachineCallableAbi, MachineFunctionId, MachinePackId,
    MachinePrimitiveDependency, MachineStackId, MachineValueId,
};

/// One compiler-known primitive target with its concrete signature transport already planned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachinePrimitiveTarget {
    role: PrimitiveRole,
    type_arguments: Box<[TypeId]>,
    abi: MachineCallableAbi,
    dependency: MachinePrimitiveDependency,
}

impl MachinePrimitiveTarget {
    pub(crate) fn new(
        role: PrimitiveRole,
        type_arguments: impl Into<Box<[TypeId]>>,
        abi: MachineCallableAbi,
        dependency: MachinePrimitiveDependency,
    ) -> Self {
        Self {
            role,
            type_arguments: type_arguments.into(),
            abi,
            dependency,
        }
    }

    #[must_use]
    pub const fn role(&self) -> PrimitiveRole {
        self.role
    }

    #[must_use]
    pub const fn type_arguments(&self) -> &[TypeId] {
        &self.type_arguments
    }

    #[must_use]
    pub const fn abi(&self) -> &MachineCallableAbi {
        &self.abi
    }

    #[must_use]
    pub const fn dependency(&self) -> &MachinePrimitiveDependency {
        &self.dependency
    }
}

/// The closed runtime target selected before target-machine instruction lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineCallTarget {
    Direct(MachineFunctionId),
    Primitive(MachinePrimitiveTarget),
}

/// Allocation context visible only for the duration of one call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineCallAllocation {
    Inherit,
    Lexical(MachineStackId),
    Explicit(MachineAddressId),
}

/// One call instruction. A literal pack occupies its dedicated hidden ABI lane rather than the
/// ordinary argument list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineCall {
    target: MachineCallTarget,
    arguments: Box<[MachineValueId]>,
    allocation: MachineCallAllocation,
    pack: Option<MachinePackId>,
}

impl MachineCall {
    pub(crate) fn new(
        target: MachineCallTarget,
        arguments: impl Into<Box<[MachineValueId]>>,
        allocation: MachineCallAllocation,
        pack: Option<MachinePackId>,
    ) -> Self {
        Self {
            target,
            arguments: arguments.into(),
            allocation,
            pack,
        }
    }

    #[must_use]
    pub const fn target(&self) -> &MachineCallTarget {
        &self.target
    }

    #[must_use]
    pub const fn arguments(&self) -> &[MachineValueId] {
        &self.arguments
    }

    #[must_use]
    pub const fn allocation(&self) -> MachineCallAllocation {
        self.allocation
    }

    #[must_use]
    pub const fn pack(&self) -> Option<MachinePackId> {
        self.pack
    }
}
