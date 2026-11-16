//! Link all cases without exposing a general projection into an unselected environment.
use super::{INVALID, Program, Result, Type, View, input_data, ordinary_slot, place};
use crate::state::{CallCase, Input, Place};
use nocter_model::ValueInputOperation;

pub(super) fn check(
    program: &Program,
    plan: &View<'_>,
    destination: usize,
    callee: &Place,
    cases: &[CallCase],
    inputs: &[Input],
) -> Result<()> {
    let data = place(program, plan, callee)?;
    let Some(Type::CallableAlternatives(expected)) = program.types.get(data) else {
        return Err(INVALID);
    };
    if expected.len() != cases.len() {
        return Err(INVALID);
    }
    let result = ordinary_slot(plan, destination)?.data;
    for (&data, case) in expected.iter().zip(cases) {
        if data != case.data {
            return Err(INVALID);
        }
        let target = program.contracts.get(&case.target).ok_or(INVALID)?;
        if target.initializer || target.result != result {
            return Err(INVALID);
        }
        let (code, environment) = program
            .types
            .get(data)
            .and_then(Type::function_identity)
            .ok_or(INVALID)?;
        if case.target.source != code {
            return Err(INVALID);
        }
        let offset = if let Some(environment) = environment {
            let input = target.inputs.first().ok_or(INVALID)?;
            if !target.environment_backing
                || input.data != environment
                || input.operation != ValueInputOperation::Read
            {
                return Err(INVALID);
            }
            1
        } else {
            0
        };
        let expected = target.inputs.get(offset..).ok_or(INVALID)?;
        if expected.len() != inputs.len() {
            return Err(INVALID);
        }
        for (actual, expected) in inputs.iter().zip(expected) {
            let data = input_data(program, plan, actual)?;
            if actual.operation() != expected.operation || data != expected.data {
                return Err(INVALID);
            }
        }
    }
    Ok(())
}
