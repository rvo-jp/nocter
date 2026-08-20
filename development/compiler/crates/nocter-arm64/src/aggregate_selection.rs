use nocter_machine::{
    MachineAggregate, MachineAggregateWrite, MachineFunctionId, MachineValueId,
    MachineValueRepresentation,
};

use crate::{
    Arm64DataSize, Arm64FunctionFrame, Arm64NocterAbi, Arm64SelectedInstruction,
    Arm64SelectedLoadExtension, Arm64SelectedMemoryAddress, Arm64SelectedRegister,
    Arm64SelectedStackAddress, Arm64SelectionError, Arm64ValuePlan, Arm64ValueStorage,
};

/// Selects one aggregate from its layout-owned byte-write recipe.
///
/// Every byte is initialized before member writes. Memory values are assembled in their stable
/// value object. Direct values use the function's shared construction object and are read into
/// their allocated lanes immediately after assembly.
pub(crate) fn select_aggregate(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    aggregate: &MachineAggregate,
    result: MachineValueId,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    validate_result_layout(program, owner, result, aggregate)?;
    let storage = values
        .value(result)
        .ok_or(Arm64SelectionError::UnknownValue(result))?;
    if matches!(storage, Arm64ValueStorage::Omitted) {
        validate_omitted_aggregate(program, owner, result, aggregate)?;
        return Ok(());
    }

    let destination = aggregate_destination(frame, result, storage)?;
    selected.push(Arm64SelectedInstruction::ZeroStack {
        destination,
        bytes: aggregate.size(),
    });
    let write_selection = AggregateWriteSelection {
        program,
        owner,
        result,
        aggregate_size: aggregate.size(),
        destination,
        values,
        frame,
    };
    for write in aggregate.writes() {
        write_selection.select(*write, selected)?;
    }
    if let Arm64ValueStorage::Direct(registers) = storage {
        let sizes = crate::memory_selection::direct_lane_sizes(aggregate.size(), registers.len())?;
        for (lane, (register, bytes)) in registers.iter().copied().zip(sizes).enumerate() {
            selected.push(Arm64SelectedInstruction::LoadMemory {
                bytes,
                extension: Arm64SelectedLoadExtension::Zero,
                destination: Arm64SelectedRegister::Virtual(register),
                source: Arm64SelectedMemoryAddress::Stack(
                    crate::memory_selection::offset_stack_address(
                        destination,
                        crate::memory_selection::lane_offset(lane)?,
                    )?,
                ),
            });
        }
    }
    Ok(())
}

struct AggregateWriteSelection<'a> {
    program: &'a nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    result: MachineValueId,
    aggregate_size: u64,
    destination: Arm64SelectedStackAddress,
    values: &'a Arm64ValuePlan,
    frame: &'a Arm64FunctionFrame,
}

impl AggregateWriteSelection<'_> {
    fn select(
        &self,
        write: MachineAggregateWrite,
        selected: &mut Vec<Arm64SelectedInstruction>,
    ) -> Result<(), Arm64SelectionError> {
        match write {
            MachineAggregateWrite::Tag { offset, value } => {
                validate_write_bounds(self.result, self.aggregate_size, offset, 1)?;
                let scratch = Arm64SelectedRegister::Fixed(aggregate_scratch());
                selected.push(Arm64SelectedInstruction::LoadImmediate {
                    size: Arm64DataSize::Bits32,
                    destination: scratch,
                    value: u64::from(value),
                });
                selected.push(Arm64SelectedInstruction::StoreMemory {
                    bytes: 1,
                    destination: Arm64SelectedMemoryAddress::Stack(
                        crate::memory_selection::offset_stack_address(self.destination, offset)?,
                    ),
                    source: scratch,
                });
                Ok(())
            }
            MachineAggregateWrite::Value { offset, value } => {
                self.select_value(offset, value, selected)
            }
        }
    }

    fn select_value(
        &self,
        offset: u64,
        value: MachineValueId,
        selected: &mut Vec<Arm64SelectedInstruction>,
    ) -> Result<(), Arm64SelectionError> {
        let size = stored_value_size(self.program, self.owner, value)?;
        validate_write_bounds(self.result, self.aggregate_size, offset, size)?;
        let destination = crate::memory_selection::offset_stack_address(self.destination, offset)?;
        match self
            .values
            .value(value)
            .ok_or(Arm64SelectionError::UnknownValue(value))?
        {
            Arm64ValueStorage::Omitted if size == 0 => Ok(()),
            Arm64ValueStorage::Direct(registers) => {
                let sizes = crate::memory_selection::direct_lane_sizes(size, registers.len())?;
                for (lane, (register, bytes)) in registers.iter().copied().zip(sizes).enumerate() {
                    selected.push(Arm64SelectedInstruction::StoreMemory {
                        bytes,
                        destination: Arm64SelectedMemoryAddress::Stack(
                            crate::memory_selection::offset_stack_address(
                                destination,
                                crate::memory_selection::lane_offset(lane)?,
                            )?,
                        ),
                        source: Arm64SelectedRegister::Virtual(register),
                    });
                }
                Ok(())
            }
            Arm64ValueStorage::Memory { size: stored, .. } if *stored == size => {
                let source = self
                    .frame
                    .memory_value(value)
                    .ok_or(Arm64SelectionError::MemoryValue(value))?;
                let source = Arm64SelectedStackAddress::FrameObject {
                    object: source,
                    offset: 0,
                };
                if source == destination {
                    return Err(Arm64SelectionError::AggregateStorageAlias(self.result));
                }
                selected.push(Arm64SelectedInstruction::CopyMemoryNonOverlapping {
                    destination: Arm64SelectedMemoryAddress::Stack(destination),
                    source: Arm64SelectedMemoryAddress::Stack(source),
                    bytes: size,
                });
                Ok(())
            }
            Arm64ValueStorage::Omitted | Arm64ValueStorage::Memory { .. } => {
                Err(Arm64SelectionError::AggregateValueShape(value))
            }
        }
    }
}

fn aggregate_destination(
    frame: &Arm64FunctionFrame,
    result: MachineValueId,
    storage: &Arm64ValueStorage,
) -> Result<Arm64SelectedStackAddress, Arm64SelectionError> {
    let object = match storage {
        Arm64ValueStorage::Direct(_) => frame
            .direct_aggregate_staging()
            .ok_or(Arm64SelectionError::MissingAggregateStaging)?,
        Arm64ValueStorage::Memory { .. } => frame
            .memory_value(result)
            .ok_or(Arm64SelectionError::MemoryValue(result))?,
        Arm64ValueStorage::Omitted => return Err(Arm64SelectionError::AggregateValueShape(result)),
    };
    Ok(Arm64SelectedStackAddress::FrameObject { object, offset: 0 })
}

fn validate_result_layout(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    result: MachineValueId,
    aggregate: &MachineAggregate,
) -> Result<(), Arm64SelectionError> {
    match program
        .function(owner)
        .and_then(|function| function.body().value(result))
        .map(nocter_machine::MachineValue::representation)
    {
        Some(MachineValueRepresentation::Stored { size, alignment })
            if size == aggregate.size() && alignment == aggregate.alignment() =>
        {
            Ok(())
        }
        Some(_) => Err(Arm64SelectionError::AggregateValueShape(result)),
        None => Err(Arm64SelectionError::UnknownValue(result)),
    }
}

fn validate_omitted_aggregate(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    result: MachineValueId,
    aggregate: &MachineAggregate,
) -> Result<(), Arm64SelectionError> {
    if aggregate.size() != 0 {
        return Err(Arm64SelectionError::AggregateValueShape(result));
    }
    for write in aggregate.writes() {
        match *write {
            MachineAggregateWrite::Tag { .. } => {
                return Err(Arm64SelectionError::AggregateWriteBounds(result));
            }
            MachineAggregateWrite::Value { value, .. }
                if stored_value_size(program, owner, value)? != 0 =>
            {
                return Err(Arm64SelectionError::AggregateWriteBounds(result));
            }
            MachineAggregateWrite::Value { .. } => {}
        }
    }
    Ok(())
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
        Some(MachineValueRepresentation::Stored { size, .. }) => Ok(size),
        Some(MachineValueRepresentation::Completion | MachineValueRepresentation::Diverging) => {
            Err(Arm64SelectionError::AggregateValueShape(value))
        }
        None => Err(Arm64SelectionError::UnknownValue(value)),
    }
}

fn validate_write_bounds(
    result: MachineValueId,
    aggregate_size: u64,
    offset: u64,
    size: u64,
) -> Result<(), Arm64SelectionError> {
    let in_bounds = offset
        .checked_add(size)
        .is_some_and(|end| end <= aggregate_size);
    if in_bounds {
        Ok(())
    } else {
        Err(Arm64SelectionError::AggregateWriteBounds(result))
    }
}

fn aggregate_scratch() -> crate::Arm64Register {
    Arm64NocterAbi::compiler_scratch_register(1)
        .expect("the ABI reserves x17 for aggregate tag materialization")
}
