//! Exact representation state follows original plan control; conflicting entries stay unqualified.
use super::{Effect, INVALID, Program, Result, Storage, Terminator, Type, View, place};
use crate::state::{Plan, slot_types::Contexts};
use nocter_model::AccessFlowPointId;
use std::{collections::BTreeMap, sync::Arc};

pub(super) fn effect(plan: &Plan, contexts: &mut Contexts, effect: &Effect) -> Result<()> {
    if let Effect::ReplaceRoot {
        destination, data, ..
    }
    | Effect::MoveRoot {
        destination, data, ..
    } = *effect
    {
        let initial = plan.slots.get(destination).ok_or(INVALID)?.data;
        let current = contexts.get(&destination).copied().unwrap_or(initial);
        if current == data {
            return Ok(());
        }
        let contexts = Arc::make_mut(contexts);
        if initial == data {
            contexts.remove(&destination);
        } else {
            contexts.insert(destination, data);
        }
    }
    Ok(())
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
                std::collections::btree_map::Entry::Occupied(entry) if entry.get() != &after => {
                    return Err(INVALID);
                }
                std::collections::btree_map::Entry::Occupied(_) => {}
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
