//! Attribute a direct nullable callable witness to its original input schema, without tag facts.
use super::{INVALID, flow};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{AccessLocalId, ValueFunctionOrigin};

pub(super) fn input_resource(
    body: &flow::Query,
    target: ValueFunctionOrigin,
) -> Result<Option<AccessLocalId>, flow::IntegrityError> {
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
    Ok(selected)
}
