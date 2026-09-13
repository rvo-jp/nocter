use std::fmt;

use nocter_runtime_contract::{
    DarwinFileJobAction, DarwinFileJobEvent, DarwinFileJobState, DarwinFileRetirementAction,
    DarwinFileRetirementEvent, DarwinFileRetirementState,
};

use crate::{Arm64AtomicUpdateRegisters, Arm64CodeBuilder, Arm64CodeError, Arm64LabelId};

/// Emits one validated file-job ownership transition and returns its inseparable cleanup action.
///
/// Both the replacement state and the required action come from the runtime-contract transition;
/// generated operation code cannot independently choose either half.
///
/// # Errors
///
/// Rejects a transition absent from the closed lifecycle or a tag not representable by the
/// backend's atomic-state instruction sequence, and propagates code construction failure.
pub fn emit_darwin_file_job_transition(
    code: &mut Arm64CodeBuilder,
    registers: Arm64AtomicUpdateRegisters,
    state: DarwinFileJobState,
    event: DarwinFileJobEvent,
    success: Arm64LabelId,
    mismatch: Arm64LabelId,
) -> Result<DarwinFileJobAction, Arm64DarwinFileLifecycleError> {
    let transition = state
        .apply(event)
        .ok_or(Arm64DarwinFileLifecycleError::InvalidJobTransition { state, event })?;
    emit_transition(
        code,
        registers,
        state.code(),
        transition.next().code(),
        success,
        mismatch,
    )?;
    Ok(transition.action())
}

/// Emits one validated file-retirement ownership transition and returns its cleanup action.
///
/// # Errors
///
/// Rejects a transition absent from the closed lifecycle or a tag not representable by the
/// backend's atomic-state instruction sequence, and propagates code construction failure.
pub fn emit_darwin_file_retirement_transition(
    code: &mut Arm64CodeBuilder,
    registers: Arm64AtomicUpdateRegisters,
    state: DarwinFileRetirementState,
    event: DarwinFileRetirementEvent,
    success: Arm64LabelId,
    mismatch: Arm64LabelId,
) -> Result<DarwinFileRetirementAction, Arm64DarwinFileLifecycleError> {
    let transition = state
        .apply(event)
        .ok_or(Arm64DarwinFileLifecycleError::InvalidRetirementTransition { state, event })?;
    emit_transition(
        code,
        registers,
        state.code(),
        transition.next().code(),
        success,
        mismatch,
    )?;
    Ok(transition.action())
}

fn emit_transition(
    code: &mut Arm64CodeBuilder,
    registers: Arm64AtomicUpdateRegisters,
    expected: u64,
    replacement: u64,
    success: Arm64LabelId,
    mismatch: Arm64LabelId,
) -> Result<(), Arm64DarwinFileLifecycleError> {
    let expected = u16::try_from(expected)
        .map_err(|_| Arm64DarwinFileLifecycleError::StateTagOutOfRange(expected))?;
    let replacement = u16::try_from(replacement)
        .map_err(|_| Arm64DarwinFileLifecycleError::StateTagOutOfRange(replacement))?;
    code.append_atomic_state_transition(registers, expected, replacement, success, mismatch)?;
    Ok(())
}

#[derive(Debug)]
pub enum Arm64DarwinFileLifecycleError {
    InvalidJobTransition {
        state: DarwinFileJobState,
        event: DarwinFileJobEvent,
    },
    InvalidRetirementTransition {
        state: DarwinFileRetirementState,
        event: DarwinFileRetirementEvent,
    },
    StateTagOutOfRange(u64),
    Code(Arm64CodeError),
}

impl fmt::Display for Arm64DarwinFileLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ARM64 Darwin file lifecycle construction failed: {self:?}"
        )
    }
}

impl std::error::Error for Arm64DarwinFileLifecycleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Code(error) => Some(error),
            Self::InvalidJobTransition { .. }
            | Self::InvalidRetirementTransition { .. }
            | Self::StateTagOutOfRange(_) => None,
        }
    }
}

impl From<Arm64CodeError> for Arm64DarwinFileLifecycleError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

#[cfg(test)]
mod tests {
    use nocter_runtime_contract::{
        DarwinFileJobAction, DarwinFileJobEvent, DarwinFileJobState, DarwinFileRetirementAction,
        DarwinFileRetirementEvent, DarwinFileRetirementState,
    };

    use super::{
        Arm64DarwinFileLifecycleError, emit_darwin_file_job_transition,
        emit_darwin_file_retirement_transition,
    };
    use crate::{Arm64AtomicUpdateRegisters, Arm64CodeBuilder, Arm64Instruction, Arm64Register};

    fn x(number: u8) -> Arm64Register {
        Arm64Register::new(number).unwrap()
    }

    fn registers() -> Arm64AtomicUpdateRegisters {
        Arm64AtomicUpdateRegisters::new(x(0), x(1), x(2)).unwrap()
    }

    #[test]
    fn job_transition_couples_runtime_state_and_cleanup_action() {
        let mut code = Arm64CodeBuilder::new();
        let success = code.create_label();
        let mismatch = code.create_label();
        let action = emit_darwin_file_job_transition(
            &mut code,
            registers(),
            DarwinFileJobState::RunningDetached,
            DarwinFileJobEvent::Publish,
            success,
            mismatch,
        )
        .unwrap();
        assert_eq!(action, DarwinFileJobAction::ReleaseDetachedCompletion);
        code.bind(success).unwrap();
        code.append(Arm64Instruction::NoOperation);
        code.bind(mismatch).unwrap();
        code.append(Arm64Instruction::NoOperation);
        assert!(code.finish().is_ok());
    }

    #[test]
    fn retirement_transition_uses_the_same_atomic_boundary() {
        let mut code = Arm64CodeBuilder::new();
        let success = code.create_label();
        let mismatch = code.create_label();
        let action = emit_darwin_file_retirement_transition(
            &mut code,
            registers(),
            DarwinFileRetirementState::Live,
            DarwinFileRetirementEvent::DropOwner,
            success,
            mismatch,
        )
        .unwrap();
        assert_eq!(action, DarwinFileRetirementAction::EnqueueDetachedClose);
    }

    #[test]
    fn invalid_transition_emits_no_partial_code() {
        let mut code = Arm64CodeBuilder::new();
        let success = code.create_label();
        let mismatch = code.create_label();
        assert!(matches!(
            emit_darwin_file_job_transition(
                &mut code,
                registers(),
                DarwinFileJobState::Released,
                DarwinFileJobEvent::Publish,
                success,
                mismatch,
            ),
            Err(Arm64DarwinFileLifecycleError::InvalidJobTransition { .. })
        ));
        code.bind(success).unwrap();
        code.bind(mismatch).unwrap();
        assert_eq!(code.finish().unwrap().instruction_count(), 0);
    }
}
