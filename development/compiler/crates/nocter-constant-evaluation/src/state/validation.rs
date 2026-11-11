use super::{Data, Effect, Operation, Place, Program, Rule, Storage, Terminator, Type, Value};
use std::collections::{BTreeMap, BTreeSet};
type Result<T> = std::result::Result<T, Rule>;
const INVALID: Rule = Rule::InvalidPlan;
fn ordinary_slot(plan: &super::Plan, id: usize) -> Result<&super::Slot> {
    if plan.environment_backing == Some(id) {
        return Err(INVALID);
    }
    plan.slots.get(id).ok_or(INVALID)
}
pub(super) fn place(program: &Program, plan: &super::Plan, place: &Place) -> Result<usize> {
    ordinary_slot(plan, place.slot)?;
    place_data(program, &plan.slots, place)
}
pub(super) fn place_data(program: &Program, slots: &[super::Slot], place: &Place) -> Result<usize> {
    let mut data = slots.get(place.slot).ok_or(INVALID)?.data;
    for projection in place.fields.iter() {
        data = match projection {
            super::Projection::Field(field) => program
                .types
                .get(data)
                .and_then(|ty| ty.component(*field))
                .ok_or(INVALID)?,
            super::Projection::Index { slot, bound } => {
                let snapshot = slots.get(*slot).ok_or(INVALID)?;
                if snapshot.storage != Storage::Owned
                    || program.types.get(snapshot.data) != Some(&Type::Usize)
                {
                    return Err(INVALID);
                }
                let Some(Type::Array { element, length }) = program.types.get(data) else {
                    return Err(INVALID);
                };
                if *length == 0 || length != bound {
                    return Err(INVALID);
                }
                *element
            }
        };
    }
    Ok(data)
}
pub(super) fn value(program: &Program, data: usize, value: &Value) -> bool {
    let mut pending = vec![(data, value.clone())];
    let mut seen = BTreeSet::new();
    while let Some((data, value)) = pending.pop() {
        if !seen.insert((data, value.identity())) {
            continue;
        }
        match (program.types.get(data), value.data()) {
            (Some(Type::Unit | Type::NamedFunction(_)), Data::Unit)
            | (Some(Type::Bool), Data::Bool(_)) => {}
            (Some(ty @ (Type::I32 | Type::Usize)), Data::Integer(value))
                if ty
                    .scalar()
                    .and_then(|ty| match ty {
                        crate::ConstantScalarType::Integer(builtin) => {
                            crate::support::integer_spec(builtin)
                        }
                        _ => None,
                    })
                    .is_some_and(|spec| spec.contains(*value)) => {}
            (Some(Type::CapturedFunction { environment, .. }), Data::Aggregate(values))
                if values.len() == 1 =>
            {
                pending.push((*environment, values[0].clone()));
            }
            (Some(Type::Record(fields)), Data::Aggregate(values))
                if fields.len() == values.len() =>
            {
                pending.extend(fields.iter().copied().zip(values.iter().cloned()));
            }
            (Some(Type::Array { element, length }), Data::Aggregate(values))
                if u64::try_from(values.len()).ok() == Some(*length) =>
            {
                pending.extend(values.iter().cloned().map(|value| (*element, value)));
            }
            (
                Some(Type::Array { element, length }),
                Data::Repeated {
                    length: count,
                    replacements,
                    ..
                },
            ) if *length == count.get() && replacements.keys().all(|&index| index < *length) => {
                pending.extend(
                    value
                        .data()
                        .children()
                        .cloned()
                        .map(|value| (*element, value)),
                );
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
        if let Type::CapturedFunction { environment, .. } = data
            && *environment >= id
        {
            return Err(INVALID);
        }
        if let Type::Array { element, length } = data
            && (*element >= program.types.len() || (*length != 0 && *element >= id))
        {
            return Err(INVALID);
        }
    }
    for global in program.statics.values() {
        if !value(program, global.data, &global.value) {
            return Err(INVALID);
        }
    }
    for (&code, plan) in &program.plans {
        match code {
            nocter_model::ValueCode::Expression(_) if !plan.initializer => return Err(INVALID),
            nocter_model::ValueCode::Closure(_) if plan.initializer => return Err(INVALID),
            _ => {}
        }
        validate_plan(program, plan)?;
        if let Some(backing) = plan.environment_backing {
            let &(input, operation) = plan.inputs.first().ok_or(INVALID)?;
            let source = plan.slots.get(input).ok_or(INVALID)?;
            let target = plan.slots.get(backing).ok_or(INVALID)?;
            if !matches!(code, nocter_model::ValueCode::Closure(_))
                || operation == nocter_model::ValueInputOperation::Take
                || target.storage != Storage::Auxiliary
                || target.data != source.data
            {
                return Err(INVALID);
            }
        }
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
    for &(slot, id) in &plan.statics {
        let source = program.statics.get(&id).ok_or(INVALID)?;
        let target = plan.slots.get(slot).ok_or(INVALID)?;
        if target.storage != Storage::Alias || target.data != source.data || !inputs.insert(slot) {
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
            if paths.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(INVALID);
            }
            let mut previous_value: Option<&super::ProjectionPath> = None;
            let mut previous_prefix = None;
            for missing in paths {
                if missing.indices().any(|(slot, _)| {
                    alternative
                        .owned
                        .get(&slot)
                        .is_none_or(|holes| !holes.is_empty())
                }) {
                    return Err(INVALID);
                }
                match missing {
                    super::Missing::Value(fields) => {
                        if fields.is_empty()
                            || previous_value.is_some_and(|previous| fields.starts_with(previous))
                        {
                            return Err(INVALID);
                        }
                        previous_value = Some(fields);
                        place(
                            program,
                            plan,
                            &Place {
                                slot,
                                fields: fields.clone(),
                            },
                        )?;
                    }
                    super::Missing::ArrayPrefix {
                        base,
                        endpoint,
                        bound,
                    } => {
                        if previous_prefix == Some(base)
                            || bounded_array(
                                program,
                                plan,
                                &Place {
                                    slot,
                                    fields: base.clone(),
                                },
                                *endpoint,
                            )? != *bound
                        {
                            return Err(INVALID);
                        }
                        previous_prefix = Some(base);
                    }
                }
            }
        }
        for effect in alternative.before.iter().chain(&alternative.after) {
            check_effect(program, plan, effect)?;
        }
    }

    Ok(())
}
fn operation(program: &Program, plan: &super::Plan, op: &Operation) -> Result<()> {
    let slot = |id: usize| ordinary_slot(plan, id);
    let data = |p: &Place| place(program, plan, p);
    let equal = |a: usize, b: usize| if a == b { Ok(()) } else { Err(INVALID) };
    match op {
        Operation::None | Operation::BoundsTrap => Ok(()),
        Operation::Literal {
            destination,
            value: v,
        } => value(program, slot(*destination)?.data, v)
            .then_some(())
            .ok_or(INVALID),
        Operation::Repeat {
            destination,
            source,
        } => repeat_operation(program, plan, *destination, source),
        Operation::Alias {
            destination,
            source,
        } => {
            if slot(*destination)?.storage != Storage::Alias {
                return Err(INVALID);
            }
            equal(slot(*destination)?.data, data(source)?)
        }
        Operation::Index {
            destination,
            base,
            index,
        }
        | Operation::IndexCopy {
            destination,
            base,
            index,
        } => index_operation(
            program,
            plan,
            *destination,
            base,
            index,
            matches!(op, Operation::Index { .. }),
        ),
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
        } => binary_operation(program, plan, destination, *operation, left, right),

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
            ) && !matches!(
                program.types.get(data(left)?),
                Some(Type::I32 | Type::Usize)
            ) {
                return Err(INVALID);
            }
            if program.types.get(slot(*destination)?.data) != Some(&Type::Bool)
                || !matches!(
                    program.types.get(data(left)?),
                    Some(Type::Bool | Type::I32 | Type::Usize)
                )
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
fn binary_operation(
    program: &Program,
    plan: &super::Plan,
    destination: &Place,
    operation: crate::CompileTimeBinaryOperation,
    left: &Place,
    right: &Place,
) -> Result<()> {
    if matches!(
        operation,
        crate::CompileTimeBinaryOperation::ShiftLeft
            | crate::CompileTimeBinaryOperation::ShiftRightSigned
            | crate::CompileTimeBinaryOperation::ShiftRightUnsigned
    ) {
        return Err(INVALID);
    }
    let data = place(program, plan, left)?;
    if !matches!(program.types.get(data), Some(Type::I32 | Type::Usize))
        || place(program, plan, right)? != data
        || place(program, plan, destination)? != data
    {
        return Err(INVALID);
    }
    Ok(())
}
fn repeat_operation(
    program: &Program,
    plan: &super::Plan,
    destination: usize,
    source: &Place,
) -> Result<()> {
    let result = ordinary_slot(plan, destination)?;
    let Some(Type::Array { element, length }) = program.types.get(result.data) else {
        return Err(INVALID);
    };
    if result.storage == Storage::Alias || *length <= 1 || *element != place(program, plan, source)?
    {
        return Err(INVALID);
    }
    Ok(())
}
fn index_operation(
    program: &Program,
    plan: &super::Plan,
    destination: usize,
    base: &Place,
    index: &Place,
    alias: bool,
) -> Result<()> {
    let Some(Type::Array { element, .. }) = program.types.get(place(program, plan, base)?) else {
        return Err(INVALID);
    };
    let result = ordinary_slot(plan, destination)?;
    if (result.storage == Storage::Alias) != alias
        || result.data != *element
        || program.types.get(place(program, plan, index)?) != Some(&Type::Usize)
    {
        return Err(INVALID);
    }
    Ok(())
}
fn call(
    program: &Program,
    plan: &super::Plan,
    destination: usize,
    target: nocter_model::ValueCode,
    inputs: &[super::Input],
) -> Result<()> {
    let callee = program.plans.get(&target).ok_or(INVALID)?;
    if callee.initializer
        || inputs.len() != callee.inputs.len()
        || ordinary_slot(plan, destination)?.data != callee.result
    {
        return Err(INVALID);
    }
    for (index, (input, &(parameter, operation))) in inputs.iter().zip(&callee.inputs).enumerate() {
        let environment_adapter = index == 0
            && matches!(target, nocter_model::ValueCode::Closure(_))
            && match (input.operation, operation) {
                (
                    nocter_model::ValueInputOperation::Edit,
                    nocter_model::ValueInputOperation::Read,
                ) => true,
                (
                    nocter_model::ValueInputOperation::Take,
                    nocter_model::ValueInputOperation::Read
                    | nocter_model::ValueInputOperation::Edit,
                ) => callee.environment_backing.is_some(),
                _ => false,
            };
        if (!environment_adapter && input.operation != operation)
            || place(program, plan, &input.place)?
                != callee.slots.get(parameter).ok_or(INVALID)?.data
        {
            return Err(INVALID);
        }
    }
    Ok(())
}
fn bounded_array(
    program: &Program,
    plan: &super::Plan,
    source: &Place,
    endpoint: usize,
) -> Result<u64> {
    if plan
        .slots
        .get(source.slot)
        .is_none_or(|slot| slot.storage != Storage::Owned)
        || plan.slots.get(endpoint).is_none_or(|slot| {
            slot.storage != Storage::Owned
                || !matches!(program.types.get(slot.data), Some(Type::Usize))
        })
    {
        return Err(INVALID);
    }
    let data = place(program, plan, source)?;
    let Some(Type::Array { length, .. }) = program.types.get(data) else {
        return Err(INVALID);
    };
    if *length == 0 {
        return Err(INVALID);
    }
    Ok(*length)
}
fn owned_place(program: &Program, plan: &super::Plan, source: &Place) -> Result<usize> {
    if plan
        .slots
        .get(source.slot)
        .is_some_and(|slot| slot.storage == Storage::Owned)
    {
        place(program, plan, source)
    } else {
        Err(INVALID)
    }
}
fn check_array_move(
    program: &Program,
    plan: &super::Plan,
    array: &Place,
    selected: &Place,
    destination: usize,
) -> Result<()> {
    let data = owned_place(program, plan, array)?;
    let Some(Type::Array { element, length }) = program.types.get(data) else {
        return Err(INVALID);
    };
    if *length == 0
        || !selected.fields.is_empty()
        || plan
            .slots
            .get(selected.slot)
            .is_none_or(|slot| slot.storage != Storage::Alias)
        || place(program, plan, selected)? != *element
        || owned_place(program, plan, &Place::root(destination))? != *element
    {
        return Err(INVALID);
    }
    Ok(())
}
fn check_effect(program: &Program, plan: &super::Plan, effect: &Effect) -> Result<()> {
    let data = |p: &Place| place(program, plan, p);
    let owned = |p: &Place| owned_place(program, plan, p);
    match effect {
        Effect::MoveArrayElement {
            array,
            selected,
            destination,
        } => {
            check_array_move(program, plan, array, selected, *destination)?;
        }
        Effect::ConsolidateArrayPrefix { source, endpoint } => {
            bounded_array(program, plan, source, *endpoint)?;
        }
        Effect::DisposeArrayElements { source, range } => {
            let data = owned(source)?;
            let Some(Type::Array { length, .. }) = program.types.get(data) else {
                return Err(INVALID);
            };
            match range {
                nocter_model::ArrayRange::Constant { .. } => {
                    if range.resolve(*length, |_| None).is_none() {
                        return Err(INVALID);
                    }
                }
                nocter_model::ArrayRange::Before(slot)
                | nocter_model::ArrayRange::Through(slot)
                | nocter_model::ArrayRange::After(slot) => {
                    bounded_array(program, plan, source, *slot)?;
                }
            }
        }
        Effect::DisposeArrayRemainder(source) => {
            if !matches!(program.types.get(owned(source)?), Some(Type::Array { length, .. }) if *length > 0)
            {
                return Err(INVALID);
            }
        }
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
        Effect::AdaptFunction {
            source,
            destination,
        } => {
            let from = program.types.get(owned(source)?).ok_or(INVALID)?;
            let to = program.types.get(owned(destination)?).ok_or(INVALID)?;
            if !same_function_payload(from, to) {
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
            let schema = program.types.get(ty).ok_or(INVALID)?;
            if schema.component_count() != u64::try_from(fields.len()).ok() {
                return Err(INVALID);
            }
            let mut seen = BTreeSet::new();
            for (field, source) in fields {
                if schema.component(*field) != Some(owned(source)?) || !seen.insert(*field) {
                    return Err(INVALID);
                }
            }
        }
    }
    Ok(())
}

// Invocation permission is a producer fact; this check preserves its exact payload representation.
fn same_function_payload(from: &Type, to: &Type) -> bool {
    match (from, to) {
        (Type::NamedFunction(a), Type::NamedFunction(b)) => a == b,
        (
            Type::CapturedFunction {
                code: a,
                environment: left,
            },
            Type::CapturedFunction {
                code: b,
                environment: right,
            },
        ) => a == b && left == right,
        _ => false,
    }
}
