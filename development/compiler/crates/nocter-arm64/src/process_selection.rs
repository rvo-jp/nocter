use nocter_machine::{
    MachineArgumentLocation, MachineCallTarget, MachineContextRequirement, MachineFunctionId,
    MachineLayoutKind, MachineOperationId, MachineOutcomeKind, MachineResultAbi,
    MachineResultLocation, MachineValueClass,
};
use nocter_runtime_contract::PrimitiveRole;

use crate::{
    Arm64FunctionFrame, Arm64NocterAbi, Arm64ProcessContextFrame, Arm64ProcessTextResult,
    Arm64ProcessTextSource, Arm64SelectedInstruction, Arm64SelectedLoadExtension,
    Arm64SelectedMemoryAddress, Arm64SelectedRegister, Arm64SelectedStackAddress,
    Arm64SelectionError,
};

pub(crate) fn select_entry(
    program: &nocter_machine::MachineProgram,
    function: MachineFunctionId,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let requirement = program
        .contexts()
        .process()
        .get(function)
        .ok_or(Arm64SelectionError::ProcessEntry(function))?;
    match (requirement, frame.process_context()) {
        (MachineContextRequirement::None, Arm64ProcessContextFrame::None) => Ok(()),
        (
            MachineContextRequirement::ProgramRoot,
            Arm64ProcessContextFrame::ProgramRoot(context),
        ) => {
            selected.push(Arm64SelectedInstruction::InitializeProcessContext { context });
            Ok(())
        }
        (
            MachineContextRequirement::Incoming,
            Arm64ProcessContextFrame::IncomingPointer(pointer),
        ) => {
            selected.push(Arm64SelectedInstruction::StoreMemory {
                bytes: word_bytes(),
                destination: Arm64SelectedMemoryAddress::Stack(
                    Arm64SelectedStackAddress::FrameObject {
                        object: pointer,
                        offset: 0,
                    },
                ),
                source: Arm64SelectedRegister::Fixed(Arm64NocterAbi::process_context_register()),
            });
            Ok(())
        }
        _ => Err(Arm64SelectionError::ProcessEntry(function)),
    }
}

pub(crate) fn select_target(
    program: &nocter_machine::MachineProgram,
    operation: MachineOperationId,
    target: &MachineCallTarget,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if !program
        .contexts()
        .process()
        .target_requires_context(target)?
    {
        return Ok(());
    }
    select_current(operation, frame, selected)
}

pub(crate) fn select_current(
    operation: MachineOperationId,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let destination = Arm64SelectedRegister::Fixed(Arm64NocterAbi::process_context_register());
    match frame.process_context() {
        Arm64ProcessContextFrame::ProgramRoot(object) => {
            selected.push(Arm64SelectedInstruction::MemoryAddress {
                destination,
                source: Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::FrameObject {
                    object,
                    offset: 0,
                }),
            });
            Ok(())
        }
        Arm64ProcessContextFrame::IncomingPointer(object) => {
            selected.push(Arm64SelectedInstruction::LoadMemory {
                bytes: word_bytes(),
                extension: Arm64SelectedLoadExtension::Zero,
                destination,
                source: Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::FrameObject {
                    object,
                    offset: 0,
                }),
            });
            Ok(())
        }
        Arm64ProcessContextFrame::None => Err(Arm64SelectionError::ProcessCall(operation)),
    }
}

/// Reloads process state for an indirect compiler-generated callback when its containing literal
/// function was marked by the fixed-point planner. A context-free containing function proves that
/// neither callback consumes the lane.
pub(crate) fn select_callback_current(
    operation: MachineOperationId,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    match frame.process_context() {
        Arm64ProcessContextFrame::None => Ok(()),
        Arm64ProcessContextFrame::ProgramRoot(_) | Arm64ProcessContextFrame::IncomingPointer(_) => {
            select_current(operation, frame, selected)
        }
    }
}

pub(crate) fn select_primitive(
    program: &nocter_machine::MachineProgram,
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    match target.role() {
        PrimitiveRole::ProcessArgumentCount => {
            validate_abi(operation, target, false, 1)?;
            selected.push(Arm64SelectedInstruction::ReadProcessArgumentCount);
        }
        PrimitiveRole::ProcessEnvironmentCount => {
            validate_abi(operation, target, false, 1)?;
            selected.push(Arm64SelectedInstruction::ReadProcessEnvironmentCount);
        }
        role => {
            let source = match role {
                PrimitiveRole::ProcessArgument => Arm64ProcessTextSource::Argument,
                PrimitiveRole::ProcessEnvironmentName => Arm64ProcessTextSource::EnvironmentName,
                PrimitiveRole::ProcessEnvironmentValue => Arm64ProcessTextSource::EnvironmentValue,
                _ => return Err(Arm64SelectionError::PrimitiveCall(operation)),
            };
            validate_arguments(operation, target, true)?;
            let result = if let Some(result) = checked_text_result(program, target) {
                result
            } else {
                validate_direct_result(operation, target, 2)?;
                Arm64ProcessTextResult::RawView
            };
            selected.push(Arm64SelectedInstruction::ReadProcessText { source, result });
        }
    }
    Ok(())
}

fn validate_abi(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    has_index: bool,
    result_words: u8,
) -> Result<(), Arm64SelectionError> {
    validate_arguments(operation, target, has_index)?;
    validate_direct_result(operation, target, result_words)
}

fn validate_arguments(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    has_index: bool,
) -> Result<(), Arm64SelectionError> {
    if !target.type_arguments().is_empty()
        || target.abi().pack().is_some()
        || target.abi().stack_argument_size() != 0
        || target.abi().arguments().len() != usize::from(has_index)
    {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    if has_index {
        let [argument] = target.abi().arguments() else {
            return Err(Arm64SelectionError::PrimitiveCall(operation));
        };
        let Some(MachineArgumentLocation::Registers(registers)) = argument.location() else {
            return Err(Arm64SelectionError::PrimitiveCall(operation));
        };
        if argument.class() != (MachineValueClass::Direct { words: 1 })
            || registers.first() != 0
            || registers.words() != 1
        {
            return Err(Arm64SelectionError::PrimitiveCall(operation));
        }
    }
    Ok(())
}

fn validate_direct_result(
    operation: MachineOperationId,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
    result_words: u8,
) -> Result<(), Arm64SelectionError> {
    let MachineResultAbi::Value(result) = target.abi().result() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    let MachineResultLocation::Registers(registers) = result.location() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    if result.class()
        != (MachineValueClass::Direct {
            words: result_words,
        })
        || registers.first() != 0
        || registers.words() != result_words
    {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    Ok(())
}

fn checked_text_result(
    program: &nocter_machine::MachineProgram,
    target: super::primitive_selection::Arm64PrimitiveTarget<'_>,
) -> Option<Arm64ProcessTextResult> {
    let MachineResultAbi::Value(result) = target.abi().result() else {
        return None;
    };
    if result.class() != MachineValueClass::Indirect
        || result.location()
            != (MachineResultLocation::CallerStorage {
                pointer_register: Arm64NocterAbi::indirect_result_register().number(),
            })
    {
        return None;
    }
    let optional = |data| match program.layouts().get(data)?.kind() {
        MachineLayoutKind::Outcome {
            kind: MachineOutcomeKind::Optional,
            tag_offset,
            payload_offset,
            primary: Some(payload),
            alternate: None,
        } => Some((*tag_offset, *payload_offset, *payload)),
        _ => None,
    };
    let (entry_tag, encoding, payload) = optional(result.ty())?;
    let (encoding_tag, text, payload) = optional(payload)?;
    let MachineLayoutKind::View {
        pointer_offset,
        length_offset,
    } = program.layouts().get(payload)?.kind()
    else {
        return None;
    };
    let text = encoding.checked_add(text)?;
    Some(Arm64ProcessTextResult::Checked {
        entry_tag,
        encoding_tag: encoding.checked_add(encoding_tag)?,
        pointer: text.checked_add(*pointer_offset)?,
        length: text.checked_add(*length_offset)?,
    })
}

fn word_bytes() -> u8 {
    u8::try_from(Arm64NocterAbi::word_size())
        .expect("the ARM64 word width fits selected byte width")
}
