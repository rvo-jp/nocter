use crate::{MachineBlockId, MachineDropFlagId, MachineOperationId, MachineValueId};

/// One CFG edge and the SSA values supplied to the destination block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineBranchTarget {
    block: MachineBlockId,
    arguments: Box<[MachineValueId]>,
}

impl MachineBranchTarget {
    pub(crate) fn new(block: MachineBlockId, arguments: impl Into<Box<[MachineValueId]>>) -> Self {
        Self {
            block,
            arguments: arguments.into(),
        }
    }

    #[must_use]
    pub const fn block(&self) -> MachineBlockId {
        self.block
    }

    #[must_use]
    pub const fn arguments(&self) -> &[MachineValueId] {
        &self.arguments
    }
}

/// A closed scalar tested by a machine switch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineSwitchValue {
    Integer(i128),
    Tag(u8),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineSwitchCase {
    value: MachineSwitchValue,
    target: MachineBranchTarget,
}

impl MachineSwitchCase {
    pub(crate) const fn new(value: MachineSwitchValue, target: MachineBranchTarget) -> Self {
        Self { value, target }
    }

    #[must_use]
    pub const fn value(&self) -> MachineSwitchValue {
        self.value
    }

    #[must_use]
    pub const fn target(&self) -> &MachineBranchTarget {
        &self.target
    }
}

/// The sole control-transfer instruction of one machine basic block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineTerminator {
    Goto(MachineBranchTarget),
    Branch {
        condition: MachineValueId,
        then_target: MachineBranchTarget,
        else_target: MachineBranchTarget,
    },
    BranchDropFlag {
        flag: MachineDropFlagId,
        initialized: MachineBranchTarget,
        uninitialized: MachineBranchTarget,
    },
    SwitchValue {
        subject: MachineValueId,
        cases: Box<[MachineSwitchCase]>,
        fallback: MachineBranchTarget,
    },
    SwitchTag {
        subject: crate::MachineAddressId,
        tag_offset: u64,
        cases: Box<[MachineSwitchCase]>,
        fallback: MachineBranchTarget,
    },
    Return(Option<MachineValueId>),
    Exit(Option<MachineValueId>),
    Trap,
    Unreachable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineBlock {
    parameters: Box<[MachineValueId]>,
    operations: Box<[MachineOperationId]>,
    terminator: MachineTerminator,
}

impl MachineBlock {
    pub(crate) fn new(
        parameters: impl Into<Box<[MachineValueId]>>,
        operations: impl Into<Box<[MachineOperationId]>>,
        terminator: MachineTerminator,
    ) -> Self {
        Self {
            parameters: parameters.into(),
            operations: operations.into(),
            terminator,
        }
    }

    #[must_use]
    pub const fn parameters(&self) -> &[MachineValueId] {
        &self.parameters
    }

    #[must_use]
    pub const fn operations(&self) -> &[MachineOperationId] {
        &self.operations
    }

    #[must_use]
    pub const fn terminator(&self) -> &MachineTerminator {
        &self.terminator
    }
}
