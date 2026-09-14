use nocter_model::{BodyNodeId, TypeId};

/// Invocation behavior selected while the call is checked.
///
/// This is intentionally stored on the checked call. Later analyses must not rediscover deferred
/// execution from a result type, because an immediate generic callable may return an existing
/// `future T` value after substitution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedCallExecution {
    Immediate,
    Deferred { output: TypeId },
}

impl CheckedCallExecution {
    /// Returns the result produced in the execution scope selected for the call target.
    ///
    /// Immediate calls execute their target while producing `invocation_result`. Deferred calls
    /// construct that outer result during invocation and produce `output` only when driven.
    #[must_use]
    pub const fn executed_result(self, invocation_result: TypeId) -> TypeId {
        match self {
            Self::Immediate => invocation_result,
            Self::Deferred { output } => output,
        }
    }

    #[must_use]
    pub const fn is_deferred(self) -> bool {
        matches!(self, Self::Deferred { .. })
    }

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
