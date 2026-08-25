use nocter_model::TypeId;

use crate::{MachineAddressId, MachineFunctionId, MachineResultAbi, MachineValueId};

/// How a successful spread item becomes one pack element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachinePackContribution {
    Direct,
    CopyBorrowed,
}

/// The validated optional result representation produced by one spread iterator callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachinePackNextOutcome {
    result: TypeId,
    item: TypeId,
    tag_offset: u64,
    payload_offset: u64,
}

impl MachinePackNextOutcome {
    pub(crate) const fn new(
        result: TypeId,
        item: TypeId,
        tag_offset: u64,
        payload_offset: u64,
    ) -> Self {
        Self {
            result,
            item,
            tag_offset,
            payload_offset,
        }
    }

    #[must_use]
    pub const fn result(self) -> TypeId {
        self.result
    }

    #[must_use]
    pub const fn item(self) -> TypeId {
        self.item
    }

    #[must_use]
    pub const fn tag_offset(self) -> u64 {
        self.tag_offset
    }

    #[must_use]
    pub const fn payload_offset(self) -> u64 {
        self.payload_offset
    }
}

/// The direct function and receiver location used to consume one spread item.
///
/// The target function remains the only authority for its ordinary call ABI. Keeping that ABI out
/// of the pack descriptor prevents callback metadata from drifting away from the function table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachinePackNext {
    receiver_offset: u64,
    target: MachineFunctionId,
    outcome: MachinePackNextOutcome,
}

impl MachinePackNext {
    pub(crate) const fn new(
        receiver_offset: u64,
        target: MachineFunctionId,
        outcome: MachinePackNextOutcome,
    ) -> Self {
        Self {
            receiver_offset,
            target,
            outcome,
        }
    }

    #[must_use]
    pub const fn receiver_offset(&self) -> u64 {
        self.receiver_offset
    }

    #[must_use]
    pub const fn target(&self) -> MachineFunctionId {
        self.target
    }

    #[must_use]
    pub const fn outcome(&self) -> MachinePackNextOutcome {
        self.outcome
    }

    #[must_use]
    pub const fn item(&self) -> TypeId {
        self.outcome.item()
    }
}

/// One acquired exact-size iterator retained by a transferred pack. Residual cleanup is already
/// closed to an ordinary compiler-generated function; no recursive destruction recipe survives.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachinePackSpread {
    iterator: MachineAddressId,
    remaining: MachineValueId,
    next: MachinePackNext,
    contribution: MachinePackContribution,
    destruction: Option<MachineFunctionId>,
}

impl MachinePackSpread {
    pub(crate) const fn new(
        iterator: MachineAddressId,
        remaining: MachineValueId,
        next: MachinePackNext,
        contribution: MachinePackContribution,
        destruction: Option<MachineFunctionId>,
    ) -> Self {
        Self {
            iterator,
            remaining,
            next,
            contribution,
            destruction,
        }
    }

    #[must_use]
    pub const fn iterator(&self) -> MachineAddressId {
        self.iterator
    }

    #[must_use]
    pub const fn remaining(&self) -> MachineValueId {
        self.remaining
    }

    #[must_use]
    pub const fn next(&self) -> &MachinePackNext {
        &self.next
    }

    #[must_use]
    pub const fn contribution(&self) -> MachinePackContribution {
        self.contribution
    }

    #[must_use]
    pub const fn destruction(&self) -> Option<MachineFunctionId> {
        self.destruction
    }
}

/// One source-ordered owner inside a pack descriptor. A cleanup target, when present, is an
/// ordinary generated machine function using the common byte-address destruction ABI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachinePackSegment {
    Value {
        value: MachineValueId,
        destruction: Option<MachineFunctionId>,
    },
    Spread(MachinePackSpread),
}

/// One caller-owned literal descriptor transferred through the hidden pack pointer lane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachinePack {
    element: TypeId,
    next: TypeId,
    next_result: MachineResultAbi,
    length: MachineValueId,
    segments: Box<[MachinePackSegment]>,
}

impl MachinePack {
    pub(crate) fn new(
        element: TypeId,
        next: TypeId,
        next_result: MachineResultAbi,
        length: MachineValueId,
        segments: impl Into<Box<[MachinePackSegment]>>,
    ) -> Self {
        Self {
            element,
            next,
            next_result,
            length,
            segments: segments.into(),
        }
    }

    #[must_use]
    pub const fn element(&self) -> TypeId {
        self.element
    }

    #[must_use]
    pub const fn next(&self) -> TypeId {
        self.next
    }

    #[must_use]
    pub const fn next_result(&self) -> MachineResultAbi {
        self.next_result
    }

    #[must_use]
    pub const fn length(&self) -> MachineValueId {
        self.length
    }

    #[must_use]
    pub const fn segments(&self) -> &[MachinePackSegment] {
        &self.segments
    }
}
