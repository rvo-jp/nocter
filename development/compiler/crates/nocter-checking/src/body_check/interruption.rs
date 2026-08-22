use nocter_model::{BodyId, BorrowCapability, TypeId};
use nocter_source_index::SourceOrigin;

use crate::ConstructionCompletionOwner;

/// One compiler-typed fact at the exact source operation that stopped body construction.
///
/// This value is not a partially successful checked node. It records only facts that were fixed
/// before the rule failure and can therefore be consumed by tooling without inventing semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypedBodyInterruption {
    body: BodyId,
    origin: SourceOrigin,
    kind: TypedBodyInterruptionKind,
}

impl TypedBodyInterruption {
    pub(crate) const fn new(
        body: BodyId,
        origin: SourceOrigin,
        kind: TypedBodyInterruptionKind,
    ) -> Self {
        Self { body, origin, kind }
    }

    #[must_use]
    pub const fn body(self) -> BodyId {
        self.body
    }

    #[must_use]
    pub const fn origin(self) -> SourceOrigin {
        self.origin
    }

    #[must_use]
    pub const fn kind(self) -> TypedBodyInterruptionKind {
        self.kind
    }
}

/// Phase-owned typed contexts that remain valid when a body rule rejects source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypedBodyInterruptionKind {
    MemberSelection {
        receiver: TypeId,
        available: BorrowCapability,
        owned: bool,
    },
    ConstructionSelection {
        owner: ConstructionCompletionOwner,
    },
}
