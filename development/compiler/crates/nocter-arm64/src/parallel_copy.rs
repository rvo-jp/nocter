use crate::parallel_copy_schedule::{ParallelAction, ParallelCopy, ParallelSource, schedule};
use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64AllocatedLocation, Arm64BaseRegister,
    Arm64CodeBuilder, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MaterializationError, Arm64NocterAbi, Arm64Register, Arm64SelectedCopy,
    Arm64SelectedFunction, Arm64SelectedRegister,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Location {
    Register(Arm64Register),
    Spill(u64),
}

pub(crate) fn emit(
    function: &Arm64SelectedFunction,
    copies: &[Arm64SelectedCopy],
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let pending = copies
        .iter()
        .map(|copy| {
            Ok(ParallelCopy {
                destination: selected_location(function, copy.destination())?,
                source: ParallelSource::Location(selected_location(function, copy.source())?),
                metadata: (),
            })
        })
        .collect::<Result<Vec<_>, Arm64MaterializationError>>()?;
    for action in schedule(pending)? {
        match action {
            ParallelAction::Save(source) => save_temporary(source, code),
            ParallelAction::Copy {
                destination,
                source,
                metadata: (),
            } => emit_copy(source, destination, code),
        }
    }
    Ok(())
}

fn selected_location(
    function: &Arm64SelectedFunction,
    selected: Arm64SelectedRegister,
) -> Result<Location, Arm64MaterializationError> {
    match selected {
        Arm64SelectedRegister::Fixed(register) => Ok(Location::Register(register)),
        Arm64SelectedRegister::Virtual(register) => {
            if register.class() != crate::Arm64RegisterClass::General {
                return Err(Arm64MaterializationError::RegisterClassMismatch(register));
            }
            match function
                .values()
                .registers()
                .location(register)
                .ok_or(Arm64MaterializationError::UnknownVirtualRegister(register))?
            {
                Arm64AllocatedLocation::GeneralRegister(register) => {
                    Ok(Location::Register(register))
                }
                Arm64AllocatedLocation::Spill(spill) => {
                    Ok(Location::Spill(spill_offset(function, spill)?))
                }
                Arm64AllocatedLocation::FloatRegister(_) => {
                    Err(Arm64MaterializationError::RegisterClassMismatch(register))
                }
            }
        }
    }
}

fn spill_offset(
    function: &Arm64SelectedFunction,
    spill: crate::Arm64SpillSlotId,
) -> Result<u64, Arm64MaterializationError> {
    let object = function
        .frame()
        .spill(spill)
        .ok_or(Arm64MaterializationError::UnknownSpill(spill))?;
    function
        .frame()
        .layout()
        .object(object)
        .map(crate::Arm64FrameObject::offset)
        .ok_or(Arm64MaterializationError::UnknownFrameObject(object))
}

fn save_temporary(source: Location, code: &mut Arm64CodeBuilder) {
    let temporary = temporary();
    match source {
        Location::Register(source) => emit_register_move(code, temporary, source),
        Location::Spill(offset) => crate::frame_access::load_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            temporary,
            offset,
        ),
    }
}

fn emit_copy(source: ParallelSource<Location>, destination: Location, code: &mut Arm64CodeBuilder) {
    let source = match source {
        ParallelSource::Location(source) => source,
        ParallelSource::Temporary => Location::Register(temporary()),
    };
    match (source, destination) {
        (Location::Register(source), Location::Register(destination)) => {
            emit_register_move(code, destination, source);
        }
        (Location::Register(source), Location::Spill(destination)) => {
            crate::frame_access::store_at_stack_offset(
                code,
                Arm64LoadStoreSize::Double,
                source,
                destination,
            );
        }
        (Location::Spill(source), Location::Register(destination)) => {
            crate::frame_access::load_at_stack_offset(
                code,
                Arm64LoadStoreSize::Double,
                destination,
                source,
            );
        }
        (Location::Spill(source), Location::Spill(destination)) => {
            let scratch = crate::frame_access::scratch(0);
            crate::frame_access::load_at_stack_offset(
                code,
                Arm64LoadStoreSize::Double,
                scratch,
                source,
            );
            crate::frame_access::store_at_stack_offset(
                code,
                Arm64LoadStoreSize::Double,
                scratch,
                destination,
            );
        }
    }
}

fn emit_register_move(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    source: Arm64Register,
) {
    if destination == source {
        return;
    }
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

fn temporary() -> Arm64Register {
    Arm64NocterAbi::argument_register(0)
        .expect("the boundary-only x0 register is available between calls")
}
