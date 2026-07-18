use super::super::aggregates::{
    aggregate_call_instruction, aggregate_type_layout,
    lower_aggregate_struct_literal_to_location_with_temporaries, push_aggregate_call_instruction,
    push_fallible_aggregate_call_instruction, supported_aggregate_copy_layout,
};
use super::super::context::{AggregateFieldKind, LoweringContext};
use super::super::functions::propagating_failure_mode;
use super::temporaries::TemporaryAllocator;
use super::{
    lower_aggregate_member_field_access, lower_bool_expression_to_value_with_temporaries,
    lower_catch_failure_mode, lower_i32_expression_to_value, lower_slice_expression_to_value,
    lower_str_expression_to_value, lower_u8_expression_to_value, lower_usize_expression_to_value,
    unsupported_non_tail_call_diagnostic,
};
use crate::abi::{ARGUMENT_REGISTER_COUNT, ValueLayout, abi_value_from_type_expr};
use crate::ast::{CallExpr, Expr};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateArgument, AggregateArgumentSource, AggregateLocation, BoolLocation, BorrowArgument,
    BorrowSource, CallTarget, DirectAggregateArgument, FallibleFailureMode, I32Location,
    Instruction, ScalarArgument, SliceLocation, SliceValue, StrLocation, Type, U8Location,
    UsizeLocation, UsizeValue,
};

pub(super) fn lower_i32_normal_call(
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

pub(super) fn lower_usize_normal_call(
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

pub(super) fn lower_u8_normal_call(
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

pub(super) fn lower_bool_normal_call(
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

pub(super) fn lower_str_normal_call(
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

pub(super) fn lower_slice_normal_call(
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

pub(super) fn lower_void_normal_call(
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

pub(super) fn lower_fallible_void_normal_call(
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

pub(super) fn lower_direct_tail_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if primitive_trap_call(call, context) {
        return Ok(vec![Instruction::Trap]);
    }

    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower direct function calls in tail return position",
        )]);
    };

    validate_tail_call_return_type(&target, &callee_name, context)?;

    let mut temporaries = TemporaryAllocator::new(context)?;
    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, &mut temporaries)?;

    if fallible_success_tail_call_requires_normal_call(&target, context)
        || arguments
            .iter()
            .any(tail_call_argument_requires_current_frame)
        || call_arguments_require_stack(&arguments, &callee_name)?
    {
        let Some(return_type) = context.call_return_type(&target).cloned() else {
            return Err(unsupported_non_tail_return_call_diagnostic(&callee_name));
        };
        instructions.push(lower_non_tail_return_call_instruction(
            return_type,
            target,
            arguments,
            &callee_name,
        )?);
        instructions.push(Instruction::Return);
        return Ok(instructions);
    }

    instructions.push(Instruction::TailCall { target, arguments });
    Ok(instructions)
}

fn tail_call_argument_requires_current_frame(argument: &ScalarArgument) -> bool {
    matches!(argument, ScalarArgument::Borrow(_)) || is_tail_call_stack_pointer_argument(argument)
}

fn fallible_success_tail_call_requires_normal_call(
    target: &CallTarget,
    context: &LoweringContext,
) -> bool {
    if !matches!(context.function_return_type(), Type::Fallible(_)) {
        return false;
    }
    matches!(context.call_return_type(target), Some(return_type) if return_type == context.return_type())
}

fn lower_non_tail_return_call_instruction(
    return_type: Type,
    target: CallTarget,
    arguments: Vec<ScalarArgument>,
    callee_name: &str,
) -> Result<Instruction, Vec<Diagnostic>> {
    match &return_type {
        Type::I32 => Ok(Instruction::CallI32 {
            destination: I32Location::Return,
            target,
            arguments,
        }),
        Type::U8 => Ok(Instruction::CallU8 {
            destination: U8Location::Return,
            target,
            arguments,
        }),
        Type::Usize => Ok(Instruction::CallUsize {
            destination: UsizeLocation::Return,
            target,
            arguments,
        }),
        Type::Bool => Ok(Instruction::CallBool {
            destination: BoolLocation::Return,
            target,
            arguments,
        }),
        Type::Str => Ok(Instruction::CallStr {
            destination: StrLocation::Return,
            target,
            arguments,
        }),
        Type::Slice { .. } => Ok(Instruction::CallSlice {
            destination: SliceLocation::Return,
            target,
            arguments,
        }),
        Type::Aggregate { layout } => Ok(aggregate_call_instruction(
            &return_type,
            AggregateLocation::Return,
            target,
            arguments,
            *layout,
        )),
        Type::DirectAggregate { layout, .. } => Ok(aggregate_call_instruction(
            &return_type,
            AggregateLocation::DirectReturn,
            target,
            arguments,
            *layout,
        )),
        Type::Never | Type::Void | Type::Fallible(_) | Type::Borrow { .. } => {
            Err(unsupported_non_tail_return_call_diagnostic(callee_name))
        }
    }
}

fn unsupported_non_tail_return_call_diagnostic(callee_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 cannot lower return call to function `{callee_name}` without tail-call support for this return type"
        ),
    )]
}

pub(super) fn lower_call_arguments(
    call: &CallExpr,
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, Vec<ScalarArgument>), Vec<Diagnostic>> {
    let Some(parameter_types) = context.call_parameter_types(target) else {
        return lower_legacy_i32_call_arguments(call, callee_name, context, temporaries);
    };

    let method_receiver = context.method_call_receiver(call);
    let argument_count = call.arguments.len() + usize::from(method_receiver.is_some());
    if parameter_types.len() != argument_count {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 cannot lower call to function `{callee_name}` with {} arguments against {} parameters",
                argument_count,
                parameter_types.len(),
            ),
        )]);
    }

    let mut instructions = Vec::new();
    let mut arguments = Vec::new();
    let call_arguments = method_receiver
        .into_iter()
        .map(|receiver| (receiver, true))
        .chain(call.arguments.iter().map(|argument| (argument, false)));
    for ((argument, is_method_receiver), parameter_type) in call_arguments.zip(parameter_types) {
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
                let argument = if is_method_receiver {
                    lower_implicit_receiver_borrow_argument(
                        argument,
                        parameter_type,
                        callee_name,
                        context,
                    )?
                } else {
                    lower_borrow_argument(argument, parameter_type, callee_name, context)?
                };
                arguments.push(ScalarArgument::Borrow(argument));
            }
            Type::Aggregate { .. } => {
                let (argument_instructions, source) = lower_aggregate_argument_source(
                    argument,
                    parameter_type,
                    callee_name,
                    context,
                    temporaries,
                )?;
                instructions.extend(argument_instructions);
                arguments.push(ScalarArgument::AggregateIndirect(AggregateArgument {
                    source,
                }));
            }
            Type::DirectAggregate { layout, words } => {
                let (argument_instructions, source) = lower_aggregate_argument_source(
                    argument,
                    parameter_type,
                    callee_name,
                    context,
                    temporaries,
                )?;
                instructions.extend(argument_instructions);
                arguments.push(ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source,
                    layout: *layout,
                    words: *words,
                }));
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

    let actual_abi_words = validate_call_argument_abi_word_count(callee_name, &arguments)?;
    if let Some(expected_abi_words) = context.call_parameter_abi_word_count(target)
        && actual_abi_words != expected_abi_words
    {
        return Err(call_argument_abi_word_count_mismatch_diagnostic(
            callee_name,
            expected_abi_words,
            actual_abi_words,
        ));
    }
    Ok((instructions, arguments))
}

pub(in crate::ir::lower) fn lower_macos_syscall_primitive_call_to_location(
    call: &CallExpr,
    destination: AggregateLocation,
    expected_layout: ValueLayout,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(arity) = macos_syscall_arity(call, context) else {
        return Ok(None);
    };
    if call.arguments.len() != arity + 1 {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower primitive `syscall{arity}` with {} `usize` arguments",
                arity + 1
            ),
        )]);
    }

    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_macos_syscall_diagnostic(
            "missing resolved primitive signature",
        ));
    };
    let Some(signature) = resolved.call_signature_for_call(call) else {
        return Err(unsupported_macos_syscall_diagnostic(
            "missing resolved call signature",
        ));
    };
    let value = abi_value_from_type_expr(&signature.return_type, resolved)
        .map_err(|_error| unsupported_macos_syscall_diagnostic("invalid return ABI layout"))?;
    if value.layout != expected_layout {
        return Err(unsupported_macos_syscall_diagnostic(
            "return layout does not match the destination aggregate",
        ));
    }

    let mut instructions = Vec::new();
    let mut words = Vec::with_capacity(call.arguments.len());
    for argument in &call.arguments {
        let lowered = lower_usize_expression_to_value(argument, context, temporaries)?;
        instructions.extend(lowered.instructions);
        words.push(lowered.value);
    }
    let mut words = words.into_iter();
    let number = words
        .next()
        .ok_or_else(|| unsupported_macos_syscall_diagnostic("missing syscall number argument"))?;
    let arguments = words.collect::<Vec<_>>();
    instructions.push(Instruction::DarwinSyscall {
        destination,
        arity: u8::try_from(arity).expect("macOS syscall arity fits in u8"),
        number,
        arguments,
    });
    Ok(Some(instructions))
}

fn validate_call_argument_abi_word_count(
    callee_name: &str,
    arguments: &[ScalarArgument],
) -> Result<usize, Vec<Diagnostic>> {
    call_argument_abi_word_count(arguments, callee_name)
}

pub(super) fn call_arguments_require_stack(
    arguments: &[ScalarArgument],
    callee_name: &str,
) -> Result<bool, Vec<Diagnostic>> {
    Ok(call_argument_abi_word_count(arguments, callee_name)? > ARGUMENT_REGISTER_COUNT)
}

fn call_argument_abi_word_count(
    arguments: &[ScalarArgument],
    callee_name: &str,
) -> Result<usize, Vec<Diagnostic>> {
    let mut count = 0_usize;
    for argument in arguments {
        count = count
            .checked_add(argument.abi_word_count())
            .ok_or_else(|| call_argument_abi_word_count_overflow_diagnostic(callee_name))?;
    }

    Ok(count)
}

fn call_argument_abi_word_count_overflow_diagnostic(callee_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!("IR v0 call argument ABI word count overflows for function `{callee_name}`"),
    )]
}

fn call_argument_abi_word_count_mismatch_diagnostic(
    callee_name: &str,
    expected: usize,
    actual: usize,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 lowered call arguments for function `{callee_name}` into {actual} ABI words, but the resolved signature expects {expected}"
        ),
    )]
}

fn lower_aggregate_argument_source(
    argument: &Expr,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, AggregateArgumentSource), Vec<Diagnostic>> {
    let expected_layout = match parameter_type {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => unreachable!("aggregate argument lowering requires aggregate parameter type"),
    };
    if !supported_aggregate_copy_layout(expected_layout) {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }

    match unwrap_group(argument) {
        Expr::Identifier(identifier) => lower_aggregate_local_argument_source(
            &identifier.name,
            AggregateValueUse::ImplicitCopy,
            expected_layout,
            parameter_type,
            callee_name,
            context,
        ),
        Expr::Member(_) => lower_aggregate_member_argument_source(
            argument,
            expected_layout,
            parameter_type,
            callee_name,
            context,
            temporaries,
        ),
        Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unary.operand.as_ref() else {
                return Err(unsupported_aggregate_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            lower_aggregate_local_argument_source(
                &identifier.name,
                AggregateValueUse::ExplicitMove,
                expected_layout,
                parameter_type,
                callee_name,
                context,
            )
        }
        Expr::StructLiteral(literal) => {
            let Some((_root_source, resolved)) = context.resolved_calls() else {
                return Err(unsupported_aggregate_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            let slot_index = temporaries.next_aggregate_slot();
            let mut instructions = vec![Instruction::ReserveAggregateSlot {
                slot_index,
                layout: expected_layout,
            }];
            instructions.extend(lower_aggregate_struct_literal_to_location_with_temporaries(
                literal,
                expected_layout,
                AggregateLocation::Slot(slot_index),
                "E8006",
                &format!("arguments for function `{callee_name}`"),
                resolved,
                context,
                temporaries,
            )?);
            Ok((instructions, AggregateArgumentSource::Slot(slot_index)))
        }
        Expr::Call(call) => lower_aggregate_call_argument_source(
            call,
            parameter_type,
            callee_name,
            context,
            temporaries,
        ),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Err(unsupported_aggregate_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            lower_aggregate_fallible_call_argument_source(
                call,
                parameter_type,
                callee_name,
                context,
                temporaries,
                propagating_failure_mode(context)?,
            )
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Err(unsupported_aggregate_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            lower_aggregate_fallible_call_argument_source(
                call,
                parameter_type,
                callee_name,
                context,
                temporaries,
                FallibleFailureMode::Trap,
            )
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Err(unsupported_aggregate_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            lower_aggregate_fallible_call_argument_source(
                call,
                parameter_type,
                callee_name,
                context,
                temporaries,
                lower_catch_failure_mode(catch, context, 0)?,
            )
        }
        _ => Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        )),
    }
}

fn lower_aggregate_local_argument_source(
    name: &str,
    value_use: AggregateValueUse,
    expected_layout: crate::abi::ValueLayout,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, AggregateArgumentSource), Vec<Diagnostic>> {
    let Some(local) = context.aggregate_local(name) else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    if local.layout != expected_layout
        || (value_use == AggregateValueUse::ImplicitCopy && !local.is_copy)
    {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    Ok((Vec::new(), AggregateArgumentSource::Slot(local.slot_index)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AggregateValueUse {
    ImplicitCopy,
    ExplicitMove,
}

fn lower_aggregate_member_argument_source(
    argument: &Expr,
    expected_layout: crate::abi::ValueLayout,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, AggregateArgumentSource), Vec<Diagnostic>> {
    let access = lower_aggregate_member_field_access(argument, context, temporaries)?
        .ok_or_else(|| unsupported_aggregate_argument_diagnostic(callee_name, parameter_type))?;
    let source = access.source;
    let source_offset = access.offset;
    let is_copy = access.is_copy;
    let AggregateFieldKind::Aggregate { layout, .. } = access.kind else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    if layout != expected_layout || !is_copy {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = access.instructions;
    instructions.push(Instruction::ReserveAggregateSlot { slot_index, layout });
    instructions.push(Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(slot_index),
        destination_offset: 0,
        source,
        source_offset,
        layout,
    });
    Ok((instructions, AggregateArgumentSource::Slot(slot_index)))
}

fn lower_aggregate_call_argument_source(
    call: &CallExpr,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, AggregateArgumentSource), Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    let Some(return_type) = context.call_return_type(&target) else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    if return_type != parameter_type {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    let Some(layout) = aggregate_type_layout(return_type) else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    if !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    let (call_instructions, arguments) =
        lower_call_arguments(call, &target, &call_name, context, temporaries)?;
    instructions.extend(call_instructions);
    push_aggregate_call_instruction(
        &mut instructions,
        return_type,
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
    );
    Ok((instructions, AggregateArgumentSource::Slot(slot_index)))
}

fn lower_aggregate_fallible_call_argument_source(
    call: &CallExpr,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<(Vec<Instruction>, AggregateArgumentSource), Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target) else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    if success_type.as_ref() != parameter_type {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    let Some(layout) = aggregate_type_layout(success_type.as_ref()) else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    if !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    let (call_instructions, arguments) =
        lower_call_arguments(call, &target, &call_name, context, temporaries)?;
    instructions.extend(call_instructions);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success_type.as_ref(),
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
        failure_mode,
    );
    Ok((instructions, AggregateArgumentSource::Slot(slot_index)))
}

fn unsupported_aggregate_argument_diagnostic(
    callee_name: &str,
    parameter_type: &Type,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower `{}` arguments for function `{callee_name}` from supported aggregate locals, struct literals, or aggregate calls",
            describe_type(parameter_type),
        ),
    )]
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

    let identifier_name = match unwrap_group(argument) {
        Expr::Borrow(borrow) => {
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
            &identifier.name
        }
        Expr::Identifier(identifier)
            if context
                .aggregate_borrow_parameter(&identifier.name)
                .is_some() =>
        {
            &identifier.name
        }
        _ => {
            return Err(unsupported_borrow_argument_diagnostic(
                callee_name,
                parameter_type,
            ));
        }
    };

    let source = lower_borrow_source_from_identifier(
        identifier_name,
        inner,
        parameter_type,
        callee_name,
        context,
    )?;

    Ok(BorrowArgument { source })
}

fn lower_implicit_receiver_borrow_argument(
    argument: &Expr,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<BorrowArgument, Vec<Diagnostic>> {
    let Type::Borrow { inner, .. } = parameter_type else {
        unreachable!("receiver borrow argument lowering requires a borrow parameter type");
    };

    let Expr::Identifier(identifier) = unwrap_group(argument) else {
        return Err(unsupported_borrow_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };

    let source = lower_borrow_source_from_identifier(
        &identifier.name,
        inner,
        parameter_type,
        callee_name,
        context,
    )?;

    Ok(BorrowArgument { source })
}

fn lower_borrow_source_from_identifier(
    identifier_name: &str,
    inner: &Type,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<BorrowSource, Vec<Diagnostic>> {
    match inner {
        Type::I32 => match context.i32_location(identifier_name) {
            Some(I32Location::Local(index)) => Ok(BorrowSource::I32(I32Location::Local(index))),
            Some(I32Location::Parameter(index)) => {
                Ok(BorrowSource::I32(I32Location::Parameter(index)))
            }
            _ => Err(unsupported_borrow_argument_diagnostic(
                callee_name,
                parameter_type,
            )),
        },
        Type::U8 => match context.u8_location(identifier_name) {
            Some(U8Location::Local(index)) => Ok(BorrowSource::U8(U8Location::Local(index))),
            Some(U8Location::Parameter(index)) => {
                Ok(BorrowSource::U8(U8Location::Parameter(index)))
            }
            _ => Err(unsupported_borrow_argument_diagnostic(
                callee_name,
                parameter_type,
            )),
        },
        Type::Usize => match context.usize_location(identifier_name) {
            Some(UsizeLocation::Local(index)) => {
                Ok(BorrowSource::Usize(UsizeLocation::Local(index)))
            }
            Some(UsizeLocation::Parameter(index)) => {
                Ok(BorrowSource::Usize(UsizeLocation::Parameter(index)))
            }
            _ => Err(unsupported_borrow_argument_diagnostic(
                callee_name,
                parameter_type,
            )),
        },
        Type::Bool => match context.bool_location(identifier_name) {
            Some(BoolLocation::Local(index)) => Ok(BorrowSource::Bool(BoolLocation::Local(index))),
            Some(BoolLocation::Parameter(index)) => {
                Ok(BorrowSource::Bool(BoolLocation::Parameter(index)))
            }
            _ => Err(unsupported_borrow_argument_diagnostic(
                callee_name,
                parameter_type,
            )),
        },
        Type::Aggregate {
            layout: expected_layout,
        }
        | Type::DirectAggregate {
            layout: expected_layout,
            ..
        } => {
            if let Some((slot_index, layout)) = context.aggregate_slot(identifier_name)
                && layout == *expected_layout
            {
                return Ok(BorrowSource::AggregateSlot(slot_index));
            }

            let required_readwrite = matches!(
                parameter_type,
                Type::Borrow {
                    is_readwrite: true,
                    ..
                }
            );
            if let Some(borrow) = context.aggregate_borrow_parameter(identifier_name)
                && borrow.layout == *expected_layout
                && (!required_readwrite || borrow.is_readwrite)
            {
                return Ok(BorrowSource::AggregateParameter(borrow.parameter_index));
            }

            Err(unsupported_borrow_argument_diagnostic(
                callee_name,
                parameter_type,
            ))
        }
        _ => Err(unsupported_borrow_argument_diagnostic(
            callee_name,
            parameter_type,
        )),
    }
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

pub(super) fn is_tail_call_stack_pointer_argument(argument: &ScalarArgument) -> bool {
    matches!(argument, ScalarArgument::AggregateIndirect(_))
}

pub(in crate::ir::lower) fn primitive_trap_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("trap" | "unreachable")
    )
}

pub(super) fn primitive_exit_raw_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(context.primitive_name_for_call(call), Some("exit_raw"))
}

fn lower_legacy_i32_call_arguments(
    call: &CallExpr,
    callee_name: &str,
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

    let _actual_abi_words = validate_call_argument_abi_word_count(callee_name, &arguments)?;
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
        return validate_call_success_return_passing(
            target,
            callee_name,
            callee_return_type,
            context,
        );
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
        return validate_call_success_return_passing(
            target,
            callee_name,
            callee_return_type,
            context,
        );
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
        return validate_call_success_return_passing(
            target,
            callee_name,
            callee_return_type,
            context,
        );
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
        return validate_call_success_return_passing(
            target,
            callee_name,
            callee_return_type,
            context,
        );
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
        return validate_call_success_return_passing(
            target,
            callee_name,
            callee_return_type,
            context,
        );
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
        return validate_call_success_return_passing(
            target,
            callee_name,
            callee_return_type,
            context,
        );
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
        return validate_call_success_return_passing(
            target,
            callee_name,
            callee_return_type,
            context,
        );
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

    if let Type::Fallible(success) = callee_return_type
        && success.as_ref() == &Type::Void
    {
        return validate_call_success_return_passing(target, callee_name, success, context);
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

    if let Type::Fallible(success) = callee_return_type
        && success.as_ref() == &Type::I32
    {
        return validate_call_success_return_passing(target, callee_name, success, context);
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

    if let Type::Fallible(success) = callee_return_type
        && success.as_ref() == &Type::Usize
    {
        return validate_call_success_return_passing(target, callee_name, success, context);
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

    if let Type::Fallible(success) = callee_return_type
        && success.as_ref() == &Type::U8
    {
        return validate_call_success_return_passing(target, callee_name, success, context);
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

    if let Type::Fallible(success) = callee_return_type
        && success.as_ref() == &Type::Bool
    {
        return validate_call_success_return_passing(target, callee_name, success, context);
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

    if let Type::Fallible(success) = callee_return_type
        && success.as_ref() == &Type::Str
    {
        return validate_call_success_return_passing(target, callee_name, success, context);
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

    if let Type::Fallible(success) = callee_return_type
        && matches!(success.as_ref(), Type::Slice { .. })
    {
        return validate_call_success_return_passing(target, callee_name, success, context);
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

fn validate_call_success_return_passing(
    target: &CallTarget,
    callee_name: &str,
    expected_success_type: &Type,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(actual) = context.call_success_return_passing(target) else {
        return Ok(());
    };
    let Some(expected) = expected_success_type.success_return_passing() else {
        return Ok(());
    };
    if actual == expected {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 call return ABI mismatch for function `{callee_name}`: expected callee success return to use `{}`, got `{}`",
            expected.description(),
            actual.description(),
        ),
    )])
}

pub(super) fn primitive_write_text_raw_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("write_text_raw")
    )
}

pub(super) fn primitive_open_read_raw_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(context.primitive_name_for_call(call), Some("open_read_raw"))
}

pub(super) fn primitive_write_bytes_raw_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("write_bytes_raw")
    )
}

pub(super) fn primitive_read_bytes_raw_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("read_bytes_raw")
    )
}

pub(super) fn primitive_close_fd_raw_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(context.primitive_name_for_call(call), Some("close_fd_raw"))
}

pub(super) fn primitive_bytes_from_str_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("bytes_from_str")
    )
}

pub(super) fn primitive_addr_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(context.primitive_name_for_call(call), Some("addr"))
}

pub(super) fn primitive_copy_str_to_ptr_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("copy_str_to_ptr")
    )
}

pub(super) fn primitive_store_u8_to_ptr_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("store_u8_to_ptr")
    )
}

pub(super) fn primitive_str_from_raw_parts_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("str_from_raw_parts")
    )
}

pub(super) fn primitive_slice_from_raw_parts_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("slice_from_raw_parts" | "slice_from_raw_parts_mut")
    )
}

pub(super) fn lower_addr_primitive_call_to_word(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    if call.arguments.len() != 1 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`addr` requires one pointer argument",
        ));
    }
    lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)
}

pub(super) fn lower_copy_str_to_ptr_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 3 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`copy_str_to_ptr` requires arguments `(destination: *u8, offset: usize, text: &str)`",
        ));
    }

    let (mut instructions, pointer) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    let offset = lower_usize_expression_to_value(&call.arguments[1], context, temporaries)?;
    instructions.extend(offset.instructions);
    let text = lower_str_expression_to_value(&call.arguments[2], context, temporaries)?;
    instructions.extend(text.instructions);
    instructions.push(Instruction::CopyStrToPointer {
        pointer,
        offset: offset.value,
        text: text.value,
    });
    Ok(instructions)
}

pub(super) fn lower_store_u8_to_ptr_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 3 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`store_u8_to_ptr` requires arguments `(destination: *u8, offset: usize, value: u8)`",
        ));
    }

    let (mut instructions, pointer) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    let offset = lower_usize_expression_to_value(&call.arguments[1], context, temporaries)?;
    instructions.extend(offset.instructions);
    let value = lower_u8_expression_to_value(&call.arguments[2], context, temporaries)?;
    instructions.extend(value.instructions);
    instructions.push(Instruction::StoreU8ToPointer {
        pointer,
        offset: offset.value,
        value: value.value,
    });
    Ok(instructions)
}

pub(super) fn lower_str_from_raw_parts_primitive_call_to_location(
    call: &CallExpr,
    destination: StrLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 2 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`str_from_raw_parts` requires arguments `(pointer: *u8, len: usize)`",
        ));
    }

    let (mut instructions, pointer) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    let len = lower_usize_expression_to_value(&call.arguments[1], context, temporaries)?;
    instructions.extend(len.instructions);
    instructions.push(Instruction::SetStrRawParts {
        destination,
        pointer,
        len: len.value,
    });
    Ok(instructions)
}

pub(super) fn lower_slice_from_raw_parts_primitive_call_to_location(
    call: &CallExpr,
    destination: SliceLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 2 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`slice_from_raw_parts` requires arguments `(pointer: *u8, len: usize)`",
        ));
    }

    let (mut instructions, pointer) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    let len = lower_usize_expression_to_value(&call.arguments[1], context, temporaries)?;
    instructions.extend(len.instructions);
    instructions.push(Instruction::SetSliceRawParts {
        destination,
        pointer,
        len: len.value,
    });
    Ok(instructions)
}

pub(super) fn lower_str_bytes_primitive_call_to_location(
    call: &CallExpr,
    destination: SliceLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (mut instructions, value) =
        lower_str_bytes_primitive_call_to_value(call, context, temporaries)?;
    instructions.push(Instruction::SetSlice { destination, value });
    Ok(instructions)
}

pub(super) fn lower_str_bytes_primitive_call_to_value(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, SliceValue), Vec<Diagnostic>> {
    if call.arguments.len() != 1 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`bytes_from_str` requires argument `(value: &str)`",
        ));
    }

    let text = lower_str_expression_to_value(&call.arguments[0], context, temporaries)?;
    Ok((text.instructions, SliceValue::StrBytes(text.value)))
}

pub(super) fn lower_close_fd_raw_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 1 {
        return Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower primitive `close_fd_raw` with argument `(i32)`",
        )]);
    }

    let fd = lower_i32_expression_to_value(&call.arguments[0], context, temporaries)?;
    let mut instructions = fd.instructions;
    instructions.push(Instruction::CloseFd { fd: fd.value });
    Ok(instructions)
}

pub(super) fn lower_exit_raw_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 1 {
        return Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower primitive `exit_raw` with argument `(i32)`",
        )]);
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let code = lower_i32_expression_to_value(&call.arguments[0], context, &mut temporaries)?;
    let mut instructions = code.instructions;
    instructions.push(Instruction::ProcessExit { code: code.value });
    Ok(instructions)
}

fn lower_read_bytes_raw_primitive_call(
    call: &CallExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 2 {
        return Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower primitive `read_bytes_raw` with arguments `(i32, &+[u8])`",
        )]);
    };

    let fd = lower_i32_expression_to_value(&call.arguments[0], context, temporaries)?;
    let buffer = lower_slice_expression_to_value(&call.arguments[1], context, temporaries)?;
    let mut instructions = fd.instructions;
    instructions.extend(buffer.instructions);
    instructions.push(Instruction::ReadSlice {
        destination,
        fd: fd.value,
        buffer: buffer.value,
        failure_mode,
    });
    Ok(instructions)
}

fn lower_open_read_raw_primitive_call(
    call: &CallExpr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 1 {
        return Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower primitive `open_read_raw` with argument `(*u8)`",
        )]);
    }

    let (mut instructions, path) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    instructions.push(Instruction::OpenRead {
        destination,
        path,
        failure_mode,
    });
    Ok(instructions)
}

fn macos_syscall_arity(call: &CallExpr, context: &LoweringContext) -> Option<usize> {
    match context.primitive_name_for_call(call)? {
        "syscall0" => Some(0),
        "syscall1" => Some(1),
        "syscall2" => Some(2),
        "syscall3" => Some(3),
        "syscall4" => Some(4),
        "syscall5" => Some(5),
        "syscall6" => Some(6),
        _ => None,
    }
}

fn unsupported_macos_syscall_diagnostic(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!("IR v0 cannot lower macOS syscall primitive: {reason}"),
    )]
}

pub(in crate::ir::lower) fn lower_pointer_address_expression_to_word(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    match expression {
        Expr::Call(call)
            if context.primitive_name_for_call(call) == Some("from_addr")
                && call.arguments.len() == 1 =>
        {
            let address =
                lower_usize_expression_to_value(&call.arguments[0], context, temporaries)?;
            Ok((address.instructions, address.value))
        }
        Expr::Member(_) => {
            let access = lower_aggregate_member_field_access(expression, context, temporaries)?
                .filter(|access| access.kind == AggregateFieldKind::Usize)
                .ok_or_else(|| {
                    unsupported_pointer_primitive_diagnostic(
                        "pointer argument must be a pointer aggregate field",
                    )
                })?;
            let destination = temporaries.next_usize()?;
            let mut instructions = access.instructions;
            instructions.push(Instruction::LoadAggregateUsize {
                destination,
                source: access.source,
                offset: access.offset,
            });
            Ok((instructions, UsizeValue::Location(destination)))
        }
        Expr::Group(group) => {
            lower_pointer_address_expression_to_word(&group.expression, context, temporaries)
        }
        _ => Err(unsupported_pointer_primitive_diagnostic(
            "pointer argument must come from `from_addr(...)` or a pointer aggregate field",
        )),
    }
}

fn unsupported_pointer_primitive_diagnostic(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!("IR v0 cannot lower pointer primitive call: {reason}"),
    )]
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

fn lower_write_bytes_raw_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 2 {
        return Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower primitive `write_bytes_raw` with arguments `(i32, &[u8])`",
        )]);
    };

    let fd = lower_i32_expression_to_value(&call.arguments[0], context, temporaries)?;
    let bytes = lower_slice_expression_to_value(&call.arguments[1], context, temporaries)?;
    let mut instructions = fd.instructions;
    instructions.extend(bytes.instructions);
    instructions.push(Instruction::WriteSlice {
        fd: fd.value,
        bytes: bytes.value,
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
        Type::DirectAggregate { .. } => "aggregate",
        Type::Borrow {
            is_readwrite: false,
            inner,
        } => match inner.as_ref() {
            Type::I32 => "&i32",
            Type::U8 => "&u8",
            Type::Usize => "&usize",
            Type::Bool => "&bool",
            Type::Aggregate { .. } => "&aggregate",
            Type::DirectAggregate { .. } => "&aggregate",
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
            Type::DirectAggregate { .. } => "&+aggregate",
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
            Type::DirectAggregate { .. } => "aggregate!",
            Type::Borrow {
                is_readwrite: false,
                inner,
            } => match inner.as_ref() {
                Type::I32 => "&i32!",
                Type::U8 => "&u8!",
                Type::Usize => "&usize!",
                Type::Bool => "&bool!",
                Type::Aggregate { .. } => "&aggregate!",
                Type::DirectAggregate { .. } => "&aggregate!",
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
                Type::DirectAggregate { .. } => "&+aggregate!",
                _ => "borrow!",
            },
            Type::Void => "void!",
            Type::Never => "never!",
            Type::Fallible(_) => "fallible",
        },
    }
}
