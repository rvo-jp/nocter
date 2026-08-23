use nocter_checking::{ConcreteDestructionPlan, SpreadMode, StaticSelection};
use nocter_model::{BodyNodeId, TypeId};

use super::ExecutablePackInput;

/// One concrete spread segment retained by an argument-pack call site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutablePackSpread {
    mode: SpreadMode,
    iteration: ExecutablePackIteration,
    contribution: TypeId,
    destruction: Option<ConcreteDestructionPlan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExecutablePackIteration {
    iterator: BodyNodeId,
    iterator_type: TypeId,
    item: TypeId,
    next: StaticSelection,
    exact_size: StaticSelection,
}

impl ExecutablePackIteration {
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

impl ExecutablePackSpread {
    pub(crate) fn new(
        mode: SpreadMode,
        iteration: ExecutablePackIteration,
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

/// One source-ordered producer in a concrete argument pack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutablePackSegment {
    Value {
        source: BodyNodeId,
        ty: TypeId,
        destruction: Option<ConcreteDestructionPlan>,
    },
    Spread(ExecutablePackSpread),
}

/// One fully specialized caller-owned argument pack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableArgumentPackPlan {
    source: BodyNodeId,
    input: ExecutablePackInput,
    transport: ExecutableArgumentPackTransport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExecutableArgumentPackTransport {
    Prepared(Box<[ExecutablePackSegment]>),
    Forwarded,
}

impl ExecutableArgumentPackPlan {
    pub(crate) fn new(
        source: BodyNodeId,
        input: ExecutablePackInput,
        segments: impl Into<Box<[ExecutablePackSegment]>>,
    ) -> Self {
        Self {
            source,
            input,
            transport: ExecutableArgumentPackTransport::Prepared(segments.into()),
        }
    }

    pub(crate) fn forwarded(source: BodyNodeId, input: ExecutablePackInput) -> Self {
        Self {
            source,
            input,
            transport: ExecutableArgumentPackTransport::Forwarded,
        }
    }

    #[must_use]
    pub const fn source(&self) -> BodyNodeId {
        self.source
    }

    #[must_use]
    pub const fn input(&self) -> ExecutablePackInput {
        self.input
    }

    #[must_use]
    pub const fn segments(&self) -> &[ExecutablePackSegment] {
        match &self.transport {
            ExecutableArgumentPackTransport::Prepared(segments) => segments,
            ExecutableArgumentPackTransport::Forwarded => &[],
        }
    }

    /// Returns whether the caller's incoming descriptor is passed through without adaptation.
    #[must_use]
    pub const fn is_forwarded(&self) -> bool {
        matches!(&self.transport, ExecutableArgumentPackTransport::Forwarded)
    }
}
