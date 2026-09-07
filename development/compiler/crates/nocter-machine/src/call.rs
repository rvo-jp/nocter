use nocter_model::TypeId;
use nocter_runtime_contract::PrimitiveRole;

use crate::identity::MachineRuntimeCallAbiId;
use crate::{
    MachineAddressId, MachineFunctionId, MachinePackId, MachinePrimitiveDependency, MachineStackId,
    MachineValueId,
};

/// One compiler-known primitive target referencing a canonical, already-planned ABI entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachinePrimitiveTarget {
    role: PrimitiveRole,
    type_arguments: Box<[TypeId]>,
    abi: MachineRuntimeCallAbiId,
    dependency: MachinePrimitiveDependency,
}

impl MachinePrimitiveTarget {
    pub(crate) fn new(
        role: PrimitiveRole,
        type_arguments: impl Into<Box<[TypeId]>>,
        abi: MachineRuntimeCallAbiId,
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
    pub(crate) const fn abi_id(&self) -> MachineRuntimeCallAbiId {
        self.abi
    }

    #[must_use]
    pub const fn dependency(&self) -> &MachinePrimitiveDependency {
        &self.dependency
    }
}

/// One imported target-service call retaining the closed descriptor and planned machine ABI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineImportedTarget {
    import: crate::MachineImportId,
    abi: MachineRuntimeCallAbiId,
}

impl MachineImportedTarget {
    pub(crate) const fn new(import: crate::MachineImportId, abi: MachineRuntimeCallAbiId) -> Self {
        Self { import, abi }
    }

    #[must_use]
    pub const fn import(&self) -> crate::MachineImportId {
        self.import
    }

    #[must_use]
    pub(crate) const fn abi_id(&self) -> MachineRuntimeCallAbiId {
        self.abi
    }
}

/// The closed runtime target selected before target-machine instruction lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineCallTarget {
    Direct(MachineFunctionId),
    Primitive(MachinePrimitiveTarget),
    Imported(MachineImportedTarget),
}

/// Allocation context visible only for the duration of one call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineCallAllocation {
    Inherit,
    Lexical(MachineStackId),
    Explicit(MachineAddressId),
}

/// The hidden pack lane either owns a prepared descriptor or forwards the incoming descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineCallPack {
    Prepared(MachinePackId),
    Forwarded,
}

/// One call instruction. An argument pack occupies its dedicated hidden ABI lane rather than the
/// ordinary argument list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineCall {
    target: MachineCallTarget,
    arguments: Box<[MachineValueId]>,
    allocation: MachineCallAllocation,
    pack: Option<MachineCallPack>,
}

impl MachineCall {
    pub(crate) fn new(
        target: MachineCallTarget,
        arguments: impl Into<Box<[MachineValueId]>>,
        allocation: MachineCallAllocation,
        pack: Option<MachineCallPack>,
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
    pub const fn pack(&self) -> Option<MachineCallPack> {
        self.pack
    }
}
