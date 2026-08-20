use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64Logical, Arm64MaterializationError, Arm64Register, Arm64SelectedAddressCalculation,
    Arm64SelectedAddressRoot, Arm64SelectedAddressStep, Arm64SelectedFunction, Arm64SelectedIndex,
    Arm64SelectedIndexAddressDomain, Arm64SelectedIndexBound, Arm64SelectedLoadExtension,
    Arm64SelectedRegister, Arm64Shift,
};

pub(crate) fn emit_resolve(
    function: &Arm64SelectedFunction,
    calculation: &Arm64SelectedAddressCalculation,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let address = crate::address_selection::runtime_address_register();
    let view_length = boundary_register(1);
    match calculation.root() {
        Arm64SelectedAddressRoot::Stack(root) => {
            let offset = crate::selected_code::stack_offset(function, root, 0)?;
            crate::frame_access::form_stack_address(code, address, offset);
        }
        Arm64SelectedAddressRoot::Pointer(pointer) => {
            move_selected(function, pointer, address, code)?;
        }
        Arm64SelectedAddressRoot::View { pointer, length } => {
            move_selected(function, pointer, address, code)?;
            move_selected(function, length, view_length, code)?;
        }
    }
    for step in calculation.steps() {
        match *step {
            Arm64SelectedAddressStep::Offset(offset) => add_offset(code, address, offset),
            Arm64SelectedAddressStep::OffsetRegister(offset) => {
                let offset = crate::selected_code::read_register(function, offset, 0, code)?;
                code.append(Arm64Instruction::AddSubtractRegister {
                    size: Arm64DataSize::Bits64,
                    operation: Arm64AddSubtract::Add,
                    set_flags: false,
                    destination: Arm64DataRegister::General(address),
                    left: Arm64DataRegister::General(address),
                    right: Arm64DataRegister::General(offset),
                });
            }
            Arm64SelectedAddressStep::Dereference => {
                load_native(code, Arm64LoadStoreSize::Double, None, address, address, 0);
            }
            Arm64SelectedAddressStep::ViewDereference {
                pointer_offset,
                length_offset,
            } => {
                load_native(
                    code,
                    Arm64LoadStoreSize::Double,
                    None,
                    view_length,
                    address,
                    length_offset,
                );
                load_native(
                    code,
                    Arm64LoadStoreSize::Double,
                    None,
                    address,
                    address,
                    pointer_offset,
                );
            }
            Arm64SelectedAddressStep::Index {
                index,
                stride,
                bound,
            } => emit_index(function, address, view_length, index, stride, bound, code)?,
        }
    }
    Ok(())
}

pub(crate) fn emit_load(
    function: &Arm64SelectedFunction,
    bytes: u8,
    extension: Arm64SelectedLoadExtension,
    destination: Arm64SelectedRegister,
    base: Arm64SelectedRegister,
    offset: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let base = crate::selected_code::read_register(function, base, 1, code)?;
    let destination = crate::selected_code::write_target(function, destination)?;
    match (load_store_size(bytes), extension) {
        (Some(size), Arm64SelectedLoadExtension::Zero) => {
            load_native(code, size, None, destination.register, base, offset);
        }
        (Some(size), Arm64SelectedLoadExtension::Sign(destination_size)) => {
            load_native(
                code,
                size,
                Some(destination_size),
                destination.register,
                base,
                offset,
            );
        }
        (None, Arm64SelectedLoadExtension::Zero) if (1..=8).contains(&bytes) => {
            emit_fragmented_load(code, bytes, destination.register, base, offset);
        }
        (None, _) => return Err(Arm64MaterializationError::InvalidMemoryWidth(bytes)),
    }
    crate::selected_code::finish_write(destination, code);
    Ok(())
}

pub(crate) fn emit_index_address(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedRegister,
    index: Arm64SelectedRegister,
    domain: Arm64SelectedIndexAddressDomain,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let address = crate::address_selection::runtime_address_register();
    let view_length = boundary_register(1);
    let (stride, bound) = match domain {
        Arm64SelectedIndexAddressDomain::Fixed {
            pointer,
            length,
            stride,
        } => {
            move_selected(function, pointer, address, code)?;
            (stride, Arm64SelectedIndexBound::Fixed(length))
        }
        Arm64SelectedIndexAddressDomain::View {
            pointer,
            length,
            stride,
        } => {
            move_selected(function, pointer, address, code)?;
            move_selected(function, length, view_length, code)?;
            (stride, Arm64SelectedIndexBound::CurrentView)
        }
    };
    emit_index(
        function,
        address,
        view_length,
        Arm64SelectedIndex::Register(index),
        stride,
        bound,
        code,
    )?;
    emit_address(
        function,
        destination,
        Arm64SelectedRegister::Fixed(address),
        0,
        code,
    )
}

pub(crate) fn emit_store(
    function: &Arm64SelectedFunction,
    bytes: u8,
    base: Arm64SelectedRegister,
    offset: u64,
    source: Arm64SelectedRegister,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let base = crate::selected_code::read_register(function, base, 1, code)?;
    let source = crate::selected_code::read_register(function, source, 0, code)?;
    if let Some(size) = load_store_size(bytes) {
        store_native(code, size, source, base, offset);
        Ok(())
    } else if (1..=8).contains(&bytes) {
        emit_fragmented_store(code, bytes, source, base, offset);
        Ok(())
    } else {
        Err(Arm64MaterializationError::InvalidMemoryWidth(bytes))
    }
}

pub(crate) fn emit_address(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedRegister,
    base: Arm64SelectedRegister,
    offset: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let base = crate::selected_code::read_register(function, base, 1, code)?;
    let destination = crate::selected_code::write_target(function, destination)?;
    move_register(code, base, destination.register);
    add_offset(code, destination.register, offset);
    crate::selected_code::finish_write(destination, code);
    Ok(())
}

fn emit_index(
    function: &Arm64SelectedFunction,
    address: Arm64Register,
    view_length: Arm64Register,
    index: Arm64SelectedIndex,
    stride: u64,
    bound: Arm64SelectedIndexBound,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    if let (Arm64SelectedIndex::Constant(index), Arm64SelectedIndexBound::Fixed(length)) =
        (index, bound)
    {
        if index >= length {
            code.append(Arm64Instruction::Break {
                immediate: crate::runtime_trap::Arm64RuntimeTrap::Bounds.immediate(),
            });
            return Ok(());
        }
        let offset = index
            .checked_mul(stride)
            .ok_or(Arm64MaterializationError::OffsetOverflow)?;
        add_offset(code, address, offset);
        return Ok(());
    }

    let index = match index {
        Arm64SelectedIndex::Constant(index) => {
            let register = compiler_scratch(0);
            crate::frame_access::load_immediate(code, register, index, Arm64DataSize::Bits64);
            register
        }
        Arm64SelectedIndex::Register(index) => {
            crate::selected_code::read_register(function, index, 0, code)?
        }
    };
    let bound = match bound {
        Arm64SelectedIndexBound::Fixed(length) => {
            let register = compiler_scratch(1);
            crate::frame_access::load_immediate(code, register, length, Arm64DataSize::Bits64);
            register
        }
        Arm64SelectedIndexBound::CurrentView => view_length,
    };
    emit_bounds_check(index, bound, code)?;
    let stride_register = compiler_scratch(1);
    crate::frame_access::load_immediate(code, stride_register, stride, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: address,
        left: index,
        right: stride_register,
        addend: Arm64DataRegister::General(address),
        subtract_product: false,
    });
    Ok(())
}

fn emit_bounds_check(
    index: Arm64Register,
    bound: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(index),
        right: Arm64DataRegister::General(bound),
    });
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::CarryClear);
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::Bounds.immediate(),
    });
    code.bind(valid)?;
    Ok(())
}

fn move_selected(
    function: &Arm64SelectedFunction,
    source: Arm64SelectedRegister,
    destination: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let source = crate::selected_code::read_register(function, source, 0, code)?;
    move_register(code, source, destination);
    Ok(())
}

fn move_register(code: &mut Arm64CodeBuilder, source: Arm64Register, destination: Arm64Register) {
    if source != destination {
        code.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: Arm64AddSubtractDestination::General(destination),
            source: Arm64BaseRegister::General(source),
            immediate: 0,
            shift_12: false,
        });
    }
}

fn add_offset(code: &mut Arm64CodeBuilder, address: Arm64Register, offset: u64) {
    if offset == 0 {
        return;
    }
    if let Some((immediate, shift_12)) = add_immediate(offset) {
        code.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: Arm64AddSubtractDestination::General(address),
            source: Arm64BaseRegister::General(address),
            immediate,
            shift_12,
        });
        return;
    }
    let offset_register = if address == compiler_scratch(1) {
        compiler_scratch(0)
    } else {
        compiler_scratch(1)
    };
    crate::frame_access::load_immediate(code, offset_register, offset, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64DataRegister::General(address),
        left: Arm64DataRegister::General(address),
        right: Arm64DataRegister::General(offset_register),
    });
}

fn load_native(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    signed_destination: Option<Arm64DataSize>,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u64,
) {
    let (base, offset) = general_memory_base(code, size, base, offset, Some(destination));
    if let Some(destination_size) = signed_destination {
        code.append(Arm64Instruction::LoadSigned {
            size,
            destination_size,
            destination: Arm64DataRegister::General(destination),
            base: Arm64BaseRegister::General(base),
            offset,
        });
    } else {
        code.append(Arm64Instruction::LoadUnsigned {
            size,
            destination: Arm64DataRegister::General(destination),
            base: Arm64BaseRegister::General(base),
            offset,
        });
    }
}

fn store_native(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    source: Arm64Register,
    base: Arm64Register,
    offset: u64,
) {
    let (base, offset) = general_memory_base(code, size, base, offset, Some(source));
    code.append(Arm64Instruction::StoreUnsigned {
        size,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::General(base),
        offset,
    });
}

fn general_memory_base(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    base: Arm64Register,
    offset: u64,
    excluded: Option<Arm64Register>,
) -> (Arm64Register, u32) {
    let scale = load_store_bytes(size);
    if offset <= 0x0fff * scale && offset.is_multiple_of(scale) {
        return (
            base,
            u32::try_from(offset).expect("scaled offset is bounded"),
        );
    }
    (
        form_address_with_offset(code, base, offset, excluded.as_slice()),
        0,
    )
}

fn form_address_with_offset(
    code: &mut Arm64CodeBuilder,
    base: Arm64Register,
    offset: u64,
    excluded: &[Arm64Register],
) -> Arm64Register {
    if offset == 0 {
        return base;
    }
    let candidates = [
        compiler_scratch(1),
        compiler_scratch(0),
        boundary_register(2),
        boundary_register(1),
        boundary_register(3),
    ];
    let mut available = candidates
        .into_iter()
        .filter(|register| *register != base && !excluded.contains(register));
    let address = available
        .next()
        .expect("the address boundary reserves enough non-value scratch lanes");
    let offset_register = available
        .next()
        .expect("the address boundary reserves enough non-value scratch lanes");
    move_register(code, base, address);
    crate::frame_access::load_immediate(code, offset_register, offset, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64DataRegister::General(address),
        left: Arm64DataRegister::General(address),
        right: Arm64DataRegister::General(offset_register),
    });
    address
}

fn emit_fragmented_load(
    code: &mut Arm64CodeBuilder,
    bytes: u8,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u64,
) {
    let base = form_address_with_offset(code, base, offset, &[destination]);
    crate::frame_access::load_immediate(code, destination, 0, Arm64DataSize::Bits64);
    let (fragment, shift) = scratch_pair(&[destination, base]);
    for (fragment_offset, size) in crate::memory_code::memory_fragments(bytes) {
        load_native(code, size, None, fragment, base, u64::from(fragment_offset));
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
}

fn emit_fragmented_store(
    code: &mut Arm64CodeBuilder,
    bytes: u8,
    source: Arm64Register,
    base: Arm64Register,
    offset: u64,
) {
    let base = form_address_with_offset(code, base, offset, &[source]);
    let (fragment, shift) = scratch_pair(&[source, base]);
    for (fragment_offset, size) in crate::memory_code::memory_fragments(bytes) {
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
        store_native(code, size, stored, base, u64::from(fragment_offset));
    }
}

fn scratch_pair(excluded: &[Arm64Register]) -> (Arm64Register, Arm64Register) {
    let candidates = [
        compiler_scratch(0),
        compiler_scratch(1),
        boundary_register(1),
        boundary_register(2),
    ];
    let mut available = candidates
        .into_iter()
        .filter(|register| !excluded.contains(register));
    (
        available
            .next()
            .expect("the address boundary retains two scratch lanes"),
        available
            .next()
            .expect("the address boundary retains two scratch lanes"),
    )
}

fn add_immediate(value: u64) -> Option<(u16, bool)> {
    if value <= 0x0fff {
        return Some((u16::try_from(value).ok()?, false));
    }
    if value.is_multiple_of(1 << 12) && value >> 12 <= 0x0fff {
        return Some((u16::try_from(value >> 12).ok()?, true));
    }
    None
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

const fn load_store_bytes(size: Arm64LoadStoreSize) -> u64 {
    match size {
        Arm64LoadStoreSize::Byte => 1,
        Arm64LoadStoreSize::Half => 2,
        Arm64LoadStoreSize::Word => 4,
        Arm64LoadStoreSize::Double => 8,
    }
}

fn compiler_scratch(index: u8) -> Arm64Register {
    crate::Arm64NocterAbi::compiler_scratch_register(index)
        .expect("the ABI reserves two compiler scratch registers")
}

fn boundary_register(index: u8) -> Arm64Register {
    crate::Arm64NocterAbi::argument_register(index)
        .expect("the ABI reserves x0 and x1 as address boundary registers")
}
