use super::{Effect, Operation, Place, Program, Rule, Storage, Terminator, Type, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
type Result<T> = std::result::Result<T, Rule>;
const INVALID: Rule = Rule::InvalidPlan;
pub(super) fn place(program: &Program, plan: &super::Plan, place: &Place) -> Result<usize> {
    let mut data = plan.slots.get(place.slot).ok_or(INVALID)?.data;
    for &field in &place.fields {
        let Type::Record(fields) = program.types.get(data).ok_or(INVALID)? else {
            return Err(INVALID);
        };
        data = *fields.get(field).ok_or(INVALID)?;
    }
    Ok(data)
}
pub(super) fn value(program: &Program, data: usize, value: &Arc<Value>) -> bool {
    let mut pending = vec![(data, value.clone())];
    let mut seen = BTreeSet::new();
    while let Some((data, value)) = pending.pop() {
        if !seen.insert((data, Arc::as_ptr(&value))) {
            continue;
        }
        match (program.types.get(data), value.as_ref()) {
            (Some(Type::Unit), Value::Unit)
            | (Some(Type::Bool), Value::Bool(_))
            | (Some(Type::I32), Value::I32(_)) => {}
            (Some(Type::Record(fields)), Value::Record(values)) if fields.len() == values.len() => {
                pending.extend(fields.iter().copied().zip(values.iter().cloned()));
            }
            _ => return false,
        }
    }
    true
}
pub(super) fn validate(program: &Program) -> Result<()> {
    for (id, data) in program.types.iter().enumerate() {
        if let Type::Record(fields) = data
            && fields.iter().any(|&field| field >= id)
        {
            return Err(INVALID);
        }
    }
    for plan in program.plans.values() {
        validate_plan(program, plan)?;
    }
    Ok(())
}
fn validate_plan(program: &Program, plan: &super::Plan) -> Result<()> {
    if plan
        .slots
        .iter()
        .any(|slot| slot.data >= program.types.len())
        || plan.result >= program.types.len()
        || plan.points.get(plan.entry).is_none()
        || (plan.initializer && !plan.inputs.is_empty())
    {
        return Err(INVALID);
    }
    let mut inputs = BTreeSet::new();
    for &(slot, operation) in &plan.inputs {
        let data = plan.slots.get(slot).ok_or(INVALID)?;
        let storage = if operation == nocter_model::ValueInputOperation::Take {
            Storage::Owned
        } else {
            Storage::Alias
        };
        if data.storage != storage || !inputs.insert(slot) {
            return Err(INVALID);
        }
    }
    for (_, point) in plan.points.iter() {
        for action in &point.actions {
            validate_action(program, plan, action)?;
        }
        let targets: Vec<_> = match &point.terminator {
            Terminator::Jump(next) => vec![*next],
            Terminator::Branch { condition, yes, no } => {
                if program.types.get(place(program, plan, condition)?) != Some(&Type::Bool) {
                    return Err(INVALID);
                }
                vec![*yes, *no]
            }
            Terminator::Return(result) => {
                if !result.fields.is_empty()
                    || plan
                        .slots
                        .get(result.slot)
                        .is_none_or(|s| s.storage != Storage::Owned)
                {
                    return Err(INVALID);
                }
                if place(program, plan, result)? != plan.result {
                    return Err(INVALID);
                }
                vec![]
            }
            Terminator::Unreachable => vec![],
        };
        if targets.iter().any(|&id| plan.points.get(id).is_none()) {
            return Err(INVALID);
        }
    }
    Ok(())
}

fn validate_action(program: &Program, plan: &super::Plan, action: &super::Action) -> Result<()> {
    if action.alternatives.is_empty() {
        return Err(INVALID);
    }
    operation(program, plan, &action.operation)?;
    let mut guards = BTreeMap::new();
    for alternative in &action.alternatives {
        let phases = (&alternative.before, &alternative.after);
        if guards
            .insert(&alternative.owned, phases)
            .is_some_and(|existing| existing != phases)
        {
            return Err(INVALID);
        }
        for (&slot, paths) in &alternative.owned {
            if plan
                .slots
                .get(slot)
                .is_none_or(|slot| slot.storage != Storage::Owned)
            {
                return Err(INVALID);
            }
            if paths.iter().any(|path| path.is_empty())
                || paths
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1] || pair[1].starts_with(&pair[0]))
            {
                return Err(INVALID);
            }
            for fields in paths {
                place(
                    program,
                    plan,
                    &Place {
                        slot,
                        fields: fields.clone(),
                    },
                )?;
            }
        }
        for effect in alternative.before.iter().chain(&alternative.after) {
            check_effect(program, plan, effect)?;
        }
    }

    Ok(())
}
fn operation(program: &Program, plan: &super::Plan, op: &Operation) -> Result<()> {
    let slot = |id: usize| plan.slots.get(id).ok_or(INVALID);
    let data = |p: &Place| place(program, plan, p);
    let equal = |a: usize, b: usize| if a == b { Ok(()) } else { Err(INVALID) };
    match op {
        Operation::None => Ok(()),
        Operation::Literal {
            destination,
            value: v,
        } => {
            if value(program, slot(*destination)?.data, &Arc::new(v.clone())) {
                Ok(())
            } else {
                Err(INVALID)
            }
        }
        Operation::Alias {
            destination,
            source,
        } => {
            if slot(*destination)?.storage != Storage::Alias {
                return Err(INVALID);
            }
            equal(slot(*destination)?.data, data(source)?)
        }
        Operation::Copy {
            destination,
            source,
        } => equal(data(destination)?, data(source)?),
        Operation::Unary {
            destination,
            operation,
            operand,
        } => {
            let expected = match operation {
                crate::CompileTimeUnaryOperation::LogicalNot => Type::Bool,
                crate::CompileTimeUnaryOperation::Negate => Type::I32,
            };
            if program.types.get(data(operand)?) != Some(&expected) {
                return Err(INVALID);
            }
            equal(slot(*destination)?.data, data(operand)?)
        }
        Operation::Binary {
            destination,
            operation,
            left,
            right,
        } => {
            if matches!(
                operation,
                crate::CompileTimeBinaryOperation::ShiftLeft
                    | crate::CompileTimeBinaryOperation::ShiftRightSigned
                    | crate::CompileTimeBinaryOperation::ShiftRightUnsigned
            ) {
                return Err(INVALID);
            }
            if program.types.get(data(left)?) != Some(&Type::I32) {
                return Err(INVALID);
            }
            equal(data(left)?, data(right)?)?;
            equal(data(destination)?, data(left)?)
        }
        Operation::Compare {
            destination,
            operation,
            left,
            right,
        } => {
            if !matches!(
                operation,
                crate::CompileTimeComparisonOperation::Equal
                    | crate::CompileTimeComparisonOperation::NotEqual
            ) && program.types.get(data(left)?) != Some(&Type::I32)
            {
                return Err(INVALID);
            }
            if program.types.get(slot(*destination)?.data) != Some(&Type::Bool)
                || !matches!(program.types.get(data(left)?), Some(Type::Bool | Type::I32))
            {
                return Err(INVALID);
            }
            equal(data(left)?, data(right)?)
        }
        Operation::Call {
            destination,
            target,
            inputs,
        } => call(program, plan, *destination, *target, inputs),
    }
}
fn call(
    program: &Program,
    plan: &super::Plan,
    destination: usize,
    target: nocter_model::AccessCallableId,
    inputs: &[super::Input],
) -> Result<()> {
    let callee = program.plans.get(&target).ok_or(INVALID)?;
    if callee.initializer
        || inputs.len() != callee.inputs.len()
        || plan.slots.get(destination).ok_or(INVALID)?.data != callee.result
    {
        return Err(INVALID);
    }
    for (input, &(parameter, operation)) in inputs.iter().zip(&callee.inputs) {
        if input.operation != operation
            || place(program, plan, &input.place)?
                != callee.slots.get(parameter).ok_or(INVALID)?.data
        {
            return Err(INVALID);
        }
    }
    Ok(())
}
fn check_effect(program: &Program, plan: &super::Plan, effect: &Effect) -> Result<()> {
    let data = |p: &Place| place(program, plan, p);
    let owned = |p: &Place| {
        if plan
            .slots
            .get(p.slot)
            .is_some_and(|s| s.storage == Storage::Owned)
        {
            data(p)
        } else {
            Err(INVALID)
        }
    };
    match effect {
        Effect::Construct(slot) => {
            owned(&Place::root(*slot))?;
        }
        Effect::Move {
            source,
            destination,
        } => {
            if owned(source)? != owned(destination)? {
                return Err(INVALID);
            }
        }
        Effect::Replace {
            source,
            destination,
        } => {
            if owned(source)? != data(destination)?
                || plan
                    .slots
                    .get(destination.slot)
                    .is_none_or(|s| s.storage != Storage::Alias)
            {
                return Err(INVALID);
            }
        }
        Effect::Dispose(source) | Effect::Release(source) | Effect::Invoke(source) => {
            owned(source)?;
        }
        Effect::Assemble {
            destination,
            fields,
        } => {
            let ty = owned(&Place::root(*destination))?;
            let Some(Type::Record(schema)) = program.types.get(ty) else {
                return Err(INVALID);
            };
            let mut seen = BTreeSet::new();
            for (field, source) in fields {
                if schema.get(*field) != Some(&owned(source)?) || !seen.insert(*field) {
                    return Err(INVALID);
                }
            }
            if seen.len() != schema.len() {
                return Err(INVALID);
            }
        }
    }
    Ok(())
}
