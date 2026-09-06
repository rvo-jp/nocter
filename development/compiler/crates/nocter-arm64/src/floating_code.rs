use crate::{
    Arm64AddSubtract, Arm64BaseRegister, Arm64BranchCondition, Arm64CodeBuilder, Arm64DataRegister,
    Arm64DataSize, Arm64FloatBinary, Arm64FloatRegister, Arm64FloatRounding, Arm64Instruction,
    Arm64MaterializationError, Arm64NocterAbi, Arm64SelectedFloatComparisonOperation,
    Arm64SelectedFloatRegister, Arm64SelectedFunction, Arm64SelectedMemoryAddress,
    Arm64SelectedRegister,
};

pub(crate) fn emit_immediate(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedFloatRegister,
    bits: u64,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let bits_register = general_scratch(0);
    crate::frame_access::load_immediate(code, bits_register, bits, size);
    let destination = write_target(function, destination)?;
    code.append(Arm64Instruction::FloatMoveFromGeneral {
        size,
        destination: destination.register,
        source: bits_register,
    });
    finish_write(destination, size, code);
    Ok(())
}

pub(crate) fn emit_move(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedFloatRegister,
    source: Arm64SelectedFloatRegister,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let source = read_register(function, source, 0, size, code)?;
    let destination = write_target(function, destination)?;
    if source != destination.register {
        code.append(Arm64Instruction::FloatMove {
            size,
            destination: destination.register,
            source,
        });
    }
    finish_write(destination, size, code);
    Ok(())
}

pub(crate) fn emit_from_bits(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedFloatRegister,
    source: Arm64SelectedRegister,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let source = crate::selected_code::read_register(function, source, 0, code)?;
    let destination = write_target(function, destination)?;
    code.append(Arm64Instruction::FloatMoveFromGeneral {
        size,
        destination: destination.register,
        source,
    });
    finish_write(destination, size, code);
    Ok(())
}

pub(crate) fn emit_to_bits(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedRegister,
    source: Arm64SelectedFloatRegister,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let source = read_register(function, source, 0, size, code)?;
    let destination = crate::selected_code::write_target(function, destination)?;
    code.append(Arm64Instruction::FloatMoveToGeneral {
        size,
        destination: destination.register,
        source,
    });
    crate::selected_code::finish_write(destination, code);
    Ok(())
}

pub(crate) fn emit_from_integer(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedFloatRegister,
    source: Arm64SelectedRegister,
    source_size: Arm64DataSize,
    target_size: Arm64DataSize,
    signed: bool,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let source = crate::selected_code::read_register(function, source, 0, code)?;
    let destination = write_target(function, destination)?;
    code.append(Arm64Instruction::FloatFromInteger {
        source_size,
        target_size,
        signed,
        destination: destination.register,
        source,
    });
    finish_write(destination, target_size, code);
    Ok(())
}

pub(crate) fn emit_widen(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedFloatRegister,
    source: Arm64SelectedFloatRegister,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let source = read_register(function, source, 0, Arm64DataSize::Bits32, code)?;
    let destination = write_target(function, destination)?;
    code.append(Arm64Instruction::FloatWiden {
        destination: destination.register,
        source,
    });
    finish_write(destination, Arm64DataSize::Bits64, code);
    Ok(())
}

pub(crate) fn emit_memory_load(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedFloatRegister,
    source: Arm64SelectedMemoryAddress,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let destination = write_target(function, destination)?;
    let (base, offset) = memory_operand(function, source, size, code)?;
    code.append(Arm64Instruction::FloatLoad {
        size,
        destination: destination.register,
        base,
        offset,
    });
    finish_write(destination, size, code);
    Ok(())
}

pub(crate) fn emit_memory_store(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedMemoryAddress,
    source: Arm64SelectedFloatRegister,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let source = read_register(function, source, 0, size, code)?;
    let (base, offset) = memory_operand(function, destination, size, code)?;
    code.append(Arm64Instruction::FloatStore {
        size,
        source,
        base,
        offset,
    });
    Ok(())
}

pub(crate) fn emit_negate(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedFloatRegister,
    operand: Arm64SelectedFloatRegister,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let operand = read_register(function, operand, 0, size, code)?;
    let destination = write_target(function, destination)?;
    code.append(Arm64Instruction::FloatNegate {
        size,
        destination: destination.register,
        source: operand,
    });
    finish_write(destination, size, code);
    Ok(())
}

pub(crate) fn emit_round(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedFloatRegister,
    source: Arm64SelectedFloatRegister,
    size: Arm64DataSize,
    operation: Arm64FloatRounding,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let source = read_register(function, source, 0, size, code)?;
    let destination = write_target(function, destination)?;
    code.append(Arm64Instruction::FloatRound {
        size,
        operation,
        destination: destination.register,
        source,
    });
    finish_write(destination, size, code);
    Ok(())
}

pub(crate) fn emit_binary(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedFloatRegister,
    left: Arm64SelectedFloatRegister,
    right: Arm64SelectedFloatRegister,
    operation: Arm64FloatBinary,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let left = read_register(function, left, 0, size, code)?;
    let right = read_register(function, right, 1, size, code)?;
    let destination = write_target(function, destination)?;
    code.append(Arm64Instruction::FloatBinary {
        size,
        operation,
        destination: destination.register,
        left,
        right,
    });
    finish_write(destination, size, code);
    Ok(())
}

pub(crate) fn emit_comparison(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedRegister,
    left: Arm64SelectedFloatRegister,
    right: Arm64SelectedFloatRegister,
    operation: Arm64SelectedFloatComparisonOperation,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let left = read_register(function, left, 0, size, code)?;
    let right = read_register(function, right, 1, size, code)?;
    emit_comparison_registers(function, destination, left, right, operation, size, code)
}

#[expect(
    clippy::too_many_arguments,
    reason = "one borrowed comparison has two addresses and one typed result"
)]
pub(crate) fn emit_borrowed_comparison(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedRegister,
    left_address: Arm64SelectedRegister,
    right_address: Arm64SelectedRegister,
    offset: u64,
    operation: Arm64SelectedFloatComparisonOperation,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let left = float_scratch(0);
    let right = float_scratch(1);
    let left_address = crate::selected_code::read_register(function, left_address, 0, code)?;
    load_from_base(left, left_address, offset, size, code);
    // Resolve the second address after the first load. A large first offset is allowed to consume
    // either general scratch register and therefore must not invalidate a pre-resolved right base.
    let right_address = crate::selected_code::read_register(function, right_address, 1, code)?;
    load_from_base(right, right_address, offset, size, code);
    emit_comparison_registers(function, destination, left, right, operation, size, code)
}

fn emit_comparison_registers(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedRegister,
    left: Arm64FloatRegister,
    right: Arm64FloatRegister,
    operation: Arm64SelectedFloatComparisonOperation,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    code.append(Arm64Instruction::FloatCompare { size, left, right });
    let destination = crate::selected_code::write_target(function, destination)?;
    code.append(Arm64Instruction::ConditionalSet {
        size: Arm64DataSize::Bits32,
        destination: destination.register,
        condition: match operation {
            Arm64SelectedFloatComparisonOperation::Equal => Arm64BranchCondition::Equal,
            // `mi` is true only for ordered less-than. Unlike `lt`, it remains false when FCMP
            // reports unordered operands through the overflow flag.
            Arm64SelectedFloatComparisonOperation::Less => Arm64BranchCondition::Minus,
        },
    });
    crate::selected_code::finish_write(destination, code);
    Ok(())
}

#[derive(Clone, Copy)]
struct WriteTarget {
    register: Arm64FloatRegister,
    spill_offset: Option<u64>,
}

fn write_target(
    function: &Arm64SelectedFunction,
    selected: Arm64SelectedFloatRegister,
) -> Result<WriteTarget, Arm64MaterializationError> {
    match selected {
        Arm64SelectedFloatRegister::Fixed(register) => Ok(WriteTarget {
            register,
            spill_offset: None,
        }),
        Arm64SelectedFloatRegister::Virtual(register) => {
            if register.class() != crate::Arm64RegisterClass::Floating {
                return Err(Arm64MaterializationError::RegisterClassMismatch(register));
            }
            match function
                .values()
                .registers()
                .location(register)
                .ok_or(Arm64MaterializationError::UnknownVirtualRegister(register))?
            {
                crate::Arm64AllocatedLocation::FloatRegister(register) => Ok(WriteTarget {
                    register,
                    spill_offset: None,
                }),
                crate::Arm64AllocatedLocation::Spill(spill) => Ok(WriteTarget {
                    register: float_scratch(0),
                    spill_offset: Some(crate::selected_code::spill_offset(function, spill)?),
                }),
                crate::Arm64AllocatedLocation::GeneralRegister(_) => {
                    Err(Arm64MaterializationError::RegisterClassMismatch(register))
                }
            }
        }
    }
}

fn finish_write(target: WriteTarget, size: Arm64DataSize, code: &mut Arm64CodeBuilder) {
    if let Some(offset) = target.spill_offset {
        store_to_stack(target.register, offset, size, code);
    }
}

fn read_register(
    function: &Arm64SelectedFunction,
    selected: Arm64SelectedFloatRegister,
    scratch: u8,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<Arm64FloatRegister, Arm64MaterializationError> {
    match selected {
        Arm64SelectedFloatRegister::Fixed(register) => Ok(register),
        Arm64SelectedFloatRegister::Virtual(register) => {
            if register.class() != crate::Arm64RegisterClass::Floating {
                return Err(Arm64MaterializationError::RegisterClassMismatch(register));
            }
            match function
                .values()
                .registers()
                .location(register)
                .ok_or(Arm64MaterializationError::UnknownVirtualRegister(register))?
            {
                crate::Arm64AllocatedLocation::FloatRegister(register) => Ok(register),
                crate::Arm64AllocatedLocation::Spill(spill) => {
                    let target = float_scratch(scratch);
                    load_from_stack(
                        target,
                        crate::selected_code::spill_offset(function, spill)?,
                        size,
                        code,
                    );
                    Ok(target)
                }
                crate::Arm64AllocatedLocation::GeneralRegister(_) => {
                    Err(Arm64MaterializationError::RegisterClassMismatch(register))
                }
            }
        }
    }
}

fn memory_operand(
    function: &Arm64SelectedFunction,
    address: Arm64SelectedMemoryAddress,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(Arm64BaseRegister, u32), Arm64MaterializationError> {
    match address {
        Arm64SelectedMemoryAddress::Stack(address) => {
            let offset = crate::selected_code::stack_offset(function, address, bytes(size))?;
            if encodable_offset(offset, size) {
                Ok((
                    Arm64BaseRegister::StackPointer,
                    u32::try_from(offset).unwrap(),
                ))
            } else {
                let scratch = general_scratch(0);
                crate::frame_access::form_stack_address(code, scratch, offset);
                Ok((Arm64BaseRegister::General(scratch), 0))
            }
        }
        Arm64SelectedMemoryAddress::Register { base, offset } => {
            let base = crate::selected_code::read_register(function, base, 0, code)?;
            Ok(register_memory_operand(base, offset, size, code))
        }
    }
}

fn load_from_base(
    destination: Arm64FloatRegister,
    base: crate::Arm64Register,
    offset: u64,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) {
    let (base, offset) = register_memory_operand(base, offset, size, code);
    code.append(Arm64Instruction::FloatLoad {
        size,
        destination,
        base,
        offset,
    });
}

fn register_memory_operand(
    base: crate::Arm64Register,
    offset: u64,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> (Arm64BaseRegister, u32) {
    if encodable_offset(offset, size) {
        return (
            Arm64BaseRegister::General(base),
            u32::try_from(offset).expect("bounded floating offset fits u32"),
        );
    }
    let address = if base == general_scratch(0) {
        general_scratch(1)
    } else {
        general_scratch(0)
    };
    crate::frame_access::load_immediate(code, address, offset, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64DataRegister::General(address),
        left: Arm64DataRegister::General(base),
        right: Arm64DataRegister::General(address),
    });
    (Arm64BaseRegister::General(address), 0)
}

pub(crate) fn load_from_stack(
    destination: Arm64FloatRegister,
    offset: u64,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) {
    let (base, offset) = stack_memory_operand(offset, size, code);
    code.append(Arm64Instruction::FloatLoad {
        size,
        destination,
        base,
        offset,
    });
}

pub(crate) fn store_to_stack(
    source: Arm64FloatRegister,
    offset: u64,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) {
    let (base, offset) = stack_memory_operand(offset, size, code);
    code.append(Arm64Instruction::FloatStore {
        size,
        source,
        base,
        offset,
    });
}

fn stack_memory_operand(
    offset: u64,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> (Arm64BaseRegister, u32) {
    if encodable_offset(offset, size) {
        return (
            Arm64BaseRegister::StackPointer,
            u32::try_from(offset).expect("bounded floating stack offset fits u32"),
        );
    }
    let scratch = general_scratch(0);
    crate::frame_access::form_stack_address(code, scratch, offset);
    (Arm64BaseRegister::General(scratch), 0)
}

const fn encodable_offset(offset: u64, size: Arm64DataSize) -> bool {
    let bytes = bytes(size);
    offset <= 0x0fff * bytes && offset.is_multiple_of(bytes)
}

const fn bytes(size: Arm64DataSize) -> u64 {
    match size {
        Arm64DataSize::Bits32 => 4,
        Arm64DataSize::Bits64 => 8,
    }
}

fn general_scratch(index: u8) -> crate::Arm64Register {
    Arm64NocterAbi::compiler_scratch_register(index)
        .expect("the ABI reserves two general materialization registers")
}

fn float_scratch(index: u8) -> Arm64FloatRegister {
    Arm64NocterAbi::floating_scratch_register(index)
        .expect("the ABI reserves two floating materialization registers")
}
