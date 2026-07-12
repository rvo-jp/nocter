use super::super::context::LoweringContext;
use super::temporaries::TemporaryAllocator;
use super::{
    lower_bool_expression_to_value_with_temporaries, lower_i32_expression_to_value,
    lower_slice_expression_to_value, lower_str_expression_to_value, lower_u8_expression_to_value,
    lower_usize_expression_to_value, unsupported_non_tail_call_diagnostic,
};
use crate::ast::{CallExpr, Expr};
use crate::diagnostics::Diagnostic;
use crate::ir::StrLocation;
use crate::ir::{
    BoolLocation, BorrowArgument, BorrowSource, CallTarget, FallibleFailureMode, I32Location,
    Instruction, ScalarArgument, SliceLocation, Type, U8Location, UsizeLocation,
};

pub(super) fn lower_i32_normal_call(
    call: &CallExpr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    validate_normal_call_return_type(&target, &identifier.name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, temporaries)?;

    instructions.push(Instruction::CallI32 {
        destination,
        target,
        arguments,
    });
    Ok(instructions)
}

pub(super) fn lower_fallible_i32_normal_call(
    call: &CallExpr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    validate_fallible_i32_normal_call_return_type(&target, &identifier.name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, temporaries)?;

    instructions.push(Instruction::CallFallibleI32 {
        destination,
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}

pub(super) fn lower_usize_normal_call(
    call: &CallExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    validate_usize_normal_call_return_type(&target, &identifier.name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, temporaries)?;

    instructions.push(Instruction::CallUsize {
        destination,
        target,
        arguments,
    });
    Ok(instructions)
}

pub(super) fn lower_fallible_usize_normal_call(
    call: &CallExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    validate_fallible_usize_normal_call_return_type(&target, &identifier.name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, temporaries)?;

    instructions.push(Instruction::CallFallibleUsize {
        destination,
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}

pub(super) fn lower_u8_normal_call(
    call: &CallExpr,
    destination: U8Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    validate_u8_normal_call_return_type(&target, &identifier.name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, temporaries)?;

    instructions.push(Instruction::CallU8 {
        destination,
        target,
        arguments,
    });
    Ok(instructions)
}

pub(super) fn lower_fallible_u8_normal_call(
    call: &CallExpr,
    destination: U8Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    validate_fallible_u8_normal_call_return_type(&target, &identifier.name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, temporaries)?;

    instructions.push(Instruction::CallFallibleU8 {
        destination,
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}

pub(super) fn lower_bool_normal_call(
    call: &CallExpr,
    destination: BoolLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    validate_bool_normal_call_return_type(&target, &identifier.name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, temporaries)?;

    instructions.push(Instruction::CallBool {
        destination,
        target,
        arguments,
    });
    Ok(instructions)
}

pub(super) fn lower_fallible_bool_normal_call(
    call: &CallExpr,
    destination: BoolLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    validate_fallible_bool_normal_call_return_type(&target, &identifier.name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, temporaries)?;

    instructions.push(Instruction::CallFallibleBool {
        destination,
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}

pub(super) fn lower_str_normal_call(
    call: &CallExpr,
    destination: StrLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    validate_str_normal_call_return_type(&target, &identifier.name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, temporaries)?;

    instructions.push(Instruction::CallStr {
        destination,
        target,
        arguments,
    });
    Ok(instructions)
}

pub(super) fn lower_fallible_str_normal_call(
    call: &CallExpr,
    destination: StrLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    validate_fallible_str_normal_call_return_type(&target, &identifier.name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, temporaries)?;

    instructions.push(Instruction::CallFallibleStr {
        destination,
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}

pub(super) fn lower_slice_normal_call(
    call: &CallExpr,
    destination: SliceLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    validate_slice_normal_call_return_type(&target, &identifier.name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, temporaries)?;

    instructions.push(Instruction::CallSlice {
        destination,
        target,
        arguments,
    });
    Ok(instructions)
}

pub(super) fn lower_fallible_slice_normal_call(
    call: &CallExpr,
    destination: SliceLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    validate_fallible_slice_normal_call_return_type(&target, &identifier.name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, temporaries)?;

    instructions.push(Instruction::CallFallibleSlice {
        destination,
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}

pub(super) fn lower_void_normal_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    validate_void_normal_call_return_type(&target, &identifier.name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, temporaries)?;

    instructions.push(Instruction::CallVoid { target, arguments });
    Ok(instructions)
}

pub(super) fn lower_fallible_void_normal_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    if primitive_write_text_raw_call(call, context) {
        let mut instructions = lower_write_text_raw_primitive_call(call, context, temporaries)?;
        instructions.push(match failure_mode {
            FallibleFailureMode::Propagate => Instruction::PropagateFailure,
            FallibleFailureMode::Trap => Instruction::TrapOnFailure,
            FallibleFailureMode::Catch { .. } => Instruction::CheckFailure { failure_mode },
        });
        return Ok(instructions);
    }

    let target = context.call_target(call, &identifier.name);
    validate_fallible_void_normal_call_return_type(&target, &identifier.name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, temporaries)?;

    instructions.push(Instruction::CallFallibleVoid {
        target,
        arguments,
        failure_mode,
    });
    Ok(instructions)
}

pub(super) fn lower_direct_tail_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if primitive_trap_call(call, context) {
        return Ok(vec![Instruction::Trap]);
    }

    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower direct function calls in tail return position",
        )]);
    };

    let target = context.call_target(call, &identifier.name);
    validate_tail_call_return_type(&target, &identifier.name, context)?;

    let mut temporaries = TemporaryAllocator::new(context)?;
    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &identifier.name, context, &mut temporaries)?;

    if arguments
        .iter()
        .any(|argument| matches!(argument, ScalarArgument::Borrow(_)))
    {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 cannot lower tail call to function `{}` with borrow arguments",
                identifier.name
            ),
        )]);
    }

    instructions.push(Instruction::TailCall { target, arguments });
    Ok(instructions)
}

pub(super) fn lower_call_arguments(
    call: &CallExpr,
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, Vec<ScalarArgument>), Vec<Diagnostic>> {
    let Some(parameter_types) = context.call_parameter_types(target) else {
        return lower_legacy_i32_call_arguments(call, context, temporaries);
    };

    if parameter_types.len() != call.arguments.len() {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 cannot lower call to function `{callee_name}` with {} arguments against {} parameters",
                call.arguments.len(),
                parameter_types.len(),
            ),
        )]);
    }

    let mut instructions = Vec::new();
    let mut arguments = Vec::new();
    for (argument, parameter_type) in call.arguments.iter().zip(parameter_types) {
        match parameter_type {
            Type::I32 => {
                let argument = lower_i32_expression_to_value(argument, context, temporaries)?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::I32(argument.value));
            }
            Type::U8 => {
                let argument = lower_u8_expression_to_value(argument, context, temporaries)?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::U8(argument.value));
            }
            Type::Usize => {
                let argument = lower_usize_expression_to_value(argument, context, temporaries)?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::Usize(argument.value));
            }
            Type::Bool => {
                let argument = lower_bool_expression_to_value_with_temporaries(
                    argument,
                    context,
                    "E8006",
                    temporaries,
                )?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::Bool(argument.value));
            }
            Type::Str => {
                let argument = lower_str_expression_to_value(argument, context, temporaries)?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::Str(argument.value));
            }
            Type::Slice { .. } => {
                let argument = lower_slice_expression_to_value(argument, context, temporaries)?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::Slice(argument.value));
            }
            Type::Borrow { .. } => {
                arguments.push(ScalarArgument::Borrow(lower_borrow_argument(
                    argument,
                    parameter_type,
                    callee_name,
                    context,
                )?));
            }
            Type::Aggregate { .. } | Type::Void | Type::Never | Type::Fallible(_) => {
                return Err(vec![Diagnostic::error(
                    "E8006",
                    format!(
                        "IR v0 can only lower scalar call arguments for function `{callee_name}`, got `{}`",
                        describe_type(parameter_type),
                    ),
                )]);
            }
        }
    }

    Ok((instructions, arguments))
}

fn lower_borrow_argument(
    argument: &Expr,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<BorrowArgument, Vec<Diagnostic>> {
    let Type::Borrow {
        is_readwrite,
        inner,
    } = parameter_type
    else {
        unreachable!("borrow argument lowering requires a borrow parameter type");
    };

    let Expr::Borrow(borrow) = unwrap_group(argument) else {
        return Err(unsupported_borrow_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };

    if borrow.is_readwrite != *is_readwrite {
        return Err(unsupported_borrow_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }

    let Expr::Identifier(identifier) = unwrap_group(&borrow.expression) else {
        return Err(unsupported_borrow_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };

    let source = match inner.as_ref() {
        Type::I32 => match context.i32_location(&identifier.name) {
            Some(I32Location::Local(index)) => BorrowSource::I32(I32Location::Local(index)),
            _ => {
                return Err(unsupported_borrow_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            }
        },
        Type::U8 => match context.u8_location(&identifier.name) {
            Some(U8Location::Local(index)) => BorrowSource::U8(U8Location::Local(index)),
            _ => {
                return Err(unsupported_borrow_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            }
        },
        Type::Usize => match context.usize_location(&identifier.name) {
            Some(UsizeLocation::Local(index)) => BorrowSource::Usize(UsizeLocation::Local(index)),
            _ => {
                return Err(unsupported_borrow_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            }
        },
        Type::Bool => match context.bool_location(&identifier.name) {
            Some(BoolLocation::Local(index)) => BorrowSource::Bool(BoolLocation::Local(index)),
            _ => {
                return Err(unsupported_borrow_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            }
        },
        Type::Aggregate {
            layout: expected_layout,
        } => match context.aggregate_slot(&identifier.name) {
            Some((slot_index, layout)) if layout == *expected_layout => {
                BorrowSource::AggregateSlot(slot_index)
            }
            _ => {
                return Err(unsupported_borrow_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            }
        },
        _ => {
            return Err(unsupported_borrow_argument_diagnostic(
                callee_name,
                parameter_type,
            ));
        }
    };

    Ok(BorrowArgument { source })
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

fn unsupported_borrow_argument_diagnostic(
    callee_name: &str,
    parameter_type: &Type,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower `{}` arguments from scalar local bindings for function `{callee_name}`",
            describe_type(parameter_type),
        ),
    )]
}

pub(super) fn primitive_trap_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("trap" | "unreachable")
    )
}

fn lower_legacy_i32_call_arguments(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, Vec<ScalarArgument>), Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    let mut arguments = Vec::new();
    for argument in &call.arguments {
        let argument = lower_i32_expression_to_value(argument, context, temporaries)?;
        instructions.extend(argument.instructions);
        arguments.push(ScalarArgument::I32(argument.value));
    }

    Ok((instructions, arguments))
}

fn validate_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Ok(());
    };

    if callee_return_type == &Type::I32 {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower normal calls returning `i32`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_usize_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Ok(());
    };

    if callee_return_type == &Type::Usize {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower normal calls returning `usize`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_u8_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Ok(());
    };

    if callee_return_type == &Type::U8 {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower normal calls returning `u8`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_bool_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Ok(());
    };

    if callee_return_type == &Type::Bool {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower normal calls returning `bool`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_str_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Ok(());
    };

    if callee_return_type == &Type::Str {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower normal calls returning `&str`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_slice_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Ok(());
    };

    if matches!(callee_return_type, Type::Slice { .. }) {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower normal calls returning a slice, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_void_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower expression statements calling functions with known `void` return type, got function `{callee_name}`"
            ),
        )]);
    };

    if callee_return_type == &Type::Void {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower expression statements calling functions returning `void`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_fallible_void_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower propagated call statements with known `void!` return type, got function `{callee_name}`"
            ),
        )]);
    };

    if matches!(callee_return_type, Type::Fallible(success) if success.as_ref() == &Type::Void) {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower propagated call statements returning `void!`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_fallible_i32_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower propagated calls with known `i32!` return type, got function `{callee_name}`"
            ),
        )]);
    };

    if matches!(callee_return_type, Type::Fallible(success) if success.as_ref() == &Type::I32) {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower propagated calls returning `i32!`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_fallible_usize_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower propagated calls with known `usize!` return type, got function `{callee_name}`"
            ),
        )]);
    };

    if matches!(callee_return_type, Type::Fallible(success) if success.as_ref() == &Type::Usize) {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower propagated calls returning `usize!`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_fallible_u8_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower propagated calls with known `u8!` return type, got function `{callee_name}`"
            ),
        )]);
    };

    if matches!(callee_return_type, Type::Fallible(success) if success.as_ref() == &Type::U8) {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower propagated calls returning `u8!`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_fallible_bool_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower propagated calls with known `bool!` return type, got function `{callee_name}`"
            ),
        )]);
    };

    if matches!(callee_return_type, Type::Fallible(success) if success.as_ref() == &Type::Bool) {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower propagated calls returning `bool!`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_fallible_str_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower propagated calls with known `&str!` return type, got function `{callee_name}`"
            ),
        )]);
    };

    if matches!(callee_return_type, Type::Fallible(success) if success.as_ref() == &Type::Str) {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower propagated calls returning `&str!`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_fallible_slice_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower propagated calls with known slice fallible return type, got function `{callee_name}`"
            ),
        )]);
    };

    if matches!(callee_return_type, Type::Fallible(success) if matches!(success.as_ref(), Type::Slice { .. }))
    {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower propagated calls returning a slice fallible type, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_tail_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Ok(());
    };

    if callee_return_type == &Type::Never || callee_return_type == context.return_type() {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 cannot lower tail call from function `{}` returning `{}` to function `{callee_name}` returning `{}`",
            context.function_name(),
            describe_type(context.return_type()),
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn primitive_write_text_raw_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("write_text_raw")
    )
}

fn lower_write_text_raw_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 2 {
        return Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower primitive `write_text_raw` with arguments `(i32, &str)`",
        )]);
    };

    let fd = lower_i32_expression_to_value(&call.arguments[0], context, temporaries)?;
    let text = lower_str_expression_to_value(&call.arguments[1], context, temporaries)?;
    let mut instructions = fd.instructions;
    instructions.extend(text.instructions);
    instructions.push(Instruction::WriteStr {
        fd: fd.value,
        text: text.value,
    });
    Ok(instructions)
}

fn describe_type(ty: &Type) -> &'static str {
    match ty {
        Type::I32 => "i32",
        Type::U8 => "u8",
        Type::Usize => "usize",
        Type::Bool => "bool",
        Type::Str => "&str",
        Type::Slice {
            is_readwrite: false,
        } => "&[u8]",
        Type::Slice { is_readwrite: true } => "&+[u8]",
        Type::Aggregate { .. } => "aggregate",
        Type::Borrow {
            is_readwrite: false,
            inner,
        } => match inner.as_ref() {
            Type::I32 => "&i32",
            Type::U8 => "&u8",
            Type::Usize => "&usize",
            Type::Bool => "&bool",
            Type::Aggregate { .. } => "&aggregate",
            _ => "borrow",
        },
        Type::Borrow {
            is_readwrite: true,
            inner,
        } => match inner.as_ref() {
            Type::I32 => "&+i32",
            Type::U8 => "&+u8",
            Type::Usize => "&+usize",
            Type::Bool => "&+bool",
            Type::Aggregate { .. } => "&+aggregate",
            _ => "borrow",
        },
        Type::Void => "void",
        Type::Never => "never",
        Type::Fallible(success) => match success.as_ref() {
            Type::I32 => "i32!",
            Type::U8 => "u8!",
            Type::Usize => "usize!",
            Type::Bool => "bool!",
            Type::Str => "&str!",
            Type::Slice {
                is_readwrite: false,
            } => "&[u8]!",
            Type::Slice { is_readwrite: true } => "&+[u8]!",
            Type::Aggregate { .. } => "aggregate!",
            Type::Borrow {
                is_readwrite: false,
                inner,
            } => match inner.as_ref() {
                Type::I32 => "&i32!",
                Type::U8 => "&u8!",
                Type::Usize => "&usize!",
                Type::Bool => "&bool!",
                Type::Aggregate { .. } => "&aggregate!",
                _ => "borrow!",
            },
            Type::Borrow {
                is_readwrite: true,
                inner,
            } => match inner.as_ref() {
                Type::I32 => "&+i32!",
                Type::U8 => "&+u8!",
                Type::Usize => "&+usize!",
                Type::Bool => "&+bool!",
                Type::Aggregate { .. } => "&+aggregate!",
                _ => "borrow!",
            },
            Type::Void => "void!",
            Type::Never => "never!",
            Type::Fallible(_) => "fallible",
        },
    }
}
