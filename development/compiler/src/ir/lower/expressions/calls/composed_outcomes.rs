use super::*;
use crate::ir::ComposedOutcomeDestination;
use crate::outcomes::OutcomeLayer;

pub(in crate::ir::lower) fn lower_composed_outcome_call(
    call: &CallExpr,
    destination: ComposedOutcomeDestination,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    outer_mode: OutcomeFailureMode,
    inner_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unavailable_call_target_diagnostic());
    };
    let Some(Type::ComposedOutcome {
        outer,
        inner,
        payload,
    }) = context.call_return_type(&target)
    else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!("IR composed outcome call expected a composed return from `{callee_name}`"),
        )]);
    };
    validate_destination(payload, destination, &callee_name)?;
    let outer = *outer;
    let inner = *inner;
    if !matches!(
        (outer, inner),
        (OutcomeLayer::Fallible, OutcomeLayer::Optional)
            | (OutcomeLayer::Optional, OutcomeLayer::Fallible)
    ) {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!("IR composed outcome call `{callee_name}` has an unsupported layer order"),
        )]);
    }

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;
    instructions.push(Instruction::CallComposedOutcome {
        destination,
        target,
        arguments,
        outer,
        inner,
        outer_mode,
        inner_mode,
    });
    Ok(instructions)
}

fn validate_destination(
    payload: &Type,
    destination: ComposedOutcomeDestination,
    callee_name: &str,
) -> Result<(), Vec<Diagnostic>> {
    let matches = matches!(
        (payload, destination),
        (Type::I32, ComposedOutcomeDestination::I32(_))
            | (Type::U8, ComposedOutcomeDestination::U8(_))
            | (Type::Usize, ComposedOutcomeDestination::Usize(_))
            | (Type::Borrow { .. }, ComposedOutcomeDestination::Borrow(_))
            | (Type::Bool, ComposedOutcomeDestination::Bool(_))
            | (Type::Str, ComposedOutcomeDestination::Str(_))
            | (Type::Slice { .. }, ComposedOutcomeDestination::Slice(_))
    );
    if matches {
        return Ok(());
    }
    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR composed outcome call destination does not match `{callee_name}` payload `{}`",
            describe_type(payload)
        ),
    )])
}
