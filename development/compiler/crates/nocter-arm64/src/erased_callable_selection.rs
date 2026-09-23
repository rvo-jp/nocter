use nocter_machine::{MachineOperationId, MachineValueId};

use crate::{
    Arm64DataSize, Arm64NocterAbi, Arm64SelectedInstruction, Arm64SelectedMemoryAddress,
    Arm64SelectedRegister, Arm64SelectedStackAddress, Arm64SelectionContext, Arm64SelectionError,
    Arm64ValueStorage,
};

pub(crate) fn select_construct(
    operation: MachineOperationId,
    erased: nocter_machine::MachineErasedCallable,
    result: Option<MachineValueId>,
    context: Arm64SelectionContext<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let result = result.ok_or(Arm64SelectionError::MissingResult(operation))?;
    let function = context
        .program()
        .function(context.owner())
        .ok_or(Arm64SelectionError::UnknownFunction(context.owner()))?;
    let result_ty = function
        .body()
        .value(result)
        .map(nocter_machine::MachineValue::ty)
        .ok_or(Arm64SelectionError::UnknownValue(result))?;
    let Some(nocter_machine::MachineLayoutKind::ErasedCallable {
        environment_offset,
        invoke_offset,
        destroy_offset,
        allocation_size_offset,
    }) = context
        .program()
        .layouts()
        .get(result_ty)
        .map(nocter_machine::MachineLayout::kind)
    else {
        return Err(Arm64SelectionError::MemoryShape(result));
    };
    let environment_layout = context
        .program()
        .layouts()
        .get(erased.environment_ty())
        .ok_or(Arm64SelectionError::MemoryShape(erased.environment()))?;
    let environment_size = environment_layout.size();
    let staged = stage_environment(erased.environment(), environment_size, context, selected)?;
    let destination = erased_destination(result, context)?;

    let mapped_size = environment_size.max(1);
    selected.push(Arm64SelectedInstruction::LoadImmediate {
        size: Arm64DataSize::Bits64,
        destination: fixed_argument(1)?,
        value: mapped_size,
    });
    selected.push(Arm64SelectedInstruction::DarwinMemoryMapAbort);
    selected.push(Arm64SelectedInstruction::StoreMemory {
        bytes: word_bytes(),
        destination: stack_offset(destination, *environment_offset)?,
        source: fixed_argument(0)?,
    });
    if let Some(source) = staged {
        selected.push(Arm64SelectedInstruction::CopyMemoryNonOverlapping {
            destination: Arm64SelectedMemoryAddress::Register {
                base: fixed_argument(0)?,
                offset: 0,
            },
            source: Arm64SelectedMemoryAddress::Stack(source),
            bytes: environment_size,
        });
    }

    let scratch = Arm64SelectedRegister::Fixed(
        Arm64NocterAbi::compiler_scratch_register(0).ok_or(Arm64SelectionError::AddressOverflow)?,
    );
    selected.push(Arm64SelectedInstruction::LoadFunctionAddress {
        destination: scratch,
        source: erased.invoke(),
    });
    selected.push(Arm64SelectedInstruction::StoreMemory {
        bytes: word_bytes(),
        destination: stack_offset(destination, *invoke_offset)?,
        source: scratch,
    });
    if let Some(destroy) = erased.destroy() {
        selected.push(Arm64SelectedInstruction::LoadFunctionAddress {
            destination: scratch,
            source: destroy,
        });
    } else {
        selected.push(Arm64SelectedInstruction::LoadImmediate {
            size: Arm64DataSize::Bits64,
            destination: scratch,
            value: 0,
        });
    }
    selected.push(Arm64SelectedInstruction::StoreMemory {
        bytes: word_bytes(),
        destination: stack_offset(destination, *destroy_offset)?,
        source: scratch,
    });
    selected.push(Arm64SelectedInstruction::LoadImmediate {
        size: Arm64DataSize::Bits64,
        destination: scratch,
        value: mapped_size,
    });
    selected.push(Arm64SelectedInstruction::StoreMemory {
        bytes: word_bytes(),
        destination: stack_offset(destination, *allocation_size_offset)?,
        source: scratch,
    });
    Ok(())
}

fn erased_destination(
    result: MachineValueId,
    context: Arm64SelectionContext<'_>,
) -> Result<Arm64SelectedStackAddress, Arm64SelectionError> {
    match context
        .values()
        .value(result)
        .ok_or(Arm64SelectionError::UnknownValue(result))?
    {
        Arm64ValueStorage::Memory { .. } => Ok(Arm64SelectedStackAddress::FrameObject {
            object: context
                .frame()
                .memory_value(result)
                .ok_or(Arm64SelectionError::MemoryValue(result))?,
            offset: 0,
        }),
        _ => Err(Arm64SelectionError::MemoryShape(result)),
    }
}

fn stage_environment(
    value: MachineValueId,
    size: u64,
    context: Arm64SelectionContext<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<Option<Arm64SelectedStackAddress>, Arm64SelectionError> {
    if size == 0 {
        return Ok(None);
    }
    match context
        .values()
        .value(value)
        .ok_or(Arm64SelectionError::UnknownValue(value))?
    {
        Arm64ValueStorage::Memory { size: stored, .. } if *stored == size => context
            .frame()
            .memory_value(value)
            .map(|object| Some(Arm64SelectedStackAddress::FrameObject { object, offset: 0 }))
            .ok_or(Arm64SelectionError::MemoryValue(value)),
        Arm64ValueStorage::Direct(registers) => {
            let destination = staging(context)?;
            let widths = crate::memory_selection::direct_lane_sizes(size, registers.len())?;
            for (lane, (register, bytes)) in registers.iter().copied().zip(widths).enumerate() {
                selected.push(Arm64SelectedInstruction::StoreMemory {
                    bytes,
                    destination: stack_offset(
                        destination,
                        crate::memory_selection::lane_offset(lane)?,
                    )?,
                    source: Arm64SelectedRegister::Virtual(register),
                });
            }
            Ok(Some(destination))
        }
        Arm64ValueStorage::Floating { register, bytes } if u64::from(*bytes) == size => {
            let destination = staging(context)?;
            let size = match bytes {
                4 => Arm64DataSize::Bits32,
                8 => Arm64DataSize::Bits64,
                _ => return Err(Arm64SelectionError::MemoryShape(value)),
            };
            selected.push(Arm64SelectedInstruction::FloatStoreMemory {
                size,
                destination: Arm64SelectedMemoryAddress::Stack(destination),
                source: crate::Arm64SelectedFloatRegister::Virtual(*register),
            });
            Ok(Some(destination))
        }
        Arm64ValueStorage::Omitted if size == 0 => Ok(None),
        Arm64ValueStorage::Omitted
        | Arm64ValueStorage::Floating { .. }
        | Arm64ValueStorage::Memory { .. } => Err(Arm64SelectionError::MemoryShape(value)),
    }
}

fn staging(
    context: Arm64SelectionContext<'_>,
) -> Result<Arm64SelectedStackAddress, Arm64SelectionError> {
    context
        .frame()
        .direct_memory_staging()
        .map(|object| Arm64SelectedStackAddress::FrameObject { object, offset: 0 })
        .ok_or(Arm64SelectionError::MissingDirectMemoryStaging)
}

fn fixed_argument(index: u8) -> Result<Arm64SelectedRegister, Arm64SelectionError> {
    Arm64NocterAbi::argument_register(index)
        .map(Arm64SelectedRegister::Fixed)
        .ok_or(Arm64SelectionError::AddressOverflow)
}

fn stack_offset(
    base: Arm64SelectedStackAddress,
    offset: u64,
) -> Result<Arm64SelectedMemoryAddress, Arm64SelectionError> {
    crate::memory_selection::offset_stack_address(base, offset)
        .map(Arm64SelectedMemoryAddress::Stack)
}

fn word_bytes() -> u8 {
    u8::try_from(Arm64NocterAbi::word_size()).expect("ARM64 word width fits u8")
}
