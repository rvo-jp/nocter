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
    BoolLocation, CallTarget, I32Location, Instruction, ScalarArgument, SliceLocation, Type,
    U8Location, UsizeLocation,
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
            Type::Void | Type::Never | Type::Fallible(_) => {
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
            "IR v0 can only lower normal calls returning `str`, got function `{callee_name}` returning `{}`",
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
            Type::Void => "void!",
            Type::Never => "never!",
            Type::Fallible(_) => "fallible",
        },
    }
}
