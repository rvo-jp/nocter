use super::{Data, Effect, Operation, Place, Program, Rule, Storage, Terminator, Type, Value};
use std::collections::{BTreeMap, BTreeSet};
mod contexts;
mod guards;
mod handoffs;
mod receipts;
mod selected_calls;
use super::slot_types::{Contexts, SlotTypes, View as Slots};
pub(super) use receipts::compatible_receipt;
struct View<'a> {
    plan: &'a super::Plan,
    slots: Slots<'a>,
}
impl std::ops::Deref for View<'_> {
    type Target = super::Plan;
    fn deref(&self) -> &Self::Target {
        self.plan
    }
}
impl<'a> View<'a> {
    fn new(plan: &'a super::Plan, contexts: &'a Contexts) -> Self {
        Self {
            plan,
            slots: Slots {
                initial: &plan.slots,
                contexts,
            },
        }
    }
}
type Result<T> = std::result::Result<T, Rule>;
const INVALID: Rule = Rule::InvalidPlan;
fn ordinary_slot(plan: &View<'_>, id: usize) -> Result<super::Slot> {
    definition(plan, id)?;
    plan.slots.get(id).ok_or(INVALID)
}
fn definition(plan: &View<'_>, id: usize) -> Result<super::Slot> {
    if plan.environment_backing == Some(id) {
        return Err(INVALID);
    }
    plan.plan.slots.get(id).copied().ok_or(INVALID)
}
fn place(program: &Program, plan: &View<'_>, place: &Place) -> Result<usize> {
    definition(plan, place.slot)?;
    place_data(program, &plan.slots, place)
}
pub(super) fn place_data(
    program: &Program,
    slots: &(impl SlotTypes + ?Sized),
    place: &Place,
) -> Result<usize> {
    slots.select(program, place)
}
pub(super) fn project_data(
    program: &Program,
    slots: &(impl SlotTypes + ?Sized),
    place: &Place,
    mut data: usize,
) -> Result<usize> {
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
                    || program.types.get(snapshot.data)
                        != Some(&Type::Integer(nocter_model::BuiltinType::Usize))
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
            | (Some(Type::Bool), Data::Bool(_))
            | (Some(Type::Text), Data::Text(_)) => {}
            (Some(Type::ScalarEnum { variants, .. }), Data::EnumTag(tag)) if tag < variants => {}
            (Some(ty @ Type::Integer(_)), Data::Integer(value))
                if ty
                    .scalar()
                    .and_then(|ty| match ty {
                        crate::ConstantScalarType::Integer(builtin) => {
                            crate::support::integer_spec(builtin)
                        }
                        _ => None,
                    })
                    .is_some_and(|spec| spec.contains(*value)) => {}

            (Some(ty @ Type::Variant { .. }), Data::Variant { primary, payload }) => {
                let expected = ty.component(usize::from(!primary));
                match (expected, payload) {
                    (None, None) => {}
                    (Some(data), Some(payload)) => pending.push((data, payload.clone())),
                    _ => return false,
                }
            }
            (Some(Type::CallableAlternatives(cases)), encoded) => {
                let Some((case, payload)) = encoded.callable() else {
                    return false;
                };
                let Some(data) = usize::try_from(case)
                    .ok()
                    .and_then(|index| cases.get(index))
                else {
                    return false;
                };
                pending.push((*data, payload.clone()));
            }
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
            (Some(Type::EmptyArray), Data::Aggregate(values)) if values.is_empty() => {}
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
    validate_added(
        program,
        0,
        program.contracts.keys().copied(),
        program.plans.keys().copied(),
        program.statics.keys().copied(),
    )
}
fn validate_types(program: &Program, first: usize) -> Result<()> {
    for (id, data) in program.types.iter().enumerate().skip(first) {
        if let Type::Integer(builtin) = data
            && crate::support::integer_spec(*builtin).is_none()
        {
            return Err(INVALID);
        }
        if let Type::Record(fields) = data
            && fields.iter().any(|&field| field >= id)
        {
            return Err(INVALID);
        }
        if matches!(data, Type::ScalarEnum { variants: 0, .. }) {
            return Err(INVALID);
        }
        if let Type::Variant { primary, alternate } = data
            && std::iter::once(primary)
                .chain(alternate.iter())
                .any(|&payload| payload >= id)
        {
            return Err(INVALID);
        }
        if let Type::CapturedFunction { environment, .. } = data
            && *environment >= id
        {
            return Err(INVALID);
        }
        if let Type::CallableAlternatives(cases) = data {
            let mut identities = BTreeSet::new();
            if cases.len() < 2 {
                return Err(INVALID);
            }
            for &case in cases {
                if case >= id {
                    return Err(INVALID);
                }
                let identity = program
                    .types
                    .get(case)
                    .and_then(Type::function_identity)
                    .ok_or(INVALID)?;
                if !identities.insert(identity) {
                    return Err(INVALID);
                }
            }
        }
        if let Type::Array { element, length } = data
            && (*length == 0 || *element >= id)
        {
            return Err(INVALID);
        }
    }
    Ok(())
}
/// Old identities and payloads cannot change across an append-only batch. Every old reference
/// was already in range and linked; validate the new types and entries through the same rules.
pub(super) fn validate_added(
    program: &Program,
    first_type: usize,
    contracts: impl IntoIterator<Item = super::CodeInstance>,
    plans: impl IntoIterator<Item = super::CodeInstance>,
    statics: impl IntoIterator<Item = nocter_model::ValueStatic>,
) -> Result<()> {
    validate_types(program, first_type)?;
    for id in statics {
        let global = program.statics.get(&id).ok_or(INVALID)?;
        if !value(program, global.data, &global.value) {
            return Err(INVALID);
        }
    }
    for code in contracts {
        let contract = program.contracts.get(&code).ok_or(INVALID)?;
        if contract.result >= program.types.len()
            || contract.inputs.iter().any(|input| {
                program
                    .types
                    .get(input.data)
                    .is_none_or(|data| matches!(data, Type::Uninhabited))
            })
            || (contract.initializer && !contract.inputs.is_empty())
            || (matches!(code.source, nocter_model::ValueCode::Expression(_))
                && !contract.initializer)
            || (matches!(code.source, nocter_model::ValueCode::Closure(_)) && contract.initializer)
            || (contract.environment_backing
                && (!matches!(code.source, nocter_model::ValueCode::Closure(_))
                    || contract.inputs.first().is_none_or(|input| {
                        input.operation == nocter_model::ValueInputOperation::Take
                    })))
        {
            return Err(INVALID);
        }
    }
    for code in plans {
        let plan = program.plans.get(&code).ok_or(INVALID)?;
        if program.contracts.get(&code) != Some(&super::CallContract::from_plan(plan)?) {
            return Err(INVALID);
        }
        match code.source {
            nocter_model::ValueCode::Expression(_) if !plan.initializer => return Err(INVALID),
            nocter_model::ValueCode::Closure(_) if plan.initializer => return Err(INVALID),
            _ => {}
        }
        validate_plan(program, code, plan)?;
        if let Some(backing) = plan.environment_backing {
            let &(input, operation) = plan.inputs.first().ok_or(INVALID)?;
            let source = plan.slots.get(input).ok_or(INVALID)?;
            let target = plan.slots.get(backing).ok_or(INVALID)?;
            if !matches!(code.source, nocter_model::ValueCode::Closure(_))
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
fn validate_plan(program: &Program, code: super::CodeInstance, plan: &super::Plan) -> Result<()> {
    if plan.slots.iter().any(|slot| {
        program
            .types
            .get(slot.data)
            .is_none_or(|data| matches!(data, Type::Uninhabited))
    }) || plan.result >= program.types.len()
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
    for condition in &plan.conditions {
        let &(slot, _) = plan.inputs.get(condition.input).ok_or(INVALID)?;
        let source = plan.slots.get(slot).ok_or(INVALID)?;
        let mut data = source.data;
        for &field in &condition.fields {
            data = match program.types.get(data) {
                Some(Type::Record(fields)) => *fields.get(field).ok_or(INVALID)?,
                Some(Type::Variant {
                    primary,
                    alternate: None,
                }) if field == 0 => *primary,
                _ => return Err(INVALID),
            };
        }
        if !matches!(
            program.types.get(data),
            Some(Type::Variant {
                alternate: None,
                ..
            })
        ) || condition.cause.code != code.source
            || plan
                .points
                .get(condition.cause.point)
                .is_none_or(|point| condition.cause.action >= point.actions.len())
        {
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
    contexts::validate(program, plan)
}

fn validate_action(program: &Program, plan: &View<'_>, action: &super::Action) -> Result<Contexts> {
    if action.alternatives.is_empty() {
        return Err(INVALID);
    }
    let mut guards = BTreeMap::new();
    let mut result = None;
    for alternative in &action.alternatives {
        let phases = (&alternative.before, &alternative.after);
        if guards
            .insert(&alternative.owned, phases)
            .is_some_and(|existing| existing != phases)
        {
            return Err(INVALID);
        }
        guards::validate(program, plan, alternative)?;
        let mut contexts = plan.slots.contexts.clone();
        let mut handoffs = handoffs::Handoffs::new(&action.operation);
        for step in alternative.sequence(&action.operation) {
            let view = &View::new(plan.plan, &contexts);
            match step {
                super::sequence::Step::Context(target, expected) => {
                    if place(program, view, target)? != expected {
                        return Err(INVALID);
                    }
                }
                super::sequence::Step::Before(effect) => {
                    handoffs.protect(program, view, effect)?;
                    check_effect(program, view, effect)?;
                    contexts::effect(plan.plan, &mut contexts, effect)?;
                }
                super::sequence::Step::Operation(op) => {
                    handoffs.complete()?;
                    operation(program, view, op)?;
                }
                super::sequence::Step::After(effect) => {
                    if matches!(effect, Effect::Invoke { .. }) {
                        return Err(INVALID);
                    }
                    check_effect(program, view, effect)?;
                    contexts::effect(plan.plan, &mut contexts, effect)?;
                }
            }
        }
        if let Some(previous) = &mut result {
            contexts::join(plan.plan, previous, &contexts)?;
        } else {
            result = Some(contexts);
        }
    }

    result.ok_or(INVALID)
}
fn operation(program: &Program, plan: &View<'_>, op: &Operation) -> Result<()> {
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
        Operation::PrimaryCondition {
            source,
            destination,
        } => {
            if !matches!(program.types.get(data(source)?), Some(Type::Variant { .. }))
                || program.types.get(slot(*destination)?.data) != Some(&Type::Bool)
            {
                return Err(INVALID);
            }
            Ok(())
        }
        Operation::RequireVariant { source, view, .. } => {
            variant_view(program, plan, source, *view)
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
            let operand_type = program.types.get(data(operand)?);
            let valid = match operation {
                crate::CompileTimeUnaryOperation::LogicalNot => operand_type == Some(&Type::Bool),
                crate::CompileTimeUnaryOperation::Negate => matches!(operand_type,
                    Some(Type::Integer(builtin)) if crate::support::integer_spec(*builtin)
                        .is_some_and(|spec| spec.signed)),
            };
            if !valid {
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
        } => compare_operation(program, plan, *destination, *operation, left, right),
        Operation::Call {
            destination,
            target,
            inputs,
        } => call(program, plan, *destination, *target, inputs),
        Operation::SelectedCall {
            destination,
            callee,
            cases,
            inputs,
        } => selected_calls::check(program, plan, *destination, callee, cases, inputs),
    }
}
fn variant_view(
    program: &Program,
    plan: &View<'_>,
    source: &Place,
    view: Option<usize>,
) -> Result<()> {
    let data = place(program, plan, source)?;
    if let Some(view) = view {
        let slot = ordinary_slot(plan, view)?;
        if slot.storage != Storage::Alias || slot.data != data {
            return Err(INVALID);
        }
    }
    if !matches!(program.types.get(data), Some(Type::Variant { .. })) {
        return Err(INVALID);
    }
    Ok(())
}
fn compare_operation(
    program: &Program,
    plan: &View<'_>,
    destination: usize,
    operation: crate::CompileTimeComparisonOperation,
    left: &Place,
    right: &Place,
) -> Result<()> {
    let data = place(program, plan, left)?;
    let ordered = !matches!(
        operation,
        crate::CompileTimeComparisonOperation::Equal
            | crate::CompileTimeComparisonOperation::NotEqual
    );
    if program.types.get(ordinary_slot(plan, destination)?.data) != Some(&Type::Bool)
        || (ordered && !matches!(program.types.get(data), Some(Type::Integer(_))))
        || !matches!(program.types.get(data), Some(Type::Bool | Type::Integer(_)))
        || place(program, plan, right)? != data
    {
        return Err(INVALID);
    }
    Ok(())
}
fn binary_operation(
    program: &Program,
    plan: &View<'_>,
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
    if !matches!(program.types.get(data), Some(Type::Integer(_)))
        || place(program, plan, right)? != data
        || place(program, plan, destination)? != data
    {
        return Err(INVALID);
    }
    Ok(())
}
fn repeat_operation(
    program: &Program,
    plan: &View<'_>,
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
    plan: &View<'_>,
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
        || program.types.get(place(program, plan, index)?)
            != Some(&Type::Integer(nocter_model::BuiltinType::Usize))
    {
        return Err(INVALID);
    }
    Ok(())
}
fn call(
    program: &Program,
    plan: &View<'_>,
    destination: usize,
    target: super::CodeInstance,
    inputs: &[super::Input],
) -> Result<()> {
    let callee = program.contracts.get(&target).ok_or(INVALID)?;
    if callee.initializer
        || inputs.len() != callee.inputs.len()
        || ordinary_slot(plan, destination)?.data != callee.result
    {
        return Err(INVALID);
    }
    for (index, (input, parameter)) in inputs.iter().zip(&callee.inputs).enumerate() {
        let operation = parameter.operation;
        let environment_adapter = index == 0
            && matches!(target.source, nocter_model::ValueCode::Closure(_))
            && match (input.operation(), operation) {
                (
                    nocter_model::ValueInputOperation::Edit,
                    nocter_model::ValueInputOperation::Read,
                ) => true,
                (
                    nocter_model::ValueInputOperation::Take,
                    nocter_model::ValueInputOperation::Read
                    | nocter_model::ValueInputOperation::Edit,
                ) => callee.environment_backing,
                _ => false,
            };
        if (!environment_adapter && input.operation() != operation)
            || input_data(program, plan, input)? != parameter.data
        {
            return Err(INVALID);
        }
    }
    Ok(())
}
fn input_data(program: &Program, plan: &View<'_>, input: &super::Input) -> Result<usize> {
    match input {
        super::Input::Take(data) => Ok(*data),
        super::Input::Read(source) | super::Input::Edit(source) => place(program, plan, source),
    }
}
fn bounded_array(
    program: &Program,
    plan: &View<'_>,
    source: &Place,
    endpoint: usize,
) -> Result<u64> {
    if plan
        .slots
        .get(source.slot)
        .is_none_or(|slot| slot.storage != Storage::Owned)
        || plan.slots.get(endpoint).is_none_or(|slot| {
            slot.storage != Storage::Owned
                || !matches!(
                    program.types.get(slot.data),
                    Some(Type::Integer(nocter_model::BuiltinType::Usize))
                )
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
fn owned_place(program: &Program, plan: &View<'_>, source: &Place) -> Result<usize> {
    if definition(plan, source.slot)?.storage == Storage::Owned {
        let data = place(program, plan, source)?;
        if matches!(program.types.get(data), Some(Type::Uninhabited)) {
            return Err(INVALID);
        }
        Ok(data)
    } else {
        Err(INVALID)
    }
}
fn check_array_move(
    program: &Program,
    plan: &View<'_>,
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
fn check_effect(program: &Program, plan: &View<'_>, effect: &Effect) -> Result<()> {
    let data = |p: &Place| place(program, plan, p);
    let owned = |p: &Place| owned_place(program, plan, p);
    match effect {
        Effect::Context { .. } => return Err(INVALID),
        Effect::ReplaceRoot { .. } | Effect::MoveRoot { .. } | Effect::ReplaceReferent { .. } => {
            check_typed_receipt(program, plan, effect)?;
        }
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
        Effect::Transport {
            source,
            destination,
        } => {
            if !super::transport::permits(program, owned(source)?, owned(destination)?, || Ok(()))?
            {
                return Err(INVALID);
            }
        }
        Effect::AdaptFunction {
            source,
            destination,
        } => {
            let from = program.types.get(owned(source)?).ok_or(INVALID)?;
            let to = program.types.get(owned(destination)?).ok_or(INVALID)?;
            if !from.same_function_payload(to) {
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
        Effect::Dispose(source) | Effect::Release(source) | Effect::Invoke { source, .. } => {
            owned(source)?;
        }
        Effect::Assemble {
            destination,
            fields,
        } => check_assembly(program, plan, *destination, fields)?,
    }
    Ok(())
}

fn check_typed_receipt(program: &Program, plan: &View<'_>, effect: &Effect) -> Result<()> {
    match effect {
        Effect::ReplaceRoot {
            source,
            destination,
            data: received,
        } => check_root_receipt(program, plan, source, &Place::root(*destination), *received),
        Effect::MoveRoot {
            source,
            destination,
            data: received,
        } => check_root_receipt(program, plan, source, destination, *received),
        Effect::ReplaceReferent {
            source,
            destination,
            owner,
            owner_data,
            view_data,
        } => check_referent_receipt(
            program,
            plan,
            source,
            destination,
            owner,
            *owner_data,
            *view_data,
        ),
        _ => Err(INVALID),
    }
}

fn check_referent_receipt(
    program: &Program,
    plan: &View<'_>,
    source: &Place,
    destination: &Place,
    owner: &Place,
    owner_data: usize,
    view_data: usize,
) -> Result<()> {
    check_root_receipt(program, plan, source, owner, owner_data)?;
    if definition(plan, destination.slot)?.storage != Storage::Alias
        || source.slot == destination.slot
        || place(program, plan, destination)? != owned_place(program, plan, owner)?
        || owned_place(program, plan, source)?
            != project_data(program, &plan.slots, destination, view_data)?
    {
        return Err(INVALID);
    }
    check_receipt_context(program, plan, destination, view_data)
}
fn check_root_receipt(
    program: &Program,
    plan: &View<'_>,
    source: &Place,
    destination: &Place,
    received: usize,
) -> Result<()> {
    if definition(plan, destination.slot)?.storage != Storage::Owned
        || source.slot == destination.slot
        || owned_place(program, plan, source)?
            != project_data(program, &plan.slots, destination, received)?
        || program.types.get(received).is_none()
    {
        return Err(INVALID);
    }
    check_receipt_context(program, plan, destination, received)
}
fn check_receipt_context(
    program: &Program,
    plan: &View<'_>,
    destination: &Place,
    received: usize,
) -> Result<()> {
    let initial = definition(plan, destination.slot)?.data;
    if let Some(candidates) = plan.slots.contexts.get(&destination.slot) {
        for &before in candidates.iter() {
            compatible_receipt(program, before, received, &destination.fields)?;
        }
    } else {
        compatible_receipt(program, initial, received, &destination.fields)?;
    }
    Ok(())
}

fn check_assembly(
    program: &Program,
    plan: &View<'_>,
    destination: usize,
    fields: &[(usize, Place)],
) -> Result<()> {
    let owned = |place: &Place| owned_place(program, plan, place);
    let ty = owned(&Place::root(destination))?;
    let schema = program.types.get(ty).ok_or(INVALID)?;
    let complete = match schema {
        Type::Variant { alternate, .. } => {
            fields.len() == 1 || (fields.is_empty() && alternate.is_none())
        }
        _ => schema.component_count() == u64::try_from(fields.len()).ok(),
    };
    if !complete {
        return Err(INVALID);
    }
    let mut seen = BTreeSet::new();
    for (field, source) in fields {
        if schema.component(*field) != Some(owned(source)?) || !seen.insert(*field) {
            return Err(INVALID);
        }
    }
    Ok(())
}
