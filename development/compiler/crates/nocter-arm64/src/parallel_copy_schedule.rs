use crate::Arm64MaterializationError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ParallelSource<L> {
    Location(L),
    Temporary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParallelCopy<L, M> {
    pub(crate) destination: L,
    pub(crate) source: ParallelSource<L>,
    pub(crate) metadata: M,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ParallelAction<L, M> {
    Save(L),
    Copy {
        destination: L,
        source: ParallelSource<L>,
        metadata: M,
    },
}

/// Schedules simultaneous assignments for any closed physical-location domain.
///
/// Register-bank-specific materializers provide locations, copy metadata, and the temporary. The
/// dependency and cycle-breaking algorithm therefore has one implementation for general,
/// floating, and future register classes.
pub(crate) fn schedule<L: Copy + Eq, M: Copy>(
    mut pending: Vec<ParallelCopy<L, M>>,
) -> Result<Vec<ParallelAction<L, M>>, Arm64MaterializationError> {
    pending.retain(|copy| copy.source != ParallelSource::Location(copy.destination));
    let mut actions = Vec::with_capacity(pending.len() + 1);
    while !pending.is_empty() {
        if let Some(index) = pending.iter().position(|candidate| {
            !pending
                .iter()
                .any(|copy| copy.source == ParallelSource::Location(candidate.destination))
        }) {
            let copy = pending.remove(index);
            actions.push(ParallelAction::Copy {
                destination: copy.destination,
                source: copy.source,
                metadata: copy.metadata,
            });
            continue;
        }

        let source = pending
            .iter()
            .find_map(|copy| match copy.source {
                ParallelSource::Location(source) => Some(source),
                ParallelSource::Temporary => None,
            })
            .ok_or(Arm64MaterializationError::InvalidParallelCopy)?;
        actions.push(ParallelAction::Save(source));
        for copy in &mut pending {
            if copy.source == ParallelSource::Location(source) {
                copy.source = ParallelSource::Temporary;
            }
        }
    }
    Ok(actions)
}

#[cfg(test)]
mod tests {
    use super::{ParallelAction, ParallelCopy, ParallelSource, schedule};

    fn copy(source: u8, destination: u8) -> ParallelCopy<u8, ()> {
        ParallelCopy {
            destination,
            source: ParallelSource::Location(source),
            metadata: (),
        }
    }

    #[test]
    fn orders_acyclic_copies_from_leaves_to_roots() {
        assert_eq!(
            schedule(vec![copy(1, 2), copy(2, 3)]).unwrap(),
            vec![
                ParallelAction::Copy {
                    destination: 3,
                    source: ParallelSource::Location(2),
                    metadata: (),
                },
                ParallelAction::Copy {
                    destination: 2,
                    source: ParallelSource::Location(1),
                    metadata: (),
                },
            ]
        );
    }

    #[test]
    fn breaks_cycles_once_and_preserves_the_saved_value() {
        assert_eq!(
            schedule(vec![copy(1, 2), copy(2, 1)]).unwrap(),
            vec![
                ParallelAction::Save(1),
                ParallelAction::Copy {
                    destination: 1,
                    source: ParallelSource::Location(2),
                    metadata: (),
                },
                ParallelAction::Copy {
                    destination: 2,
                    source: ParallelSource::Temporary,
                    metadata: (),
                },
            ]
        );
    }

    #[test]
    fn one_saved_source_can_feed_multiple_cycle_destinations() {
        assert_eq!(
            schedule(vec![copy(1, 2), copy(1, 3), copy(3, 1)]).unwrap(),
            vec![
                ParallelAction::Copy {
                    destination: 2,
                    source: ParallelSource::Location(1),
                    metadata: (),
                },
                ParallelAction::Save(1),
                ParallelAction::Copy {
                    destination: 1,
                    source: ParallelSource::Location(3),
                    metadata: (),
                },
                ParallelAction::Copy {
                    destination: 3,
                    source: ParallelSource::Temporary,
                    metadata: (),
                },
            ]
        );
    }
}
