use nocter_machine::{
    MachineArgumentLocation, MachineLayoutKind, MachineOperationId, MachinePrimitiveTarget,
    MachineResultAbi, MachineResultLocation, MachineValueClass,
};
use nocter_runtime_contract::PrimitiveRole;

use crate::{
    Arm64NocterAbi, Arm64SelectedInstruction, Arm64SelectedMemoryAddress, Arm64SelectedRegister,
    Arm64SelectionContext, Arm64SelectionError,
};

pub(crate) fn select_operation(
    operation: MachineOperationId,
    source: &nocter_machine::MachineOperation,
    context: Arm64SelectionContext<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    match source.kind() {
        nocter_machine::MachineOperationKind::ReportError { place } => {
            select_report(operation, *place, source.result(), context, selected)
        }
        nocter_machine::MachineOperationKind::ReleaseError { place } => {
            select_release(operation, *place, source.result(), context, selected)
        }
        _ => unreachable!("the caller routes only error lifetime operations"),
    }
}

fn select_report(
    operation: MachineOperationId,
    place: nocter_machine::MachineAddressId,
    result: Option<nocter_machine::MachineValueId>,
    context: Arm64SelectionContext<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if result.is_some() {
        return Err(Arm64SelectionError::ErrorReport(operation));
    }
    let address = context
        .program()
        .function(context.owner())
        .and_then(|function| function.body().address(place))
        .ok_or(Arm64SelectionError::ErrorReport(operation))?;
    let layout = context
        .program()
        .layouts()
        .get(address.ty())
        .ok_or(Arm64SelectionError::ErrorReport(operation))?;
    let error_schema = context.program().layouts().target().error();
    if layout.size() != error_schema.size()
        || layout.alignment() != error_schema.alignment()
        || !matches!(layout.kind(), MachineLayoutKind::ErrorHandle)
        || address.stored_size() != Some(error_schema.size())
        || address.stored_alignment() != Some(error_schema.alignment())
    {
        return Err(Arm64SelectionError::ErrorReport(operation));
    }
    let place = context.addresses().use_address(place, selected)?;
    let buffer = context
        .frame()
        .error_report_buffer()
        .ok_or(Arm64SelectionError::ErrorReport(operation))?;
    selected.push(Arm64SelectedInstruction::ReportError { place, buffer });
    Ok(())
}

fn select_release(
    operation: MachineOperationId,
    place: nocter_machine::MachineAddressId,
    result: Option<nocter_machine::MachineValueId>,
    context: Arm64SelectionContext<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if result.is_some() {
        return Err(Arm64SelectionError::ErrorReport(operation));
    }
    let place = context.addresses().use_address(place, selected)?;
    selected.push(Arm64SelectedInstruction::ReleaseError { place });
    Ok(())
}

pub(crate) fn select_primitive(
    operation: MachineOperationId,
    target: &MachinePrimitiveTarget,
    frame: &crate::Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if !target.type_arguments().is_empty()
        || target.abi().pack().is_some()
        || target.abi().stack_argument_size() != 0
    {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    let instruction = match target.role() {
        PrimitiveRole::NewError => {
            validate_arguments(operation, target, &[(0, 2), (2, 2)])?;
            validate_result(operation, target, 1)?;
            let buffer = construction_buffer(operation, frame)?;
            stage_arguments(buffer, 4, selected);
            Arm64SelectedInstruction::ConstructErrorLeaf { buffer }
        }
        PrimitiveRole::ErrorContext => {
            validate_arguments(operation, target, &[(0, 1), (1, 2)])?;
            validate_result(operation, target, 1)?;
            let buffer = construction_buffer(operation, frame)?;
            stage_arguments(buffer, 3, selected);
            Arm64SelectedInstruction::ConstructErrorContext { buffer }
        }
        PrimitiveRole::ErrorCode => {
            validate_arguments(operation, target, &[(0, 1)])?;
            validate_result(operation, target, 2)?;
            Arm64SelectedInstruction::ReadErrorCode
        }
        PrimitiveRole::ErrorMessage => {
            validate_arguments(operation, target, &[(0, 1)])?;
            validate_result(operation, target, 2)?;
            Arm64SelectedInstruction::ReadErrorMessage
        }
        PrimitiveRole::AllocationFailureError => {
            validate_arguments(operation, target, &[])?;
            validate_result(operation, target, 1)?;
            Arm64SelectedInstruction::LoadAllocationFailureError
        }
        _ => unreachable!("the caller routes only built-in error primitives"),
    };
    selected.push(instruction);
    Ok(())
}

fn validate_arguments(
    operation: MachineOperationId,
    target: &MachinePrimitiveTarget,
    expected: &[(u8, u8)],
) -> Result<(), Arm64SelectionError> {
    if target.abi().arguments().len() != expected.len() {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    for (argument, (first, words)) in target.abi().arguments().iter().zip(expected) {
        let Some(MachineArgumentLocation::Registers(registers)) = argument.location() else {
            return Err(Arm64SelectionError::PrimitiveCall(operation));
        };
        if argument.class() != (MachineValueClass::Direct { words: *words })
            || registers.first() != *first
            || registers.words() != *words
        {
            return Err(Arm64SelectionError::PrimitiveCall(operation));
        }
    }
    Ok(())
}

fn validate_result(
    operation: MachineOperationId,
    target: &MachinePrimitiveTarget,
    words: u8,
) -> Result<(), Arm64SelectionError> {
    let MachineResultAbi::Value(result) = target.abi().result() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    let MachineResultLocation::Registers(registers) = result.location() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    if result.class() != (MachineValueClass::Direct { words })
        || registers.first() != 0
        || registers.words() != words
    {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    Ok(())
}

fn construction_buffer(
    operation: MachineOperationId,
    frame: &crate::Arm64FunctionFrame,
) -> Result<crate::Arm64FrameObjectId, Arm64SelectionError> {
    frame
        .error_construction_buffer()
        .ok_or(Arm64SelectionError::PrimitiveCall(operation))
}

fn stage_arguments(
    buffer: crate::Arm64FrameObjectId,
    words: u8,
    selected: &mut Vec<Arm64SelectedInstruction>,
) {
    for lane in 0..words {
        selected.push(Arm64SelectedInstruction::StoreMemory {
            bytes: u8::try_from(Arm64NocterAbi::word_size())
                .expect("the target word width fits an instruction byte count"),
            destination: Arm64SelectedMemoryAddress::Stack(
                crate::Arm64SelectedStackAddress::FrameObject {
                    object: buffer,
                    offset: u64::from(lane) * Arm64NocterAbi::word_size(),
                },
            ),
            source: Arm64SelectedRegister::Fixed(
                Arm64NocterAbi::argument_register(lane)
                    .expect("validated error arguments fit the ABI register window"),
            ),
        });
    }
}
