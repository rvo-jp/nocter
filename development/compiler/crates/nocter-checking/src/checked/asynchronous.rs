use nocter_model::{BodyNodeId, TypeId};

/// Invocation behavior selected while the call is checked.
///
/// This is intentionally stored on the checked call. Later analyses must not rediscover deferred
/// execution from a result type, because an immediate generic callable may return an existing
/// `async T` value after substitution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedCallExecution {
    Immediate,
    Deferred { output: TypeId },
}

impl CheckedCallExecution {
    pub(super) fn rebind(
        &mut self,
        semantics: &super::CheckedSemanticRebinder<'_>,
    ) -> Result<(), super::CheckedSemanticRebindError> {
        if let Self::Deferred { output } = self {
            *output = semantics.ty(*output)?;
        }
        Ok(())
    }
}

/// One suspension whose operand ownership and output type were decided during body checking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedAwait {
    computation: BodyNodeId,
}

impl CheckedAwait {
    pub(crate) const fn new(computation: BodyNodeId) -> Self {
        Self { computation }
    }

    #[must_use]
    pub const fn computation(self) -> BodyNodeId {
        self.computation
    }
}
