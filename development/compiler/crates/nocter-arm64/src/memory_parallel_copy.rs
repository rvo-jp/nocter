use crate::{
    Arm64CodeBuilder, Arm64MaterializationError, Arm64SelectedFunction, Arm64SelectedMemoryAddress,
    Arm64SelectedMemoryCopy, Arm64SelectedStackAddress,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Source {
    Address(Arm64SelectedStackAddress),
    Temporary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingCopy {
    destination: Arm64SelectedStackAddress,
    source: Source,
    bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Save {
        source: Arm64SelectedStackAddress,
        bytes: u64,
    },
    Copy(PendingCopy),
}

pub(crate) fn emit(
    function: &Arm64SelectedFunction,
    copies: &[Arm64SelectedMemoryCopy],
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let pending = copies
        .iter()
        .map(|copy| PendingCopy {
            destination: copy.destination(),
            source: Source::Address(copy.source()),
            bytes: copy.bytes(),
        })
        .collect();
    for action in schedule(pending)? {
        match action {
            Action::Save { source, bytes } => {
                emit_copy(function, temporary(function)?, source, bytes, code)?;
            }
            Action::Copy(copy) => {
                let source = match copy.source {
                    Source::Address(source) => source,
                    Source::Temporary => temporary(function)?,
                };
                emit_copy(function, copy.destination, source, copy.bytes, code)?;
            }
        }
    }
    Ok(())
}

fn schedule(mut pending: Vec<PendingCopy>) -> Result<Vec<Action>, Arm64MaterializationError> {
    pending.retain(|copy| copy.source != Source::Address(copy.destination));
    let mut actions = Vec::with_capacity(pending.len() + 1);
    while !pending.is_empty() {
        if let Some(index) = pending.iter().position(|candidate| {
            !pending
                .iter()
                .any(|copy| copy.source == Source::Address(candidate.destination))
        }) {
            actions.push(Action::Copy(pending.remove(index)));
            continue;
        }

        let (source, bytes) = pending
            .iter()
            .find_map(|copy| match copy.source {
                Source::Address(source) => Some((source, copy.bytes)),
                Source::Temporary => None,
            })
            .ok_or(Arm64MaterializationError::InvalidParallelCopy)?;
        actions.push(Action::Save { source, bytes });
        for copy in &mut pending {
            if copy.source == Source::Address(source) {
                if copy.bytes != bytes {
                    return Err(Arm64MaterializationError::InvalidParallelCopy);
                }
                copy.source = Source::Temporary;
            }
        }
    }
    Ok(actions)
}

fn temporary(
    function: &Arm64SelectedFunction,
) -> Result<Arm64SelectedStackAddress, Arm64MaterializationError> {
    function
        .frame()
        .memory_edge_staging()
        .map(|object| Arm64SelectedStackAddress::FrameObject { object, offset: 0 })
        .ok_or(Arm64MaterializationError::MissingMemoryEdgeStaging)
}

fn emit_copy(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedStackAddress,
    source: Arm64SelectedStackAddress,
    bytes: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::memory_code::emit_memory_copy(
        function,
        Arm64SelectedMemoryAddress::Stack(destination),
        Arm64SelectedMemoryAddress::Stack(source),
        bytes,
        code,
    )
}

#[cfg(test)]
mod tests {
    use super::{Action, PendingCopy, Source, schedule};
    use crate::Arm64SelectedStackAddress;

    const fn address(offset: u64) -> Arm64SelectedStackAddress {
        Arm64SelectedStackAddress::Incoming(offset)
    }

    const fn copy(destination: u64, source: u64) -> PendingCopy {
        PendingCopy {
            destination: address(destination),
            source: Source::Address(address(source)),
            bytes: 24,
        }
    }

    #[test]
    fn schedules_acyclic_memory_assignments_without_a_temporary() {
        assert_eq!(
            schedule(vec![copy(1, 2), copy(2, 3)]).unwrap(),
            vec![Action::Copy(copy(1, 2)), Action::Copy(copy(2, 3))]
        );
    }

    #[test]
    fn schedules_memory_cycles_through_one_temporary() {
        assert_eq!(
            schedule(vec![copy(1, 2), copy(2, 1)]).unwrap(),
            vec![
                Action::Save {
                    source: address(2),
                    bytes: 24,
                },
                Action::Copy(copy(2, 1)),
                Action::Copy(PendingCopy {
                    destination: address(1),
                    source: Source::Temporary,
                    bytes: 24,
                }),
            ]
        );
    }

    #[test]
    fn removes_identity_memory_assignments() {
        assert!(schedule(vec![copy(1, 1)]).unwrap().is_empty());
    }
}
