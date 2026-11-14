//! Context joins retain uncertainty until an actual typed use or fresh receipt resolves it.
use super::{Effect, INVALID, Program, Result, Storage, Terminator, Type, View, place};
use crate::state::{Plan, slot_types::Contexts};
use nocter_model::AccessFlowPointId;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub(super) fn effect(plan: &Plan, contexts: &mut Contexts, effect: &Effect) -> Result<()> {
    if let Effect::ReplaceRoot {
        destination, data, ..
    }
    | Effect::MoveRoot {
        destination, data, ..
    } = *effect
    {
        let initial = plan.slots.get(destination).ok_or(INVALID)?.data;
        let current = contexts.get(&destination).copied().unwrap_or(Some(initial));
        if current == Some(data) {
            return Ok(());
        }
        let contexts = Arc::make_mut(contexts);
        if initial == data {
            contexts.remove(&destination);
        } else {
            contexts.insert(destination, Some(data));
        }
    }
    Ok(())
}
pub(super) fn join(plan: &Plan, current: &mut Contexts, incoming: &Contexts) -> Result<bool> {
    if Arc::ptr_eq(current, incoming) {
        return Ok(false);
    }
    let keys: BTreeSet<_> = current.keys().chain(incoming.keys()).copied().collect();
    let mut unresolved = Vec::new();
    for slot in keys {
        let initial = Some(plan.slots.get(slot).ok_or(INVALID)?.data);
        let left = current.get(&slot).copied().unwrap_or(initial);
        let right = incoming.get(&slot).copied().unwrap_or(initial);
        if left.is_some() && left != right {
            unresolved.push(slot);
        }
    }
    if unresolved.is_empty() {
        return Ok(false);
    }
    let current = Arc::make_mut(current);
    for slot in unresolved {
        current.insert(slot, None);
    }
    Ok(true)
}
pub(super) fn validate(program: &Program, plan: &Plan) -> Result<()> {
    let mut entries = BTreeMap::from([(plan.entry, Contexts::default())]);
    let mut pending = vec![plan.entry];
    while let Some(id) = pending.pop() {
        let (after, targets) = point(program, plan, id, &entries[&id])?;
        for target in targets {
            match entries.entry(target) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(after.clone());
                    pending.push(target);
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    if join(plan, entry.get_mut(), &after)? {
                        pending.push(target);
                    }
                }
            }
        }
    }
    // Dormant plan operations still require valid contracts, without an invented entry value.
    for (id, _) in plan.points.iter() {
        if !entries.contains_key(&id) {
            point(program, plan, id, &Contexts::default())?;
        }
    }
    Ok(())
}
fn point(
    program: &Program,
    plan: &Plan,
    id: AccessFlowPointId,
    entry: &Contexts,
) -> Result<(Contexts, Vec<AccessFlowPointId>)> {
    let block = plan.points.get(id).ok_or(INVALID)?;
    let mut current = entry.clone();
    for action in &block.actions {
        current = super::validate_action(program, &View::new(plan, &current), action)?;
    }
    let view = &View::new(plan, &current);
    let targets = match &block.terminator {
        Terminator::Jump(next) => vec![*next],
        Terminator::Branch { condition, yes, no } => {
            if program.types.get(place(program, view, condition)?) != Some(&Type::Bool) {
                return Err(INVALID);
            }
            vec![*yes, *no]
        }
        Terminator::Return(result) => {
            if !result.fields.is_empty()
                || plan
                    .slots
                    .get(result.slot)
                    .is_none_or(|slot| slot.storage != Storage::Owned)
                || place(program, view, result)? != plan.result
            {
                return Err(INVALID);
            }
            vec![]
        }
        Terminator::Unreachable => vec![],
    };
    if targets.iter().any(|&id| plan.points.get(id).is_none()) {
        return Err(INVALID);
    }
    Ok((current, targets))
}
