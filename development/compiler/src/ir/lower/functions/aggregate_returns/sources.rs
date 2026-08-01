use super::*;

pub(in crate::ir::lower::functions) fn lower_aggregate_local_return_to_location(
    name: &str,
    value_use: AggregateValueUse,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
    let Some(local) = context.aggregate_local(name) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if local.layout != expected_layout
        || (value_use == AggregateValueUse::ImplicitCopy && !local.is_copy)
    {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    Ok(vec![Instruction::CopyAggregate {
        destination,
        source: AggregateLocation::Slot(local.slot_index),
        layout: local.layout,
    }])
}

pub(in crate::ir::lower::functions) fn lower_aggregate_member_return_to_location(
    expression: &Expr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let access = lower_aggregate_member_field_access(expression, context, &mut temporaries)?
        .ok_or_else(|| unsupported_aggregate_return_diagnostic(function_name))?;
    let source = access.source;
    let source_offset = access.offset;
    let is_copy = access.is_copy;
    let Some(layout) = access.kind.copy_aggregate_layout() else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if layout != expected_layout || !is_copy || !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let mut instructions = access.instructions;
    instructions.push(Instruction::CopyAggregateRange {
        destination,
        destination_offset: 0,
        source,
        source_offset,
        layout,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::functions) fn lower_aggregate_fallible_call_return_to_location(
    call: &crate::ast::CallExpr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if success_type.as_ref() != return_type {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }
    validate_aggregate_call_success_return_passing(&target, return_type, function_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    let (layout, _) = aggregate_return_layout_and_destination(return_type);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        return_type,
        destination,
        target,
        arguments,
        layout,
        failure_mode,
    );
    Ok(instructions)
}

pub(in crate::ir::lower::functions) fn lower_aggregate_call_return_to_location(
    call: &crate::ast::CallExpr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if primitive_take_value_at_ptr_call(call, context) {
        let (layout, _) = aggregate_return_layout_and_destination(return_type);
        let mut temporaries = TemporaryAllocator::new(context)?;
        return lower_take_value_at_ptr_primitive_call(
            call,
            PointerTakeDestination::Aggregate {
                location: destination,
                layout,
            },
            context,
            &mut temporaries,
        );
    }
    if macos_syscall_primitive_call(call, context)
        && let Some(layout) = aggregate_call_return_layout_from_resolved(call, context)
    {
        let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
        if layout != expected_layout {
            return Err(unsupported_aggregate_return_diagnostic(function_name));
        }
        let mut temporaries = TemporaryAllocator::new(context)?;
        let Some(instructions) = lower_macos_syscall_primitive_call_to_location(
            call,
            destination,
            expected_layout,
            context,
            &mut temporaries,
        )?
        else {
            return Err(unsupported_aggregate_return_diagnostic(function_name));
        };
        return Ok(instructions);
    }

    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    let Some(callee_return_type) = context.call_return_type(&target) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if callee_return_type != return_type {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }
    validate_aggregate_call_success_return_passing(&target, return_type, function_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    let (layout, _) = aggregate_return_layout_and_destination(return_type);
    push_aggregate_call_instruction(
        &mut instructions,
        return_type,
        destination,
        target,
        arguments,
        layout,
    );
    Ok(instructions)
}
