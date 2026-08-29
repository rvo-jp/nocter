use nocter_model::{BodyNodeId, BorrowCapability, ParameterId, TypeId, TypeKind};

use super::TypedIteration;

/// How one spread iterator contributes values to a typed argument pack.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SpreadMode {
    Copy,
    Borrow,
    Move,
}

impl SpreadMode {
    /// Returns the value type contributed by one yielded iterator item.
    #[must_use]
    pub fn contribution_type(
        self,
        types: &nocter_model::TypeStore,
        item: TypeId,
    ) -> Option<TypeId> {
        match self {
            Self::Copy => match types.get(item) {
                Some(TypeKind::Borrow {
                    capability: BorrowCapability::Readonly,
                    referent,
                }) => Some(*referent),
                _ => None,
            },
            Self::Borrow => matches!(
                types.get(item),
                Some(TypeKind::Borrow {
                    capability: BorrowCapability::Readonly,
                    ..
                })
            )
            .then_some(item),
            Self::Move => Some(item),
        }
    }
}

/// One source-ordered producer in a typed argument pack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArgumentPackSegment {
    Value(BodyNodeId),
    Spread {
        mode: SpreadMode,
        iteration: TypedIteration,
        exact_size: super::StaticSelection,
    },
}

/// One complete caller-owned pack prepared for a callable invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedArgumentPack {
    transport: ArgumentPackTransport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ArgumentPackTransport {
    Prepared(Box<[ArgumentPackSegment]>),
    Forwarded(ParameterId),
}

impl CheckedArgumentPack {
    pub(super) fn rebind(
        &mut self,
        semantics: &super::CheckedSemanticRebinder<'_>,
    ) -> Result<(), super::CheckedSemanticRebindError> {
        if let ArgumentPackTransport::Prepared(segments) = &mut self.transport {
            for segment in segments {
                if let ArgumentPackSegment::Spread {
                    iteration,
                    exact_size,
                    ..
                } = segment
                {
                    iteration.rebind(semantics)?;
                    exact_size.rebind(semantics)?;
                }
            }
        }
        Ok(())
    }
    pub(crate) fn new(segments: impl Into<Box<[ArgumentPackSegment]>>) -> Self {
        Self {
            transport: ArgumentPackTransport::Prepared(segments.into()),
        }
    }

    pub(crate) fn forwarded(parameter: ParameterId) -> Self {
        Self {
            transport: ArgumentPackTransport::Forwarded(parameter),
        }
    }

    #[must_use]
    pub const fn segments(&self) -> &[ArgumentPackSegment] {
        match &self.transport {
            ArgumentPackTransport::Prepared(segments) => segments,
            ArgumentPackTransport::Forwarded(_) => &[],
        }
    }

    /// Returns the current callable's incoming pack when this call tail-forwards it directly.
    #[must_use]
    pub const fn forwarded_parameter(&self) -> Option<ParameterId> {
        match &self.transport {
            ArgumentPackTransport::Prepared(_) => None,
            ArgumentPackTransport::Forwarded(parameter) => Some(*parameter),
        }
    }
}
