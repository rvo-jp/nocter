use super::*;

pub(super) fn i32_destination_reserved_abi_words(destination: I32Location) -> usize {
    usize::from(matches!(destination, I32Location::Local(_)))
}

pub(super) fn u8_destination_reserved_abi_words(destination: U8Location) -> usize {
    usize::from(matches!(destination, U8Location::Local(_)))
}

pub(super) fn usize_destination_reserved_abi_words(destination: UsizeLocation) -> usize {
    usize::from(matches!(destination, UsizeLocation::Local(_)))
}

pub(super) fn bool_destination_reserved_abi_words(destination: BoolLocation) -> usize {
    usize::from(matches!(destination, BoolLocation::Local(_)))
}

pub(super) fn str_destination_reserved_abi_words(destination: StrLocation) -> usize {
    if matches!(destination, StrLocation::Local(_)) {
        2
    } else {
        0
    }
}

pub(super) fn slice_destination_reserved_abi_words(destination: SliceLocation) -> usize {
    if matches!(destination, SliceLocation::Local(_)) {
        2
    } else {
        0
    }
}

pub(super) fn lower_i32_fallible_expression_to_location(
    expression: &Expr,
    destination: I32Location,
    context: &LoweringContext,
    failure_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_stored_outcome_expression(
        expression,
        crate::ir::ComposedOutcomeDestination::I32(destination),
        context,
        failure_mode.clone(),
    )? {
        return Ok(instructions);
    }
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_fallible_i32_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )
        }
        Expr::Group(group) => lower_i32_fallible_expression_to_location(
            &group.expression,
            destination,
            context,
            failure_mode,
        ),
        _ => Err(unsupported_i32_expression_diagnostic()),
    }
}

pub(super) fn lower_u8_fallible_expression_to_location(
    expression: &Expr,
    destination: U8Location,
    context: &LoweringContext,
    failure_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_stored_outcome_expression(
        expression,
        crate::ir::ComposedOutcomeDestination::U8(destination),
        context,
        failure_mode.clone(),
    )? {
        return Ok(instructions);
    }
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_fallible_u8_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )
        }
        Expr::Group(group) => lower_u8_fallible_expression_to_location(
            &group.expression,
            destination,
            context,
            failure_mode,
        ),
        _ => Err(unsupported_u8_expression_diagnostic()),
    }
}

pub(super) fn lower_usize_fallible_expression_to_location(
    expression: &Expr,
    destination: UsizeLocation,
    context: &LoweringContext,
    failure_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_stored_outcome_expression(
        expression,
        crate::ir::ComposedOutcomeDestination::Usize(destination),
        context,
        failure_mode.clone(),
    )? {
        return Ok(instructions);
    }
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_fallible_usize_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )
        }
        Expr::Group(group) => lower_usize_fallible_expression_to_location(
            &group.expression,
            destination,
            context,
            failure_mode,
        ),
        _ => Err(unsupported_usize_expression_diagnostic()),
    }
}

pub(super) fn lower_str_fallible_expression_to_location(
    expression: &Expr,
    destination: StrLocation,
    context: &LoweringContext,
    failure_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_stored_outcome_expression(
        expression,
        crate::ir::ComposedOutcomeDestination::Str(destination),
        context,
        failure_mode.clone(),
    )? {
        return Ok(instructions);
    }
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_fallible_str_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )
        }
        Expr::Group(group) => lower_str_fallible_expression_to_location(
            &group.expression,
            destination,
            context,
            failure_mode,
        ),
        _ => Err(unsupported_str_expression_diagnostic()),
    }
}

pub(super) fn lower_slice_fallible_expression_to_location(
    expression: &Expr,
    destination: SliceLocation,
    context: &LoweringContext,
    failure_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_stored_outcome_expression(
        expression,
        crate::ir::ComposedOutcomeDestination::Slice(destination),
        context,
        failure_mode.clone(),
    )? {
        return Ok(instructions);
    }
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_fallible_slice_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )
        }
        Expr::Group(group) => lower_slice_fallible_expression_to_location(
            &group.expression,
            destination,
            context,
            failure_mode,
        ),
        _ => Err(unsupported_slice_expression_diagnostic()),
    }
}

pub(super) fn lower_bool_fallible_expression_to_location(
    expression: &Expr,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    failure_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_stored_outcome_expression(
        expression,
        crate::ir::ComposedOutcomeDestination::Bool(destination),
        context,
        failure_mode.clone(),
    )? {
        return Ok(instructions);
    }
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_fallible_bool_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )
        }
        Expr::Group(group) => lower_bool_fallible_expression_to_location(
            &group.expression,
            destination,
            context,
            diagnostic_code,
            failure_mode,
        ),
        _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    }
}
