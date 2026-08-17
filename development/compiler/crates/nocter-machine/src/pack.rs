use nocter_model::TypeId;

use crate::{MachineAddressId, MachineCallTarget, MachineDestructionPlan, MachineValueId};

/// How a successful spread item becomes one pack element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachinePackContribution {
    Direct,
    CopyBorrowed,
}

/// The already selected call used to consume one item from a spread iterator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachinePackNext {
    receiver: MachineValueId,
    target: MachineCallTarget,
    result: TypeId,
    item: TypeId,
}

impl MachinePackNext {
    pub(crate) const fn new(
        receiver: MachineValueId,
        target: MachineCallTarget,
        result: TypeId,
        item: TypeId,
    ) -> Self {
        Self {
            receiver,
            target,
            result,
            item,
        }
    }

    #[must_use]
    pub const fn receiver(&self) -> MachineValueId {
        self.receiver
    }

    #[must_use]
    pub const fn target(&self) -> &MachineCallTarget {
        &self.target
    }

    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }

    #[must_use]
    pub const fn item(&self) -> TypeId {
        self.item
    }
}

/// One acquired exact-size iterator retained by a transferred pack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachinePackSpread {
    iterator: MachineAddressId,
    remaining: MachineValueId,
    next: MachinePackNext,
    contribution: MachinePackContribution,
    destruction: Option<MachineDestructionPlan>,
}

impl MachinePackSpread {
    pub(crate) const fn new(
        iterator: MachineAddressId,
        remaining: MachineValueId,
        next: MachinePackNext,
        contribution: MachinePackContribution,
        destruction: Option<MachineDestructionPlan>,
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
    pub const fn destruction(&self) -> Option<&MachineDestructionPlan> {
        self.destruction.as_ref()
    }
}

/// One source-ordered owner inside a pack descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachinePackSegment {
    Value {
        value: MachineValueId,
        destruction: Option<MachineDestructionPlan>,
    },
    Spread(MachinePackSpread),
}

/// One caller-owned literal descriptor transferred through the hidden pack pointer lane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachinePack {
    element: TypeId,
    next: TypeId,
    length: MachineValueId,
    segments: Box<[MachinePackSegment]>,
}

impl MachinePack {
    pub(crate) fn new(
        element: TypeId,
        next: TypeId,
        length: MachineValueId,
        segments: impl Into<Box<[MachinePackSegment]>>,
    ) -> Self {
        Self {
            element,
            next,
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
    pub const fn length(&self) -> MachineValueId {
        self.length
    }

    #[must_use]
    pub const fn segments(&self) -> &[MachinePackSegment] {
        &self.segments
    }
}
