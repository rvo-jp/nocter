use nocter_machine::{
    MachineAddressId, MachineAddressRoot, MachineAddressStep, MachineDataId, MachineFunctionId,
    MachineLayoutKind, MachineScalar, MachineValueId,
};

use crate::{
    Arm64DataSize, Arm64FunctionFrame, Arm64LoadStoreSize, Arm64NocterAbi,
    Arm64SelectedInstruction, Arm64SelectedLoadExtension, Arm64SelectedRegister,
    Arm64SelectedStackAddress, Arm64SelectionError, Arm64ValuePlan,
};

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

pub(crate) fn select_direct_load(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    source: MachineAddressId,
    result: MachineValueId,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let address = machine_address(program, owner, source)?;
    let base = select_stack_address(program, owner, source, frame)?;
    let registers = crate::selection::direct_value(values, result)?;
    let sizes = direct_lane_sizes(address.size(), registers.len())?;
    let extension = direct_load_extension(program, owner, result, sizes.first().copied())?;
    for (lane, (register, bytes)) in registers.iter().copied().zip(sizes).enumerate() {
        selected.push(Arm64SelectedInstruction::LoadStack {
            bytes,
            extension: if lane == 0 {
                extension
            } else {
                Arm64SelectedLoadExtension::Zero
            },
            destination: Arm64SelectedRegister::Virtual(register),
            source: offset_stack_address(base, lane_offset(lane)?)?,
        });
    }
    Ok(())
}

pub(crate) fn select_direct_store(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    destination: MachineAddressId,
    value: MachineValueId,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let address = machine_address(program, owner, destination)?;
    let base = select_stack_address(program, owner, destination, frame)?;
    let registers = crate::selection::direct_value(values, value)?;
    let sizes = direct_lane_sizes(address.size(), registers.len())?;
    for (lane, (register, bytes)) in registers.iter().copied().zip(sizes).enumerate() {
        selected.push(Arm64SelectedInstruction::StoreStack {
            bytes,
            destination: offset_stack_address(base, lane_offset(lane)?)?,
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

pub(crate) fn select_stack_address(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    address: MachineAddressId,
    frame: &Arm64FunctionFrame,
) -> Result<Arm64SelectedStackAddress, Arm64SelectionError> {
    let address = machine_address(program, owner, address)?;
    let MachineAddressRoot::Stack(stack) = address.root() else {
        return Err(Arm64SelectionError::NonStackAddress);
    };
    let mut offset = 0_u64;
    for step in address.steps() {
        let MachineAddressStep::Offset(step) = step else {
            return Err(Arm64SelectionError::ProjectedAddress);
        };
        offset = offset
            .checked_add(*step)
            .ok_or(Arm64SelectionError::AddressOverflow)?;
    }
    frame_stack(frame, stack, offset)
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

fn direct_lane(offset: u64, lane_count: usize) -> Result<usize, Arm64SelectionError> {
    if !offset.is_multiple_of(Arm64NocterAbi::WORD_SIZE) {
        return Err(Arm64SelectionError::DirectLaneOffset(offset));
    }
    let lane = usize::try_from(offset / Arm64NocterAbi::WORD_SIZE)
        .map_err(|_| Arm64SelectionError::DirectLaneOffset(offset))?;
    (lane < lane_count)
        .then_some(lane)
        .ok_or(Arm64SelectionError::DirectLaneOffset(offset))
}

fn direct_lane_sizes(size: u64, lane_count: usize) -> Result<Vec<u8>, Arm64SelectionError> {
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

fn offset_stack_address(
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
