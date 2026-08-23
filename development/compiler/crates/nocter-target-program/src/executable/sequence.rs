use nocter_checking::AllocationSelection;
use nocter_model::{BodyNodeId, ExecutableItemId, TypeId};

use super::{ExecutablePackInput, ExecutablePackSegment};

/// One typed sequence construction whose elements use the shared argument-pack transport.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableSequencePlan {
    source: BodyNodeId,
    constructor: ExecutableItemId,
    input: ExecutablePackInput,
    result: TypeId,
    segments: Box<[ExecutablePackSegment]>,
    allocation: AllocationSelection,
}

impl ExecutableSequencePlan {
    pub(crate) fn new(
        source: BodyNodeId,
        constructor: ExecutableItemId,
        input: ExecutablePackInput,
        result: TypeId,
        segments: impl Into<Box<[ExecutablePackSegment]>>,
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
    pub const fn segments(&self) -> &[ExecutablePackSegment] {
        &self.segments
    }

    #[must_use]
    pub const fn allocation(&self) -> AllocationSelection {
        self.allocation
    }
}
