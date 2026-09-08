use std::fmt;

use crate::{
    Arm64AddSubtract, Arm64AsyncFunctionPlan, Arm64BranchCondition, Arm64Code, Arm64CodeBuilder,
    Arm64CodeError, Arm64DataRegister, Arm64DataSize, Arm64FunctionTarget, Arm64Instruction,
    Arm64LoadStoreSize, Arm64NocterAbi, Arm64Register,
};

pub(crate) fn materialize(
    plan: &Arm64AsyncFunctionPlan,
    target: Arm64FunctionTarget,
) -> Result<Arm64Code, Arm64AsyncConsumeError> {
    validate_target(plan, target)?;
    let schema = Arm64NocterAbi::asynchronous();
    let frame = abi_register(schema.frame_argument_register())?;
    let output = abi_register(schema.output_argument_register())?;
    if frame == output {
        return Err(Arm64AsyncConsumeError::RegisterContract);
    }
    let mut code = Arm64CodeBuilder::new();
    emit_completed_check(plan, frame, &mut code)?;
    if let Some(stored) = plan.frame().output() {
        for (offset, bytes) in crate::memory_code::exact_memory_chunks(stored.size()) {
            let transfer = scratch(0)?;
            let size = load_store_size(bytes)?;
            crate::address_code::load_native(
                &mut code,
                size,
                None,
                transfer,
                frame,
                checked_add(stored.offset(), offset)?,
            );
            crate::address_code::store_native(&mut code, size, transfer, output, offset);
        }
    }
    crate::address_code::move_register(&mut code, frame, abi_register(0)?);
    crate::frame_access::load_immediate(
        &mut code,
        abi_register(1)?,
        plan.frame().size(),
        Arm64DataSize::Bits64,
    );
    crate::darwin_memory_code::emit_unmap(
        &mut code,
        crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameReleaseFailure,
    )?;
    code.append(Arm64Instruction::Return {
        target: Arm64NocterAbi::link_register(),
    });
    code.finish().map_err(Arm64AsyncConsumeError::Code)
}

fn validate_target(
    plan: &Arm64AsyncFunctionPlan,
    target: Arm64FunctionTarget,
) -> Result<(), Arm64AsyncConsumeError> {
    if target.owner() != plan.owner() {
        return Err(Arm64AsyncConsumeError::ForeignTarget {
            expected: plan.owner(),
            actual: target.owner(),
        });
    }
    target
        .asynchronous()
        .ok_or(Arm64AsyncConsumeError::ImmediateTarget(plan.owner()))?;
    Ok(())
}

fn emit_completed_check(
    plan: &Arm64AsyncFunctionPlan,
    frame: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConsumeError> {
    let actual = scratch(0)?;
    let expected = scratch(1)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        actual,
        frame,
        plan.frame().state_tag().offset(),
    );
    crate::frame_access::load_immediate(
        code,
        expected,
        plan.frame().completed_tag(),
        Arm64DataSize::Bits64,
    );
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(actual),
        right: Arm64DataRegister::General(expected),
    });
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::Equal);
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameStateCorruption.immediate(),
    });
    code.bind(valid)?;
    Ok(())
}

fn load_store_size(bytes: u8) -> Result<Arm64LoadStoreSize, Arm64AsyncConsumeError> {
    match bytes {
        1 => Ok(Arm64LoadStoreSize::Byte),
        2 => Ok(Arm64LoadStoreSize::Half),
        4 => Ok(Arm64LoadStoreSize::Word),
        8 => Ok(Arm64LoadStoreSize::Double),
        _ => Err(Arm64AsyncConsumeError::InvalidMemoryWidth(bytes)),
    }
}

fn checked_add(left: u64, right: u64) -> Result<u64, Arm64AsyncConsumeError> {
    left.checked_add(right)
        .ok_or(Arm64AsyncConsumeError::OffsetOverflow)
}

fn abi_register(index: u8) -> Result<Arm64Register, Arm64AsyncConsumeError> {
    Arm64NocterAbi::argument_register(index).ok_or(Arm64AsyncConsumeError::RegisterOverflow)
}

fn scratch(index: u8) -> Result<Arm64Register, Arm64AsyncConsumeError> {
    Arm64NocterAbi::compiler_scratch_register(index).ok_or(Arm64AsyncConsumeError::RegisterOverflow)
}

#[derive(Debug)]
pub enum Arm64AsyncConsumeError {
    ForeignTarget {
        expected: nocter_machine::MachineFunctionId,
        actual: nocter_machine::MachineFunctionId,
    },
    ImmediateTarget(nocter_machine::MachineFunctionId),
    RegisterOverflow,
    RegisterContract,
    InvalidMemoryWidth(u8),
    OffsetOverflow,
    Code(Arm64CodeError),
}

impl fmt::Display for Arm64AsyncConsumeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 async consume emission failed: {self:?}")
    }
}

impl std::error::Error for Arm64AsyncConsumeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Code(error) => Some(error),
            Self::ForeignTarget { .. }
            | Self::ImmediateTarget(_)
            | Self::RegisterOverflow
            | Self::RegisterContract
            | Self::InvalidMemoryWidth(_)
            | Self::OffsetOverflow => None,
        }
    }
}

impl From<Arm64CodeError> for Arm64AsyncConsumeError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}
