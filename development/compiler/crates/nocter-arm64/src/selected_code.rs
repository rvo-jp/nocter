use std::fmt;

use nocter_machine::{MachineBlockId, MachineDataId, MachineFunctionId};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64AllocatedLocation, Arm64BaseRegister,
    Arm64BranchCondition, Arm64Code, Arm64CodeBuilder, Arm64CodeError, Arm64DataRegister,
    Arm64DataSize, Arm64FrameCode, Arm64Instruction, Arm64LoadStoreSize, Arm64NocterAbi,
    Arm64Register, Arm64SelectedBinaryOperation, Arm64SelectedComparisonOperation,
    Arm64SelectedEdge, Arm64SelectedFunction, Arm64SelectedInstruction, Arm64SelectedLoadExtension,
    Arm64SelectedRegister, Arm64SelectedStackAddress, Arm64SelectedTerminator,
    Arm64SelectedUnaryOperation, Arm64Shift,
};

impl Arm64SelectedFunction {
    /// Materializes physical instructions and spill traffic after selection and frame placement.
    ///
    /// # Errors
    ///
    /// Rejects missing function/block mappings, invalid virtual locations, frame-address overflow,
    /// malformed object offsets, and concrete code-encoding failures.
    pub fn materialize(
        &self,
        functions: &[(MachineFunctionId, crate::Arm64FunctionId)],
        data: &[(MachineDataId, crate::Arm64DataId)],
        pack_callbacks: &[(crate::Arm64PackCallbackKey, crate::Arm64FunctionId)],
    ) -> Result<Arm64Code, Arm64MaterializationError> {
        let mut code = Arm64CodeBuilder::new();
        let labels = self
            .blocks()
            .map(|(block, _)| (block, code.create_label()))
            .collect::<Vec<_>>();
        Arm64FrameCode::emit_prologue(self.frame().layout(), &mut code);
        let context = InstructionMaterialization {
            function: self,
            functions,
            data,
            pack_callbacks,
        };
        for instruction in self.entry_instructions() {
            emit_instruction(context, instruction, &mut code)?;
        }
        code.branch(block_label(&labels, self.entry())?, false);
        for (block_id, block) in self.blocks() {
            code.bind(block_label(&labels, block_id)?)?;
            for instruction in block.instructions() {
                emit_instruction(context, instruction, &mut code)?;
            }
            emit_terminator(self, block.terminator(), &labels, &mut code)?;
        }
        code.finish().map_err(Arm64MaterializationError::Code)
    }
}

#[derive(Clone, Copy)]
struct InstructionMaterialization<'selected> {
    function: &'selected Arm64SelectedFunction,
    functions: &'selected [(MachineFunctionId, crate::Arm64FunctionId)],
    data: &'selected [(MachineDataId, crate::Arm64DataId)],
    pack_callbacks: &'selected [(crate::Arm64PackCallbackKey, crate::Arm64FunctionId)],
}

fn emit_instruction(
    context: InstructionMaterialization<'_>,
    instruction: &Arm64SelectedInstruction,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let function = context.function;
    match *instruction {
        Arm64SelectedInstruction::LoadImmediate {
            size,
            destination,
            value,
        } => emit_immediate(function, destination, value, size, code),
        Arm64SelectedInstruction::LoadDataAddress {
            destination,
            source,
        } => crate::memory_code::emit_data_address(
            function,
            destination,
            data_target(context.data, source)?,
            code,
        ),
        Arm64SelectedInstruction::LoadPackCallbackAddress {
            destination,
            pack,
            kind,
        } => emit_pack_callback_address(context, destination, pack, kind, code),
        Arm64SelectedInstruction::Move {
            size,
            destination,
            source,
        } => emit_move(function, destination, source, size, code),
        Arm64SelectedInstruction::LoadMemory {
            bytes,
            extension,
            destination,
            source,
        } => crate::memory_code::emit_memory_load(
            function,
            bytes,
            extension,
            destination,
            source,
            code,
        ),
        Arm64SelectedInstruction::StoreMemory {
            bytes,
            destination,
            source,
        } => crate::memory_code::emit_memory_store(function, bytes, destination, source, code),
        Arm64SelectedInstruction::ZeroStack { destination, bytes } => {
            crate::memory_code::emit_stack_zero(function, destination, bytes, code)
        }
        Arm64SelectedInstruction::CopyMemoryNonOverlapping { .. }
        | Arm64SelectedInstruction::CopyMemoryNonOverlappingDynamic { .. } => {
            crate::memory_code::emit_selected_copy(function, instruction, code)
        }
        Arm64SelectedInstruction::ResolveAddress(address) => {
            emit_resolved_address(function, address, code)
        }
        Arm64SelectedInstruction::IndexAddress {
            destination,
            index,
            domain,
        } => crate::address_code::emit_index_address(function, destination, index, domain, code),
        Arm64SelectedInstruction::MemoryAddress {
            destination,
            source,
        } => crate::memory_code::emit_memory_address(function, destination, source, code),
        Arm64SelectedInstruction::Unary {
            size,
            operation,
            destination,
            operand,
        } => emit_unary(function, operation, destination, operand, size, code),
        Arm64SelectedInstruction::Binary {
            size,
            operation,
            destination,
            left,
            right,
        } => emit_binary(function, operation, destination, left, right, size, code),
        Arm64SelectedInstruction::DarwinSystemCall { .. }
        | Arm64SelectedInstruction::ExitProcess { .. }
        | Arm64SelectedInstruction::Break { .. } => {
            crate::system_primitive_code::emit_selected(function, instruction, code)
        }
        Arm64SelectedInstruction::CompareBorrowed { .. } => {
            emit_selected_borrowed_comparison(function, instruction, code)
        }
        Arm64SelectedInstruction::Call(target) => {
            function_target(context.functions, target).map(|target| code.call(target))
        }
        Arm64SelectedInstruction::CallRegister(target) => {
            emit_indirect_call(function, target, code)
        }
    }
}

fn emit_selected_borrowed_comparison(
    function: &Arm64SelectedFunction,
    instruction: &Arm64SelectedInstruction,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let Arm64SelectedInstruction::CompareBorrowed {
        size,
        extension,
        operation,
        offset,
        destination,
        left,
        right,
    } = *instruction
    else {
        unreachable!("borrowed comparison emitter receives a borrowed comparison")
    };
    emit_borrowed_comparison(
        function,
        BorrowedComparisonMaterialization {
            load_size: size,
            extension,
            operation,
            offset,
            destination,
            left_address: left,
            right_address: right,
        },
        code,
    )
}

fn emit_pack_callback_address(
    context: InstructionMaterialization<'_>,
    destination: Arm64SelectedRegister,
    pack: nocter_machine::MachinePackId,
    kind: crate::Arm64PackCallbackKind,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let destination = write_target(context.function, destination)?;
    code.load_function_address(
        pack_callback_target(
            context.pack_callbacks,
            crate::Arm64PackCallbackKey::new(context.function.owner(), pack, kind),
        )?,
        destination.register,
    );
    finish_write(destination, code);
    Ok(())
}

fn emit_indirect_call(
    function: &Arm64SelectedFunction,
    target: Arm64SelectedRegister,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let target = read_register(function, target, 0, code)?;
    code.append(Arm64Instruction::BranchRegister { target, link: true });
    Ok(())
}

fn emit_resolved_address(
    function: &Arm64SelectedFunction,
    address: nocter_machine::MachineAddressId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let calculation = function
        .addresses()
        .calculation(address)
        .ok_or(Arm64MaterializationError::UnknownSelectedAddress(address))?;
    crate::address_code::emit_resolve(function, calculation, code)
}

fn emit_unary(
    function: &Arm64SelectedFunction,
    operation: Arm64SelectedUnaryOperation,
    destination: Arm64SelectedRegister,
    operand: Arm64SelectedRegister,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let operand = read_register(function, operand, 0, code)?;
    let destination = write_target(function, destination)?;
    match operation {
        Arm64SelectedUnaryOperation::LogicalNot => {
            code.append(Arm64Instruction::AddSubtractImmediate {
                size,
                operation: Arm64AddSubtract::Subtract,
                set_flags: true,
                destination: Arm64AddSubtractDestination::Zero,
                source: Arm64BaseRegister::General(operand),
                immediate: 0,
                shift_12: false,
            });
            code.append(Arm64Instruction::ConditionalSet {
                size: Arm64DataSize::Bits32,
                destination: destination.register,
                condition: Arm64BranchCondition::Equal,
            });
        }
        Arm64SelectedUnaryOperation::Negate => {
            code.append(Arm64Instruction::AddSubtractRegister {
                size,
                operation: Arm64AddSubtract::Subtract,
                set_flags: false,
                destination: Arm64DataRegister::General(destination.register),
                left: Arm64DataRegister::Zero,
                right: Arm64DataRegister::General(operand),
            });
        }
    }
    finish_write(destination, code);
    Ok(())
}

fn emit_binary(
    function: &Arm64SelectedFunction,
    operation: Arm64SelectedBinaryOperation,
    destination: Arm64SelectedRegister,
    left: Arm64SelectedRegister,
    right: Arm64SelectedRegister,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let left = read_register(function, left, 0, code)?;
    let right = read_register(function, right, 1, code)?;
    let destination = write_target(function, destination)?;
    match operation {
        Arm64SelectedBinaryOperation::Add | Arm64SelectedBinaryOperation::Subtract => {
            code.append(Arm64Instruction::AddSubtractRegister {
                size,
                operation: if operation == Arm64SelectedBinaryOperation::Add {
                    Arm64AddSubtract::Add
                } else {
                    Arm64AddSubtract::Subtract
                },
                set_flags: false,
                destination: Arm64DataRegister::General(destination.register),
                left: Arm64DataRegister::General(left),
                right: Arm64DataRegister::General(right),
            });
        }
        Arm64SelectedBinaryOperation::Multiply => {
            code.append(Arm64Instruction::MultiplyAdd {
                size,
                destination: destination.register,
                left,
                right,
                addend: Arm64DataRegister::Zero,
                subtract_product: false,
            });
        }
        Arm64SelectedBinaryOperation::Divide { signed } => {
            code.append(Arm64Instruction::Divide {
                size,
                destination: destination.register,
                left,
                right,
                signed,
            });
        }
        Arm64SelectedBinaryOperation::Remainder { signed } => {
            emit_remainder(code, size, destination.register, left, right, signed);
        }
        Arm64SelectedBinaryOperation::ShiftLeft
        | Arm64SelectedBinaryOperation::ShiftRight { .. } => {
            let operation = match operation {
                Arm64SelectedBinaryOperation::ShiftLeft => Arm64Shift::Left,
                Arm64SelectedBinaryOperation::ShiftRight { signed: true } => {
                    Arm64Shift::RightArithmetic
                }
                Arm64SelectedBinaryOperation::ShiftRight { signed: false } => {
                    Arm64Shift::RightLogical
                }
                _ => unreachable!(),
            };
            code.append(Arm64Instruction::VariableShift {
                size,
                operation,
                destination: destination.register,
                value: left,
                amount: right,
            });
        }
        Arm64SelectedBinaryOperation::Equal | Arm64SelectedBinaryOperation::Less { .. } => {
            emit_comparison(code, size, destination.register, left, right, operation);
        }
    }
    finish_write(destination, code);
    Ok(())
}

fn emit_remainder(
    code: &mut Arm64CodeBuilder,
    size: Arm64DataSize,
    destination: Arm64Register,
    left: Arm64Register,
    right: Arm64Register,
    signed: bool,
) {
    let quotient = Arm64NocterAbi::argument_register(0)
        .expect("the boundary-only x0 register is available between calls");
    code.append(Arm64Instruction::Divide {
        size,
        destination: quotient,
        left,
        right,
        signed,
    });
    code.append(Arm64Instruction::MultiplyAdd {
        size,
        destination,
        left: quotient,
        right,
        addend: Arm64DataRegister::General(left),
        subtract_product: true,
    });
}

fn emit_comparison(
    code: &mut Arm64CodeBuilder,
    size: Arm64DataSize,
    destination: Arm64Register,
    left: Arm64Register,
    right: Arm64Register,
    operation: Arm64SelectedBinaryOperation,
) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(left),
        right: Arm64DataRegister::General(right),
    });
    let condition = match operation {
        Arm64SelectedBinaryOperation::Equal => Arm64BranchCondition::Equal,
        Arm64SelectedBinaryOperation::Less { signed: true } => Arm64BranchCondition::SignedLess,
        Arm64SelectedBinaryOperation::Less { signed: false } => Arm64BranchCondition::CarryClear,
        _ => unreachable!(),
    };
    code.append(Arm64Instruction::ConditionalSet {
        size: Arm64DataSize::Bits32,
        destination,
        condition,
    });
}

#[derive(Clone, Copy)]
struct BorrowedComparisonMaterialization {
    load_size: Arm64LoadStoreSize,
    extension: Arm64SelectedLoadExtension,
    operation: Arm64SelectedComparisonOperation,
    offset: u64,
    destination: Arm64SelectedRegister,
    left_address: Arm64SelectedRegister,
    right_address: Arm64SelectedRegister,
}

fn emit_borrowed_comparison(
    function: &Arm64SelectedFunction,
    comparison: BorrowedComparisonMaterialization,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let left_address = read_register(function, comparison.left_address, 0, code)?;
    let right_address = read_register(function, comparison.right_address, 1, code)?;
    let left = Arm64NocterAbi::argument_register(0)
        .expect("the boundary-only x0 register is available between calls");
    let right = Arm64NocterAbi::argument_register(1)
        .expect("the boundary-only x1 register is available between calls");
    load_from_address(
        code,
        comparison.load_size,
        comparison.extension,
        left,
        left_address,
        comparison.offset,
    );
    load_from_address(
        code,
        comparison.load_size,
        comparison.extension,
        right,
        right_address,
        comparison.offset,
    );
    let data_size = match comparison.extension {
        Arm64SelectedLoadExtension::Sign(size) => size,
        Arm64SelectedLoadExtension::Zero => match comparison.load_size {
            Arm64LoadStoreSize::Double => Arm64DataSize::Bits64,
            Arm64LoadStoreSize::Byte | Arm64LoadStoreSize::Half | Arm64LoadStoreSize::Word => {
                Arm64DataSize::Bits32
            }
        },
    };
    let destination = write_target(function, comparison.destination)?;
    emit_comparison(
        code,
        data_size,
        destination.register,
        left,
        right,
        match comparison.operation {
            Arm64SelectedComparisonOperation::Equal => Arm64SelectedBinaryOperation::Equal,
            Arm64SelectedComparisonOperation::Less { signed } => {
                Arm64SelectedBinaryOperation::Less { signed }
            }
        },
    );
    finish_write(destination, code);
    Ok(())
}

fn load_from_address(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    extension: Arm64SelectedLoadExtension,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u64,
) {
    let bytes = load_store_bytes(size);
    let (base, offset) = if offset <= 0x0fff * bytes && offset.is_multiple_of(bytes) {
        (
            Arm64BaseRegister::General(base),
            u32::try_from(offset).expect("scaled address offset is bounded"),
        )
    } else {
        crate::frame_access::load_immediate(code, destination, offset, Arm64DataSize::Bits64);
        code.append(Arm64Instruction::AddSubtractRegister {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: Arm64DataRegister::General(destination),
            left: Arm64DataRegister::General(base),
            right: Arm64DataRegister::General(destination),
        });
        (Arm64BaseRegister::General(destination), 0)
    };
    code.append(match extension {
        Arm64SelectedLoadExtension::Zero => Arm64Instruction::LoadUnsigned {
            size,
            destination: Arm64DataRegister::General(destination),
            base,
            offset,
        },
        Arm64SelectedLoadExtension::Sign(destination_size) => Arm64Instruction::LoadSigned {
            size,
            destination_size,
            destination: Arm64DataRegister::General(destination),
            base,
            offset,
        },
    });
}

fn emit_terminator(
    function: &Arm64SelectedFunction,
    terminator: &Arm64SelectedTerminator,
    labels: &[(MachineBlockId, crate::Arm64LabelId)],
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    match terminator {
        Arm64SelectedTerminator::Goto(edge) => {
            emit_edge(function, edge, labels, code)?;
        }
        Arm64SelectedTerminator::Branch {
            condition,
            then_edge,
            else_edge,
        } => {
            let condition = read_register(function, *condition, 0, code)?;
            code.append(Arm64Instruction::AddSubtractImmediate {
                size: Arm64DataSize::Bits32,
                operation: Arm64AddSubtract::Subtract,
                set_flags: true,
                destination: Arm64AddSubtractDestination::Zero,
                source: Arm64BaseRegister::General(condition),
                immediate: 0,
                shift_12: false,
            });
            let then_copy_label = then_edge.has_copies().then(|| code.create_label());
            let then_target = if let Some(copy_label) = then_copy_label {
                copy_label
            } else {
                block_label(labels, then_edge.target())?
            };
            code.branch_conditional(then_target, Arm64BranchCondition::NotEqual);
            emit_edge(function, else_edge, labels, code)?;
            if let Some(copy_label) = then_copy_label {
                code.bind(copy_label)?;
                emit_edge(function, then_edge, labels, code)?;
            }
        }
        Arm64SelectedTerminator::Switch {
            subject,
            cases,
            fallback,
        } => crate::switch_code::emit(function, subject, cases, fallback, labels, code)?,
        Arm64SelectedTerminator::Return => {
            Arm64FrameCode::emit_epilogue(function.frame().layout(), code);
        }
        Arm64SelectedTerminator::Exit(status) => {
            crate::system_primitive_code::emit_exit(function, *status, code)?;
        }
        Arm64SelectedTerminator::Trap => {
            code.append(Arm64Instruction::Break {
                immediate: crate::runtime_trap::Arm64RuntimeTrap::MirTrap.immediate(),
            });
        }
        Arm64SelectedTerminator::Unreachable => {
            code.append(Arm64Instruction::Break {
                immediate: crate::runtime_trap::Arm64RuntimeTrap::MirUnreachable.immediate(),
            });
        }
    }
    Ok(())
}

pub(crate) fn emit_edge(
    function: &Arm64SelectedFunction,
    edge: &Arm64SelectedEdge,
    labels: &[(MachineBlockId, crate::Arm64LabelId)],
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::memory_parallel_copy::emit(function, edge.memory_copies(), code)?;
    crate::parallel_copy::emit(function, edge.copies(), code)?;
    code.branch(block_label(labels, edge.target())?, false);
    Ok(())
}

fn emit_immediate(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedRegister,
    value: u64,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let destination = write_target(function, destination)?;
    crate::frame_access::load_immediate(code, destination.register, value, size);
    finish_write(destination, code);
    Ok(())
}

fn emit_move(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedRegister,
    source: Arm64SelectedRegister,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let source = read_register(function, source, 0, code)?;
    let destination = write_target(function, destination)?;
    if source != destination.register {
        code.append(Arm64Instruction::AddSubtractImmediate {
            size,
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: Arm64AddSubtractDestination::General(destination.register),
            source: Arm64BaseRegister::General(source),
            immediate: 0,
            shift_12: false,
        });
    }
    finish_write(destination, code);
    Ok(())
}

#[derive(Clone, Copy)]
pub(crate) struct WriteTarget {
    pub(crate) register: Arm64Register,
    pub(crate) spill_offset: Option<u64>,
}

pub(crate) fn write_target(
    function: &Arm64SelectedFunction,
    selected: Arm64SelectedRegister,
) -> Result<WriteTarget, Arm64MaterializationError> {
    match selected {
        Arm64SelectedRegister::Fixed(register) => Ok(WriteTarget {
            register,
            spill_offset: None,
        }),
        Arm64SelectedRegister::Virtual(register) => {
            match function
                .values()
                .registers()
                .location(register)
                .ok_or(Arm64MaterializationError::UnknownVirtualRegister(register))?
            {
                Arm64AllocatedLocation::Register(register) => Ok(WriteTarget {
                    register,
                    spill_offset: None,
                }),
                Arm64AllocatedLocation::Spill(spill) => Ok(WriteTarget {
                    register: crate::frame_access::scratch(0),
                    spill_offset: Some(spill_offset(function, spill)?),
                }),
            }
        }
    }
}

pub(crate) fn finish_write(target: WriteTarget, code: &mut Arm64CodeBuilder) {
    if let Some(offset) = target.spill_offset {
        crate::frame_access::store_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            target.register,
            offset,
        );
    }
}

pub(crate) fn read_register(
    function: &Arm64SelectedFunction,
    selected: Arm64SelectedRegister,
    scratch: u8,
    code: &mut Arm64CodeBuilder,
) -> Result<Arm64Register, Arm64MaterializationError> {
    match selected {
        Arm64SelectedRegister::Fixed(register) => Ok(register),
        Arm64SelectedRegister::Virtual(register) => {
            match function
                .values()
                .registers()
                .location(register)
                .ok_or(Arm64MaterializationError::UnknownVirtualRegister(register))?
            {
                Arm64AllocatedLocation::Register(register) => Ok(register),
                Arm64AllocatedLocation::Spill(spill) => {
                    let register = crate::frame_access::scratch(scratch);
                    crate::frame_access::load_at_stack_offset(
                        code,
                        Arm64LoadStoreSize::Double,
                        register,
                        spill_offset(function, spill)?,
                    );
                    Ok(register)
                }
            }
        }
    }
}

fn spill_offset(
    function: &Arm64SelectedFunction,
    spill: crate::Arm64SpillSlotId,
) -> Result<u64, Arm64MaterializationError> {
    let object = function
        .frame()
        .spill(spill)
        .ok_or(Arm64MaterializationError::UnknownSpill(spill))?;
    function
        .frame()
        .layout()
        .object(object)
        .map(crate::Arm64FrameObject::offset)
        .ok_or(Arm64MaterializationError::UnknownFrameObject(object))
}

pub(crate) fn stack_offset(
    function: &Arm64SelectedFunction,
    address: Arm64SelectedStackAddress,
    access_size: u64,
) -> Result<u64, Arm64MaterializationError> {
    match address {
        Arm64SelectedStackAddress::FrameObject { object, offset } => {
            let object_layout = function
                .frame()
                .layout()
                .object(object)
                .ok_or(Arm64MaterializationError::UnknownFrameObject(object))?;
            let end = offset
                .checked_add(access_size)
                .ok_or(Arm64MaterializationError::OffsetOverflow)?;
            if end > object_layout.size() {
                return Err(Arm64MaterializationError::FrameObjectBounds(object));
            }
            object_layout
                .offset()
                .checked_add(offset)
                .ok_or(Arm64MaterializationError::OffsetOverflow)
        }
        Arm64SelectedStackAddress::Outgoing(offset) => {
            let end = offset
                .checked_add(access_size)
                .ok_or(Arm64MaterializationError::OffsetOverflow)?;
            if end > function.frame().layout().outgoing_argument_size() {
                return Err(Arm64MaterializationError::OutgoingBounds(offset));
            }
            Ok(offset)
        }
        Arm64SelectedStackAddress::Incoming(offset) => function
            .frame()
            .layout()
            .size()
            .checked_add(offset)
            .ok_or(Arm64MaterializationError::OffsetOverflow),
    }
}

pub(crate) fn block_label(
    labels: &[(MachineBlockId, crate::Arm64LabelId)],
    block: MachineBlockId,
) -> Result<crate::Arm64LabelId, Arm64MaterializationError> {
    labels
        .get(block.index())
        .and_then(|(actual, label)| (*actual == block).then_some(*label))
        .ok_or(Arm64MaterializationError::UnknownBlock(block))
}

fn function_target(
    functions: &[(MachineFunctionId, crate::Arm64FunctionId)],
    target: MachineFunctionId,
) -> Result<crate::Arm64FunctionId, Arm64MaterializationError> {
    functions
        .get(target.index())
        .and_then(|(actual, selected)| (*actual == target).then_some(*selected))
        .ok_or(Arm64MaterializationError::UnknownFunction(target))
}

fn pack_callback_target(
    callbacks: &[(crate::Arm64PackCallbackKey, crate::Arm64FunctionId)],
    source: crate::Arm64PackCallbackKey,
) -> Result<crate::Arm64FunctionId, Arm64MaterializationError> {
    callbacks
        .binary_search_by_key(&source, |(key, _)| *key)
        .ok()
        .and_then(|index| callbacks.get(index).map(|(_, target)| *target))
        .ok_or(Arm64MaterializationError::UnknownPackCallback(source))
}

fn data_target(
    data: &[(MachineDataId, crate::Arm64DataId)],
    source: MachineDataId,
) -> Result<crate::Arm64DataId, Arm64MaterializationError> {
    data.get(source.index())
        .and_then(|(actual, target)| (*actual == source).then_some(*target))
        .ok_or(Arm64MaterializationError::UnknownData(source))
}

const fn load_store_bytes(size: Arm64LoadStoreSize) -> u64 {
    match size {
        Arm64LoadStoreSize::Byte => 1,
        Arm64LoadStoreSize::Half => 2,
        Arm64LoadStoreSize::Word => 4,
        Arm64LoadStoreSize::Double => 8,
    }
}

#[derive(Debug)]
pub enum Arm64MaterializationError {
    UnknownFunction(MachineFunctionId),
    UnknownPackCallback(crate::Arm64PackCallbackKey),
    InvalidPackCallback(crate::Arm64PackCallbackKey),
    UnknownData(MachineDataId),
    UnknownBlock(MachineBlockId),
    UnknownSelectedAddress(nocter_machine::MachineAddressId),
    UnknownVirtualRegister(crate::Arm64VirtualRegister),
    UnknownSpill(crate::Arm64SpillSlotId),
    UnknownFrameObject(crate::Arm64FrameObjectId),
    FrameObjectBounds(crate::Arm64FrameObjectId),
    OutgoingBounds(u64),
    OffsetOverflow,
    InvalidMemoryWidth(u8),
    OverlappingStackCopy,
    InvalidParallelCopy,
    MissingMemoryEdgeStaging,
    InvalidSystemCallArity(u8),
    InvalidSwitchWidth(usize),
    PackCallbackFrame(crate::Arm64FrameLayoutError),
    Code(Arm64CodeError),
}

impl fmt::Display for Arm64MaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 materialization failed: {self:?}")
    }
}

impl std::error::Error for Arm64MaterializationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Code(error) => Some(error),
            Self::PackCallbackFrame(error) => Some(error),
            _ => None,
        }
    }
}

impl From<crate::Arm64FrameLayoutError> for Arm64MaterializationError {
    fn from(error: crate::Arm64FrameLayoutError) -> Self {
        Self::PackCallbackFrame(error)
    }
}

impl From<Arm64CodeError> for Arm64MaterializationError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}
