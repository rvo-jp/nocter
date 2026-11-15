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

/// A closed call boundary, independent of whether an executable implementation is available.
pub(in crate::value_access) struct CallBoundary {
    pub inputs: Box<[nocter_declarations::value_access::FunctionInput<DataInstanceKey>]>,
    pub result: DataInstanceKey,
    pub environment_backing: bool,
}
pub(in crate::value_access) fn boundary(
    program: &Program,
    evidence: &Evidence,
    body: Option<&flow::Query>,
    frame: &CodeInstanceKey,
    work: &mut WorkBudget,
) -> Result<CallBoundary, InstantiationFailure> {
    work.step().map_err(|_| InstantiationFailure::WorkLimit)?;
    if let Some(body) = body {
        if body.typed().code() != frame.code() {
            return Err(InstantiationFailure::WrongOwner);
        }
        let inputs = body
            .typed()
            .body()
            .inputs()
            .ordered()
            .map(|channel| {
                work.step().map_err(|_| InstantiationFailure::WorkLimit)?;
                Ok(nocter_declarations::value_access::FunctionInput {
                    data: input(program, evidence, body, frame, channel.local, work)?,
                    operation: channel.operation,
                })
            })
            .collect::<Result<_, InstantiationFailure>>()?;
        return Ok(CallBoundary {
            inputs,
            result: result(program, evidence, body, frame, work)?,
            environment_backing: body.typed().body().inputs().environment().is_some_and(
                |channel| channel.operation != nocter_model::ValueInputOperation::Take,
            ),
        });
    }
    let nocter_model::ValueCode::Declaration(code) = frame.code() else {
        return Err(InstantiationFailure::Unresolved);
    };
    let contract = program
        .function_contract(code.into())
        .ok_or(InstantiationFailure::Unresolved)?;
    let inputs = contract
        .inputs
        .iter()
        .enumerate()
        .map(|(index, channel)| {
            work.step().map_err(|_| InstantiationFailure::WorkLimit)?;
            let data = frame
                .input_data(index)
                .cloned()
                .map_or_else(|| program.instantiate_data(channel.data, frame), Ok)
                .map_err(|_| InstantiationFailure::Unresolved)?;
            if data.data() != channel.data {
                return Err(InstantiationFailure::Unresolved);
            }
            Ok(nocter_declarations::value_access::FunctionInput {
                data,
                operation: channel.operation,
            })
        })
        .collect::<Result<_, InstantiationFailure>>()?;
    // A declaration can describe the boundary without describing any returned producer.
    // Context-dependent result families still require their original body evidence.
    let result = program
        .instantiate_closed_data(contract.result)
        .map_err(|_| InstantiationFailure::Unresolved)?;
    Ok(CallBoundary {
        inputs,
        result,
        environment_backing: false,
    })
}
#[cfg(test)]
mod tests;
