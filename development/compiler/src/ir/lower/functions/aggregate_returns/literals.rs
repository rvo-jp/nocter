use super::*;

pub(in crate::ir::lower::functions) fn lower_aggregate_struct_literal_return_to_location(
    literal: &StructLiteralExpr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);

    if let Some(return_type_expr) = aggregate_return_value_type_expr(context)
        && let Some(value) = context.abi_value_for_type_expr(return_type_expr)
        && value.layout == expected_layout
        && let AbiType::Struct(fields) = &value.ty
        && let Some(drop_kind @ (AggregateDrop::Direct(_) | AggregateDrop::Struct(_))) =
            context.aggregate_drop_for_type_expr(return_type_expr)
    {
        return lower_tracked_aggregate_struct_literal_return(
            literal,
            fields,
            expected_layout,
            destination,
            function_name,
            resolved,
            drop_kind,
            context,
        );
    }

    let subject = format!("returns from function `{function_name}`");
    let aggregate_slot_mark = context.aggregate_slot_mark();
    let lowered_direct = lower_aggregate_struct_literal_to_location(
        literal,
        expected_layout,
        destination,
        "E8007",
        &subject,
        resolved,
        context,
    );
    Ok(match lowered_direct {
        Ok(instructions) => instructions,
        Err(error) if matches!(destination, AggregateLocation::DirectReturn) => {
            context.restore_aggregate_slot_mark(aggregate_slot_mark);
            lower_direct_aggregate_struct_literal_return_through_slot(
                literal,
                expected_layout,
                &subject,
                resolved,
                context,
            )
            .map_err(|_| error)?
        }
        Err(error) => return Err(error),
    })
}

fn lower_tracked_aggregate_struct_literal_return(
    literal: &StructLiteralExpr,
    fields: &[crate::abi::AbiField],
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    drop_kind: AggregateDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(expected_layout) {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let mut initialization_context = context.clone();
    let slot_index = initialization_context.reserve_aggregate_slot_index();
    let progress = StructInitializationProgress::new(
        fields,
        literal,
        &drop_kind,
        &mut initialization_context,
    )?;
    if !initialization_context.register_temporary_struct_fields_drop(
        slot_index,
        expected_layout,
        drop_kind,
        progress.drop_states(),
    ) {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let subject = format!("returns from function `{function_name}`");
    let mut temporaries = TemporaryAllocator::new(&initialization_context)?;
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(progress.initialize());
    instructions.extend(
        lower_aggregate_struct_literal_to_location_at_offset_with_temporaries(
            literal,
            expected_layout,
            AggregateLocation::Slot(slot_index),
            0,
            "E8007",
            &subject,
            resolved,
            &initialization_context,
            &mut temporaries,
            Some(&progress),
        )?,
    );
    instructions.push(Instruction::CopyAggregate {
        destination,
        source: AggregateLocation::Slot(slot_index),
        layout: expected_layout,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::functions) fn lower_aggregate_array_literal_return_to_location(
    literal: &ArrayLiteralExpr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
    let Some(value) = fixed_array_return_abi_value(resolved, context) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if !matches!(&value.ty, AbiType::Array { .. }) || value.layout != expected_layout {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    if array_literal_requires_runtime_progress(literal)
        && let Some(return_type_expr) = aggregate_return_value_type_expr(context)
        && let Some(drop_kind @ AggregateDrop::Array(_)) =
            context.aggregate_drop_for_type_expr(return_type_expr)
    {
        return lower_tracked_aggregate_array_literal_return(
            literal,
            &value.ty,
            expected_layout,
            destination,
            function_name,
            resolved,
            drop_kind,
            context,
        );
    }

    let subject = format!("returns from function `{function_name}`");
    let aggregate_slot_mark = context.aggregate_slot_mark();
    let lowered_direct = lower_aggregate_array_literal_to_location(
        literal,
        &value.ty,
        expected_layout,
        destination,
        "E8007",
        &subject,
        resolved,
        context,
    );
    Ok(match lowered_direct {
        Ok(instructions) => instructions,
        Err(error) if matches!(destination, AggregateLocation::DirectReturn) => {
            context.restore_aggregate_slot_mark(aggregate_slot_mark);
            lower_direct_aggregate_array_literal_return_through_slot(
                literal,
                &value.ty,
                expected_layout,
                &subject,
                resolved,
                context,
            )
            .map_err(|_| error)?
        }
        Err(error) => return Err(error),
    })
}

fn aggregate_return_value_type_expr<'context>(
    context: &'context LoweringContext<'_>,
) -> Option<&'context TypeExpr> {
    let mut ty = context.function_return_type_expr()?;
    loop {
        match ty {
            TypeExpr::Fallible(fallible) => ty = &fallible.success,
            TypeExpr::Optional(optional) => ty = &optional.inner,
            _ => return Some(ty),
        }
    }
}

fn lower_tracked_aggregate_array_literal_return(
    literal: &ArrayLiteralExpr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    drop_kind: AggregateDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(expected_layout) {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let mut initialization_context = context.clone();
    let slot_index = initialization_context.reserve_aggregate_slot_index();
    let AbiType::Array { element, .. } = expected_type else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    let initialized = initialization_context.reserve_drop_state_usize_local()?;
    let progress = ArrayInitializationProgress::with_allocator(
        literal,
        element,
        &drop_kind,
        initialized,
        &mut initialization_context,
    )?;
    if !initialization_context.register_temporary_array_prefix_drop(
        slot_index,
        expected_layout,
        drop_kind,
        progress.location(),
        progress.element_states(),
    ) {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let subject = format!("returns from function `{function_name}`");
    let mut temporaries = TemporaryAllocator::new(&initialization_context)?;
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(progress.initialize());
    instructions.extend(lower_aggregate_array_literal_to_location_with_progress(
        literal,
        expected_type,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        0,
        "E8007",
        &subject,
        resolved,
        &initialization_context,
        &mut temporaries,
        Some(&progress),
    )?);
    instructions.push(Instruction::CopyAggregate {
        destination,
        source: AggregateLocation::Slot(slot_index),
        layout: expected_layout,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::functions) fn fixed_array_return_abi_value(
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Option<AbiValue> {
    let mut ty = context.function_return_type_expr()?;
    loop {
        match ty {
            TypeExpr::Fallible(fallible) => ty = &fallible.success,
            TypeExpr::Optional(optional) => ty = &optional.inner,
            _ => {
                return abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
                    context.resolved_source(source)
                })
                .ok();
            }
        }
    }
}

pub(in crate::ir::lower::functions) fn payload_enum_return_abi_value(
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Option<AbiValue> {
    let mut ty = context.function_return_type_expr()?;
    loop {
        match ty {
            TypeExpr::Fallible(fallible) => ty = &fallible.success,
            TypeExpr::Optional(optional) => ty = &optional.inner,
            _ => {
                let value = abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
                    context.resolved_source(source)
                })
                .ok()?;
                return matches!(value.ty, AbiType::Enum(_)).then_some(value);
            }
        }
    }
}

pub(in crate::ir::lower::functions) fn lower_payload_enum_constructor_return_to_location(
    expression: &Expr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
    let Some(value) = payload_enum_return_abi_value(resolved, context) else {
        return Ok(None);
    };
    if value.layout != expected_layout {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    lower_payload_enum_constructor_value_to_location(
        expression,
        &value,
        expected_layout,
        destination,
        function_name,
        resolved,
        context,
    )
}

pub(in crate::ir::lower::functions) fn lower_payload_enum_constructor_value_to_location(
    expression: &Expr,
    value: &AbiValue,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if matches!(&value.ty, AbiType::Enum(_))
        && let Some(return_type_expr) = aggregate_return_value_type_expr(context)
        && let Some(drop_kind @ AggregateDrop::PayloadEnum(_)) =
            context.aggregate_drop_for_type_expr(return_type_expr)
    {
        return lower_tracked_payload_enum_constructor_return(
            expression,
            &value.ty,
            expected_layout,
            destination,
            function_name,
            resolved,
            drop_kind,
            context,
        )
        .map(Some);
    }

    let subject = format!("returns from function `{function_name}`");
    let aggregate_slot_mark = context.aggregate_slot_mark();
    let lowered_direct = lower_payload_enum_constructor_to_location(
        expression,
        &value.ty,
        expected_layout,
        destination,
        "E8007",
        &subject,
        resolved,
        context,
    );
    let instructions = match lowered_direct {
        Ok(Some(instructions)) => instructions,
        Ok(None) => return Ok(None),
        Err(error) if matches!(destination, AggregateLocation::DirectReturn) => {
            context.restore_aggregate_slot_mark(aggregate_slot_mark);
            lower_direct_payload_enum_constructor_return_through_slot(
                expression,
                &value.ty,
                expected_layout,
                &subject,
                resolved,
                context,
            )
            .map_err(|_| error)?
        }
        Err(error) => return Err(error),
    };
    Ok(Some(instructions))
}

#[allow(clippy::too_many_arguments)]
fn lower_tracked_payload_enum_constructor_return(
    expression: &Expr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    drop_kind: AggregateDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(expected_layout) {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }
    let AbiType::Enum(enum_) = expected_type else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };

    let mut initialization_context = context.clone();
    let slot_index = initialization_context.reserve_aggregate_slot_index();
    let progress = PayloadInitializationProgress::with_allocator(
        expression,
        enum_,
        &drop_kind,
        &mut initialization_context,
    )?;
    if !initialization_context.register_temporary_payload_fields_drop(
        slot_index,
        expected_layout,
        drop_kind,
        progress.tag(),
        progress.drop_states(),
    ) {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let subject = format!("returns from function `{function_name}`");
    let mut temporaries = TemporaryAllocator::new(&initialization_context)?;
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(progress.initialize());
    let Some(mut constructor) = lower_payload_enum_constructor_to_location_with_progress(
        expression,
        expected_type,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        "E8007",
        &subject,
        resolved,
        &initialization_context,
        &mut temporaries,
        Some(&progress),
    )?
    else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    instructions.append(&mut constructor);
    instructions.push(Instruction::CopyAggregate {
        destination,
        source: AggregateLocation::Slot(slot_index),
        layout: expected_layout,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::functions) fn lower_direct_payload_enum_constructor_return_through_slot(
    expression: &Expr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(expected_layout) {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    let Some(mut constructor_instructions) = lower_payload_enum_constructor_to_location(
        expression,
        expected_type,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        "E8007",
        subject,
        resolved,
        context,
    )?
    else {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    };
    instructions.append(&mut constructor_instructions);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(slot_index),
        layout: expected_layout,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::functions) fn lower_direct_aggregate_array_literal_return_through_slot(
    literal: &ArrayLiteralExpr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(expected_layout) {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(lower_aggregate_array_literal_to_location(
        literal,
        expected_type,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        "E8007",
        subject,
        resolved,
        context,
    )?);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(slot_index),
        layout: expected_layout,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::functions) fn lower_direct_aggregate_struct_literal_return_through_slot(
    literal: &StructLiteralExpr,
    expected_layout: crate::abi::ValueLayout,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(expected_layout) {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(lower_aggregate_struct_literal_to_location_with_temporaries(
        literal,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        "E8007",
        subject,
        resolved,
        context,
        &mut temporaries,
    )?);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(slot_index),
        layout: expected_layout,
    });
    Ok(instructions)
}
