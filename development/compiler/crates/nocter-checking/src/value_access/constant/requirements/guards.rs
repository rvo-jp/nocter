//! Attribute a direct nullable callable witness to its original input schema, without tag facts.
use super::{INVALID, flow};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{AccessLocalId, ValueFunctionOrigin};
use std::collections::BTreeSet;

/// Scoped to the owning immutable Requirements body; neither case proves an actual input tag.
#[derive(Clone, Copy, Debug)]
pub(super) enum InputResource {
    Stable(AccessLocalId),
    Unproved(AccessLocalId),
}
impl InputResource {
    pub(super) fn local(self) -> AccessLocalId {
        match self {
            Self::Stable(local) | Self::Unproved(local) => local,
        }
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
        let Some(PayloadSchema::Optional(primary)) = typed.program().data_schema(data) else {
            continue;
        };
        let Some(PayloadSchema::Function(witness)) = typed.program().data_schema(primary) else {
            continue;
        };
        if witness.origin != target {
            continue;
        }
        if selected.replace(input.local).is_some() {
            return Err(INVALID);
        }
    }
    Ok(selected.map(|local| {
        if writes.is_some_and(|writes| !writes.contains(&local)) {
            InputResource::Stable(local)
        } else {
            InputResource::Unproved(local)
        }
    }))
}

/// Entry absence can discharge only a callee value derived from that exact original input.
/// Opaque calls/writes/constructions supply no such proof. This walks the existing source graph.
pub(super) fn derives_from_input(
    body: &flow::Query,
    control: &crate::value_access::InstanceControl,
    expression: nocter_model::AccessExpressionId,
    local: AccessLocalId,
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
    let mut reached = false;
    for call in sources.call(expression).ok_or(Failure::Unresolved)? {
        if !control.reachable(call.origin.point) {
            continue;
        }
        reached = true;
        let mut pending = vec![call.callee];
        let mut visited = BTreeSet::new();
        let mut anchored = false;
        while let Some(value) = pending.pop() {
            work.step().map_err(|_| Failure::WorkLimit)?;
            if !visited.insert(value) {
                continue;
            }
            match &sources.node(value).ok_or(Failure::Unresolved)?.value {
                ValueSource::Input { local: source, .. } if *source == local => anchored = true,
                ValueSource::Adapt { source }
                | ValueSource::Transport { source }
                | ValueSource::Select { source, .. } => pending.push(*source),
                ValueSource::Merge { point, inputs, .. } => {
                    let before = pending.len();
                    pending.extend(
                        inputs
                            .iter()
                            .filter(|(boundary, _)| control.edge_reachable(boundary.point, *point))
                            .map(|(_, source)| *source),
                    );
                    if pending.len() == before {
                        return Ok(false);
                    }
                }
                ValueSource::Alternatives(inputs) if !inputs.is_empty() => pending.extend(inputs),
                _ => return Ok(false),
            }
        }
        if !anchored {
            return Ok(false);
        }
    }
    Ok(reached)
}
