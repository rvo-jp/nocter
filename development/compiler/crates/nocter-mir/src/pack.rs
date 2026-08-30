use nocter_model::{MirPlaceId, MirValueId, TypeId};

use crate::{MirCallTarget, MirDestructionPlan};

/// The sole compiler-owned argument pack accepted by a callable body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirPackInput {
    element: TypeId,
    next: TypeId,
}

impl MirPackInput {
    #[must_use]
    pub const fn new(element: TypeId, next: TypeId) -> Self {
        Self { element, next }
    }

    #[must_use]
    pub const fn element(self) -> TypeId {
        self.element
    }

    #[must_use]
    pub const fn next(self) -> TypeId {
        self.next
    }
}

/// One complete caller-owned pack transferred through the hidden call lane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirPackArgument {
    element: TypeId,
    next: TypeId,
    length: MirValueId,
    segments: Box<[MirPackSegment]>,
}

/// The hidden pack lane of one call: either a newly prepared descriptor or the current input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirCallPack {
    Prepared(MirPackArgument),
    Forwarded(MirPackInput),
}

impl MirCallPack {
    #[must_use]
    pub const fn element(&self) -> TypeId {
        match self {
            Self::Prepared(pack) => pack.element(),
            Self::Forwarded(pack) => pack.element(),
        }
    }

    #[must_use]
    pub const fn next(&self) -> TypeId {
        match self {
            Self::Prepared(pack) => pack.next(),
            Self::Forwarded(pack) => pack.next(),
        }
    }

    #[must_use]
    pub const fn prepared(&self) -> Option<&MirPackArgument> {
        match self {
            Self::Prepared(pack) => Some(pack),
            Self::Forwarded(_) => None,
        }
    }
}

impl MirPackArgument {
    #[must_use]
    pub fn new(
        element: TypeId,
        next: TypeId,
        length: MirValueId,
        segments: impl Into<Box<[MirPackSegment]>>,
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
    pub const fn length(&self) -> MirValueId {
        self.length
    }

    #[must_use]
    pub const fn segments(&self) -> &[MirPackSegment] {
        &self.segments
    }
}

/// One source-ordered owner inside a transferred argument pack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirPackSegment {
    Value {
        value: MirValueId,
        destruction: Option<MirDestructionPlan>,
    },
    KeyedValue {
        key: MirValueId,
        key_destruction: Option<MirDestructionPlan>,
        value: MirValueId,
        value_destruction: Option<MirDestructionPlan>,
    },
    Spread(MirPackSpread),
}

/// How one successful iterator item becomes the sequence literal's element type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirPackContribution {
    Direct,
    CopyBorrowed,
}

/// One acquired exact-size iterator and the already selected operation used to consume it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirPackSpread {
    iterator: MirPlaceId,
    remaining: MirValueId,
    next: MirPackNext,
    contribution: MirPackContribution,
    destruction: Option<MirDestructionPlan>,
}

/// The exact selected call used to consume one acquired spread iterator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirPackNext {
    receiver: MirValueId,
    target: MirCallTarget,
    result: TypeId,
    item: TypeId,
}

impl MirPackSpread {
    #[must_use]
    pub fn new(
        iterator: MirPlaceId,
        remaining: MirValueId,
        next: MirPackNext,
        contribution: MirPackContribution,
        destruction: Option<MirDestructionPlan>,
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
    pub const fn iterator(&self) -> MirPlaceId {
        self.iterator
    }

    #[must_use]
    pub const fn remaining(&self) -> MirValueId {
        self.remaining
    }

    #[must_use]
    pub const fn receiver(&self) -> MirValueId {
        self.next.receiver()
    }

    #[must_use]
    pub const fn next_target(&self) -> &MirCallTarget {
        self.next.target()
    }

    #[must_use]
    pub const fn next_result(&self) -> TypeId {
        self.next.result()
    }

    #[must_use]
    pub const fn item(&self) -> TypeId {
        self.next.item()
    }

    #[must_use]
    pub const fn contribution(&self) -> MirPackContribution {
        self.contribution
    }

    #[must_use]
    pub const fn destruction(&self) -> Option<&MirDestructionPlan> {
        self.destruction.as_ref()
    }
}

impl MirPackNext {
    #[must_use]
    pub const fn new(
        receiver: MirValueId,
        target: MirCallTarget,
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
    pub const fn receiver(&self) -> MirValueId {
        self.receiver
    }

    #[must_use]
    pub const fn target(&self) -> &MirCallTarget {
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
