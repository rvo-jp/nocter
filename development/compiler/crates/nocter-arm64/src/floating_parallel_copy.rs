use crate::parallel_copy_schedule::{ParallelAction, ParallelCopy, ParallelSource, schedule};
use crate::{
    Arm64AllocatedLocation, Arm64CodeBuilder, Arm64DataSize, Arm64FloatRegister, Arm64Instruction,
    Arm64MaterializationError, Arm64NocterAbi, Arm64SelectedFloatCopy, Arm64SelectedFloatRegister,
    Arm64SelectedFunction,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Location {
    Register(Arm64FloatRegister),
    Spill(u64),
}

pub(crate) fn emit(
    function: &Arm64SelectedFunction,
    copies: &[Arm64SelectedFloatCopy],
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let pending = copies
        .iter()
        .map(|copy| {
            Ok(ParallelCopy {
                destination: selected_location(function, copy.destination())?,
                source: ParallelSource::Location(selected_location(function, copy.source())?),
                metadata: copy.size(),
            })
        })
        .collect::<Result<Vec<_>, Arm64MaterializationError>>()?;
    for action in schedule(pending)? {
        match action {
            ParallelAction::Save(source) => save_temporary(source, code),
            ParallelAction::Copy {
                destination,
                source,
                metadata: size,
            } => emit_copy(source, destination, size, code),
        }
    }
    Ok(())
}

fn selected_location(
    function: &Arm64SelectedFunction,
    selected: Arm64SelectedFloatRegister,
) -> Result<Location, Arm64MaterializationError> {
    match selected {
        Arm64SelectedFloatRegister::Fixed(register) => Ok(Location::Register(register)),
        Arm64SelectedFloatRegister::Virtual(register) => {
            if register.class() != crate::Arm64RegisterClass::Floating {
                return Err(Arm64MaterializationError::RegisterClassMismatch(register));
            }
            match function
                .values()
                .registers()
                .location(register)
                .ok_or(Arm64MaterializationError::UnknownVirtualRegister(register))?
            {
                Arm64AllocatedLocation::FloatRegister(register) => Ok(Location::Register(register)),
                Arm64AllocatedLocation::Spill(spill) => Ok(Location::Spill(
                    crate::selected_code::spill_offset(function, spill)?,
                )),
                Arm64AllocatedLocation::GeneralRegister(_) => {
                    Err(Arm64MaterializationError::RegisterClassMismatch(register))
                }
            }
        }
    }
}

fn save_temporary(source: Location, code: &mut Arm64CodeBuilder) {
    match source {
        Location::Register(source) => {
            move_register(temporary(), source, Arm64DataSize::Bits64, code);
        }
        Location::Spill(offset) => {
            crate::floating_code::load_from_stack(temporary(), offset, Arm64DataSize::Bits64, code);
        }
    }
}

fn emit_copy(
    source: ParallelSource<Location>,
    destination: Location,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) {
    let source = match source {
        ParallelSource::Location(source) => source,
        ParallelSource::Temporary => Location::Register(temporary()),
    };
    match (source, destination) {
        (Location::Register(source), Location::Register(destination)) => {
            move_register(destination, source, size, code);
        }
        (Location::Register(source), Location::Spill(destination)) => {
            crate::floating_code::store_to_stack(source, destination, size, code);
        }
        (Location::Spill(source), Location::Register(destination)) => {
            crate::floating_code::load_from_stack(destination, source, size, code);
        }
        (Location::Spill(source), Location::Spill(destination)) => {
            let scratch = temporary();
            crate::floating_code::load_from_stack(scratch, source, size, code);
            crate::floating_code::store_to_stack(scratch, destination, size, code);
        }
    }
}

fn move_register(
    destination: Arm64FloatRegister,
    source: Arm64FloatRegister,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) {
    if destination != source {
        code.append(Arm64Instruction::FloatMove {
            size,
            destination,
            source,
        });
    }
}

fn temporary() -> Arm64FloatRegister {
    Arm64NocterAbi::floating_argument_register(0)
        .expect("the boundary-only v0 register is available between calls")
}
