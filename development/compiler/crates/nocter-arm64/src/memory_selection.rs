use nocter_machine::{
    MachineAddressId, MachineDataId, MachineFunctionId, MachineLayoutKind, MachineScalar,
    MachineValueId,
};

use crate::{
    Arm64DataSize, Arm64FunctionFrame, Arm64LoadStoreSize, Arm64NocterAbi,
    Arm64SelectedAddressPlan, Arm64SelectedInstruction, Arm64SelectedLoadExtension,
    Arm64SelectedMemoryAddress, Arm64SelectedRegister, Arm64SelectedStackAddress,
    Arm64SelectionError, Arm64ValuePlan,
};

pub(crate) fn select_operation(
    scope: (&nocter_machine::MachineProgram, MachineFunctionId),
    operation_id: nocter_machine::MachineOperationId,
    operation: &nocter_machine::MachineOperation,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    addresses: &Arm64SelectedAddressPlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    match operation.kind() {
        nocter_machine::MachineOperationKind::Load { source } => select_load(
            scope,
            *source,
            operation
                .result()
                .ok_or(Arm64SelectionError::MissingResult(operation_id))?,
            values,
            frame,
            addresses,
            selected,
        ),
        nocter_machine::MachineOperationKind::Store { destination, value } => select_store(
            scope,
            *destination,
            *value,
            values,
            frame,
            addresses,
            selected,
        ),
        nocter_machine::MachineOperationKind::AddressOf { source } => {
            let result = operation
                .result()
                .ok_or(Arm64SelectionError::MissingResult(operation_id))?;
            let address = machine_address(scope.0, scope.1, *source)?;
            match address.extent() {
                nocter_machine::MachineAddressExtent::Stored { .. } => {
                    let source = addresses.use_address(*source, selected)?;
                    selected.push(Arm64SelectedInstruction::MemoryAddress {
                        destination: one_word(values, result)?,
                        source,
                    });
                }
                nocter_machine::MachineAddressExtent::View => {
                    let (pointer, length) = addresses.use_view_address(*source, selected)?;
                    let result_layout = scope
                        .0
                        .function(scope.1)
                        .and_then(|function| function.body().value(result))
                        .and_then(|value| scope.0.layouts().get(value.ty()))
                        .ok_or(Arm64SelectionError::MemoryShape(result))?;
                    let nocter_machine::MachineLayoutKind::View {
                        pointer_offset,
                        length_offset,
                    } = result_layout.kind()
                    else {
                        return Err(Arm64SelectionError::MemoryShape(result));
                    };
                    let destinations = crate::selection::direct_value(values, result)?;
                    let pointer_lane = direct_lane(*pointer_offset, destinations.len())?;
                    let length_lane = direct_lane(*length_offset, destinations.len())?;
                    selected.push(Arm64SelectedInstruction::Move {
                        size: Arm64DataSize::Bits64,
                        destination: Arm64SelectedRegister::Virtual(destinations[pointer_lane]),
                        source: pointer,
                    });
                    selected.push(Arm64SelectedInstruction::Move {
                        size: Arm64DataSize::Bits64,
                        destination: Arm64SelectedRegister::Virtual(destinations[length_lane]),
                        source: length,
                    });
                }
            }
            Ok(())
        }
        _ => unreachable!("the caller classifies memory operations exhaustively"),
    }
}

pub(crate) fn select_text_constant(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    data: MachineDataId,
    result: MachineValueId,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let registers = crate::selection::direct_value(values, result)?;
    let ty = program
        .function(owner)
        .and_then(|function| function.body().value(result))
        .map(nocter_machine::MachineValue::ty)
        .ok_or(Arm64SelectionError::UnknownValue(result))?;
    let (pointer_lane, length_lane) = match program
        .layouts()
        .get(ty)
        .map(nocter_machine::MachineLayout::kind)
    {
        Some(MachineLayoutKind::View {
            pointer_offset,
            length_offset,
        }) => (
            direct_lane(*pointer_offset, registers.len())?,
            direct_lane(*length_offset, registers.len())?,
        ),
        _ => return Err(Arm64SelectionError::TextRepresentation(result)),
    };
    let length = program
        .data()
        .get(data)
        .map(nocter_machine::MachineData::bytes)
        .and_then(|bytes| u64::try_from(bytes.len()).ok())
        .ok_or(Arm64SelectionError::UnknownData(data))?;
    selected.push(Arm64SelectedInstruction::LoadDataAddress {
        destination: Arm64SelectedRegister::Virtual(registers[pointer_lane]),
        source: data,
    });
    selected.push(Arm64SelectedInstruction::LoadImmediate {
        size: Arm64DataSize::Bits64,
        destination: Arm64SelectedRegister::Virtual(registers[length_lane]),
        value: length,
    });
    Ok(())
}

pub(crate) fn select_load(
    scope: (&nocter_machine::MachineProgram, MachineFunctionId),
    source: MachineAddressId,
    result: MachineValueId,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    addresses: &Arm64SelectedAddressPlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let (program, owner) = scope;
    let address = machine_address(program, owner, source)?;
    let address_size = address
        .stored_size()
        .ok_or(Arm64SelectionError::MemoryShape(result))?;
    if stored_value_size(program, owner, result)? != address_size {
        return Err(Arm64SelectionError::MemoryShape(result));
    }
    match values
        .value(result)
        .ok_or(Arm64SelectionError::UnknownValue(result))?
    {
        crate::Arm64ValueStorage::Omitted if address_size == 0 => return Ok(()),
        crate::Arm64ValueStorage::Memory { size, .. } if *size == address_size => {
            let object = frame
                .memory_value(result)
                .ok_or(Arm64SelectionError::MemoryValue(result))?;
            let source = addresses.use_address(source, selected)?;
            selected.push(Arm64SelectedInstruction::CopyMemoryNonOverlapping {
                destination: Arm64SelectedMemoryAddress::Stack(
                    Arm64SelectedStackAddress::FrameObject { object, offset: 0 },
                ),
                source,
                bytes: *size,
            });
            return Ok(());
        }
        crate::Arm64ValueStorage::Direct(_) => {}
        crate::Arm64ValueStorage::Omitted | crate::Arm64ValueStorage::Memory { .. } => {
            return Err(Arm64SelectionError::MemoryShape(result));
        }
    }
    let base = addresses.use_address(source, selected)?;
    let registers = crate::selection::direct_value(values, result)?;
    let sizes = direct_lane_sizes(address_size, registers.len())?;
    let extension = direct_load_extension(program, owner, result, sizes.first().copied())?;
    for (lane, (register, bytes)) in registers.iter().copied().zip(sizes).enumerate() {
        selected.push(Arm64SelectedInstruction::LoadMemory {
            bytes,
            extension: if lane == 0 {
                extension
            } else {
                Arm64SelectedLoadExtension::Zero
            },
            destination: Arm64SelectedRegister::Virtual(register),
            source: offset_memory_address(base, lane_offset(lane)?)?,
        });
    }
    Ok(())
}

pub(crate) fn select_store(
    scope: (&nocter_machine::MachineProgram, MachineFunctionId),
    destination: MachineAddressId,
    value: MachineValueId,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    addresses: &Arm64SelectedAddressPlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let (program, owner) = scope;
    let address = machine_address(program, owner, destination)?;
    let address_size = address
        .stored_size()
        .ok_or(Arm64SelectionError::MemoryShape(value))?;
    if stored_value_size(program, owner, value)? != address_size {
        return Err(Arm64SelectionError::MemoryShape(value));
    }
    match values
        .value(value)
        .ok_or(Arm64SelectionError::UnknownValue(value))?
    {
        crate::Arm64ValueStorage::Omitted if address_size == 0 => return Ok(()),
        crate::Arm64ValueStorage::Memory { size, .. } if *size == address_size => {
            let object = frame
                .memory_value(value)
                .ok_or(Arm64SelectionError::MemoryValue(value))?;
            let destination = addresses.use_address(destination, selected)?;
            selected.push(Arm64SelectedInstruction::CopyMemoryNonOverlapping {
                destination,
                source: Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::FrameObject {
                    object,
                    offset: 0,
                }),
                bytes: *size,
            });
            return Ok(());
        }
        crate::Arm64ValueStorage::Direct(_) => {}
        crate::Arm64ValueStorage::Omitted | crate::Arm64ValueStorage::Memory { .. } => {
            return Err(Arm64SelectionError::MemoryShape(value));
        }
    }
    let base = addresses.use_address(destination, selected)?;
    let registers = crate::selection::direct_value(values, value)?;
    let sizes = direct_lane_sizes(address_size, registers.len())?;
    for (lane, (register, bytes)) in registers.iter().copied().zip(sizes).enumerate() {
        selected.push(Arm64SelectedInstruction::StoreMemory {
            bytes,
            destination: offset_memory_address(base, lane_offset(lane)?)?,
            source: Arm64SelectedRegister::Virtual(register),
        });
    }
    Ok(())
}

fn direct_load_extension(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    value: MachineValueId,
    lane_bytes: Option<u8>,
) -> Result<Arm64SelectedLoadExtension, Arm64SelectionError> {
    let ty = program
        .function(owner)
        .and_then(|function| function.body().value(value))
        .map(nocter_machine::MachineValue::ty)
        .ok_or(Arm64SelectionError::UnknownValue(value))?;
    let Some(MachineLayoutKind::Scalar(scalar)) = program
        .layouts()
        .get(ty)
        .map(nocter_machine::MachineLayout::kind)
    else {
        return Ok(Arm64SelectedLoadExtension::Zero);
    };
    let (size, extension, _) = scalar_memory_representation(*scalar)?;
    if lane_bytes != Some(load_store_bytes(size)) {
        return Err(Arm64SelectionError::DirectMemoryShape(value));
    }
    Ok(extension)
}

pub(crate) fn scalar_memory_representation(
    scalar: MachineScalar,
) -> Result<(Arm64LoadStoreSize, Arm64SelectedLoadExtension, bool), Arm64SelectionError> {
    match scalar {
        MachineScalar::Bool => Ok((
            Arm64LoadStoreSize::Byte,
            Arm64SelectedLoadExtension::Zero,
            false,
        )),
        MachineScalar::Integer { bits, signed } => {
            let (size, register_size) = match bits {
                8 => (Arm64LoadStoreSize::Byte, Arm64DataSize::Bits32),
                16 => (Arm64LoadStoreSize::Half, Arm64DataSize::Bits32),
                32 => (Arm64LoadStoreSize::Word, Arm64DataSize::Bits32),
                64 => (Arm64LoadStoreSize::Double, Arm64DataSize::Bits64),
                _ => return Err(Arm64SelectionError::UnsupportedScalarRepresentation(scalar)),
            };
            let extension = if signed && matches!(bits, 8 | 16) {
                Arm64SelectedLoadExtension::Sign(register_size)
            } else {
                Arm64SelectedLoadExtension::Zero
            };
            Ok((size, extension, signed))
        }
    }
}

fn machine_address(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    address: MachineAddressId,
) -> Result<&nocter_machine::MachineAddress, Arm64SelectionError> {
    program
        .function(owner)
        .and_then(|function| function.body().address(address))
        .ok_or(Arm64SelectionError::UnknownAddress(address))
}

fn stored_value_size(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    value: MachineValueId,
) -> Result<u64, Arm64SelectionError> {
    match program
        .function(owner)
        .and_then(|function| function.body().value(value))
        .map(nocter_machine::MachineValue::representation)
    {
        Some(nocter_machine::MachineValueRepresentation::Stored { size, .. }) => Ok(size),
        Some(
            nocter_machine::MachineValueRepresentation::Completion
            | nocter_machine::MachineValueRepresentation::Diverging,
        ) => Err(Arm64SelectionError::MemoryShape(value)),
        None => Err(Arm64SelectionError::UnknownValue(value)),
    }
}

pub(crate) fn direct_lane(offset: u64, lane_count: usize) -> Result<usize, Arm64SelectionError> {
    if !offset.is_multiple_of(Arm64NocterAbi::WORD_SIZE) {
        return Err(Arm64SelectionError::DirectLaneOffset(offset));
    }
    let lane = usize::try_from(offset / Arm64NocterAbi::WORD_SIZE)
        .map_err(|_| Arm64SelectionError::DirectLaneOffset(offset))?;
    (lane < lane_count)
        .then_some(lane)
        .ok_or(Arm64SelectionError::DirectLaneOffset(offset))
}

pub(crate) fn direct_lane_sizes(
    size: u64,
    lane_count: usize,
) -> Result<Vec<u8>, Arm64SelectionError> {
    if size == 0 && lane_count == 0 {
        return Ok(Vec::new());
    }
    let expected = size.div_ceil(Arm64NocterAbi::WORD_SIZE);
    if usize::try_from(expected).ok() != Some(lane_count) {
        return Err(Arm64SelectionError::DirectMemoryWidth(size));
    }
    (0..lane_count)
        .map(|lane| {
            let offset = lane_offset(lane)?;
            let remaining = size
                .checked_sub(offset)
                .ok_or(Arm64SelectionError::AddressOverflow)?;
            u8::try_from(remaining.min(Arm64NocterAbi::WORD_SIZE))
                .map_err(|_| Arm64SelectionError::DirectMemoryWidth(size))
        })
        .collect()
}

pub(crate) fn lane_offset(lane: usize) -> Result<u64, Arm64SelectionError> {
    u64::try_from(lane)
        .ok()
        .and_then(|lane| lane.checked_mul(Arm64NocterAbi::WORD_SIZE))
        .ok_or(Arm64SelectionError::AddressOverflow)
}

pub(crate) fn offset_stack_address(
    address: Arm64SelectedStackAddress,
    additional: u64,
) -> Result<Arm64SelectedStackAddress, Arm64SelectionError> {
    let add = |offset: u64| {
        offset
            .checked_add(additional)
            .ok_or(Arm64SelectionError::AddressOverflow)
    };
    match address {
        Arm64SelectedStackAddress::FrameObject { object, offset } => {
            Ok(Arm64SelectedStackAddress::FrameObject {
                object,
                offset: add(offset)?,
            })
        }
        Arm64SelectedStackAddress::Outgoing(offset) => {
            Ok(Arm64SelectedStackAddress::Outgoing(add(offset)?))
        }
        Arm64SelectedStackAddress::Incoming(offset) => {
            Ok(Arm64SelectedStackAddress::Incoming(add(offset)?))
        }
    }
}

pub(crate) fn offset_memory_address(
    address: Arm64SelectedMemoryAddress,
    additional: u64,
) -> Result<Arm64SelectedMemoryAddress, Arm64SelectionError> {
    match address {
        Arm64SelectedMemoryAddress::Stack(address) => Ok(Arm64SelectedMemoryAddress::Stack(
            offset_stack_address(address, additional)?,
        )),
        Arm64SelectedMemoryAddress::Register { base, offset } => {
            Ok(Arm64SelectedMemoryAddress::Register {
                base,
                offset: offset
                    .checked_add(additional)
                    .ok_or(Arm64SelectionError::AddressOverflow)?,
            })
        }
    }
}

pub(crate) fn parameter_lane_sizes(
    function: &nocter_machine::MachineFunction,
    stack: nocter_machine::MachineStackId,
    words: u8,
) -> Result<Vec<u8>, Arm64SelectionError> {
    let size = function
        .body()
        .stack(stack)
        .map(nocter_machine::MachineStackObject::size)
        .ok_or(Arm64SelectionError::UnknownStack(stack))?;
    direct_lane_sizes(size, usize::from(words))
}

pub(crate) fn frame_stack(
    frame: &Arm64FunctionFrame,
    stack: nocter_machine::MachineStackId,
    offset: u64,
) -> Result<Arm64SelectedStackAddress, Arm64SelectionError> {
    frame
        .stack_object(stack)
        .map(|object| Arm64SelectedStackAddress::FrameObject { object, offset })
        .ok_or(Arm64SelectionError::UnknownStack(stack))
}

const fn load_store_bytes(size: Arm64LoadStoreSize) -> u8 {
    match size {
        Arm64LoadStoreSize::Byte => 1,
        Arm64LoadStoreSize::Half => 2,
        Arm64LoadStoreSize::Word => 4,
        Arm64LoadStoreSize::Double => 8,
    }
}

fn one_word(
    values: &Arm64ValuePlan,
    value: MachineValueId,
) -> Result<Arm64SelectedRegister, Arm64SelectionError> {
    match crate::selection::direct_value(values, value)? {
        [register] => Ok(Arm64SelectedRegister::Virtual(*register)),
        _ => Err(Arm64SelectionError::ExpectedOneWord(value)),
    }
}
