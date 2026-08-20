use crate::{
    Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64Logical, Arm64MaterializationError, Arm64NocterAbi, Arm64Register, Arm64SelectedFunction,
    Arm64SelectedLoadExtension, Arm64SelectedMemoryAddress, Arm64SelectedRegister,
    Arm64SelectedStackAddress, Arm64Shift,
};

pub(crate) fn emit_data_address(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedRegister,
    source: crate::Arm64DataId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let destination = crate::selected_code::write_target(function, destination)?;
    code.load_data_address(source, destination.register);
    crate::selected_code::finish_write(destination, code);
    Ok(())
}

pub(crate) fn emit_memory_load(
    function: &Arm64SelectedFunction,
    bytes: u8,
    extension: Arm64SelectedLoadExtension,
    destination: Arm64SelectedRegister,
    source: Arm64SelectedMemoryAddress,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    match source {
        Arm64SelectedMemoryAddress::Stack(source) => {
            emit_stack_load(function, bytes, extension, destination, source, code)
        }
        Arm64SelectedMemoryAddress::Register { base, offset } => crate::address_code::emit_load(
            function,
            bytes,
            extension,
            destination,
            base,
            offset,
            code,
        ),
    }
}

fn emit_stack_load(
    function: &Arm64SelectedFunction,
    bytes: u8,
    extension: Arm64SelectedLoadExtension,
    destination: Arm64SelectedRegister,
    source: Arm64SelectedStackAddress,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let offset = crate::selected_code::stack_offset(function, source, u64::from(bytes))?;
    let destination = crate::selected_code::write_target(function, destination)?;
    match (load_store_size(bytes), extension) {
        (Some(size), Arm64SelectedLoadExtension::Zero) => {
            crate::frame_access::load_at_stack_offset(code, size, destination.register, offset);
        }
        (Some(size), Arm64SelectedLoadExtension::Sign(destination_size)) => {
            crate::frame_access::load_signed_at_stack_offset(
                code,
                size,
                destination_size,
                destination.register,
                offset,
            );
        }
        (None, Arm64SelectedLoadExtension::Zero) => {
            emit_fragmented_load(code, bytes, destination.register, offset)?;
        }
        (None, Arm64SelectedLoadExtension::Sign(_)) => {
            return Err(Arm64MaterializationError::InvalidMemoryWidth(bytes));
        }
    }
    crate::selected_code::finish_write(destination, code);
    Ok(())
}

pub(crate) fn emit_memory_store(
    function: &Arm64SelectedFunction,
    bytes: u8,
    destination: Arm64SelectedMemoryAddress,
    source: Arm64SelectedRegister,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    match destination {
        Arm64SelectedMemoryAddress::Stack(destination) => {
            emit_stack_store(function, bytes, destination, source, code)
        }
        Arm64SelectedMemoryAddress::Register { base, offset } => {
            crate::address_code::emit_store(function, bytes, base, offset, source, code)
        }
    }
}

fn emit_stack_store(
    function: &Arm64SelectedFunction,
    bytes: u8,
    destination: Arm64SelectedStackAddress,
    source: Arm64SelectedRegister,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let source = crate::selected_code::read_register(function, source, 0, code)?;
    let offset = crate::selected_code::stack_offset(function, destination, u64::from(bytes))?;
    if let Some(size) = load_store_size(bytes) {
        crate::frame_access::store_at_stack_offset(code, size, source, offset);
    } else {
        emit_fragmented_store(code, bytes, source, offset)?;
    }
    Ok(())
}

pub(crate) fn emit_stack_zero(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedStackAddress,
    bytes: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let zero = boundary_register(0);
    crate::frame_access::load_immediate(code, zero, 0, Arm64DataSize::Bits64);
    for (offset, width) in exact_memory_chunks(bytes) {
        emit_stack_store(
            function,
            width,
            offset_stack_address(destination, offset)?,
            Arm64SelectedRegister::Fixed(zero),
            code,
        )?;
    }
    Ok(())
}

pub(crate) fn emit_memory_copy(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedMemoryAddress,
    source: Arm64SelectedMemoryAddress,
    bytes: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    if let (
        Arm64SelectedMemoryAddress::Stack(destination),
        Arm64SelectedMemoryAddress::Stack(source),
    ) = (destination, source)
    {
        validate_nonoverlapping_copy(function, destination, source, bytes)?;
    }
    let transfer = Arm64SelectedRegister::Fixed(crate::frame_access::scratch(0));
    for (offset, width) in exact_memory_chunks(bytes) {
        emit_memory_load(
            function,
            width,
            Arm64SelectedLoadExtension::Zero,
            transfer,
            offset_memory_address(source, offset)?,
            code,
        )?;
        emit_memory_store(
            function,
            width,
            offset_memory_address(destination, offset)?,
            transfer,
            code,
        )?;
    }
    Ok(())
}

pub(crate) fn emit_selected_copy(
    function: &Arm64SelectedFunction,
    instruction: &crate::Arm64SelectedInstruction,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    match *instruction {
        crate::Arm64SelectedInstruction::CopyMemoryNonOverlapping {
            destination,
            source,
            bytes,
        } => emit_memory_copy(function, destination, source, bytes, code),
        crate::Arm64SelectedInstruction::CopyMemoryNonOverlappingDynamic {
            destination,
            source,
            bytes,
        } => crate::primitive_memory_code::emit_dynamic_copy(
            function,
            destination,
            source,
            bytes,
            code,
        ),
        _ => unreachable!("selected copy routing accepts only copy instructions"),
    }
}

fn validate_nonoverlapping_copy(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedStackAddress,
    source: Arm64SelectedStackAddress,
    bytes: u64,
) -> Result<(), Arm64MaterializationError> {
    let destination_start = crate::selected_code::stack_offset(function, destination, bytes)?;
    let source_start = crate::selected_code::stack_offset(function, source, bytes)?;
    let destination_end = destination_start
        .checked_add(bytes)
        .ok_or(Arm64MaterializationError::OffsetOverflow)?;
    let source_end = source_start
        .checked_add(bytes)
        .ok_or(Arm64MaterializationError::OffsetOverflow)?;
    if bytes != 0 && destination_start < source_end && source_start < destination_end {
        Err(Arm64MaterializationError::OverlappingStackCopy)
    } else {
        Ok(())
    }
}

pub(crate) fn emit_memory_address(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedRegister,
    source: Arm64SelectedMemoryAddress,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    match source {
        Arm64SelectedMemoryAddress::Stack(source) => {
            let offset = crate::selected_code::stack_offset(function, source, 0)?;
            let destination = crate::selected_code::write_target(function, destination)?;
            crate::frame_access::form_stack_address(code, destination.register, offset);
            crate::selected_code::finish_write(destination, code);
            Ok(())
        }
        Arm64SelectedMemoryAddress::Register { base, offset } => {
            crate::address_code::emit_address(function, destination, base, offset, code)
        }
    }
}

fn emit_fragmented_load(
    code: &mut Arm64CodeBuilder,
    bytes: u8,
    destination: Arm64Register,
    offset: u64,
) -> Result<(), Arm64MaterializationError> {
    validate_fragmented_width(bytes)?;
    crate::frame_access::load_immediate(code, destination, 0, Arm64DataSize::Bits64);
    let fragment = boundary_register(0);
    let shift = boundary_register(1);
    for (fragment_offset, size) in memory_fragments(bytes) {
        crate::frame_access::load_at_stack_offset(
            code,
            size,
            fragment,
            offset
                .checked_add(u64::from(fragment_offset))
                .ok_or(Arm64MaterializationError::OffsetOverflow)?,
        );
        if fragment_offset != 0 {
            crate::frame_access::load_immediate(
                code,
                shift,
                u64::from(fragment_offset) * 8,
                Arm64DataSize::Bits64,
            );
            code.append(Arm64Instruction::VariableShift {
                size: Arm64DataSize::Bits64,
                operation: Arm64Shift::Left,
                destination: fragment,
                value: fragment,
                amount: shift,
            });
        }
        code.append(Arm64Instruction::LogicalRegister {
            size: Arm64DataSize::Bits64,
            operation: Arm64Logical::Or,
            destination: Arm64DataRegister::General(destination),
            left: Arm64DataRegister::General(destination),
            right: Arm64DataRegister::General(fragment),
        });
    }
    Ok(())
}

fn emit_fragmented_store(
    code: &mut Arm64CodeBuilder,
    bytes: u8,
    source: Arm64Register,
    offset: u64,
) -> Result<(), Arm64MaterializationError> {
    validate_fragmented_width(bytes)?;
    let fragment = boundary_register(0);
    let shift = boundary_register(1);
    for (fragment_offset, size) in memory_fragments(bytes) {
        let stored = if fragment_offset == 0 {
            source
        } else {
            crate::frame_access::load_immediate(
                code,
                shift,
                u64::from(fragment_offset) * 8,
                Arm64DataSize::Bits64,
            );
            code.append(Arm64Instruction::VariableShift {
                size: Arm64DataSize::Bits64,
                operation: Arm64Shift::RightLogical,
                destination: fragment,
                value: source,
                amount: shift,
            });
            fragment
        };
        crate::frame_access::store_at_stack_offset(
            code,
            size,
            stored,
            offset
                .checked_add(u64::from(fragment_offset))
                .ok_or(Arm64MaterializationError::OffsetOverflow)?,
        );
    }
    Ok(())
}

fn validate_fragmented_width(bytes: u8) -> Result<(), Arm64MaterializationError> {
    if (1..=8).contains(&bytes) {
        Ok(())
    } else {
        Err(Arm64MaterializationError::InvalidMemoryWidth(bytes))
    }
}

fn exact_memory_chunks(bytes: u64) -> impl Iterator<Item = (u64, u8)> {
    let mut remaining = bytes;
    let mut offset = 0_u64;
    std::iter::from_fn(move || {
        if remaining == 0 {
            return None;
        }
        let width = if remaining >= 8 {
            8
        } else if remaining >= 4 {
            4
        } else if remaining >= 2 {
            2
        } else {
            1
        };
        let chunk = (offset, width);
        offset += u64::from(width);
        remaining -= u64::from(width);
        Some(chunk)
    })
}

fn offset_stack_address(
    address: Arm64SelectedStackAddress,
    additional: u64,
) -> Result<Arm64SelectedStackAddress, Arm64MaterializationError> {
    let add = |offset: u64| {
        offset
            .checked_add(additional)
            .ok_or(Arm64MaterializationError::OffsetOverflow)
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

fn offset_memory_address(
    address: Arm64SelectedMemoryAddress,
    additional: u64,
) -> Result<Arm64SelectedMemoryAddress, Arm64MaterializationError> {
    match address {
        Arm64SelectedMemoryAddress::Stack(address) => Ok(Arm64SelectedMemoryAddress::Stack(
            offset_stack_address(address, additional)?,
        )),
        Arm64SelectedMemoryAddress::Register { base, offset } => {
            Ok(Arm64SelectedMemoryAddress::Register {
                base,
                offset: offset
                    .checked_add(additional)
                    .ok_or(Arm64MaterializationError::OffsetOverflow)?,
            })
        }
    }
}

pub(crate) fn memory_fragments(bytes: u8) -> impl Iterator<Item = (u8, Arm64LoadStoreSize)> {
    let mut remaining = bytes;
    let mut offset = 0;
    std::iter::from_fn(move || {
        if remaining == 0 {
            return None;
        }
        let (width, size) = if remaining >= 4 {
            (4, Arm64LoadStoreSize::Word)
        } else if remaining >= 2 {
            (2, Arm64LoadStoreSize::Half)
        } else {
            (1, Arm64LoadStoreSize::Byte)
        };
        let fragment = (offset, size);
        offset += width;
        remaining -= width;
        Some(fragment)
    })
}

fn boundary_register(index: u8) -> Arm64Register {
    Arm64NocterAbi::argument_register(index)
        .expect("the ABI reserves x0 and x1 as materialization boundary registers")
}

const fn load_store_size(bytes: u8) -> Option<Arm64LoadStoreSize> {
    match bytes {
        1 => Some(Arm64LoadStoreSize::Byte),
        2 => Some(Arm64LoadStoreSize::Half),
        4 => Some(Arm64LoadStoreSize::Word),
        8 => Some(Arm64LoadStoreSize::Double),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{exact_memory_chunks, memory_fragments};
    use crate::Arm64LoadStoreSize;

    #[test]
    fn decomposes_non_native_direct_widths_without_crossing_the_value_boundary() {
        assert_eq!(
            memory_fragments(3).collect::<Vec<_>>(),
            vec![(0, Arm64LoadStoreSize::Half), (2, Arm64LoadStoreSize::Byte),]
        );
        assert_eq!(
            memory_fragments(7).collect::<Vec<_>>(),
            vec![
                (0, Arm64LoadStoreSize::Word),
                (4, Arm64LoadStoreSize::Half),
                (6, Arm64LoadStoreSize::Byte),
            ]
        );
    }

    #[test]
    fn decomposes_arbitrary_ranges_without_crossing_the_object_boundary() {
        assert_eq!(
            exact_memory_chunks(15).collect::<Vec<_>>(),
            vec![(0, 8), (8, 4), (12, 2), (14, 1)]
        );
    }
}
