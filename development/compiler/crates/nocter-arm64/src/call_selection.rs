use nocter_machine::{
    MachineArgumentLocation, MachineBlockId, MachineCall, MachineCallTarget, MachineFunctionId,
    MachineFunctionKind, MachineOperationId, MachineResultAbi, MachineResultLocation,
    MachineValueClass, MachineValueId,
};

use crate::{
    Arm64DataSize, Arm64FunctionFrame, Arm64NocterAbi, Arm64SelectedInstruction,
    Arm64SelectedLoadExtension, Arm64SelectedMemoryAddress, Arm64SelectedRegister,
    Arm64SelectedStackAddress, Arm64SelectionContext, Arm64SelectionError, Arm64ValuePlan,
    Arm64ValueStorage,
};

pub(crate) fn select_parameters(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    function: &nocter_machine::MachineFunction,
    frame: &Arm64FunctionFrame,
) -> Result<Box<[Arm64SelectedInstruction]>, Arm64SelectionError> {
    let mut selected = Vec::new();
    // The platform entry registers are ordinary argument registers. Capture them before root
    // allocation initialization or any future hidden-context initializer can use those registers
    // as materialization scratch.
    crate::process_selection::select_entry(program, owner, frame, &mut selected)?;
    crate::allocation_selection::select_entry(program, owner, frame, &mut selected)?;
    let MachineFunctionKind::Callable(abi) = function.kind() else {
        return if function.body().parameters().is_empty() {
            Ok(selected.into_boxed_slice())
        } else {
            Err(Arm64SelectionError::Parameters(function.linkage()))
        };
    };
    if function.body().parameters().len() != abi.arguments().len() {
        return Err(Arm64SelectionError::Parameters(function.linkage()));
    }
    select_incoming_result_pointer(function.linkage(), abi, frame, &mut selected)?;
    select_incoming_pack_pointer(function.linkage(), abi, frame, &mut selected)?;
    // Persist every register-carried input before materializing any indirect value. Large address
    // calculations may use argument registers as late scratch lanes, but never before all incoming
    // register values have crossed into callee-owned frame objects.
    for (stack, argument) in function
        .body()
        .parameters()
        .iter()
        .copied()
        .zip(abi.arguments())
    {
        select_register_parameter(function, frame, stack, argument, &mut selected)?;
    }
    for (stack, argument) in function
        .body()
        .parameters()
        .iter()
        .copied()
        .zip(abi.arguments())
    {
        match (argument.class(), argument.location()) {
            (MachineValueClass::Zero, None) => {}
            (
                MachineValueClass::Direct { words },
                Some(MachineArgumentLocation::Registers(registers)),
            ) if words == registers.words() => {}
            (MachineValueClass::Direct { words }, Some(MachineArgumentLocation::Stack(slot))) => {
                select_direct_stack_parameter(function, frame, stack, words, slot, &mut selected)?;
            }
            (MachineValueClass::Indirect, Some(location)) => {
                select_indirect_parameter(function, frame, stack, location, &mut selected)?;
            }
            _ => return Err(Arm64SelectionError::ParameterTransport(function.linkage())),
        }
    }
    Ok(selected.into_boxed_slice())
}

fn select_register_parameter(
    function: &nocter_machine::MachineFunction,
    frame: &Arm64FunctionFrame,
    stack: nocter_machine::MachineStackId,
    argument: &nocter_machine::MachineArgumentAbi,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    match (argument.class(), argument.location()) {
        (
            MachineValueClass::Direct { words },
            Some(MachineArgumentLocation::Registers(registers)),
        ) if words == registers.words() => {
            let sizes = crate::memory_selection::parameter_lane_sizes(function, stack, words)?;
            for (lane, bytes) in sizes.into_iter().enumerate() {
                selected.push(Arm64SelectedInstruction::StoreMemory {
                    bytes,
                    destination: Arm64SelectedMemoryAddress::Stack(
                        crate::memory_selection::frame_stack(
                            frame,
                            stack,
                            crate::memory_selection::lane_offset(lane)?,
                        )?,
                    ),
                    source: Arm64SelectedRegister::Fixed(argument_register(
                        registers.first(),
                        lane,
                    )?),
                });
            }
        }
        (MachineValueClass::Indirect, Some(MachineArgumentLocation::Registers(registers)))
            if registers.words() == 1 =>
        {
            selected.push(Arm64SelectedInstruction::StoreMemory {
                bytes: word_bytes(),
                destination: Arm64SelectedMemoryAddress::Stack(
                    crate::memory_selection::frame_stack(frame, stack, 0)?,
                ),
                source: Arm64SelectedRegister::Fixed(abi_register(registers.first())?),
            });
        }
        (MachineValueClass::Zero, None)
        | (
            MachineValueClass::Direct { .. } | MachineValueClass::Indirect,
            Some(MachineArgumentLocation::Stack(_)),
        ) => {}
        _ => return Err(Arm64SelectionError::ParameterTransport(function.linkage())),
    }
    Ok(())
}

pub(crate) fn select_call(
    context: Arm64SelectionContext<'_>,
    operation: MachineOperationId,
    call: &MachineCall,
    result: Option<MachineValueId>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let abi = match call.target() {
        MachineCallTarget::Direct(target) => {
            let target_function = context
                .program()
                .function(*target)
                .ok_or(Arm64SelectionError::UnknownFunction(*target))?;
            let MachineFunctionKind::Callable(abi) = target_function.kind() else {
                return Err(Arm64SelectionError::NonCallableTarget(*target));
            };
            abi
        }
        MachineCallTarget::Primitive(target) => target.abi(),
    };
    crate::pack_selection::select_call_pack(context, operation, call, abi, selected)?;
    crate::allocation_selection::select_call(
        context.program(),
        operation,
        call,
        context.frame(),
        context.addresses(),
        selected,
    )?;
    crate::process_selection::select_target(
        context.program(),
        operation,
        call.target(),
        context.frame(),
        selected,
    )?;
    select_call_arguments(
        operation,
        call,
        abi,
        context.values(),
        context.frame(),
        selected,
    )?;
    select_call_result_storage(
        operation,
        abi.result(),
        result,
        context.values(),
        context.frame(),
        selected,
    )?;
    match call.target() {
        MachineCallTarget::Direct(target) => {
            selected.push(Arm64SelectedInstruction::Call(*target));
        }
        MachineCallTarget::Primitive(target) => {
            crate::primitive_selection::select(
                context.program(),
                context.frame(),
                operation,
                target,
                selected,
            )?;
        }
    }
    select_call_result(operation, abi.result(), result, context.values(), selected)
}

pub(crate) fn select_return(
    function: &nocter_machine::MachineFunction,
    block: MachineBlockId,
    value: Option<MachineValueId>,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let MachineFunctionKind::Callable(abi) = function.kind() else {
        return Err(Arm64SelectionError::RootReturn(block));
    };
    match (abi.result(), value) {
        (MachineResultAbi::Completion, None) | (MachineResultAbi::Diverging, _) => Ok(()),
        (MachineResultAbi::Value(returned), Some(value)) => match returned.location() {
            MachineResultLocation::Omitted => Ok(()),
            MachineResultLocation::Registers(span) => {
                let sources = crate::selection::direct_value(values, value)?;
                if usize::from(span.words()) != sources.len() {
                    return Err(Arm64SelectionError::ReturnTransport(block));
                }
                for (lane, source) in sources.iter().copied().enumerate() {
                    selected.push(Arm64SelectedInstruction::Move {
                        size: Arm64DataSize::Bits64,
                        destination: Arm64SelectedRegister::Fixed(argument_register(
                            span.first(),
                            lane,
                        )?),
                        source: Arm64SelectedRegister::Virtual(source),
                    });
                }
                Ok(())
            }
            MachineResultLocation::CallerStorage { pointer_register } => {
                select_indirect_return(block, value, pointer_register, values, frame, selected)
            }
        },
        _ => Err(Arm64SelectionError::ReturnTransport(block)),
    }
}

fn select_incoming_result_pointer(
    owner: nocter_machine::MachineLinkageId,
    abi: &nocter_machine::MachineCallableAbi,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let MachineResultAbi::Value(returned) = abi.result() else {
        return Ok(());
    };
    let MachineResultLocation::CallerStorage { pointer_register } = returned.location() else {
        return Ok(());
    };
    if abi_register(pointer_register)? != Arm64NocterAbi::indirect_result_register() {
        return Err(Arm64SelectionError::ResultAbi(owner));
    }
    let object = frame
        .indirect_result_pointer()
        .ok_or(Arm64SelectionError::MissingIndirectResultPointer)?;
    selected.push(Arm64SelectedInstruction::StoreMemory {
        bytes: word_bytes(),
        destination: Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::FrameObject {
            object,
            offset: 0,
        }),
        source: Arm64SelectedRegister::Fixed(abi_register(pointer_register)?),
    });
    Ok(())
}

fn select_direct_stack_parameter(
    function: &nocter_machine::MachineFunction,
    frame: &Arm64FunctionFrame,
    stack: nocter_machine::MachineStackId,
    words: u8,
    slot: nocter_machine::MachineStackSlot,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let sizes = crate::memory_selection::parameter_lane_sizes(function, stack, words)?;
    let transport_size = u64::from(words)
        .checked_mul(Arm64NocterAbi::word_size())
        .ok_or(Arm64SelectionError::AddressOverflow)?;
    if slot.size() < transport_size {
        return Err(Arm64SelectionError::ParameterTransport(function.linkage()));
    }
    for (lane, bytes) in sizes.into_iter().enumerate() {
        let offset = crate::memory_selection::lane_offset(lane)?;
        selected.push(Arm64SelectedInstruction::LoadMemory {
            bytes,
            extension: Arm64SelectedLoadExtension::Zero,
            destination: Arm64SelectedRegister::Fixed(scratch_boundary()),
            source: Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::Incoming(
                slot.offset()
                    .checked_add(offset)
                    .ok_or(Arm64SelectionError::AddressOverflow)?,
            )),
        });
        selected.push(Arm64SelectedInstruction::StoreMemory {
            bytes,
            destination: Arm64SelectedMemoryAddress::Stack(crate::memory_selection::frame_stack(
                frame, stack, offset,
            )?),
            source: Arm64SelectedRegister::Fixed(scratch_boundary()),
        });
    }
    Ok(())
}

fn select_indirect_parameter(
    function: &nocter_machine::MachineFunction,
    frame: &Arm64FunctionFrame,
    stack: nocter_machine::MachineStackId,
    location: MachineArgumentLocation,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let size = function
        .body()
        .stack(stack)
        .map(nocter_machine::MachineStackObject::size)
        .ok_or(Arm64SelectionError::UnknownStack(stack))?;
    let pointer = match location {
        MachineArgumentLocation::Registers(registers) if registers.words() == 1 => {
            let pointer = Arm64SelectedRegister::Fixed(scratch_boundary());
            selected.push(Arm64SelectedInstruction::LoadMemory {
                bytes: word_bytes(),
                extension: Arm64SelectedLoadExtension::Zero,
                destination: pointer,
                source: Arm64SelectedMemoryAddress::Stack(crate::memory_selection::frame_stack(
                    frame, stack, 0,
                )?),
            });
            pointer
        }
        MachineArgumentLocation::Stack(slot) if slot.size() >= Arm64NocterAbi::word_size() => {
            let pointer = Arm64SelectedRegister::Fixed(scratch_boundary());
            selected.push(Arm64SelectedInstruction::LoadMemory {
                bytes: word_bytes(),
                extension: Arm64SelectedLoadExtension::Zero,
                destination: pointer,
                source: Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::Incoming(
                    slot.offset(),
                )),
            });
            pointer
        }
        _ => return Err(Arm64SelectionError::ParameterTransport(function.linkage())),
    };
    selected.push(Arm64SelectedInstruction::CopyMemoryNonOverlapping {
        destination: Arm64SelectedMemoryAddress::Stack(crate::memory_selection::frame_stack(
            frame, stack, 0,
        )?),
        source: Arm64SelectedMemoryAddress::Register {
            base: pointer,
            offset: 0,
        },
        bytes: size,
    });
    Ok(())
}

fn select_call_arguments(
    operation: MachineOperationId,
    call: &MachineCall,
    abi: &nocter_machine::MachineCallableAbi,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if call.arguments().len() != abi.arguments().len() {
        return Err(Arm64SelectionError::CallArguments(operation));
    }
    for (value, argument) in call.arguments().iter().copied().zip(abi.arguments()) {
        match (argument.class(), argument.location()) {
            (MachineValueClass::Zero, None) => {}
            (
                MachineValueClass::Direct { words },
                Some(MachineArgumentLocation::Registers(registers)),
            ) if words == registers.words() => {
                select_direct_register_argument(
                    operation, value, words, registers, values, selected,
                )?;
            }
            (MachineValueClass::Direct { words }, Some(MachineArgumentLocation::Stack(slot))) => {
                select_direct_stack_argument(operation, value, words, slot, values, selected)?;
            }
            (MachineValueClass::Indirect, Some(location)) => {
                select_indirect_argument(operation, value, location, values, frame, selected)?;
            }
            _ => return Err(Arm64SelectionError::CallArguments(operation)),
        }
    }
    Ok(())
}

fn select_direct_register_argument(
    operation: MachineOperationId,
    value: MachineValueId,
    words: u8,
    registers: nocter_machine::MachineRegisterSpan,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let sources = crate::selection::direct_value(values, value)?;
    if sources.len() != usize::from(words) {
        return Err(Arm64SelectionError::CallArguments(operation));
    }
    for (lane, source) in sources.iter().copied().enumerate() {
        selected.push(Arm64SelectedInstruction::Move {
            size: Arm64DataSize::Bits64,
            destination: Arm64SelectedRegister::Fixed(argument_register(registers.first(), lane)?),
            source: Arm64SelectedRegister::Virtual(source),
        });
    }
    Ok(())
}

fn select_direct_stack_argument(
    operation: MachineOperationId,
    value: MachineValueId,
    words: u8,
    slot: nocter_machine::MachineStackSlot,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let sources = crate::selection::direct_value(values, value)?;
    let transport_size = u64::from(words)
        .checked_mul(Arm64NocterAbi::word_size())
        .ok_or(Arm64SelectionError::AddressOverflow)?;
    if sources.len() != usize::from(words) || slot.size() < transport_size {
        return Err(Arm64SelectionError::CallArguments(operation));
    }
    for (lane, source) in sources.iter().copied().enumerate() {
        selected.push(Arm64SelectedInstruction::StoreMemory {
            bytes: word_bytes(),
            destination: Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::Outgoing(
                slot.offset()
                    .checked_add(crate::memory_selection::lane_offset(lane)?)
                    .ok_or(Arm64SelectionError::AddressOverflow)?,
            )),
            source: Arm64SelectedRegister::Virtual(source),
        });
    }
    Ok(())
}

fn select_indirect_argument(
    operation: MachineOperationId,
    value: MachineValueId,
    location: MachineArgumentLocation,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let Arm64ValueStorage::Memory { .. } = values
        .value(value)
        .ok_or(Arm64SelectionError::UnknownValue(value))?
    else {
        return Err(Arm64SelectionError::CallArguments(operation));
    };
    let object = frame
        .memory_value(value)
        .ok_or(Arm64SelectionError::MemoryValue(value))?;
    let source = Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::FrameObject {
        object,
        offset: 0,
    });
    match location {
        MachineArgumentLocation::Registers(registers) if registers.words() == 1 => {
            selected.push(Arm64SelectedInstruction::MemoryAddress {
                destination: Arm64SelectedRegister::Fixed(abi_register(registers.first())?),
                source,
            });
        }
        MachineArgumentLocation::Stack(slot) if slot.size() >= Arm64NocterAbi::word_size() => {
            let pointer = Arm64SelectedRegister::Fixed(scratch_boundary());
            selected.push(Arm64SelectedInstruction::MemoryAddress {
                destination: pointer,
                source,
            });
            selected.push(Arm64SelectedInstruction::StoreMemory {
                bytes: word_bytes(),
                destination: Arm64SelectedMemoryAddress::Stack(
                    Arm64SelectedStackAddress::Outgoing(slot.offset()),
                ),
                source: pointer,
            });
        }
        _ => return Err(Arm64SelectionError::CallArguments(operation)),
    }
    Ok(())
}

pub(crate) fn select_call_result_storage(
    operation: MachineOperationId,
    abi: MachineResultAbi,
    result: Option<MachineValueId>,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let MachineResultAbi::Value(returned) = abi else {
        return Ok(());
    };
    let MachineResultLocation::CallerStorage { pointer_register } = returned.location() else {
        return Ok(());
    };
    if abi_register(pointer_register)? != Arm64NocterAbi::indirect_result_register() {
        return Err(Arm64SelectionError::ResultTransport(operation));
    }
    let result = result.ok_or(Arm64SelectionError::MissingResult(operation))?;
    let object = frame
        .memory_value(result)
        .ok_or(Arm64SelectionError::MemoryValue(result))?;
    if !matches!(values.value(result), Some(Arm64ValueStorage::Memory { .. })) {
        return Err(Arm64SelectionError::ResultTransport(operation));
    }
    selected.push(Arm64SelectedInstruction::MemoryAddress {
        destination: Arm64SelectedRegister::Fixed(abi_register(pointer_register)?),
        source: Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::FrameObject {
            object,
            offset: 0,
        }),
    });
    Ok(())
}

pub(crate) fn select_call_result(
    operation: MachineOperationId,
    abi: MachineResultAbi,
    result: Option<MachineValueId>,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    match (abi, result) {
        (MachineResultAbi::Completion | MachineResultAbi::Diverging, _) => Ok(()),
        (MachineResultAbi::Value(returned), Some(result)) => match returned.location() {
            MachineResultLocation::Omitted | MachineResultLocation::CallerStorage { .. } => Ok(()),
            MachineResultLocation::Registers(span) => {
                let destinations = crate::selection::direct_value(values, result)?;
                if usize::from(span.words()) != destinations.len() {
                    return Err(Arm64SelectionError::ResultTransport(operation));
                }
                for (lane, destination) in destinations.iter().copied().enumerate() {
                    selected.push(Arm64SelectedInstruction::Move {
                        size: Arm64DataSize::Bits64,
                        destination: Arm64SelectedRegister::Virtual(destination),
                        source: Arm64SelectedRegister::Fixed(argument_register(
                            span.first(),
                            lane,
                        )?),
                    });
                }
                Ok(())
            }
        },
        (MachineResultAbi::Value(_), None) => Err(Arm64SelectionError::MissingResult(operation)),
    }
}

fn select_incoming_pack_pointer(
    owner: nocter_machine::MachineLinkageId,
    abi: &nocter_machine::MachineCallableAbi,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let Some(pack) = abi.pack() else {
        return if frame.pack_input_pointer().is_none() {
            Ok(())
        } else {
            Err(Arm64SelectionError::PackAbi(owner))
        };
    };
    if pack.pointer().words() != 1 {
        return Err(Arm64SelectionError::PackAbi(owner));
    }
    let object = frame
        .pack_input_pointer()
        .ok_or(Arm64SelectionError::PackAbi(owner))?;
    selected.push(Arm64SelectedInstruction::StoreMemory {
        bytes: word_bytes(),
        destination: Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::FrameObject {
            object,
            offset: 0,
        }),
        source: Arm64SelectedRegister::Fixed(abi_register(pack.pointer().first())?),
    });
    Ok(())
}

fn select_indirect_return(
    block: MachineBlockId,
    value: MachineValueId,
    pointer_register: u8,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if abi_register(pointer_register)? != Arm64NocterAbi::indirect_result_register() {
        return Err(Arm64SelectionError::ReturnTransport(block));
    }
    let (size, object) = match values
        .value(value)
        .ok_or(Arm64SelectionError::UnknownValue(value))?
    {
        Arm64ValueStorage::Memory { size, .. } => (
            *size,
            frame
                .memory_value(value)
                .ok_or(Arm64SelectionError::MemoryValue(value))?,
        ),
        Arm64ValueStorage::Omitted | Arm64ValueStorage::Direct(_) => {
            return Err(Arm64SelectionError::ReturnTransport(block));
        }
    };
    let saved = frame
        .indirect_result_pointer()
        .ok_or(Arm64SelectionError::MissingIndirectResultPointer)?;
    let pointer = Arm64SelectedRegister::Fixed(scratch_boundary());
    selected.push(Arm64SelectedInstruction::LoadMemory {
        bytes: word_bytes(),
        extension: Arm64SelectedLoadExtension::Zero,
        destination: pointer,
        source: Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::FrameObject {
            object: saved,
            offset: 0,
        }),
    });
    selected.push(Arm64SelectedInstruction::CopyMemoryNonOverlapping {
        destination: Arm64SelectedMemoryAddress::Register {
            base: pointer,
            offset: 0,
        },
        source: Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::FrameObject {
            object,
            offset: 0,
        }),
        bytes: size,
    });
    Ok(())
}

fn argument_register(first: u8, lane: usize) -> Result<crate::Arm64Register, Arm64SelectionError> {
    let lane = u8::try_from(lane).map_err(|_| Arm64SelectionError::RegisterOverflow)?;
    first
        .checked_add(lane)
        .and_then(Arm64NocterAbi::argument_register)
        .ok_or(Arm64SelectionError::RegisterOverflow)
}

fn abi_register(index: u8) -> Result<crate::Arm64Register, Arm64SelectionError> {
    crate::Arm64Register::new(index).ok_or(Arm64SelectionError::RegisterOverflow)
}

fn scratch_boundary() -> crate::Arm64Register {
    Arm64NocterAbi::compiler_scratch_register(1)
        .expect("the ABI reserves x17 for call boundary staging")
}

fn word_bytes() -> u8 {
    u8::try_from(Arm64NocterAbi::word_size())
        .expect("the target word size fits selected byte width")
}
