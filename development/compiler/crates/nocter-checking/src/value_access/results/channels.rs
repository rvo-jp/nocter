//! Body-channel metadata shared by native and evaluator consumers; no value or ABI permission.
use super::{
    Evidence, InstantiationFailure, ValueSourceBoundary, ValueSourceSelection, ValueSourceSlot,
};
use crate::value_access::flow;
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{CodeInstanceKey, DataInstanceKey, Program};
use nocter_model::{AccessLocalId, ValuePath};
mod prepared;
pub(in crate::value_access) use prepared::arguments as instantiate_prepared_data;
pub use prepared::{PreparedData, PreparedDataFailure};

fn input_type(
    body: &flow::Query,
    frame: &CodeInstanceKey,
    local: AccessLocalId,
) -> Result<nocter_model::TypeId, InstantiationFailure> {
    if body.typed().code() != frame.code() {
        return Err(InstantiationFailure::WrongOwner);
    }
    if !body
        .typed()
        .body()
        .inputs()
        .ordered()
        .any(|input| input.local == local)
    {
        return Err(InstantiationFailure::Unresolved);
    }
    Ok(body
        .typed()
        .body()
        .locals()
        .get(local)
        .ok_or(InstantiationFailure::Unresolved)?
        .data)
}

pub(in crate::value_access) fn completed_input(
    program: &Program,
    evidence: &Evidence,
    body: &flow::Query,
    frame: &CodeInstanceKey,
    local: AccessLocalId,
    work: &mut WorkBudget,
) -> Result<DataInstanceKey, InstantiationFailure> {
    let data = input_type(body, frame, local)?;
    super::instantiate_source_completed_input_data(program, evidence, frame, local, data, work)
}

pub(in crate::value_access) fn input(
    program: &Program,
    evidence: &Evidence,
    body: &flow::Query,
    frame: &CodeInstanceKey,
    local: AccessLocalId,
    work: &mut WorkBudget,
) -> Result<DataInstanceKey, InstantiationFailure> {
    let data = input_type(body, frame, local)?;
    if let Ok(key) = program.instantiate_closed_data(data) {
        return Ok(key);
    }
    super::instantiate_stored_data(
        program,
        evidence,
        frame,
        &ValueSourceSelection {
            boundary: ValueSourceBoundary {
                point: body.graph().entry(),
                action: 0,
            },
            slot: ValueSourceSlot::Local(local),
            path: ValuePath::default(),
        },
        data,
        work,
    )
}

pub(in crate::value_access) fn result(
    program: &Program,
    evidence: &Evidence,
    body: &flow::Query,
    frame: &CodeInstanceKey,
    work: &mut WorkBudget,
) -> Result<DataInstanceKey, InstantiationFailure> {
    if body.typed().code() != frame.code() {
        return Err(InstantiationFailure::WrongOwner);
    }
    let data = body.typed().result_data();
    // A declaration-closed contract can describe a diverging result without producing it.
    if let Ok(key) = program.instantiate_closed_data(data) {
        return Ok(key);
    }
    super::instantiate_result_data(program, evidence, frame, data, work)
}
