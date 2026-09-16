use nocter_model::{BodyNodeId, ClosureId};

/// One checked conversion from a concrete closure environment into an owning erased callable.
///
/// The containing checked node owns the erased target type. This value retains the exact source
/// closure identity so executable closure never has to recover a witness from type shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedCallableErasure {
    value: BodyNodeId,
    closure: ClosureId,
}

impl CheckedCallableErasure {
    #[must_use]
    pub const fn new(value: BodyNodeId, closure: ClosureId) -> Self {
        Self { value, closure }
    }

    #[must_use]
    pub const fn value(self) -> BodyNodeId {
        self.value
    }

    #[must_use]
    pub const fn closure(self) -> ClosureId {
        self.closure
    }

    pub(super) fn rebind(
        &mut self,
        semantics: &super::CheckedSemanticRebinder<'_>,
    ) -> Result<(), super::CheckedSemanticRebindError> {
        self.closure = semantics.closure(self.closure)?;
        Ok(())
    }
}
