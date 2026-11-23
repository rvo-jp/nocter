//! Attribute nullable callable witnesses to exact original input components, without tag facts.
use super::{INVALID, flow};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{AccessLocalId, ValueFunctionOrigin};
use std::collections::BTreeSet;

/// Scoped to the owning immutable Requirements body; this proves no actual input tag.
#[derive(Clone, Debug)]
pub(super) struct InputResource {
    pub(super) local: AccessLocalId,
    pub(super) fields: Box<[usize]>,
    pub(super) stable: bool,
}
impl InputResource {
    pub(super) fn local(&self) -> AccessLocalId {
        self.local
    }
}
/// Missing source correspondence is not a proof that a body leaves its input unchanged.
pub(super) fn writes(body: &flow::Query) -> Option<BTreeSet<AccessLocalId>> {
    let typed = body.typed();
    let flow = typed
        .result_evidence
        .as_ref()?
        .value_sources(typed.code())?
        .flow()?;
    let inputs = typed
        .body()
        .inputs()
        .ordered()
        .map(|input| input.local)
        .collect::<BTreeSet<_>>();
    Some(
        flow.definitions()
            .filter_map(|((_, slot), _)| match slot {
                crate::value_access::ValueSourceSlot::Local(local) if inputs.contains(&local) => {
                    Some(local)
                }
                _ => None,
            })
            .collect(),
    )
}

pub(super) fn input_resource(
    body: &flow::Query,
    target: ValueFunctionOrigin,
    writes: Option<&BTreeSet<AccessLocalId>>,
) -> Result<Option<InputResource>, flow::IntegrityError> {
    if !matches!(target, ValueFunctionOrigin::Parameter(_)) {
        return Ok(None);
    }
    let typed = body.typed();
    let mut selected = None;
    for input in typed.body().inputs().ordered() {
        let data = typed.body().locals().get(input.local).ok_or(INVALID)?.data;
        let Some(fields) = nullable_path(typed.program(), data, target) else {
            continue;
        };
        if selected
            .replace(InputResource {
                local: input.local,
                fields,
                stable: writes.is_some_and(|writes| !writes.contains(&input.local)),
            })
            .is_some()
        {
            return Ok(None);
        }
    }
    Ok(selected)
}

enum Candidate {
    Absent,
    Unique(Box<[usize]>),
    Ambiguous,
}

// Summarize shared schemas once. Arrays, failure-selected carriers and nominal fields
// need their own predicates; they cannot manufacture a static tuple/optional attribution.
fn nullable_path(
    program: &nocter_declarations::value_access::Program,
    root: nocter_model::TypeId,
    target: ValueFunctionOrigin,
) -> Option<Box<[usize]>> {
    use nocter_declarations::value_access::AggregateForm;
    use std::collections::BTreeMap;
    let mut done = BTreeMap::new();
    let mut pending = vec![(root, false)];
    while let Some((data, expanded)) = pending.pop() {
        if done.contains_key(&data) {
            continue;
        }
        let mut result = Candidate::Absent;
        match program.data_schema(data) {
            Some(PayloadSchema::Optional(payload)) if matches!(program.data_schema(payload), Some(PayloadSchema::Function(witness)) if witness.origin == target) =>
            {
                result = Candidate::Unique(Box::new([]));
            }
            Some(PayloadSchema::Optional(payload)) => {
                if !expanded {
                    pending.push((data, true));
                    pending.push((payload, false));
                    continue;
                }
                result = match &done[&payload] {
                    Candidate::Absent => Candidate::Absent,
                    Candidate::Unique(path) => {
                        Candidate::Unique(std::iter::once(0).chain(path.iter().copied()).collect())
                    }
                    Candidate::Ambiguous => Candidate::Ambiguous,
                };
            }
            Some(PayloadSchema::Aggregate(schema)) => {
                if let AggregateForm::Tuple(fields) = schema.form() {
                    if !expanded {
                        pending.push((data, true));
                        pending.extend(fields.iter().rev().map(|&field| (field, false)));
                        continue;
                    }
                    for (index, field) in fields.iter().enumerate() {
                        match &done[field] {
                            Candidate::Absent => {}
                            Candidate::Unique(path) if matches!(result, Candidate::Absent) => {
                                result = Candidate::Unique(
                                    std::iter::once(index).chain(path.iter().copied()).collect(),
                                );
                            }
                            _ => {
                                result = Candidate::Ambiguous;
                                break;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        done.insert(data, result);
    }
    match done.remove(&root)? {
        Candidate::Unique(path) => Some(path),
        _ => None,
    }
}

/// Interpret fixed ordinals using the original input schema, without observing any value.
fn expected_path(
    body: &flow::Query,
    local: AccessLocalId,
    fields: &[usize],
    work: &mut nocter_constant_evaluation::state::WorkBudget,
) -> Result<Option<Vec<nocter_model::ValueProjection<()>>>, crate::value_access::InstantiationFailure>
{
    use crate::value_access::InstantiationFailure as Failure;
    let mut data = body
        .typed()
        .body()
        .locals()
        .get(local)
        .ok_or(Failure::Unresolved)?
        .data;
    let mut expected = Vec::with_capacity(fields.len() + 1);
    for &field in fields {
        work.step().map_err(|_| Failure::WorkLimit)?;
        match body.typed().program().data_schema(data) {
            Some(PayloadSchema::Aggregate(schema)) => {
                let nocter_declarations::value_access::AggregateForm::Tuple(fields) = schema.form()
                else {
                    return Ok(None);
                };
                data = *fields.get(field).ok_or(Failure::Unresolved)?;
                expected.push(nocter_model::ValueProjection::Tuple(field));
            }
            Some(PayloadSchema::Optional(payload)) if field == 0 => {
                data = payload;
                expected.push(nocter_model::ValueProjection::Variant(true));
            }
            _ => return Ok(None),
        }
    }
    if !matches!(
        body.typed().program().data_schema(data),
        Some(PayloadSchema::Optional(_))
    ) {
        return Ok(None);
    }
    expected.push(nocter_model::ValueProjection::Variant(true));
    Ok(Some(expected))
}

/// Entry absence can discharge only a callee value derived from that exact original input.
/// Opaque calls/writes/constructions supply no such proof. This walks the existing source graph.
pub(super) fn derives_from_input(
    body: &flow::Query,
    control: &crate::value_access::InstanceControl,
    expression: nocter_model::AccessExpressionId,
    local: AccessLocalId,
    fields: &[usize],
    work: &mut nocter_constant_evaluation::state::WorkBudget,
) -> Result<bool, crate::value_access::InstantiationFailure> {
    use crate::value_access::InstantiationFailure as Failure;
    let Some(input) = body
        .typed()
        .body()
        .inputs()
        .ordered()
        .find(|input| input.local == local)
    else {
        return Err(Failure::Unresolved);
    };
    // A mutable/owned entry needs a whole-body no-write boundary until general alias
    // interference is qualified. Read inputs retain ordinary source access protection.
    if input.operation != nocter_model::ValueInputOperation::Read {
        for (_, point) in body.graph().points().iter() {
            for action in &point.actions {
                work.step().map_err(|_| Failure::WorkLimit)?;
                match action.operation {
                    flow::Operation::Assign { .. } | flow::Operation::CompoundAssign { .. } => {
                        return Ok(false);
                    }
                    flow::Operation::CommitCall(call) if call != expression => {
                        // Other calls may hide alias effects. Only the guarded invocation itself
                        // is excluded by entry absence; general sibling-call effects are unproved.
                        return Ok(false);
                    }
                    _ => {}
                }
            }
        }
    }
    let Some(expected) = expected_path(body, local, fields, work)? else {
        return Ok(false);
    };
    derives_from_projection(body, control, expression, local, &expected, work)
}

/// Original input lineage only; this supplies no tag, bounds, value or invocation permission.
fn derives_from_projection(
    body: &flow::Query,
    control: &crate::value_access::InstanceControl,
    expression: nocter_model::AccessExpressionId,
    local: AccessLocalId,
    expected: &[nocter_model::ValueProjection<()>],
    work: &mut nocter_constant_evaluation::state::WorkBudget,
) -> Result<bool, crate::value_access::InstantiationFailure> {
    use crate::value_access::{InstantiationFailure as Failure, ValueSource};
    let sources = body
        .typed()
        .result_evidence
        .as_ref()
        .and_then(|evidence| evidence.value_sources(body.typed().code()))
        .ok_or(Failure::Unresolved)?;
    let mut reached = false;
    for call in sources.call(expression).ok_or(Failure::Unresolved)? {
        if !control.reachable(call.origin.point) {
            continue;
        }
        reached = true;
        let Some(callee) = call.callee.witness_source() else {
            return Ok(false);
        };
        let mut pending = vec![(callee, expected.len())];
        let mut visited = BTreeSet::new();
        let mut anchored = false;
        while let Some((value, remaining)) = pending.pop() {
            work.step().map_err(|_| Failure::WorkLimit)?;
            if !visited.insert((value, remaining)) {
                continue;
            }
            match &sources.node(value).ok_or(Failure::Unresolved)?.value {
                ValueSource::Input { local: source, .. } if *source == local && remaining == 0 => {
                    anchored = true;
                }
                ValueSource::Adapt { source } | ValueSource::Transport { source } => {
                    pending.push((*source, remaining));
                }
                ValueSource::Select { source, path } => {
                    let Some(start) = remaining.checked_sub(path.fields().len()) else {
                        return Ok(false);
                    };
                    for (actual, expected) in path.fields().iter().zip(&expected[start..remaining])
                    {
                        work.step().map_err(|_| Failure::WorkLimit)?;
                        if actual != expected {
                            return Ok(false);
                        }
                    }
                    pending.push((*source, start));
                }
                ValueSource::Merge { point, inputs, .. } => {
                    let before = pending.len();
                    pending.extend(
                        inputs
                            .iter()
                            .filter(|(boundary, _)| control.edge_reachable(boundary.point, *point))
                            .map(|(_, source)| (*source, remaining)),
                    );
                    if pending.len() == before {
                        return Ok(false);
                    }
                }
                ValueSource::Alternatives(inputs) if !inputs.is_empty() => {
                    pending.extend(inputs.iter().map(|&source| (source, remaining)));
                }
                _ => return Ok(false),
            }
        }
        if !anchored {
            return Ok(false);
        }
    }
    Ok(reached)
}

/// First qualified capability residual: an unmodified read input's direct nominal function field.
/// Schema matching proposes a path; the shared original-source walk must independently prove it.
pub(super) fn stable_callable_field_input(
    body: &flow::Query,
    control: &crate::value_access::InstanceControl,
    expression: nocter_model::AccessExpressionId,
    written: Option<&BTreeSet<AccessLocalId>>,
    work: &mut nocter_constant_evaluation::state::WorkBudget,
) -> Result<bool, crate::value_access::InstantiationFailure> {
    use crate::value_access::{ExpressionKind, InstantiationFailure as Failure};
    use nocter_declarations::value_access::AggregateForm;
    use nocter_model::{ValueField, ValueInputOperation, ValueProjection};
    let ExpressionKind::Call {
        target: target @ ValueFunctionOrigin::Contract(_),
        ..
    } = &body
        .typed()
        .body()
        .expressions()
        .get(expression)
        .ok_or(Failure::Unresolved)?
        .kind
    else {
        return Ok(false);
    };
    let Some(written) = written else {
        return Ok(false);
    };
    for input in body.typed().body().inputs().ordered() {
        work.step().map_err(|_| Failure::WorkLimit)?;
        if input.operation != ValueInputOperation::Read || written.contains(&input.local) {
            continue;
        }
        let data = body
            .typed()
            .body()
            .locals()
            .get(input.local)
            .ok_or(Failure::Unresolved)?
            .data;
        let Some(PayloadSchema::Aggregate(schema)) = body.typed().program().data_schema(data)
        else {
            continue;
        };
        let AggregateForm::Record(record) = schema.form() else {
            continue;
        };
        for &field in record.fields() {
            work.step().map_err(|_| Failure::WorkLimit)?;
            let component = schema
                .component(ValueField::Record(field))
                .ok_or(Failure::Unresolved)?;
            if matches!(body.typed().program().data_schema(component.data), Some(PayloadSchema::Function(witness)) if witness.origin == *target)
                && derives_from_projection(
                    body,
                    control,
                    expression,
                    input.local,
                    &[ValueProjection::Record(field)],
                    work,
                )?
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_field_lineage_is_budgeted_and_retry_does_not_inherit_partial_proofs() {
        use nocter_constant_evaluation::state::{Limits, WorkBudget};
        let fixture = nocter_test_support::ValueSourceFixture::new(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/native/value-access/received-record-callable-conditions.nct"
        )));
        let unit = crate::value_access::accepted::SourceUnit::prepare(fixture.syntax())
            .unwrap()
            .into_unit();
        let program = unit.program();
        let invoke = program
            .resolve(program.symbols().get("invoke").unwrap())
            .unwrap();
        let frame = program.bind_code(invoke.into(), []).unwrap();
        let body = unit.code_body(invoke.into()).unwrap();
        let call = body
            .typed()
            .body()
            .expressions()
            .iter()
            .find_map(|(id, expression)| {
                matches!(
                    expression.kind,
                    crate::value_access::ExpressionKind::Call { .. }
                )
                .then_some(id)
            })
            .unwrap();
        let control = unit
            .instantiate_control(&frame, &mut WorkBudget::new(Limits::default().steps()))
            .unwrap();
        let written = writes(body);
        let mut exhausted = WorkBudget::new(std::num::NonZeroU64::new(1).unwrap());
        exhausted.step().unwrap();
        assert_eq!(
            stable_callable_field_input(body, &control, call, written.as_ref(), &mut exhausted),
            Err(crate::value_access::InstantiationFailure::WorkLimit)
        );
        assert_eq!(
            stable_callable_field_input(
                body,
                &control,
                call,
                written.as_ref(),
                &mut WorkBudget::new(Limits::default().steps())
            ),
            Ok(true)
        );
    }
    #[test]
    fn nullable_component_attribution_preserves_ambiguity_without_expanding_shared_schemas() {
        let source = nocter_test_support::ValueSourceFixture::new(
            "func root(take value: (func(): i32)?): void {}",
        );
        let declarations =
            nocter_declaration_lowering::access_declarations::Query::prepare(source.syntax())
                .unwrap();
        let context = declarations.bind(source.syntax()).unwrap();
        let program = context.program();
        let root = program
            .resolve(program.symbols().get("root").unwrap())
            .unwrap();
        let input = program.codes().get(root).unwrap().parameters()[0];
        let optional = program.parameters().get(input).unwrap().data();
        let Some(PayloadSchema::Optional(payload)) = program.data_schema(optional) else {
            panic!("optional input")
        };
        let Some(PayloadSchema::Function(witness)) = program.data_schema(payload) else {
            panic!("callable leaf")
        };
        let mut body = program.body_types(root).unwrap();
        let mut shared = program.data_type(nocter_model::BuiltinType::I32).unwrap();
        for _ in 0..64 {
            shared = body.tuple(&[shared, shared]).unwrap();
        }
        let unique = body.tuple(&[shared, optional]).unwrap();
        let ambiguous = body.tuple(&[optional, optional]).unwrap();
        let body = body.finish();
        assert_eq!(
            nullable_path(&body, unique, witness.origin).as_deref(),
            Some(&[1][..])
        );
        assert!(nullable_path(&body, ambiguous, witness.origin).is_none());
        assert!(nullable_path(&body, shared, witness.origin).is_none());
    }
}
