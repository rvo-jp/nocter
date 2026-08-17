use nocter_checking::{AllocationSelection, ConcreteDestructionPlan, SpreadMode, StaticSelection};
use nocter_model::{BodyNodeId, ExecutableItemId, TypeId};

use super::ExecutablePackInput;

/// One concrete spread segment retained by a sequence-literal call site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableSequenceSpread {
    mode: SpreadMode,
    iteration: ExecutableSequenceIteration,
    contribution: TypeId,
    destruction: Option<ConcreteDestructionPlan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExecutableSequenceIteration {
    iterator: BodyNodeId,
    iterator_type: TypeId,
    item: TypeId,
    next: StaticSelection,
    exact_size: StaticSelection,
}

impl ExecutableSequenceIteration {
    pub(crate) const fn new(
        iterator: BodyNodeId,
        iterator_type: TypeId,
        item: TypeId,
        next: StaticSelection,
        exact_size: StaticSelection,
    ) -> Self {
        Self {
            iterator,
            iterator_type,
            item,
            next,
            exact_size,
        }
    }
}

impl ExecutableSequenceSpread {
    pub(crate) fn new(
        mode: SpreadMode,
        iteration: ExecutableSequenceIteration,
        contribution: TypeId,
        destruction: Option<ConcreteDestructionPlan>,
    ) -> Self {
        Self {
            mode,
            iteration,
            contribution,
            destruction,
        }
    }

    #[must_use]
    pub const fn mode(&self) -> SpreadMode {
        self.mode
    }

    #[must_use]
    pub const fn iterator(&self) -> BodyNodeId {
        self.iteration.iterator
    }

    #[must_use]
    pub const fn iterator_type(&self) -> TypeId {
        self.iteration.iterator_type
    }

    #[must_use]
    pub const fn item(&self) -> TypeId {
        self.iteration.item
    }

    #[must_use]
    pub const fn contribution(&self) -> TypeId {
        self.contribution
    }

    #[must_use]
    pub const fn next(&self) -> &StaticSelection {
        &self.iteration.next
    }

    #[must_use]
    pub const fn exact_size(&self) -> &StaticSelection {
        &self.iteration.exact_size
    }

    #[must_use]
    pub const fn destruction(&self) -> Option<&ConcreteDestructionPlan> {
        self.destruction.as_ref()
    }
}

/// One source-ordered producer in a concrete sequence-literal pack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutableSequenceSegment {
    Value {
        source: BodyNodeId,
        ty: TypeId,
        destruction: Option<ConcreteDestructionPlan>,
    },
    Spread(ExecutableSequenceSpread),
}

/// One complete call-site plan for a compiler-owned sequence-literal pack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableSequencePlan {
    source: BodyNodeId,
    constructor: ExecutableItemId,
    input: ExecutablePackInput,
    result: TypeId,
    segments: Box<[ExecutableSequenceSegment]>,
    allocation: AllocationSelection,
}

impl ExecutableSequencePlan {
    pub(crate) fn new(
        source: BodyNodeId,
        constructor: ExecutableItemId,
        input: ExecutablePackInput,
        result: TypeId,
        segments: impl Into<Box<[ExecutableSequenceSegment]>>,
        allocation: AllocationSelection,
    ) -> Self {
        Self {
            source,
            constructor,
            input,
            result,
            segments: segments.into(),
            allocation,
        }
    }

    #[must_use]
    pub const fn source(&self) -> BodyNodeId {
        self.source
    }

    #[must_use]
    pub const fn constructor(&self) -> ExecutableItemId {
        self.constructor
    }

    #[must_use]
    pub const fn input(&self) -> ExecutablePackInput {
        self.input
    }

    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }

    #[must_use]
    pub const fn segments(&self) -> &[ExecutableSequenceSegment] {
        &self.segments
    }

    #[must_use]
    pub const fn allocation(&self) -> AllocationSelection {
        self.allocation
    }
}
