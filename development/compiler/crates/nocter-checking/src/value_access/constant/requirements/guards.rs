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
