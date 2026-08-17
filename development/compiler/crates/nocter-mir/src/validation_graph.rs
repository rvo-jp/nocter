use nocter_model::MirValueId;

use crate::{MirBranchTarget, MirPlace, MirPlaceRoot, MirProjectionKind, MirTerminator};

pub(crate) fn place_values(place: &MirPlace) -> impl Iterator<Item = MirValueId> + '_ {
    let root = match place.root() {
        MirPlaceRoot::Local(_) => None,
        MirPlaceRoot::Dereference { value, .. } => Some(value),
    };
    root.into_iter()
        .chain(
            place
                .projections()
                .iter()
                .filter_map(|projection| match projection.kind() {
                    MirProjectionKind::DynamicIndex(value) => Some(value),
                    _ => None,
                }),
        )
}

pub(crate) fn successors(terminator: &MirTerminator) -> impl Iterator<Item = &MirBranchTarget> {
    let mut targets = Vec::new();
    match terminator {
        MirTerminator::Goto(target) => targets.push(target),
        MirTerminator::Branch {
            then_target,
            else_target,
            ..
        } => targets.extend([then_target, else_target]),
        MirTerminator::BranchDropFlag {
            initialized,
            uninitialized,
            ..
        } => targets.extend([initialized, uninitialized]),
        MirTerminator::Switch {
            cases, fallback, ..
        } => {
            targets.extend(cases.iter().map(crate::MirSwitchCase::target));
            targets.push(fallback);
        }
        MirTerminator::Return(_) | MirTerminator::Trap | MirTerminator::Unreachable => {}
    }
    targets.into_iter()
}
