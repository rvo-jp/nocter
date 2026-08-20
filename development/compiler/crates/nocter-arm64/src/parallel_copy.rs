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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Source {
    Location(Location),
    Temporary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingCopy {
    destination: Location,
    source: Source,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Save(Location),
    Copy {
        destination: Location,
        source: Source,
    },
}

pub(crate) fn emit(
    function: &Arm64SelectedFunction,
    copies: &[Arm64SelectedCopy],
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let pending = copies
        .iter()
        .map(|copy| {
            Ok(PendingCopy {
                destination: selected_location(function, copy.destination())?,
                source: Source::Location(selected_location(function, copy.source())?),
            })
        })
        .collect::<Result<Vec<_>, Arm64MaterializationError>>()?;
    for action in schedule(pending)? {
        match action {
            Action::Save(source) => save_temporary(source, code),
            Action::Copy {
                destination,
                source,
            } => emit_copy(source, destination, code),
        }
    }
    Ok(())
}

fn schedule(mut pending: Vec<PendingCopy>) -> Result<Vec<Action>, Arm64MaterializationError> {
    pending.retain(|copy| copy.source != Source::Location(copy.destination));
    let mut actions = Vec::with_capacity(pending.len() + 1);
    while !pending.is_empty() {
        if let Some(index) = pending.iter().position(|candidate| {
            !pending
                .iter()
                .any(|copy| copy.source == Source::Location(candidate.destination))
        }) {
            let copy = pending.remove(index);
            actions.push(Action::Copy {
                destination: copy.destination,
                source: copy.source,
            });
            continue;
        }

        let source = pending
            .iter()
            .find_map(|copy| match copy.source {
                Source::Location(source) => Some(source),
                Source::Temporary => None,
            })
            .ok_or(Arm64MaterializationError::InvalidParallelCopy)?;
        actions.push(Action::Save(source));
        for copy in &mut pending {
            if copy.source == Source::Location(source) {
                copy.source = Source::Temporary;
            }
        }
    }
    Ok(actions)
}

fn selected_location(
    function: &Arm64SelectedFunction,
    selected: Arm64SelectedRegister,
) -> Result<Location, Arm64MaterializationError> {
    match selected {
        Arm64SelectedRegister::Fixed(register) => Ok(Location::Register(register)),
        Arm64SelectedRegister::Virtual(register) => match function
            .values()
            .registers()
            .location(register)
            .ok_or(Arm64MaterializationError::UnknownVirtualRegister(register))?
        {
            Arm64AllocatedLocation::Register(register) => Ok(Location::Register(register)),
            Arm64AllocatedLocation::Spill(spill) => {
                Ok(Location::Spill(spill_offset(function, spill)?))
            }
        },
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

fn emit_copy(source: Source, destination: Location, code: &mut Arm64CodeBuilder) {
    let source = match source {
        Source::Location(source) => source,
        Source::Temporary => Location::Register(temporary()),
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

#[cfg(test)]
mod tests {
    use super::{Action, Location, PendingCopy, Source, schedule};

    fn register(number: u8) -> Location {
        Location::Register(crate::Arm64Register::new(number).unwrap())
    }

    fn copy(source: u8, destination: u8) -> PendingCopy {
        PendingCopy {
            destination: register(destination),
            source: Source::Location(register(source)),
        }
    }

    #[test]
    fn orders_acyclic_copies_from_leaves_to_roots() {
        assert_eq!(
            schedule(vec![copy(1, 2), copy(2, 3)]).unwrap(),
            vec![
                Action::Copy {
                    destination: register(3),
                    source: Source::Location(register(2)),
                },
                Action::Copy {
                    destination: register(2),
                    source: Source::Location(register(1)),
                },
            ]
        );
    }

    #[test]
    fn breaks_cycles_once_and_preserves_the_saved_value() {
        assert_eq!(
            schedule(vec![copy(1, 2), copy(2, 1)]).unwrap(),
            vec![
                Action::Save(register(1)),
                Action::Copy {
                    destination: register(1),
                    source: Source::Location(register(2)),
                },
                Action::Copy {
                    destination: register(2),
                    source: Source::Temporary,
                },
            ]
        );
    }

    #[test]
    fn one_saved_source_can_feed_multiple_cycle_destinations() {
        assert_eq!(
            schedule(vec![copy(1, 2), copy(1, 3), copy(3, 1)]).unwrap(),
            vec![
                Action::Copy {
                    destination: register(2),
                    source: Source::Location(register(1)),
                },
                Action::Save(register(1)),
                Action::Copy {
                    destination: register(1),
                    source: Source::Location(register(3)),
                },
                Action::Copy {
                    destination: register(3),
                    source: Source::Temporary,
                },
            ]
        );
    }
}
