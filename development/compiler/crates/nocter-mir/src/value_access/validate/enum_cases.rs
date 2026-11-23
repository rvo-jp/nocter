//! Enum fields use the exact incoming case until an effect can invalidate the inspected carrier.
use super::{Body, Context, Effect, Failure, Operation, Place, Storage, Terminator};
use crate::value_access::{Action, Alternative};
use nocter_model::AccessFlowPointId;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub(super) struct CaseEdge<'a> {
    pub source: &'a Place,
    pub tag: u64,
}
impl CaseEdge<'_> {
    pub(super) fn contains(self, place: &Place) -> bool {
        self.source.slot == place.slot
            && self.source.root == place.root
            && place.projections.starts_with(&self.source.projections)
    }
}

pub(super) fn incoming(body: &Body) -> BTreeMap<AccessFlowPointId, Option<CaseEdge<'_>>> {
    if !body
        .terminators
        .values()
        .any(|term| matches!(term, Terminator::EnumSwitch { .. }))
    {
        return BTreeMap::new();
    }
    let mut incoming = BTreeMap::from([(body.entry, None)]);
    let mut enter = |point, edge| {
        incoming
            .entry(point)
            .and_modify(|old| *old = None)
            .or_insert(edge);
    };
    for terminator in body.terminators.values() {
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
    incoming
}

pub(super) fn action<'a>(
    context: &Context<'a>,
    active: &mut Option<CaseEdge<'a>>,
    action: &Action,
) -> Result<(), Failure> {
    alternatives(context, active, &action.alternatives, |path| &path.before)?;
    effects(context, active, &action.call_bindings)?;
    for operation in &action.operations {
        context.with_case(*active).operation(operation)?;
        match operation {
            Operation::Copy { destination, .. }
                if active.is_some_and(|edge| isolated_write(context, edge, destination)) => {}
            Operation::Storage(effect) => retain(context, active, effect),
            _ => *active = None,
        }
    }
    alternatives(context, active, &action.alternatives, |path| &path.after)
}

fn alternatives<'a>(
    context: &Context<'a>,
    active: &mut Option<CaseEdge<'a>>,
    alternatives: &[Alternative],
    phase: impl Fn(&Alternative) -> &[Effect],
) -> Result<(), Failure> {
    let entry = *active;
    for alternative in alternatives {
        let mut route = entry;
        effects(context, &mut route, phase(alternative))?;
        if route.is_none() {
            *active = None;
        }
    }
    Ok(())
}

fn effects<'a>(
    context: &Context<'a>,
    active: &mut Option<CaseEdge<'a>>,
    effects: &[Effect],
) -> Result<(), Failure> {
    for effect in effects {
        context.with_case(*active).effect(effect)?;
        retain(context, active, effect);
    }
    Ok(())
}

fn retain(context: &Context<'_>, active: &mut Option<CaseEdge<'_>>, effect: &Effect) {
    let Some(edge) = *active else { return };
    let owns = |place: &Place| has_payload(context, place.slot);
    let inside_owner = |place: &Place| {
        owns(edge.source)
            && edge.contains(place)
            && place.projections.len() > edge.source.projections.len()
    };
    let preserves = match effect {
        Effect::BindAddress { destination, .. } => *destination != edge.source.slot,
        Effect::Transfer { destination, .. } | Effect::Initialize(destination) => {
            isolated_write(context, edge, destination)
        }
        // Ending or disposing one selected child of the prepared owner leaves its tag intact.
        Effect::End(place) => inside_owner(place) || isolated_write(context, edge, place),
        Effect::Dispose { source, .. } => inside_owner(source),
        _ => false,
    };
    if !preserves {
        *active = None;
    }
}

fn isolated_write(context: &Context<'_>, edge: CaseEdge<'_>, destination: &Place) -> bool {
    // Distinct payload slots cannot alias. An address slot can reference either payload.
    edge.source.slot != destination.slot
        && has_payload(context, edge.source.slot)
        && has_payload(context, destination.slot)
}

fn has_payload(context: &Context<'_>, slot: nocter_model::MirLocalId) -> bool {
    context
        .body
        .slots
        .get(slot)
        .is_some_and(|slot| slot.storage == Storage::Payload)
}
