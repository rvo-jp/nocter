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

// Summarize shared schemas once. Arrays, tag-selected outer carriers and nominal fields
// need their own predicates; they cannot manufacture a fixed tuple-field attribution.
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
    use crate::value_access::{InstantiationFailure as Failure, ValueSource};
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
    let sources = body
        .typed()
        .result_evidence
        .as_ref()
        .and_then(|evidence| evidence.value_sources(body.typed().code()))
        .ok_or(Failure::Unresolved)?;
    let expected: Vec<_> = fields
        .iter()
        .copied()
        .map(nocter_model::ValueProjection::Tuple)
        .chain([nocter_model::ValueProjection::Variant(true)])
        .collect();
    let mut reached = false;
    for call in sources.call(expression).ok_or(Failure::Unresolved)? {
        if !control.reachable(call.origin.point) {
            continue;
        }
        reached = true;
        let mut pending = vec![(call.callee, expected.len())];
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

#[cfg(test)]
mod tests {
    use super::*;

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
