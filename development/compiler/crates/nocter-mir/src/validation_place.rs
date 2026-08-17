use nocter_model::BorrowCapability;

use crate::{MirFunction, MirPlace, MirPlaceRoot, MirProjectionKind, MirValidationError};

#[derive(Clone, Copy)]
pub(crate) struct PlaceFacts {
    pub(crate) writable: bool,
    pub(crate) movable: bool,
}

pub(crate) fn place_facts(
    function: &MirFunction,
    place: &MirPlace,
) -> Result<PlaceFacts, MirValidationError> {
    let (mut facts, mut crossed_borrow) = match place.root() {
        MirPlaceRoot::Local(local) => {
            let local = function
                .locals()
                .get(local)
                .copied()
                .ok_or(MirValidationError::UnknownLocal(local))?;
            (
                PlaceFacts {
                    writable: local.is_mutable(),
                    movable: true,
                },
                false,
            )
        }
        MirPlaceRoot::Dereference { capability, .. } => (
            PlaceFacts {
                writable: capability == BorrowCapability::ReadWrite,
                movable: false,
            },
            true,
        ),
    };
    for projection in place.projections() {
        match projection.kind() {
            MirProjectionKind::BorrowDereference(capability) => {
                // Mutating through a stored `&+T` depends on the borrow capability, not on
                // whether the slot containing that first borrow can itself be reassigned. An
                // earlier readonly dereference remains an authority ceiling.
                if crossed_borrow {
                    facts.writable &= capability == BorrowCapability::ReadWrite;
                } else {
                    facts.writable = capability == BorrowCapability::ReadWrite;
                    crossed_borrow = true;
                }
                facts.movable = false;
            }
            MirProjectionKind::FixedIndex(_) | MirProjectionKind::DynamicIndex(_) => {
                facts.movable = false;
            }
            _ => {}
        }
    }
    Ok(facts)
}
