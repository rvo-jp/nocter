use nocter_machine::{
    MachineArgumentLocation, MachineLayoutKind, MachineOperationId, MachinePrimitiveTarget,
    MachineResultAbi, MachineResultLocation, MachineValueClass,
};

use crate::{
    Arm64NocterAbi, Arm64SelectedInstruction, Arm64SelectedMemoryAddress, Arm64SelectedRegister,
    Arm64SelectionContext, Arm64SelectionError, Arm64ValueStorage,
};

use crate::error_layout::Arm64ErrorLayout;

pub(crate) fn select_report(
    operation: MachineOperationId,
    source: &nocter_machine::MachineOperation,
    context: Arm64SelectionContext<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let nocter_machine::MachineOperationKind::ReportError { error } = source.kind() else {
        unreachable!("error selection accepts only report-error operations")
    };
    if source.result().is_some() {
        return Err(Arm64SelectionError::ErrorReport(operation));
    }
    let value = context
        .program()
        .function(context.owner())
        .and_then(|function| function.body().value(*error))
        .ok_or(Arm64SelectionError::ErrorReport(operation))?;
    let layout = context
        .program()
        .layouts()
        .get(value.ty())
        .ok_or(Arm64SelectionError::ErrorReport(operation))?;
    if layout.size() != Arm64ErrorLayout::SIZE
        || layout.alignment() != Arm64ErrorLayout::ALIGNMENT
        || !matches!(
            layout.kind(),
            MachineLayoutKind::Error {
                code_offset: Arm64ErrorLayout::CODE_OFFSET,
                message_offset: Arm64ErrorLayout::MESSAGE_OFFSET,
            }
        )
        || !matches!(
            context.values().value(*error),
            Some(Arm64ValueStorage::Memory { size, alignment })
                if *size == Arm64ErrorLayout::SIZE && *alignment == Arm64ErrorLayout::ALIGNMENT
        )
    {
        return Err(Arm64SelectionError::ErrorReport(operation));
    }
    let error = context
        .frame()
        .memory_value(*error)
        .ok_or(Arm64SelectionError::ErrorReport(operation))?;
    let buffer = context
        .frame()
        .error_report_buffer()
        .ok_or(Arm64SelectionError::ErrorReport(operation))?;
    selected.push(Arm64SelectedInstruction::ReportError { error, buffer });
    Ok(())
}

pub(crate) fn select_new_error(
    operation: MachineOperationId,
    target: &MachinePrimitiveTarget,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if !target.type_arguments().is_empty()
        || target.abi().arguments().len() != 2
        || target.abi().pack().is_some()
        || target.abi().stack_argument_size() != 0
    {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    for (position, argument) in target.abi().arguments().iter().enumerate() {
        let Some(MachineArgumentLocation::Registers(registers)) = argument.location() else {
            return Err(Arm64SelectionError::PrimitiveCall(operation));
        };
        let first = u8::try_from(position * 2)
            .map_err(|_| Arm64SelectionError::PrimitiveCall(operation))?;
        if argument.class() != (MachineValueClass::Direct { words: 2 })
            || registers.first() != first
            || registers.words() != 2
        {
            return Err(Arm64SelectionError::PrimitiveCall(operation));
        }
    }
    let MachineResultAbi::Value(result) = target.abi().result() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    let MachineResultLocation::CallerStorage { pointer_register } = result.location() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    if result.class() != MachineValueClass::Indirect
        || pointer_register != Arm64NocterAbi::indirect_result_register().number()
    {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    for lane in 0_u8..4 {
        selected.push(Arm64SelectedInstruction::StoreMemory {
            bytes: u8::try_from(Arm64NocterAbi::WORD_SIZE)
                .expect("the target word width fits an instruction byte count"),
            destination: Arm64SelectedMemoryAddress::Register {
                base: Arm64SelectedRegister::Fixed(Arm64NocterAbi::indirect_result_register()),
                offset: u64::from(lane) * Arm64NocterAbi::WORD_SIZE,
            },
            source: Arm64SelectedRegister::Fixed(
                Arm64NocterAbi::argument_register(lane)
                    .expect("the error payload uses four ABI input registers"),
            ),
        });
    }
    Ok(())
}
