use super::*;

pub(super) fn direct_optional_otherwise_call<'a>(
    value: &'a Expr,
    context: &LoweringContext,
) -> Result<Option<(&'a CallExpr, &'a Block)>, Vec<Diagnostic>> {
    let Expr::Otherwise(otherwise) = unwrap_group(value) else {
        return Ok(None);
    };
    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return Ok(None);
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(return_type) = context.call_return_type_expr(call) else {
        return Ok(None);
    };
    if !return_type_expr_is_top_level_optional(&return_type, resolved) {
        return Ok(None);
    }
    Ok(Some((call, &otherwise.fallback)))
}

pub(super) fn lower_i32_otherwise_aggregate_field_assignment(
    value: &Expr,
    destination: AggregateLocation,
    offset: u32,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let temporary = context.next_i32_local_location()?;
    let expression_context = context.with_reserved_local_abi_words(1);
    let Some(mut instructions) =
        lower_i32_optional_otherwise_to_location(value, temporary, &expression_context)?
    else {
        return Ok(None);
    };
    instructions.push(Instruction::StoreAggregateI32 {
        destination,
        offset,
        value: I32Value::Location(temporary),
    });
    Ok(Some(instructions))
}

pub(super) fn lower_u8_otherwise_aggregate_field_assignment(
    value: &Expr,
    destination: AggregateLocation,
    offset: u32,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let temporary = context.next_u8_local_location()?;
    let expression_context = context.with_reserved_local_abi_words(1);
    let Some(mut instructions) =
        lower_u8_optional_otherwise_to_location(value, temporary, &expression_context)?
    else {
        return Ok(None);
    };
    instructions.push(Instruction::StoreAggregateU8 {
        destination,
        offset,
        value: U8Value::Location(temporary),
    });
    Ok(Some(instructions))
}

pub(super) fn lower_usize_otherwise_aggregate_field_assignment(
    value: &Expr,
    destination: AggregateLocation,
    offset: u32,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let temporary = context.next_usize_local_location()?;
    let expression_context = context.with_reserved_local_abi_words(1);
    let Some(mut instructions) =
        lower_usize_optional_otherwise_to_location(value, temporary, &expression_context)?
    else {
        return Ok(None);
    };
    instructions.push(Instruction::StoreAggregateUsize {
        destination,
        offset,
        value: UsizeValue::Location(temporary),
    });
    Ok(Some(instructions))
}

pub(super) fn lower_bool_otherwise_aggregate_field_assignment(
    value: &Expr,
    destination: AggregateLocation,
    offset: u32,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let temporary = context.next_bool_local_location()?;
    let expression_context = context.with_reserved_local_abi_words(1);
    let Some(mut instructions) =
        lower_bool_optional_otherwise_to_location(value, temporary, &expression_context)?
    else {
        return Ok(None);
    };
    instructions.push(Instruction::StoreAggregateBool {
        destination,
        offset,
        value: BoolValue::Location(temporary),
    });
    Ok(Some(instructions))
}

pub(super) fn lower_str_otherwise_aggregate_field_assignment(
    value: &Expr,
    destination: AggregateLocation,
    offset: u32,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let temporary = context.next_str_local_location()?;
    let expression_context = context.with_reserved_local_abi_words(2);
    let Some(mut instructions) =
        lower_str_optional_otherwise_to_location(value, temporary, &expression_context)?
    else {
        return Ok(None);
    };
    let mut temporaries = TemporaryAllocator::new(&expression_context)?;
    push_store_str_view_to_aggregate_field(
        &mut instructions,
        destination,
        offset,
        StrValue::Location(temporary),
        &mut temporaries,
        unsupported_assignment_diagnostic,
    )?;
    Ok(Some(instructions))
}

pub(super) fn lower_slice_otherwise_aggregate_field_assignment(
    value: &Expr,
    destination: AggregateLocation,
    offset: u32,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let temporary = context.next_slice_local_location()?;
    let expression_context = context.with_reserved_local_abi_words(2);
    let Some(mut instructions) =
        lower_slice_optional_otherwise_to_location(value, temporary, &expression_context)?
    else {
        return Ok(None);
    };
    let mut temporaries = TemporaryAllocator::new(&expression_context)?;
    push_store_slice_view_to_aggregate_field(
        &mut instructions,
        destination,
        offset,
        SliceValue::Location(temporary),
        &mut temporaries,
        unsupported_assignment_diagnostic,
    )?;
    Ok(Some(instructions))
}
