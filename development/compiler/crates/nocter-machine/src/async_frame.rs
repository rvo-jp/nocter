use crate::{MachineAddressId, MachineBlockId, MachineDropFlagId, MachineStackId, MachineValueId};

/// One body resource stored in a deferred computation frame.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MachineFrameField {
    Pack,
    Stack(MachineStackId),
    Value(MachineValueId),
    DropFlag(MachineDropFlagId),
}

/// One ordered cancellation action, fully specialized for the machine program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineCancellationAction {
    ReleaseAwaited(MachineValueId),
    Destroy {
        address: MachineAddressId,
        initialized: Option<MachineDropFlagId>,
        destruction: crate::MachineFunctionId,
    },
    ReleaseRegion(MachineStackId),
    DestroyPack,
}

/// Resources owned before the first poll of a deferred computation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineInitialAsyncState {
    fields: Box<[MachineFrameField]>,
    cancellation: Box<[MachineCancellationAction]>,
}

impl MachineInitialAsyncState {
    pub(crate) fn new(
        fields: impl Into<Box<[MachineFrameField]>>,
        cancellation: impl Into<Box<[MachineCancellationAction]>>,
    ) -> Self {
        Self {
            fields: fields.into(),
            cancellation: cancellation.into(),
        }
    }

    #[must_use]
    pub const fn fields(&self) -> &[MachineFrameField] {
        &self.fields
    }

    #[must_use]
    pub const fn cancellation(&self) -> &[MachineCancellationAction] {
        &self.cancellation
    }
}

/// One exact suspension state and the continuation resources retained by it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineSuspensionState {
    suspend: MachineBlockId,
    resume: MachineBlockId,
    awaited: MachineValueId,
    fields: Box<[MachineFrameField]>,
    cancellation: Box<[MachineCancellationAction]>,
}

impl MachineSuspensionState {
    pub(crate) fn new(
        suspend: MachineBlockId,
        resume: MachineBlockId,
        awaited: MachineValueId,
        fields: impl Into<Box<[MachineFrameField]>>,
        cancellation: impl Into<Box<[MachineCancellationAction]>>,
    ) -> Self {
        Self {
            suspend,
            resume,
            awaited,
            fields: fields.into(),
            cancellation: cancellation.into(),
        }
    }

    #[must_use]
    pub const fn suspend(&self) -> MachineBlockId {
        self.suspend
    }

    #[must_use]
    pub const fn resume(&self) -> MachineBlockId {
        self.resume
    }

    #[must_use]
    pub const fn awaited(&self) -> MachineValueId {
        self.awaited
    }

    #[must_use]
    pub const fn fields(&self) -> &[MachineFrameField] {
        &self.fields
    }

    #[must_use]
    pub const fn cancellation(&self) -> &[MachineCancellationAction] {
        &self.cancellation
    }
}

/// The closed target-independent lifecycle contract for one deferred function body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineAsyncFrame {
    output_representation: crate::MachineValueRepresentation,
    initial: MachineInitialAsyncState,
    states: Box<[MachineSuspensionState]>,
    completed_destruction: Option<crate::MachineFunctionId>,
}

impl MachineAsyncFrame {
    pub(crate) fn new(
        output_representation: crate::MachineValueRepresentation,
        initial: MachineInitialAsyncState,
        states: impl Into<Box<[MachineSuspensionState]>>,
        completed_destruction: Option<crate::MachineFunctionId>,
    ) -> Self {
        Self {
            output_representation,
            initial,
            states: states.into(),
            completed_destruction,
        }
    }

    /// Closed storage and transport class of the completed inner output.
    #[must_use]
    pub const fn output_representation(&self) -> crate::MachineValueRepresentation {
        self.output_representation
    }

    #[must_use]
    pub const fn initial(&self) -> &MachineInitialAsyncState {
        &self.initial
    }

    #[must_use]
    pub const fn states(&self) -> &[MachineSuspensionState] {
        &self.states
    }

    #[must_use]
    pub const fn completed_destruction(&self) -> Option<crate::MachineFunctionId> {
        self.completed_destruction
    }
}
