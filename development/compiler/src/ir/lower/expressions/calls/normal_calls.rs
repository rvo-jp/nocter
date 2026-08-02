use super::*;

pub(in crate::ir::lower::expressions) fn lower_i32_normal_call(
    call: &CallExpr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_normal_call_return_type(&target, &callee_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;

    instructions.push(Instruction::CallI32 {
        destination,
        target,
        arguments,
    });
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_fallible_i32_normal_call(
    call: &CallExpr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if primitive_open_read_raw_call(call, context) {
        return lower_open_read_raw_primitive_call(
            call,
            destination,
            context,
            temporaries,
            failure_mode,
        );
    }

    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_fallible_i32_normal_call_return_type(&target, &callee_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;

    instructions.push(Instruction::CallFallibleI32 {
        destination,
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_usize_normal_call(
    call: &CallExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_usize_normal_call_return_type(&target, &callee_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;

    instructions.push(Instruction::CallUsize {
        destination,
        target,
        arguments,
    });
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_fallible_usize_normal_call(
    call: &CallExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if primitive_read_bytes_raw_call(call, context) {
        return lower_read_bytes_raw_primitive_call(
            call,
            destination,
            context,
            temporaries,
            failure_mode,
        );
    }

    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_fallible_usize_normal_call_return_type(&target, &callee_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;

    instructions.push(Instruction::CallFallibleUsize {
        destination,
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_borrow_normal_call(
    call: &CallExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };
    validate_borrow_normal_call_return_type(&target, &callee_name, context)?;
    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;
    instructions.push(Instruction::CallBorrow {
        destination,
        target,
        arguments,
    });
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_fallible_borrow_normal_call(
    call: &CallExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };
    validate_fallible_borrow_normal_call_return_type(&target, &callee_name, context)?;
    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;
    instructions.push(Instruction::CallFallibleBorrow {
        destination,
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_u8_normal_call(
    call: &CallExpr,
    destination: U8Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_u8_normal_call_return_type(&target, &callee_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;

    instructions.push(Instruction::CallU8 {
        destination,
        target,
        arguments,
    });
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_fallible_u8_normal_call(
    call: &CallExpr,
    destination: U8Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_fallible_u8_normal_call_return_type(&target, &callee_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;

    instructions.push(Instruction::CallFallibleU8 {
        destination,
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_bool_normal_call(
    call: &CallExpr,
    destination: BoolLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_bool_normal_call_return_type(&target, &callee_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;

    instructions.push(Instruction::CallBool {
        destination,
        target,
        arguments,
    });
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_fallible_bool_normal_call(
    call: &CallExpr,
    destination: BoolLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_fallible_bool_normal_call_return_type(&target, &callee_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;

    instructions.push(Instruction::CallFallibleBool {
        destination,
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_str_normal_call(
    call: &CallExpr,
    destination: StrLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_str_normal_call_return_type(&target, &callee_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;

    instructions.push(Instruction::CallStr {
        destination,
        target,
        arguments,
    });
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_fallible_str_normal_call(
    call: &CallExpr,
    destination: StrLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_fallible_str_normal_call_return_type(&target, &callee_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;

    instructions.push(Instruction::CallFallibleStr {
        destination,
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_slice_normal_call(
    call: &CallExpr,
    destination: SliceLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_slice_normal_call_return_type(&target, &callee_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;

    instructions.push(Instruction::CallSlice {
        destination,
        target,
        arguments,
    });
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_fallible_slice_normal_call(
    call: &CallExpr,
    destination: SliceLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_fallible_slice_normal_call_return_type(&target, &callee_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;

    instructions.push(Instruction::CallFallibleSlice {
        destination,
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_void_normal_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_void_normal_call_return_type(&target, &callee_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;

    instructions.push(Instruction::CallVoid { target, arguments });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_fallible_void_normal_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let primitive_instructions = if primitive_write_text_raw_call(call, context) {
        Some(lower_write_text_raw_primitive_call(
            call,
            context,
            temporaries,
        )?)
    } else if primitive_write_bytes_raw_call(call, context) {
        Some(lower_write_bytes_raw_primitive_call(
            call,
            context,
            temporaries,
        )?)
    } else {
        None
    };
    if let Some(mut instructions) = primitive_instructions {
        instructions.push(match failure_mode {
            FallibleFailureMode::Propagate => Instruction::PropagateFailure,
            FallibleFailureMode::Trap => Instruction::TrapOnFailure,
            FallibleFailureMode::PropagateWithCleanup { .. }
            | FallibleFailureMode::Handle { .. }
            | FallibleFailureMode::Recover { .. }
            | FallibleFailureMode::Catch { .. } => Instruction::CheckFailure { failure_mode },
        });
        return Ok(instructions);
    }

    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_non_tail_call_diagnostic());
    };
    validate_fallible_void_normal_call_return_type(&target, &callee_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, temporaries)?;

    instructions.push(Instruction::CallFallibleVoid {
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}
