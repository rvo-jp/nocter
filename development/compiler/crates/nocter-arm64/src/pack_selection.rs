use nocter_machine::{
    MachineCall, MachineCallableAbi, MachineOperationId, MachinePackId, MachinePackSegment,
    MachineValueId, MachineValueRepresentation,
};

use crate::{
    Arm64PackCallbackKind, Arm64PackDescriptorLayout, Arm64PackSegmentLayout,
    Arm64SelectedInstruction, Arm64SelectedLoadExtension, Arm64SelectedMemoryAddress,
    Arm64SelectedRegister, Arm64SelectedStackAddress, Arm64SelectionError, Arm64ValueStorage,
};

pub(crate) fn select_call_pack(
    context: crate::Arm64SelectionContext<'_>,
    operation: MachineOperationId,
    call: &MachineCall,
    abi: &MachineCallableAbi,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let (pack_id, pack_abi) = match (call.pack(), abi.pack()) {
        (None, None) => return Ok(()),
        (Some(pack), Some(abi)) => (pack, abi),
        _ => return Err(Arm64SelectionError::CallPack(operation)),
    };
    if !call.arguments().is_empty() {
        return Err(Arm64SelectionError::CallPack(operation));
    }
    let body = context
        .program()
        .function(context.owner())
        .ok_or(Arm64SelectionError::UnknownFunction(context.owner()))?
        .body();
    let pack = body
        .pack(pack_id)
        .ok_or(Arm64SelectionError::CallPack(operation))?;
    if pack.element() != pack_abi.element()
        || pack.next() != pack_abi.next()
        || pack.next_result() != pack_abi.next_result()
    {
        return Err(Arm64SelectionError::CallPack(operation));
    }
    let pack_frame = context
        .frame()
        .pack(pack_id)
        .ok_or(Arm64SelectionError::CallPack(operation))?;
    if pack.segments().len() != pack_frame.state_layout().segments().len() {
        return Err(Arm64SelectionError::CallPack(operation));
    }
    selected.push(Arm64SelectedInstruction::ZeroStack {
        destination: frame_object(pack_frame.state(), 0),
        bytes: pack_frame.state_layout().size(),
    });
    for (segment, layout) in pack
        .segments()
        .iter()
        .zip(pack_frame.state_layout().segments())
    {
        select_segment(
            context,
            operation,
            segment,
            *layout,
            pack_frame.state(),
            selected,
        )?;
    }
    select_descriptor(
        context,
        operation,
        pack_id,
        pack,
        pack_abi.pointer().first(),
        selected,
    )
}

fn select_segment(
    context: crate::Arm64SelectionContext<'_>,
    operation: MachineOperationId,
    segment: &MachinePackSegment,
    layout: Arm64PackSegmentLayout,
    state: crate::Arm64FrameObjectId,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    match (segment, layout) {
        (
            MachinePackSegment::Value { value, .. },
            Arm64PackSegmentLayout::Value {
                value_offset, size, ..
            },
        ) => copy_value_to_frame(
            context,
            operation,
            *value,
            state,
            value_offset,
            size,
            selected,
        ),
        (
            MachinePackSegment::Spread(spread),
            Arm64PackSegmentLayout::Spread {
                remaining_offset,
                iterator_offset,
                iterator_size,
                ..
            },
        ) => {
            copy_value_to_frame(
                context,
                operation,
                spread.remaining(),
                state,
                remaining_offset,
                crate::Arm64NocterAbi::WORD_SIZE,
                selected,
            )?;
            if iterator_size != 0 {
                let source = context
                    .addresses()
                    .use_address(spread.iterator(), selected)?;
                selected.push(Arm64SelectedInstruction::CopyMemoryNonOverlapping {
                    destination: frame_memory(state, iterator_offset),
                    source,
                    bytes: iterator_size,
                });
            }
            Ok(())
        }
        _ => Err(Arm64SelectionError::CallPack(operation)),
    }
}

fn copy_value_to_frame(
    context: crate::Arm64SelectionContext<'_>,
    operation: MachineOperationId,
    value: MachineValueId,
    destination: crate::Arm64FrameObjectId,
    offset: u64,
    expected_size: u64,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let machine_value = context
        .program()
        .function(context.owner())
        .and_then(|function| function.body().value(value))
        .ok_or(Arm64SelectionError::UnknownValue(value))?;
    let MachineValueRepresentation::Stored { size, .. } = machine_value.representation() else {
        return Err(Arm64SelectionError::CallPack(operation));
    };
    if size != expected_size {
        return Err(Arm64SelectionError::CallPack(operation));
    }
    match context
        .values()
        .value(value)
        .ok_or(Arm64SelectionError::UnknownValue(value))?
    {
        Arm64ValueStorage::Omitted if size == 0 => Ok(()),
        Arm64ValueStorage::Direct(registers) => {
            let expected_words = size.div_ceil(crate::Arm64NocterAbi::WORD_SIZE);
            if usize::try_from(expected_words).ok() != Some(registers.len()) {
                return Err(Arm64SelectionError::CallPack(operation));
            }
            for (lane, register) in registers.iter().copied().enumerate() {
                let lane_offset = u64::try_from(lane)
                    .ok()
                    .and_then(|lane| lane.checked_mul(crate::Arm64NocterAbi::WORD_SIZE))
                    .ok_or(Arm64SelectionError::AddressOverflow)?;
                let width =
                    u8::try_from((size - lane_offset).min(crate::Arm64NocterAbi::WORD_SIZE))
                        .map_err(|_| Arm64SelectionError::AddressOverflow)?;
                selected.push(Arm64SelectedInstruction::StoreMemory {
                    bytes: width,
                    destination: frame_memory(
                        destination,
                        offset
                            .checked_add(lane_offset)
                            .ok_or(Arm64SelectionError::AddressOverflow)?,
                    ),
                    source: Arm64SelectedRegister::Virtual(register),
                });
            }
            Ok(())
        }
        Arm64ValueStorage::Memory { size: actual, .. } if *actual == size => {
            let source = context
                .frame()
                .memory_value(value)
                .ok_or(Arm64SelectionError::MemoryValue(value))?;
            selected.push(Arm64SelectedInstruction::CopyMemoryNonOverlapping {
                destination: frame_memory(destination, offset),
                source: frame_memory(source, 0),
                bytes: size,
            });
            Ok(())
        }
        Arm64ValueStorage::Omitted | Arm64ValueStorage::Memory { .. } => {
            Err(Arm64SelectionError::CallPack(operation))
        }
    }
}

fn select_descriptor(
    context: crate::Arm64SelectionContext<'_>,
    operation: MachineOperationId,
    pack_id: MachinePackId,
    pack: &nocter_machine::MachinePack,
    pointer_register: u8,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let frame = context
        .frame()
        .pack(pack_id)
        .ok_or(Arm64SelectionError::CallPack(operation))?;
    selected.push(Arm64SelectedInstruction::ZeroStack {
        destination: frame_object(frame.descriptor(), 0),
        bytes: Arm64PackDescriptorLayout::SIZE,
    });
    let scratch = Arm64SelectedRegister::Fixed(crate::frame_access::scratch(0));
    selected.push(Arm64SelectedInstruction::MemoryAddress {
        destination: scratch,
        source: frame_memory(frame.state(), 0),
    });
    selected.push(Arm64SelectedInstruction::StoreMemory {
        bytes: word_bytes(),
        destination: frame_memory(
            frame.descriptor(),
            Arm64PackDescriptorLayout::STATE_POINTER_OFFSET,
        ),
        source: scratch,
    });
    copy_value_to_frame(
        context,
        operation,
        pack.length(),
        frame.descriptor(),
        Arm64PackDescriptorLayout::LENGTH_OFFSET,
        crate::Arm64NocterAbi::WORD_SIZE,
        selected,
    )?;
    for (kind, offset) in [
        (
            Arm64PackCallbackKind::Next,
            Arm64PackDescriptorLayout::NEXT_CALLBACK_OFFSET,
        ),
        (
            Arm64PackCallbackKind::Destroy,
            Arm64PackDescriptorLayout::DESTROY_CALLBACK_OFFSET,
        ),
    ] {
        selected.push(Arm64SelectedInstruction::LoadPackCallbackAddress {
            destination: scratch,
            pack: pack_id,
            kind,
        });
        selected.push(Arm64SelectedInstruction::StoreMemory {
            bytes: word_bytes(),
            destination: frame_memory(frame.descriptor(), offset),
            source: scratch,
        });
    }
    let pointer = crate::Arm64NocterAbi::argument_register(pointer_register)
        .ok_or(Arm64SelectionError::CallPack(operation))?;
    selected.push(Arm64SelectedInstruction::MemoryAddress {
        destination: Arm64SelectedRegister::Fixed(pointer),
        source: frame_memory(frame.descriptor(), 0),
    });
    Ok(())
}

pub(crate) fn select_pack_operation(
    context: crate::Arm64SelectionContext<'_>,
    operation: MachineOperationId,
    kind: &nocter_machine::MachineOperationKind,
    result: Option<MachineValueId>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let function = context
        .program()
        .function(context.owner())
        .ok_or(Arm64SelectionError::UnknownFunction(context.owner()))?;
    let nocter_machine::MachineFunctionKind::Callable(abi) = function.kind() else {
        return Err(Arm64SelectionError::PackAbi(function.linkage()));
    };
    let pack = abi
        .pack()
        .ok_or(Arm64SelectionError::PackAbi(function.linkage()))?;
    let pointer = context
        .frame()
        .pack_input_pointer()
        .ok_or(Arm64SelectionError::PackAbi(function.linkage()))?;
    let descriptor = Arm64SelectedRegister::Fixed(crate::frame_access::scratch(0));
    selected.push(Arm64SelectedInstruction::LoadMemory {
        bytes: word_bytes(),
        extension: Arm64SelectedLoadExtension::Zero,
        destination: descriptor,
        source: frame_memory(pointer, 0),
    });
    match kind {
        nocter_machine::MachineOperationKind::PackLength => {
            let result = result.ok_or(Arm64SelectionError::MissingResult(operation))?;
            let [destination] = crate::selection::direct_value(context.values(), result)? else {
                return Err(Arm64SelectionError::ExpectedOneWord(result));
            };
            selected.push(Arm64SelectedInstruction::LoadMemory {
                bytes: word_bytes(),
                extension: Arm64SelectedLoadExtension::Zero,
                destination: Arm64SelectedRegister::Virtual(*destination),
                source: Arm64SelectedMemoryAddress::Register {
                    base: descriptor,
                    offset: Arm64PackDescriptorLayout::LENGTH_OFFSET,
                },
            });
            Ok(())
        }
        nocter_machine::MachineOperationKind::PackNext => select_callback(
            operation,
            Arm64PackDescriptorLayout::NEXT_CALLBACK_OFFSET,
            Some((pack.next_result(), result)),
            context.frame(),
            context.values(),
            descriptor,
            selected,
        ),
        nocter_machine::MachineOperationKind::DestroyPack => select_callback(
            operation,
            Arm64PackDescriptorLayout::DESTROY_CALLBACK_OFFSET,
            None,
            context.frame(),
            context.values(),
            descriptor,
            selected,
        ),
        _ => Err(Arm64SelectionError::UnsupportedOperation {
            operation,
            kind: "non-pack",
        }),
    }
}

fn select_callback(
    operation: MachineOperationId,
    callback_offset: u64,
    result: Option<(nocter_machine::MachineResultAbi, Option<MachineValueId>)>,
    frame: &crate::Arm64FunctionFrame,
    values: &crate::Arm64ValuePlan,
    descriptor: Arm64SelectedRegister,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let callback = Arm64SelectedRegister::Fixed(crate::frame_access::scratch(1));
    selected.push(Arm64SelectedInstruction::LoadMemory {
        bytes: word_bytes(),
        extension: Arm64SelectedLoadExtension::Zero,
        destination: callback,
        source: Arm64SelectedMemoryAddress::Register {
            base: descriptor,
            offset: callback_offset,
        },
    });
    selected.push(Arm64SelectedInstruction::LoadMemory {
        bytes: word_bytes(),
        extension: Arm64SelectedLoadExtension::Zero,
        destination: Arm64SelectedRegister::Fixed(
            crate::Arm64NocterAbi::argument_register(0)
                .ok_or(Arm64SelectionError::CallPack(operation))?,
        ),
        source: Arm64SelectedMemoryAddress::Register {
            base: descriptor,
            offset: Arm64PackDescriptorLayout::STATE_POINTER_OFFSET,
        },
    });
    if let Some((abi, result)) = result {
        crate::call_selection::select_call_result_storage(
            operation, abi, result, values, frame, selected,
        )?;
    }
    crate::allocation_selection::select_current(operation, frame, selected)?;
    crate::process_selection::select_callback_current(operation, frame, selected)?;
    selected.push(Arm64SelectedInstruction::CallRegister(callback));
    if let Some((abi, result)) = result {
        crate::call_selection::select_call_result(operation, abi, result, values, selected)?;
    }
    Ok(())
}

const fn frame_object(object: crate::Arm64FrameObjectId, offset: u64) -> Arm64SelectedStackAddress {
    Arm64SelectedStackAddress::FrameObject { object, offset }
}

const fn frame_memory(
    object: crate::Arm64FrameObjectId,
    offset: u64,
) -> Arm64SelectedMemoryAddress {
    Arm64SelectedMemoryAddress::Stack(frame_object(object, offset))
}

fn word_bytes() -> u8 {
    u8::try_from(crate::Arm64NocterAbi::WORD_SIZE).expect("ARM64 word width fits u8")
}
