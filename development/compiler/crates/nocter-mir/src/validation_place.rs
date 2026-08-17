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
    let mut facts = match place.root() {
        MirPlaceRoot::Local(local) => {
            let local = function
                .locals()
                .get(local)
                .copied()
                .ok_or(MirValidationError::UnknownLocal(local))?;
            PlaceFacts {
                writable: local.is_mutable(),
                movable: true,
            }
        }
        MirPlaceRoot::Dereference { capability, .. } => PlaceFacts {
            writable: capability == BorrowCapability::ReadWrite,
            movable: false,
        },
    };
    for projection in place.projections() {
        match projection.kind() {
            MirProjectionKind::BorrowDereference(capability) => {
                facts.writable &= capability == BorrowCapability::ReadWrite;
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
