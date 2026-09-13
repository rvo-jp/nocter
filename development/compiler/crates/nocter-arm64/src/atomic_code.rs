use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DataRegister, Arm64DataSize, Arm64Instruction,
    Arm64LabelId, Arm64MoveWide, Arm64Register,
};

/// Non-aliasing scratch registers for one generated acquire/release state transition.
///
/// Exclusive load overwrites `observed`, and exclusive store overwrites `status`. Keeping both
/// distinct from the address is part of the operation contract rather than a convention imposed
/// on every generated-service caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64AtomicUpdateRegisters {
    address: Arm64Register,
    observed: Arm64Register,
    status: Arm64Register,
}

impl Arm64AtomicUpdateRegisters {
    #[must_use]
    pub const fn new(
        address: Arm64Register,
        observed: Arm64Register,
        status: Arm64Register,
    ) -> Option<Self> {
        if address.number() == observed.number()
            || address.number() == status.number()
            || observed.number() == status.number()
        {
            None
        } else {
            Some(Self {
                address,
                observed,
                status,
            })
        }
    }

    #[must_use]
    pub const fn address(self) -> Arm64Register {
        self.address
    }

    #[must_use]
    pub const fn observed(self) -> Arm64Register {
        self.observed
    }

    #[must_use]
    pub const fn status(self) -> Arm64Register {
        self.status
    }
}

impl Arm64CodeBuilder {
    /// Atomically replaces one 64-bit state word when it equals `expected`.
    ///
    /// The successful path has acquire/release ordering. A mismatched observation clears the
    /// exclusive reservation before branching. Lost reservations retry internally; callers see
    /// only an exact success or mismatch outcome.
    ///
    /// # Errors
    ///
    /// Propagates an internal label-binding failure. Immediate encoding limits are checked when
    /// the completed code builder is finished.
    pub fn append_atomic_state_transition(
        &mut self,
        registers: Arm64AtomicUpdateRegisters,
        expected: u16,
        replacement: u16,
        success: Arm64LabelId,
        mismatch: Arm64LabelId,
    ) -> Result<(), Arm64CodeError> {
        let retry = self.create_label();
        let observed_mismatch = self.create_label();
        self.bind(retry)?;
        self.append(Arm64Instruction::LoadAcquireExclusive {
            size: Arm64DataSize::Bits64,
            destination: Arm64DataRegister::General(registers.observed),
            base: Arm64BaseRegister::General(registers.address),
        });
        self.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Subtract,
            set_flags: true,
            destination: Arm64AddSubtractDestination::Zero,
            source: Arm64BaseRegister::General(registers.observed),
            immediate: expected,
            shift_12: false,
        });
        self.branch_conditional(observed_mismatch, Arm64BranchCondition::NotEqual);
        self.append(Arm64Instruction::MoveWide {
            size: Arm64DataSize::Bits64,
            operation: Arm64MoveWide::Zero,
            destination: registers.observed,
            immediate: replacement,
            shift: 0,
        });
        self.append(Arm64Instruction::StoreReleaseExclusive {
            size: Arm64DataSize::Bits64,
            status: registers.status,
            source: Arm64DataRegister::General(registers.observed),
            base: Arm64BaseRegister::General(registers.address),
        });
        self.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits32,
            operation: Arm64AddSubtract::Subtract,
            set_flags: true,
            destination: Arm64AddSubtractDestination::Zero,
            source: Arm64BaseRegister::General(registers.status),
            immediate: 0,
            shift_12: false,
        });
        self.branch_conditional(retry, Arm64BranchCondition::NotEqual);
        self.branch(success, false);
        self.bind(observed_mismatch)?;
        self.append(Arm64Instruction::ClearExclusive);
        self.branch(mismatch, false);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Arm64AtomicUpdateRegisters;
    use crate::{Arm64CodeBuilder, Arm64Instruction, Arm64Register};

    fn x(number: u8) -> Arm64Register {
        Arm64Register::new(number).unwrap()
    }

    #[test]
    fn atomic_update_registers_reject_every_alias() {
        assert_eq!(Arm64AtomicUpdateRegisters::new(x(0), x(0), x(1)), None);
        assert_eq!(Arm64AtomicUpdateRegisters::new(x(0), x(1), x(0)), None);
        assert_eq!(Arm64AtomicUpdateRegisters::new(x(0), x(1), x(1)), None);
        assert!(Arm64AtomicUpdateRegisters::new(x(0), x(1), x(2)).is_some());
    }

    #[test]
    fn state_transition_retries_lost_reservations_and_clears_mismatch() {
        let mut code = Arm64CodeBuilder::new();
        let success = code.create_label();
        let mismatch = code.create_label();
        code.append_atomic_state_transition(
            Arm64AtomicUpdateRegisters::new(x(0), x(1), x(2)).unwrap(),
            1,
            3,
            success,
            mismatch,
        )
        .unwrap();
        code.bind(success).unwrap();
        code.append(Arm64Instruction::NoOperation);
        code.bind(mismatch).unwrap();
        code.append(Arm64Instruction::NoOperation);
        let code = code.finish().unwrap();
        let words = code
            .bytes()
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();

        assert_eq!(words[0], 0xc85f_fc01);
        assert_eq!(words[4], 0xc802_fc01);
        assert_eq!(words[6], 0x54ff_ff41);
        assert_eq!(words[8], 0xd503_3f5f);
        assert_eq!(code.label_offset(success), Some(40));
        assert_eq!(code.label_offset(mismatch), Some(44));
    }
}
