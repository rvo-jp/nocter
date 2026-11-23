//! An enum field address must be formed on its exact case edge, before effects can retag the input.
use super::{Context, Effect, Failure, Operation, Place, Storage, Terminator};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub(super) struct CaseEdge<'a> {
    pub source: &'a Place,
    pub tag: u64,
}

pub(super) fn validate(context: &Context<'_>) -> Result<(), Failure> {
    let body = context.body;
    if !body.actions.values().any(|action| {
        action
            .operations
            .iter()
            .any(|op| matches!(op, Operation::BindEnumAddress { .. }))
    }) {
        return Ok(());
    }

    let mut incoming = BTreeMap::from([(body.entry, None)]);
    for terminator in body.terminators.values() {
        let mut enter = |point, edge| {
            incoming
                .entry(point)
                .and_modify(|old| *old = None)
                .or_insert(edge);
        };
        match terminator {
            Terminator::Jump(next) => enter(*next, None),
            Terminator::Branch { yes, no, .. } => {
                enter(*yes, None);
                enter(*no, None);
            }
            Terminator::EnumSwitch {
                source,
                cases,
                fallback,
            } => {
                for &(tag, point) in cases.iter() {
                    enter(point, Some(CaseEdge { source, tag }));
                }
                enter(*fallback, None);
            }
            Terminator::Return(_) | Terminator::Unreachable => {}
        }
    }

    for (at, action) in &body.actions {
        let active = incoming
            .get_mut(&at.point)
            .ok_or_else(|| context.invalid())?;
        for effect in action
            .alternatives
            .iter()
            .flat_map(|path| &path.before)
            .chain(&action.call_bindings)
        {
            retain(context, active, effect);
        }
        for operation in &action.operations {
            match operation {
                Operation::BindEnumAddress {
                    destination,
                    source,
                } => {
                    let edge = active.ok_or_else(|| context.invalid())?;
                    context.require(*destination != edge.source.slot)?;
                    context.guarded_place(source, Some(edge))?;
                }
                Operation::Copy { destination, .. }
                    if active.is_some_and(|edge| isolated_write(context, edge, destination)) => {}
                Operation::Storage(effect) => retain(context, active, effect),
                _ => *active = None,
            }
        }
        for effect in action.alternatives.iter().flat_map(|path| &path.after) {
            retain(context, active, effect);
        }
    }
    Ok(())
}

fn retain(context: &Context<'_>, active: &mut Option<CaseEdge<'_>>, effect: &Effect) {
    let Some(edge) = *active else { return };
    let preserves = match effect {
        Effect::BindAddress { destination, .. } => *destination != edge.source.slot,
        Effect::Transfer { destination, .. }
        | Effect::Initialize(destination)
        | Effect::End(destination) => isolated_write(context, edge, destination),
        _ => false,
    };
    if !preserves {
        *active = None;
    }
}

fn isolated_write(context: &Context<'_>, edge: CaseEdge<'_>, destination: &Place) -> bool {
    // Distinct payload slots cannot alias. An address slot can reference either payload.
    edge.source.slot != destination.slot
        && [edge.source.slot, destination.slot].into_iter().all(|id| {
            context
                .body
                .slots
                .get(id)
                .is_some_and(|slot| slot.storage == Storage::Payload)
        })
}
