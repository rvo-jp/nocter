use super::*;

pub(in crate::ir::lower) fn aggregate_call_instruction(
    return_type: &Type,
    destination: AggregateLocation,
    target: CallTarget,
    arguments: Vec<ScalarArgument>,
    layout: ValueLayout,
) -> Instruction {
    match return_type {
        Type::Aggregate { .. } => Instruction::CallAggregate {
            destination,
            target,
            arguments,
        },
        Type::DirectAggregate { .. } => Instruction::CallDirectAggregate {
            destination,
            target,
            arguments,
            layout,
        },
        _ => unreachable!("aggregate call instruction requires aggregate return type"),
    }
}

pub(in crate::ir::lower) fn push_aggregate_call_instruction(
    instructions: &mut Vec<Instruction>,
    return_type: &Type,
    destination: AggregateLocation,
    target: CallTarget,
    arguments: Vec<ScalarArgument>,
    layout: ValueLayout,
) {
    instructions.push(aggregate_call_instruction(
        return_type,
        destination,
        target,
        arguments,
        layout,
    ));
}

pub(in crate::ir::lower) fn fallible_aggregate_call_instruction(
    success_type: &Type,
    destination: AggregateLocation,
    target: CallTarget,
    arguments: Vec<ScalarArgument>,
    layout: ValueLayout,
    failure_mode: OutcomeFailureMode,
) -> Instruction {
    match success_type {
        Type::Aggregate { .. } => Instruction::CallOutcomeAggregate {
            destination,
            target,
            arguments,
            failure_mode,
        },
        Type::DirectAggregate { .. } => Instruction::CallOutcomeDirectAggregate {
            destination,
            target,
            arguments,
            layout,
            failure_mode,
        },
        _ => unreachable!("fallible aggregate call instruction requires aggregate success type"),
    }
}

pub(in crate::ir::lower) fn push_fallible_aggregate_call_instruction(
    instructions: &mut Vec<Instruction>,
    success_type: &Type,
    destination: AggregateLocation,
    target: CallTarget,
    arguments: Vec<ScalarArgument>,
    layout: ValueLayout,
    failure_mode: OutcomeFailureMode,
) {
    instructions.push(fallible_aggregate_call_instruction(
        success_type,
        destination,
        target,
        arguments,
        layout,
        failure_mode,
    ));
}
