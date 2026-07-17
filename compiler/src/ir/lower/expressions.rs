use super::aggregates::{
    aggregate_call_return_layout_from_resolved, aggregate_fields_from_type_expr,
    aggregate_type_layout, push_aggregate_call_instruction,
    push_fallible_aggregate_call_instruction, supported_aggregate_copy_layout,
};
use super::bindings::{lower_assignment, lower_local_binding};
use super::context::{AggregateFieldKind, LoweringContext};
use super::errors::{ErrorPayload, lower_error_payload};
use super::functions::{
    append_scope_end_drops_before_exit, lower_never_expression_with_scope_drops,
    lower_value_return_with_scope_drops, mark_lowered_statement_aggregate_uses,
    propagating_failure_mode,
};
use super::literals::{
    lower_i32_literal, lower_str_literal, lower_u8_literal, lower_usize_literal,
};
use super::types::scalar_or_view_type_from_type_expr;
mod calls;
mod predicates;
mod temporaries;

use crate::ast::{
    BinaryExpr, BinaryOperator, Block, CallExpr, CatchExpr, Expr, IndexExpr, Stmt,
    TypeConversionExpr, UnaryExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolComparisonOperator, BoolLocation, BoolLogicalOperator, BoolValue,
    FallibleFailureMode, I32ComparisonOperator, I32Location, I32Value, Instruction, ScalarArgument,
    SliceLocation, SliceValue, StrLocation, StrValue, Type, U8Location, U8Value, UsizeLocation,
    UsizeValue,
};
pub(super) use calls::lower_macos_syscall_primitive_call_to_location;
pub(super) use calls::lower_pointer_address_expression_to_word;
pub(super) use calls::primitive_trap_call;
use calls::{
    call_arguments_require_stack, is_tail_call_stack_pointer_argument,
    lower_addr_primitive_call_to_word, lower_bool_normal_call, lower_call_arguments,
    lower_close_fd_raw_primitive_call, lower_copy_str_to_ptr_primitive_call,
    lower_direct_tail_call, lower_fallible_bool_normal_call, lower_fallible_i32_normal_call,
    lower_fallible_slice_normal_call, lower_fallible_str_normal_call,
    lower_fallible_u8_normal_call, lower_fallible_usize_normal_call,
    lower_fallible_void_normal_call, lower_i32_normal_call, lower_slice_normal_call,
    lower_store_u8_to_ptr_primitive_call, lower_str_bytes_primitive_call_to_location,
    lower_str_bytes_primitive_call_to_value, lower_str_from_raw_parts_primitive_call_to_location,
    lower_str_normal_call, lower_u8_normal_call, lower_usize_normal_call, lower_void_normal_call,
    primitive_addr_call, primitive_bytes_from_str_call, primitive_close_fd_raw_call,
    primitive_copy_str_to_ptr_call, primitive_store_u8_to_ptr_call,
    primitive_str_from_raw_parts_call, primitive_write_bytes_raw_call,
    primitive_write_text_raw_call,
};
use predicates::{
    bool_comparison_contains_call, bool_comparison_needs_temporaries,
    expressions_are_lowerable_bool_comparison_operands, expressions_are_lowerable_bool_values,
    expressions_are_lowerable_usize_values, i32_comparison_needs_temporaries,
    is_i32_binary_operator, is_usize_binary_operator, short_circuit_bool_expression_contains_call,
    u8_comparison_is_lowerable, usize_comparison_needs_temporaries,
};
pub(super) use predicates::{
    expression_contains_call, expression_contains_interpolated_string,
    expression_is_lowerable_bool_binding, expression_is_unsupported_bool_comparison_binding,
};
pub(super) use temporaries::TemporaryAllocator;
use temporaries::{
    LoweredI32Value, LoweredSliceValue, LoweredStrValue, LoweredU8Value, LoweredUsizeValue,
};

pub(super) fn lower_i32_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_i32_expression_to_location(expression, I32Location::Return, context)
}

pub(super) fn lower_i32_expression_to_location(
    expression: &Expr,
    destination: I32Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_i32_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Propagate(propagation) => lower_i32_fallible_expression_to_location(
            &propagation.expression,
            destination,
            context,
            propagating_failure_mode(context)?,
        ),
        Expr::Force(force) => lower_i32_fallible_expression_to_location(
            &force.expression,
            destination,
            context,
            FallibleFailureMode::Trap,
        ),
        Expr::Catch(catch) => lower_i32_fallible_expression_to_location(
            &catch.expression,
            destination,
            context,
            lower_catch_failure_mode(
                catch,
                context,
                i32_destination_reserved_abi_words(destination),
            )?,
        ),
        Expr::Binary(binary) if is_i32_binary_operator(binary.operator) => {
            lower_i32_binary_expression_to_location(binary, destination, context)
        }
        Expr::TypeConversion(conversion)
            if type_conversion_target_is(conversion, context, Type::I32) =>
        {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered =
                lower_i32_conversion_expression_to_value(conversion, context, &mut temporaries)?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetI32 {
                destination,
                value: lowered.value,
            });
            Ok(instructions)
        }
        Expr::Member(_) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_aggregate_i32_field_to_location(
                expression,
                destination,
                context,
                &mut temporaries,
            )
        }
        Expr::Group(group) => {
            lower_i32_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_i32_value(expression, context)
            .map(|value| vec![Instruction::SetI32 { destination, value }]),
    }
}

pub(super) fn lower_u8_expression_to_location(
    expression: &Expr,
    destination: U8Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_u8_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Propagate(propagation) => lower_u8_fallible_expression_to_location(
            &propagation.expression,
            destination,
            context,
            propagating_failure_mode(context)?,
        ),
        Expr::Force(force) => lower_u8_fallible_expression_to_location(
            &force.expression,
            destination,
            context,
            FallibleFailureMode::Trap,
        ),
        Expr::Catch(catch) => lower_u8_fallible_expression_to_location(
            &catch.expression,
            destination,
            context,
            lower_catch_failure_mode(
                catch,
                context,
                u8_destination_reserved_abi_words(destination),
            )?,
        ),
        Expr::Index(index) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered = lower_u8_index_expression_to_value(index, context, &mut temporaries)?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetU8 {
                destination,
                value: lowered.value,
            });
            Ok(instructions)
        }
        Expr::TypeConversion(conversion)
            if type_conversion_target_is(conversion, context, Type::U8) =>
        {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered =
                lower_u8_expression_to_value(&conversion.expression, context, &mut temporaries)?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetU8 {
                destination,
                value: lowered.value,
            });
            Ok(instructions)
        }
        Expr::Member(_) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_aggregate_u8_field_to_location(expression, destination, context, &mut temporaries)
        }
        Expr::Group(group) => {
            lower_u8_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_u8_value(expression, context)
            .map(|value| vec![Instruction::SetU8 { destination, value }]),
    }
}

pub(super) fn lower_usize_expression_to_location(
    expression: &Expr,
    destination: UsizeLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            if let Some(value) = lower_builtin_len_call_to_value(call, context, &mut temporaries) {
                let lowered = value?;
                let mut instructions = lowered.instructions;
                instructions.push(Instruction::SetUsize {
                    destination,
                    value: lowered.value,
                });
                return Ok(instructions);
            }
            if primitive_addr_call(call, context) {
                let (mut instructions, value) =
                    lower_addr_primitive_call_to_word(call, context, &mut temporaries)?;
                instructions.push(Instruction::SetUsize { destination, value });
                return Ok(instructions);
            }

            lower_usize_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Propagate(propagation) => lower_usize_fallible_expression_to_location(
            &propagation.expression,
            destination,
            context,
            propagating_failure_mode(context)?,
        ),
        Expr::Force(force) => lower_usize_fallible_expression_to_location(
            &force.expression,
            destination,
            context,
            FallibleFailureMode::Trap,
        ),
        Expr::Catch(catch) => lower_usize_fallible_expression_to_location(
            &catch.expression,
            destination,
            context,
            lower_catch_failure_mode(
                catch,
                context,
                usize_destination_reserved_abi_words(destination),
            )?,
        ),
        Expr::Binary(binary) if is_usize_binary_operator(binary.operator) => {
            lower_usize_binary_expression_to_location(binary, destination, context)
        }
        Expr::TypeConversion(conversion)
            if type_conversion_target_is(conversion, context, Type::Usize) =>
        {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered =
                lower_usize_conversion_expression_to_value(conversion, context, &mut temporaries)?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetUsize {
                destination,
                value: lowered.value,
            });
            Ok(instructions)
        }
        Expr::Member(_) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_aggregate_usize_field_to_location(
                expression,
                destination,
                context,
                &mut temporaries,
            )
        }
        Expr::Group(group) => {
            lower_usize_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_usize_value(expression, context)
            .map(|value| vec![Instruction::SetUsize { destination, value }]),
    }
}

pub(super) fn lower_str_expression_to_location(
    expression: &Expr,
    destination: StrLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            if primitive_str_from_raw_parts_call(call, context) {
                return lower_str_from_raw_parts_primitive_call_to_location(
                    call,
                    destination,
                    context,
                    &mut temporaries,
                );
            }
            lower_str_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Propagate(propagation) => lower_str_fallible_expression_to_location(
            &propagation.expression,
            destination,
            context,
            propagating_failure_mode(context)?,
        ),
        Expr::Force(force) => lower_str_fallible_expression_to_location(
            &force.expression,
            destination,
            context,
            FallibleFailureMode::Trap,
        ),
        Expr::Catch(catch) => lower_str_fallible_expression_to_location(
            &catch.expression,
            destination,
            context,
            lower_catch_failure_mode(
                catch,
                context,
                str_destination_reserved_abi_words(destination),
            )?,
        ),
        Expr::Group(group) => {
            lower_str_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_str_value(expression, context)
            .map(|value| vec![Instruction::SetStr { destination, value }]),
    }
}

pub(super) fn lower_slice_expression_to_location(
    expression: &Expr,
    destination: SliceLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            if primitive_bytes_from_str_call(call, context) {
                return lower_str_bytes_primitive_call_to_location(
                    call,
                    destination,
                    context,
                    &mut temporaries,
                );
            }
            lower_slice_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Propagate(propagation) => lower_slice_fallible_expression_to_location(
            &propagation.expression,
            destination,
            context,
            propagating_failure_mode(context)?,
        ),
        Expr::Force(force) => lower_slice_fallible_expression_to_location(
            &force.expression,
            destination,
            context,
            FallibleFailureMode::Trap,
        ),
        Expr::Catch(catch) => lower_slice_fallible_expression_to_location(
            &catch.expression,
            destination,
            context,
            lower_catch_failure_mode(
                catch,
                context,
                slice_destination_reserved_abi_words(destination),
            )?,
        ),
        Expr::Group(group) => {
            lower_slice_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_slice_value(expression, context)
            .map(|value| vec![Instruction::SetSlice { destination, value }]),
    }
}

pub(super) fn lower_void_expression_statement(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if primitive_copy_str_to_ptr_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                return lower_copy_str_to_ptr_primitive_call(call, context, &mut temporaries)
                    .map(Some);
            }
            if primitive_store_u8_to_ptr_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                return lower_store_u8_to_ptr_primitive_call(call, context, &mut temporaries)
                    .map(Some);
            }
            if primitive_close_fd_raw_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                return lower_close_fd_raw_primitive_call(call, context, &mut temporaries)
                    .map(Some);
            }

            let Some((target, _call_name)) = context.direct_call_target_and_name(call) else {
                return Ok(None);
            };
            if context.call_return_type(&target) != Some(&Type::Void) {
                return Ok(None);
            }

            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_void_normal_call(call, context, &mut temporaries).map(Some)
        }
        Expr::Group(group) => lower_void_expression_statement(&group.expression, context),
        Expr::Propagate(propagation) => lower_fallible_void_expression_statement(
            &propagation.expression,
            context,
            propagating_failure_mode(context)?,
        ),
        Expr::Force(force) => lower_fallible_void_expression_statement(
            &force.expression,
            context,
            FallibleFailureMode::Trap,
        ),
        Expr::Catch(catch) => lower_fallible_void_expression_statement(
            &catch.expression,
            context,
            lower_catch_failure_mode(catch, context, 0)?,
        ),
        _ => Ok(None),
    }
}

fn lower_fallible_void_expression_statement(
    expression: &Expr,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let target = context
                .direct_call_target_and_name(call)
                .map(|(target, _call_name)| target);
            if !primitive_write_text_raw_call(call, context)
                && !primitive_write_bytes_raw_call(call, context)
                && !matches!(
                    target.as_ref().and_then(|target| context.call_return_type(target)),
                    Some(Type::Fallible(success)) if success.as_ref() == &Type::Void
                )
            {
                return Ok(None);
            }

            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_fallible_void_normal_call(call, context, &mut temporaries, failure_mode).map(Some)
        }
        Expr::Group(group) => {
            lower_fallible_void_expression_statement(&group.expression, context, failure_mode)
        }
        _ => Ok(None),
    }
}

pub(super) fn lower_catch_failure_mode(
    catch: &CatchExpr,
    context: &LoweringContext,
    reserved_abi_words: usize,
) -> Result<FallibleFailureMode, Vec<Diagnostic>> {
    let mut catch_context = context.with_reserved_local_abi_words(reserved_abi_words);
    let (code, message) = catch_context.define_error_local(catch.error_name.clone())?;
    let instructions = lower_catch_block(&catch.catch_block, &mut catch_context)?;

    Ok(FallibleFailureMode::Catch {
        code,
        message,
        instructions,
    })
}

fn lower_catch_block(
    block: &Block,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((last, leading)) = block.statements.split_last() else {
        return Err(unsupported_catch_block_diagnostic());
    };

    let mut instructions = lower_catch_leading_statements(leading, context)?;
    let function_return_type = context.function_return_type().clone();
    let success_type = function_return_type.success_type().clone();

    match last {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression
                && let Some(return_instructions) =
                    lower_never_expression_with_scope_drops(expression, context)?
            {
                instructions.extend(return_instructions);
                return Ok(instructions);
            }

            if let Some(expression) = &statement.expression
                && matches!(function_return_type, Type::Fallible(_))
                && let Some((root_source, resolved)) = context.resolved_calls()
                && let Some(payload) =
                    lower_error_payload(expression, resolved, root_source, Some(context))?
            {
                instructions.extend(append_scope_end_drops_before_exit(
                    lower_fallible_failure(payload),
                    context,
                )?);
                return Ok(instructions);
            }

            if let Some(expression) = &statement.expression
                && let Some(return_instructions) = lower_value_return_with_scope_drops(
                    &success_type,
                    expression,
                    &function_return_type,
                    context,
                )?
            {
                instructions.extend(return_instructions);
                return Ok(instructions);
            }

            let return_instructions = match (&success_type, &statement.expression) {
                (Type::I32, Some(expression)) => lower_i32_return_expression(expression, context),
                (Type::U8, Some(expression)) => lower_u8_return_expression(expression, context),
                (Type::Usize, Some(expression)) => {
                    lower_usize_return_expression(expression, context)
                }
                (Type::Bool, Some(expression)) => {
                    lower_bool_return_expression(expression, context, "E8007")
                }
                (Type::Str, Some(expression)) => lower_str_return_expression(expression, context),
                (Type::Slice { .. }, Some(expression)) => {
                    lower_slice_return_expression(expression, context)
                }
                (Type::Void, None) => Ok(vec![Instruction::Return]),
                (Type::Void, Some(_)) => Err(unsupported_catch_block_diagnostic()),
                (Type::Never, Some(_)) => Err(unsupported_catch_block_diagnostic()),
                (Type::I32, None)
                | (Type::U8, None)
                | (Type::Usize, None)
                | (Type::Bool, None)
                | (Type::Str, None)
                | (Type::Slice { .. }, None)
                | (Type::Aggregate { .. }, _)
                | (Type::DirectAggregate { .. }, _)
                | (Type::Borrow { .. }, _)
                | (Type::Never, None) => Err(unsupported_catch_block_diagnostic()),
                (Type::Fallible(_), _) => {
                    unreachable!("fallible success type must be unwrapped")
                }
            }?;
            let return_instructions =
                mark_fallible_success_returns(&function_return_type, return_instructions);
            instructions.extend(append_scope_end_drops_before_exit(
                return_instructions,
                context,
            )?);
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) =
                lower_never_expression_with_scope_drops(&statement.expression, context)?
            else {
                if success_type == Type::Void
                    && let Some(void_instructions) =
                        lower_void_expression_statement(&statement.expression, context)?
                {
                    instructions.extend(void_instructions);
                    instructions.extend(append_scope_end_drops_before_exit(
                        vec![success_return_instruction(&function_return_type)],
                        context,
                    )?);
                    return Ok(instructions);
                }

                return Err(unsupported_catch_block_diagnostic());
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(unsupported_catch_block_diagnostic()),
    }
}

fn lower_catch_leading_statements(
    statements: &[Stmt],
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();

    for statement in statements {
        match statement {
            Stmt::Binding(statement) => {
                instructions.extend(lower_local_binding(statement, context)?);
            }
            Stmt::Assignment(statement) => {
                instructions.extend(lower_assignment(statement, context)?);
            }
            Stmt::Expression(statement) => {
                let Some(void_instructions) =
                    lower_void_expression_statement(&statement.expression, context)?
                else {
                    return Err(unsupported_catch_block_diagnostic());
                };
                instructions.extend(void_instructions);
            }
            _ => return Err(unsupported_catch_block_diagnostic()),
        }
        mark_lowered_statement_aggregate_uses(statement, context);
    }

    Ok(instructions)
}

fn lower_fallible_failure(payload: ErrorPayload) -> Vec<Instruction> {
    let (code, message) = payload.into_str_values();
    vec![Instruction::ReturnFallibleFailure { code, message }]
}

fn i32_destination_reserved_abi_words(destination: I32Location) -> usize {
    usize::from(matches!(destination, I32Location::Local(_)))
}

fn u8_destination_reserved_abi_words(destination: U8Location) -> usize {
    usize::from(matches!(destination, U8Location::Local(_)))
}

fn usize_destination_reserved_abi_words(destination: UsizeLocation) -> usize {
    usize::from(matches!(destination, UsizeLocation::Local(_)))
}

fn bool_destination_reserved_abi_words(destination: BoolLocation) -> usize {
    usize::from(matches!(destination, BoolLocation::Local(_)))
}

fn str_destination_reserved_abi_words(destination: StrLocation) -> usize {
    if matches!(destination, StrLocation::Local(_)) {
        2
    } else {
        0
    }
}

fn slice_destination_reserved_abi_words(destination: SliceLocation) -> usize {
    if matches!(destination, SliceLocation::Local(_)) {
        2
    } else {
        0
    }
}

fn unsupported_catch_block_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        "IR v0 can only lower catch blocks containing leading scalar local bindings, scalar assignments, or void call statements followed by `return`",
    )]
}

fn lower_i32_fallible_expression_to_location(
    expression: &Expr,
    destination: I32Location,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
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

fn lower_u8_fallible_expression_to_location(
    expression: &Expr,
    destination: U8Location,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
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

fn lower_usize_fallible_expression_to_location(
    expression: &Expr,
    destination: UsizeLocation,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
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

fn lower_str_fallible_expression_to_location(
    expression: &Expr,
    destination: StrLocation,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
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

fn lower_slice_fallible_expression_to_location(
    expression: &Expr,
    destination: SliceLocation,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
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

fn lower_i32_binary_expression_to_location(
    binary: &BinaryExpr,
    destination: I32Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_i32_binary_expression_to_location_with_temporaries(
        binary,
        destination,
        context,
        &mut temporaries,
    )
}

fn lower_i32_binary_expression_to_location_with_temporaries(
    binary: &BinaryExpr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let left = lower_i32_expression_to_value(&binary.left, context, temporaries)?;
    let right = lower_i32_expression_to_value(&binary.right, context, temporaries)?;
    let mut instructions = left.instructions;
    instructions.extend(right.instructions);
    instructions.push(i32_binary_instruction(
        binary.operator,
        destination,
        left.value,
        right.value,
    )?);
    Ok(instructions)
}

fn lower_i32_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredI32Value, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let temporary = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_i32_normal_call(call, temporary, context, temporaries)?,
                value: I32Value::Location(temporary),
            })
        }
        Expr::Propagate(propagation) => {
            let temporary = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_i32_fallible_expression_to_location(
                    &propagation.expression,
                    temporary,
                    context,
                    propagating_failure_mode(context)?,
                )?,
                value: I32Value::Location(temporary),
            })
        }
        Expr::Force(force) => {
            let temporary = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_i32_fallible_expression_to_location(
                    &force.expression,
                    temporary,
                    context,
                    FallibleFailureMode::Trap,
                )?,
                value: I32Value::Location(temporary),
            })
        }
        Expr::Catch(catch) => {
            let temporary = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_i32_fallible_expression_to_location(
                    &catch.expression,
                    temporary,
                    context,
                    lower_catch_failure_mode(
                        catch,
                        context,
                        i32_destination_reserved_abi_words(temporary),
                    )?,
                )?,
                value: I32Value::Location(temporary),
            })
        }
        Expr::Binary(binary) if is_i32_binary_operator(binary.operator) => {
            let temporary = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_i32_binary_expression_to_location_with_temporaries(
                    binary,
                    temporary,
                    context,
                    temporaries,
                )?,
                value: I32Value::Location(temporary),
            })
        }
        Expr::TypeConversion(conversion)
            if type_conversion_target_is(conversion, context, Type::I32) =>
        {
            lower_i32_conversion_expression_to_value(conversion, context, temporaries)
        }
        Expr::Member(_) => {
            let temporary = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_aggregate_i32_field_to_location(
                    expression,
                    temporary,
                    context,
                    temporaries,
                )?,
                value: I32Value::Location(temporary),
            })
        }
        Expr::Group(group) => {
            lower_i32_expression_to_value(&group.expression, context, temporaries)
        }
        _ => Ok(LoweredI32Value {
            instructions: Vec::new(),
            value: lower_i32_value(expression, context)?,
        }),
    }
}

pub(super) fn lower_i32_expression_to_word(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, I32Value), Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    let lowered = lower_i32_expression_to_value(expression, context, &mut temporaries)?;
    Ok((lowered.instructions, lowered.value))
}

fn lower_u8_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredU8Value, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let temporary = temporaries.next_u8()?;
            Ok(LoweredU8Value {
                instructions: lower_u8_normal_call(call, temporary, context, temporaries)?,
                value: U8Value::Location(temporary),
            })
        }
        Expr::Propagate(propagation) => {
            let temporary = temporaries.next_u8()?;
            Ok(LoweredU8Value {
                instructions: lower_u8_fallible_expression_to_location(
                    &propagation.expression,
                    temporary,
                    context,
                    propagating_failure_mode(context)?,
                )?,
                value: U8Value::Location(temporary),
            })
        }
        Expr::Force(force) => {
            let temporary = temporaries.next_u8()?;
            Ok(LoweredU8Value {
                instructions: lower_u8_fallible_expression_to_location(
                    &force.expression,
                    temporary,
                    context,
                    FallibleFailureMode::Trap,
                )?,
                value: U8Value::Location(temporary),
            })
        }
        Expr::Catch(catch) => {
            let temporary = temporaries.next_u8()?;
            Ok(LoweredU8Value {
                instructions: lower_u8_fallible_expression_to_location(
                    &catch.expression,
                    temporary,
                    context,
                    lower_catch_failure_mode(
                        catch,
                        context,
                        u8_destination_reserved_abi_words(temporary),
                    )?,
                )?,
                value: U8Value::Location(temporary),
            })
        }
        Expr::Index(index) => lower_u8_index_expression_to_value(index, context, temporaries),
        Expr::TypeConversion(conversion)
            if type_conversion_target_is(conversion, context, Type::U8) =>
        {
            lower_u8_expression_to_value(&conversion.expression, context, temporaries)
        }
        Expr::Member(_) => {
            let temporary = temporaries.next_u8()?;
            Ok(LoweredU8Value {
                instructions: lower_aggregate_u8_field_to_location(
                    expression,
                    temporary,
                    context,
                    temporaries,
                )?,
                value: U8Value::Location(temporary),
            })
        }
        Expr::Group(group) => lower_u8_expression_to_value(&group.expression, context, temporaries),
        _ => Ok(LoweredU8Value {
            instructions: Vec::new(),
            value: lower_u8_value(expression, context)?,
        }),
    }
}

pub(super) fn lower_u8_expression_to_word(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, U8Value), Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    let lowered = lower_u8_expression_to_value(expression, context, &mut temporaries)?;
    Ok((lowered.instructions, lowered.value))
}

fn lower_i32_conversion_expression_to_value(
    conversion: &TypeConversionExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredI32Value, Vec<Diagnostic>> {
    if let Ok(value) = lower_i32_value(&conversion.expression, context) {
        return Ok(LoweredI32Value {
            instructions: Vec::new(),
            value,
        });
    }

    let value = lower_u8_expression_to_value(&conversion.expression, context, temporaries)?;
    Ok(LoweredI32Value {
        instructions: value.instructions,
        value: I32Value::U8ZeroExtend(Box::new(value.value)),
    })
}

fn lower_usize_conversion_expression_to_value(
    conversion: &TypeConversionExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    if let Ok(value) = lower_usize_value(&conversion.expression, context) {
        return Ok(LoweredUsizeValue {
            instructions: Vec::new(),
            value,
        });
    }

    let value = lower_u8_expression_to_value(&conversion.expression, context, temporaries)?;
    Ok(LoweredUsizeValue {
        instructions: value.instructions,
        value: UsizeValue::U8ZeroExtend(Box::new(value.value)),
    })
}

fn type_conversion_target_is(
    conversion: &TypeConversionExpr,
    context: &LoweringContext,
    expected: Type,
) -> bool {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return false;
    };
    scalar_or_view_type_from_type_expr(&conversion.ty, resolved) == Some(expected)
}

fn lower_usize_binary_expression_to_location(
    binary: &BinaryExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_usize_binary_expression_to_location_with_temporaries(
        binary,
        destination,
        context,
        &mut temporaries,
    )
}

fn lower_usize_binary_expression_to_location_with_temporaries(
    binary: &BinaryExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let left = lower_usize_expression_to_value(&binary.left, context, temporaries)?;
    let right = lower_usize_expression_to_value(&binary.right, context, temporaries)?;
    let mut instructions = left.instructions;
    instructions.extend(right.instructions);
    instructions.push(usize_binary_instruction(
        binary.operator,
        destination,
        left.value,
        right.value,
    )?);
    Ok(instructions)
}

fn lower_usize_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if let Some(value) = lower_builtin_len_call_to_value(call, context, temporaries) {
                return value;
            }
            if primitive_addr_call(call, context) {
                let (instructions, value) =
                    lower_addr_primitive_call_to_word(call, context, temporaries)?;
                return Ok(LoweredUsizeValue {
                    instructions,
                    value,
                });
            }

            let temporary = temporaries.next_usize()?;
            Ok(LoweredUsizeValue {
                instructions: lower_usize_normal_call(call, temporary, context, temporaries)?,
                value: UsizeValue::Location(temporary),
            })
        }
        Expr::Propagate(propagation) => {
            let temporary = temporaries.next_usize()?;
            Ok(LoweredUsizeValue {
                instructions: lower_usize_fallible_expression_to_location(
                    &propagation.expression,
                    temporary,
                    context,
                    propagating_failure_mode(context)?,
                )?,
                value: UsizeValue::Location(temporary),
            })
        }
        Expr::Force(force) => {
            let temporary = temporaries.next_usize()?;
            Ok(LoweredUsizeValue {
                instructions: lower_usize_fallible_expression_to_location(
                    &force.expression,
                    temporary,
                    context,
                    FallibleFailureMode::Trap,
                )?,
                value: UsizeValue::Location(temporary),
            })
        }
        Expr::Catch(catch) => {
            let temporary = temporaries.next_usize()?;
            Ok(LoweredUsizeValue {
                instructions: lower_usize_fallible_expression_to_location(
                    &catch.expression,
                    temporary,
                    context,
                    lower_catch_failure_mode(
                        catch,
                        context,
                        usize_destination_reserved_abi_words(temporary),
                    )?,
                )?,
                value: UsizeValue::Location(temporary),
            })
        }
        Expr::Binary(binary) if is_usize_binary_operator(binary.operator) => {
            let temporary = temporaries.next_usize()?;
            Ok(LoweredUsizeValue {
                instructions: lower_usize_binary_expression_to_location_with_temporaries(
                    binary,
                    temporary,
                    context,
                    temporaries,
                )?,
                value: UsizeValue::Location(temporary),
            })
        }
        Expr::TypeConversion(conversion)
            if type_conversion_target_is(conversion, context, Type::Usize) =>
        {
            lower_usize_conversion_expression_to_value(conversion, context, temporaries)
        }
        Expr::Member(_) => {
            let temporary = temporaries.next_usize()?;
            Ok(LoweredUsizeValue {
                instructions: lower_aggregate_usize_field_to_location(
                    expression,
                    temporary,
                    context,
                    temporaries,
                )?,
                value: UsizeValue::Location(temporary),
            })
        }
        Expr::Group(group) => {
            lower_usize_expression_to_value(&group.expression, context, temporaries)
        }
        _ => Ok(LoweredUsizeValue {
            instructions: Vec::new(),
            value: lower_usize_value(expression, context)?,
        }),
    }
}

fn lower_str_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let temporary = temporaries.next_str()?;
            if primitive_str_from_raw_parts_call(call, context) {
                return Ok(LoweredStrValue {
                    instructions: lower_str_from_raw_parts_primitive_call_to_location(
                        call,
                        temporary,
                        context,
                        temporaries,
                    )?,
                    value: StrValue::Location(temporary),
                });
            }
            Ok(LoweredStrValue {
                instructions: lower_str_normal_call(call, temporary, context, temporaries)?,
                value: StrValue::Location(temporary),
            })
        }
        Expr::Propagate(propagation) => {
            let temporary = temporaries.next_str()?;
            Ok(LoweredStrValue {
                instructions: lower_str_fallible_expression_to_location(
                    &propagation.expression,
                    temporary,
                    context,
                    propagating_failure_mode(context)?,
                )?,
                value: StrValue::Location(temporary),
            })
        }
        Expr::Force(force) => {
            let temporary = temporaries.next_str()?;
            Ok(LoweredStrValue {
                instructions: lower_str_fallible_expression_to_location(
                    &force.expression,
                    temporary,
                    context,
                    FallibleFailureMode::Trap,
                )?,
                value: StrValue::Location(temporary),
            })
        }
        Expr::Catch(catch) => {
            let temporary = temporaries.next_str()?;
            Ok(LoweredStrValue {
                instructions: lower_str_fallible_expression_to_location(
                    &catch.expression,
                    temporary,
                    context,
                    lower_catch_failure_mode(
                        catch,
                        context,
                        str_destination_reserved_abi_words(temporary),
                    )?,
                )?,
                value: StrValue::Location(temporary),
            })
        }
        Expr::Group(group) => {
            lower_str_expression_to_value(&group.expression, context, temporaries)
        }
        _ => Ok(LoweredStrValue {
            instructions: Vec::new(),
            value: lower_str_value(expression, context)?,
        }),
    }
}

fn lower_slice_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredSliceValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if primitive_bytes_from_str_call(call, context) {
                let (instructions, value) =
                    lower_str_bytes_primitive_call_to_value(call, context, temporaries)?;
                return Ok(LoweredSliceValue {
                    instructions,
                    value,
                });
            }
            let temporary = temporaries.next_slice()?;
            Ok(LoweredSliceValue {
                instructions: lower_slice_normal_call(call, temporary, context, temporaries)?,
                value: SliceValue::Location(temporary),
            })
        }
        Expr::Propagate(propagation) => {
            let temporary = temporaries.next_slice()?;
            Ok(LoweredSliceValue {
                instructions: lower_slice_fallible_expression_to_location(
                    &propagation.expression,
                    temporary,
                    context,
                    propagating_failure_mode(context)?,
                )?,
                value: SliceValue::Location(temporary),
            })
        }
        Expr::Force(force) => {
            let temporary = temporaries.next_slice()?;
            Ok(LoweredSliceValue {
                instructions: lower_slice_fallible_expression_to_location(
                    &force.expression,
                    temporary,
                    context,
                    FallibleFailureMode::Trap,
                )?,
                value: SliceValue::Location(temporary),
            })
        }
        Expr::Catch(catch) => {
            let temporary = temporaries.next_slice()?;
            Ok(LoweredSliceValue {
                instructions: lower_slice_fallible_expression_to_location(
                    &catch.expression,
                    temporary,
                    context,
                    lower_catch_failure_mode(
                        catch,
                        context,
                        slice_destination_reserved_abi_words(temporary),
                    )?,
                )?,
                value: SliceValue::Location(temporary),
            })
        }
        Expr::Group(group) => {
            lower_slice_expression_to_value(&group.expression, context, temporaries)
        }
        _ => Ok(LoweredSliceValue {
            instructions: Vec::new(),
            value: lower_slice_value(expression, context)?,
        }),
    }
}

fn i32_binary_instruction(
    operator: BinaryOperator,
    destination: I32Location,
    left: I32Value,
    right: I32Value,
) -> Result<Instruction, Vec<Diagnostic>> {
    match operator {
        BinaryOperator::Add => Ok(Instruction::AddI32 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Subtract => Ok(Instruction::SubtractI32 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Multiply => Ok(Instruction::MultiplyI32 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Divide => Ok(Instruction::DivideI32 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Remainder => Ok(Instruction::RemainderI32 {
            destination,
            left,
            right,
        }),
        BinaryOperator::ShiftLeft => Ok(Instruction::ShiftLeftI32 {
            destination,
            left,
            right,
        }),
        BinaryOperator::ShiftRight => Ok(Instruction::ShiftRightI32 {
            destination,
            left,
            right,
        }),
        _ => Err(unsupported_i32_expression_diagnostic()),
    }
}

fn usize_binary_instruction(
    operator: BinaryOperator,
    destination: UsizeLocation,
    left: UsizeValue,
    right: UsizeValue,
) -> Result<Instruction, Vec<Diagnostic>> {
    match operator {
        BinaryOperator::Add => Ok(Instruction::AddUsize {
            destination,
            left,
            right,
        }),
        BinaryOperator::Subtract => Ok(Instruction::SubtractUsize {
            destination,
            left,
            right,
        }),
        BinaryOperator::Multiply => Ok(Instruction::MultiplyUsize {
            destination,
            left,
            right,
        }),
        BinaryOperator::Divide => Ok(Instruction::DivideUsize {
            destination,
            left,
            right,
        }),
        BinaryOperator::Remainder => Ok(Instruction::RemainderUsize {
            destination,
            left,
            right,
        }),
        BinaryOperator::ShiftLeft => Ok(Instruction::ShiftLeftUsize {
            destination,
            left,
            right,
        }),
        BinaryOperator::ShiftRight => Ok(Instruction::ShiftRightUsize {
            destination,
            left,
            right,
        }),
        _ => Err(unsupported_usize_expression_diagnostic()),
    }
}

pub(super) fn lower_i32_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => lower_direct_tail_call(call, context),
        Expr::Group(group) => lower_i32_return_expression(&group.expression, context),
        _ => {
            let mut instructions = lower_i32_expression(expression, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

pub(super) fn success_return_instruction(return_type: &Type) -> Instruction {
    if matches!(return_type, Type::Fallible(_)) {
        Instruction::ReturnFallibleSuccess
    } else {
        Instruction::Return
    }
}

pub(super) fn mark_fallible_success_returns(
    return_type: &Type,
    instructions: Vec<Instruction>,
) -> Vec<Instruction> {
    if !matches!(return_type, Type::Fallible(_)) {
        return instructions;
    }

    replace_success_returns(instructions)
}

fn replace_success_returns(instructions: Vec<Instruction>) -> Vec<Instruction> {
    instructions
        .into_iter()
        .map(|instruction| match instruction {
            Instruction::Return => Instruction::ReturnFallibleSuccess,
            Instruction::If {
                condition,
                then_instructions,
                else_instructions,
            } => Instruction::If {
                condition,
                then_instructions: replace_success_returns(then_instructions),
                else_instructions: replace_success_returns(else_instructions),
            },
            instruction => instruction,
        })
        .collect()
}

pub(super) fn lower_u8_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => lower_direct_tail_call(call, context),
        Expr::Group(group) => lower_u8_return_expression(&group.expression, context),
        _ => {
            let mut instructions =
                lower_u8_expression_to_location(expression, U8Location::Return, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

pub(super) fn lower_never_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) if primitive_trap_call(call, context) => Ok(Some(vec![Instruction::Trap])),
        Expr::Call(call) => {
            let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
                return Ok(None);
            };
            if context.call_return_type(&target) != Some(&Type::Never) {
                return Ok(None);
            }

            let mut temporaries = TemporaryAllocator::new(context)?;
            let (mut instructions, arguments) =
                lower_call_arguments(call, &target, &call_name, context, &mut temporaries)?;
            let requires_current_frame = arguments
                .iter()
                .any(never_tail_call_argument_requires_current_frame);
            if requires_current_frame || call_arguments_require_stack(&arguments, &call_name)? {
                instructions.push(Instruction::CallVoid { target, arguments });
                instructions.push(Instruction::Trap);
                return Ok(Some(instructions));
            }
            instructions.push(Instruction::TailCall { target, arguments });
            Ok(Some(instructions))
        }
        Expr::Group(group) => lower_never_return_expression(&group.expression, context),
        _ => Ok(None),
    }
}

fn never_tail_call_argument_requires_current_frame(argument: &ScalarArgument) -> bool {
    matches!(argument, ScalarArgument::Borrow(_)) || is_tail_call_stack_pointer_argument(argument)
}

pub(super) fn lower_usize_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            if let Some(value) = lower_builtin_len_call_to_value(call, context, &mut temporaries) {
                let lowered = value?;
                let mut instructions = lowered.instructions;
                instructions.push(Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: lowered.value,
                });
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if primitive_addr_call(call, context) {
                let (mut instructions, value) =
                    lower_addr_primitive_call_to_word(call, context, &mut temporaries)?;
                instructions.push(Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value,
                });
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }

            lower_direct_tail_call(call, context)
        }
        Expr::Group(group) => lower_usize_return_expression(&group.expression, context),
        _ => {
            let mut instructions =
                lower_usize_expression_to_location(expression, UsizeLocation::Return, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

pub(super) fn lower_str_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            if primitive_str_from_raw_parts_call(call, context) {
                let mut instructions = lower_str_from_raw_parts_primitive_call_to_location(
                    call,
                    StrLocation::Return,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            lower_direct_tail_call(call, context)
        }
        Expr::Group(group) => lower_str_return_expression(&group.expression, context),
        _ => {
            let mut instructions =
                lower_str_expression_to_location(expression, StrLocation::Return, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

pub(super) fn lower_slice_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if primitive_bytes_from_str_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                let mut instructions = lower_str_bytes_primitive_call_to_location(
                    call,
                    SliceLocation::Return,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            lower_direct_tail_call(call, context)
        }
        Expr::Group(group) => lower_slice_return_expression(&group.expression, context),
        _ => {
            let mut instructions =
                lower_slice_expression_to_location(expression, SliceLocation::Return, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

pub(super) fn lower_bool_return_expression(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => lower_direct_tail_call(call, context),
        Expr::Group(group) => {
            lower_bool_return_expression(&group.expression, context, diagnostic_code)
        }
        _ => {
            let mut instructions = lower_bool_expression_to_location(
                expression,
                BoolLocation::Return,
                context,
                diagnostic_code,
            )?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

pub(super) fn lower_bool_expression_to_location(
    expression: &Expr,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Binary(binary) if short_circuit_bool_expression_contains_call(binary) => {
            lower_short_circuit_bool_expression_to_location(
                binary,
                destination,
                context,
                diagnostic_code,
            )
        }
        Expr::Binary(binary) if bool_comparison_contains_call(binary, context) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let comparison = lower_bool_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = comparison.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: comparison.value,
            });
            Ok(instructions)
        }
        Expr::Binary(binary) if bool_comparison_needs_temporaries(binary, context) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let comparison = lower_bool_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = comparison.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: comparison.value,
            });
            Ok(instructions)
        }
        Expr::Binary(binary) if u8_comparison_is_lowerable(binary, context) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let comparison = lower_u8_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = comparison.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: comparison.value,
            });
            Ok(instructions)
        }
        Expr::Binary(binary) if i32_comparison_needs_temporaries(binary, context) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let comparison = lower_i32_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = comparison.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: comparison.value,
            });
            Ok(instructions)
        }
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_bool_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Propagate(propagation) => lower_bool_fallible_expression_to_location(
            &propagation.expression,
            destination,
            context,
            diagnostic_code,
            propagating_failure_mode(context)?,
        ),
        Expr::Force(force) => lower_bool_fallible_expression_to_location(
            &force.expression,
            destination,
            context,
            diagnostic_code,
            FallibleFailureMode::Trap,
        ),
        Expr::Catch(catch) => lower_bool_fallible_expression_to_location(
            &catch.expression,
            destination,
            context,
            diagnostic_code,
            lower_catch_failure_mode(
                catch,
                context,
                bool_destination_reserved_abi_words(destination),
            )?,
        ),
        Expr::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let operand = lower_bool_expression_to_value_with_temporaries(
                &unary.operand,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = operand.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: BoolValue::Not(Box::new(operand.value)),
            });
            Ok(instructions)
        }
        Expr::Member(_) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_aggregate_bool_field_to_location(
                expression,
                destination,
                context,
                diagnostic_code,
                &mut temporaries,
            )
        }
        Expr::Group(group) => lower_bool_expression_to_location(
            &group.expression,
            destination,
            context,
            diagnostic_code,
        ),
        _ => Ok(vec![Instruction::SetBool {
            destination,
            value: lower_bool_value(expression, context, diagnostic_code)?,
        }]),
    }
}

fn lower_bool_fallible_expression_to_location(
    expression: &Expr,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
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

fn lower_short_circuit_bool_expression_to_location(
    binary: &BinaryExpr,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_bool_expression_to_branch(
        &Expr::Binary(binary.clone()),
        vec![Instruction::SetBool {
            destination,
            value: BoolValue::Const(true),
        }],
        vec![Instruction::SetBool {
            destination,
            value: BoolValue::Const(false),
        }],
        context,
        diagnostic_code,
    )
}

fn lower_bool_expression_to_branch(
    expression: &Expr,
    then_instructions: Vec<Instruction>,
    else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Expr::Binary(binary) = unwrap_group(expression)
        && short_circuit_bool_expression_contains_call(binary)
    {
        return lower_short_circuit_bool_expression_to_branch(
            binary,
            then_instructions,
            else_instructions,
            context,
            diagnostic_code,
        );
    }

    let condition = lower_bool_expression_to_value(expression, context, diagnostic_code)?;
    let mut instructions = condition.instructions;
    instructions.push(Instruction::If {
        condition: condition.value,
        then_instructions,
        else_instructions,
    });
    Ok(instructions)
}

fn lower_short_circuit_bool_expression_to_branch(
    binary: &BinaryExpr,
    then_instructions: Vec<Instruction>,
    else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match binary.operator {
        BinaryOperator::LogicalAnd => lower_bool_expression_to_branch(
            &binary.left,
            lower_bool_expression_to_branch(
                &binary.right,
                then_instructions,
                else_instructions.clone(),
                context,
                diagnostic_code,
            )?,
            else_instructions,
            context,
            diagnostic_code,
        ),
        BinaryOperator::LogicalOr => lower_bool_expression_to_branch(
            &binary.left,
            then_instructions.clone(),
            lower_bool_expression_to_branch(
                &binary.right,
                then_instructions,
                else_instructions,
                context,
                diagnostic_code,
            )?,
            context,
            diagnostic_code,
        ),
        _ => unreachable!("short-circuit bool expression must be && or ||"),
    }
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

fn lower_aggregate_i32_field_to_location(
    expression: &Expr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let access = lower_aggregate_member_field_access(expression, context, temporaries)?
        .filter(|access| access.kind == AggregateFieldKind::I32)
        .ok_or_else(unsupported_i32_expression_diagnostic)?;
    let mut instructions = access.instructions;
    instructions.push(Instruction::LoadAggregateI32 {
        destination,
        source: access.source,
        offset: access.offset,
    });
    Ok(instructions)
}

fn lower_aggregate_u8_field_to_location(
    expression: &Expr,
    destination: U8Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let access = lower_aggregate_member_field_access(expression, context, temporaries)?
        .filter(|access| access.kind == AggregateFieldKind::U8)
        .ok_or_else(unsupported_u8_expression_diagnostic)?;
    let mut instructions = access.instructions;
    instructions.push(Instruction::LoadAggregateU8 {
        destination,
        source: access.source,
        offset: access.offset,
    });
    Ok(instructions)
}

fn lower_aggregate_usize_field_to_location(
    expression: &Expr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let access = lower_aggregate_member_field_access(expression, context, temporaries)?
        .filter(|access| access.kind == AggregateFieldKind::Usize)
        .ok_or_else(unsupported_usize_expression_diagnostic)?;
    let mut instructions = access.instructions;
    instructions.push(Instruction::LoadAggregateUsize {
        destination,
        source: access.source,
        offset: access.offset,
    });
    Ok(instructions)
}

fn lower_aggregate_bool_field_to_location(
    expression: &Expr,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let access = lower_aggregate_member_field_access(expression, context, temporaries)?
        .filter(|access| access.kind == AggregateFieldKind::Bool)
        .ok_or_else(|| unsupported_bool_expression_diagnostic(diagnostic_code))?;
    let mut instructions = access.instructions;
    instructions.push(Instruction::LoadAggregateBool {
        destination,
        source: access.source,
        offset: access.offset,
    });
    Ok(instructions)
}

pub(super) struct LoweredAggregateFieldAccess {
    pub(super) instructions: Vec<Instruction>,
    pub(super) source: AggregateLocation,
    pub(super) offset: u32,
    pub(super) kind: AggregateFieldKind,
    pub(super) is_copy: bool,
}

pub(super) fn lower_aggregate_member_field_access(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredAggregateFieldAccess>, Vec<Diagnostic>> {
    let Some(access) = aggregate_member_access(expression, context)? else {
        return Ok(None);
    };
    match access.root {
        AggregateMemberRoot::Identifier(identifier_name) => Ok(context
            .aggregate_field(identifier_name, &access.field_path)
            .map(|field| LoweredAggregateFieldAccess {
                instructions: Vec::new(),
                source: field.source,
                offset: field.offset,
                kind: field.kind,
                is_copy: field.is_copy,
            })),
        AggregateMemberRoot::Call(call) => {
            lower_aggregate_call_member_field_access(call, &access.field_path, context, temporaries)
        }
        AggregateMemberRoot::FallibleCall(call, failure_mode) => {
            lower_aggregate_fallible_call_member_field_access(
                call,
                &access.field_path,
                context,
                temporaries,
                failure_mode,
            )
        }
    }
}

struct AggregateMemberAccess<'a> {
    root: AggregateMemberRoot<'a>,
    field_path: String,
}

enum AggregateMemberRoot<'a> {
    Identifier(&'a str),
    Call(&'a CallExpr),
    FallibleCall(&'a CallExpr, FallibleFailureMode),
}

fn aggregate_member_access<'a>(
    expression: &'a Expr,
    context: &LoweringContext,
) -> Result<Option<AggregateMemberAccess<'a>>, Vec<Diagnostic>> {
    let Expr::Member(member) = unwrap_group(expression) else {
        return Ok(None);
    };
    let Some((root, mut fields)) = aggregate_member_root_and_path(&member.object, context)? else {
        return Ok(None);
    };
    fields.push(member.member.as_str());
    Ok(Some(AggregateMemberAccess {
        root,
        field_path: fields.join("."),
    }))
}

fn aggregate_member_root_and_path<'a>(
    expression: &'a Expr,
    context: &LoweringContext,
) -> Result<Option<(AggregateMemberRoot<'a>, Vec<&'a str>)>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Ok(Some((
            AggregateMemberRoot::Identifier(&identifier.name),
            Vec::new(),
        ))),
        Expr::Call(call) => Ok(Some((AggregateMemberRoot::Call(call), Vec::new()))),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Ok(None);
            };
            Ok(Some((
                AggregateMemberRoot::FallibleCall(call, propagating_failure_mode(context)?),
                Vec::new(),
            )))
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Ok(None);
            };
            Ok(Some((
                AggregateMemberRoot::FallibleCall(call, FallibleFailureMode::Trap),
                Vec::new(),
            )))
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Ok(None);
            };
            Ok(Some((
                AggregateMemberRoot::FallibleCall(
                    call,
                    lower_catch_failure_mode(catch, context, 0)?,
                ),
                Vec::new(),
            )))
        }
        Expr::Member(member) => {
            let Some((root, mut fields)) = aggregate_member_root_and_path(&member.object, context)?
            else {
                return Ok(None);
            };
            fields.push(member.member.as_str());
            Ok(Some((root, fields)))
        }
        _ => Ok(None),
    }
}

fn lower_aggregate_call_member_field_access(
    call: &CallExpr,
    member_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredAggregateFieldAccess>, Vec<Diagnostic>> {
    if macos_syscall_primitive_call(call, context)
        && let Some(layout) = aggregate_call_return_layout_from_resolved(call, context)
    {
        if !supported_aggregate_copy_layout(layout) {
            return Ok(None);
        }
        let Some(field) = aggregate_call_field(call, member_name, context) else {
            return Ok(None);
        };

        let slot_index = temporaries.next_aggregate_slot();
        let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
        let Some(mut syscall_instructions) = lower_macos_syscall_primitive_call_to_location(
            call,
            AggregateLocation::Slot(slot_index),
            layout,
            context,
            temporaries,
        )?
        else {
            return Ok(None);
        };
        instructions.append(&mut syscall_instructions);

        return Ok(Some(LoweredAggregateFieldAccess {
            instructions,
            source: AggregateLocation::Slot(slot_index),
            offset: field.offset,
            kind: field.kind,
            is_copy: true,
        }));
    }

    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };
    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let Some(layout) = aggregate_type_layout(&return_type) else {
        return Ok(None);
    };
    if !supported_aggregate_copy_layout(layout) {
        return Ok(None);
    }
    let Some(field) = aggregate_call_field(call, member_name, context) else {
        return Ok(None);
    };

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    let (mut argument_instructions, arguments) =
        lower_call_arguments(call, &target, &call_name, context, temporaries)?;
    instructions.append(&mut argument_instructions);
    push_aggregate_call_instruction(
        &mut instructions,
        &return_type,
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
    );

    Ok(Some(LoweredAggregateFieldAccess {
        instructions,
        source: AggregateLocation::Slot(slot_index),
        offset: field.offset,
        kind: field.kind,
        is_copy: true,
    }))
}

fn macos_syscall_primitive_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some(
            "syscall0"
                | "syscall1"
                | "syscall2"
                | "syscall3"
                | "syscall4"
                | "syscall5"
                | "syscall6"
        )
    )
}

fn lower_aggregate_fallible_call_member_field_access(
    call: &CallExpr,
    member_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Option<LoweredAggregateFieldAccess>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let Some(layout) = aggregate_type_layout(success_type.as_ref()) else {
        return Ok(None);
    };
    if !supported_aggregate_copy_layout(layout) {
        return Ok(None);
    }
    let Some(field) = aggregate_call_field(call, member_name, context) else {
        return Ok(None);
    };

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    let (mut argument_instructions, arguments) =
        lower_call_arguments(call, &target, &call_name, context, temporaries)?;
    instructions.append(&mut argument_instructions);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success_type.as_ref(),
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
        failure_mode,
    );

    Ok(Some(LoweredAggregateFieldAccess {
        instructions,
        source: AggregateLocation::Slot(slot_index),
        offset: field.offset,
        kind: field.kind,
        is_copy: true,
    }))
}

pub(super) fn aggregate_call_field(
    call: &CallExpr,
    member_name: &str,
    context: &LoweringContext,
) -> Option<super::context::AggregateField> {
    let (_root_source, resolved) = context.resolved_calls()?;
    let signature = resolved.call_signature_for_call(call)?;
    aggregate_fields_from_type_expr(&signature.return_type, resolved)?
        .into_iter()
        .find(|field| field.name == member_name)
}

pub(super) struct LoweredBoolValue {
    pub(super) instructions: Vec<Instruction>,
    pub(super) value: BoolValue,
}

pub(super) fn lower_bool_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_bool_expression_to_value_with_temporaries(
        expression,
        context,
        diagnostic_code,
        &mut temporaries,
    )
}

fn lower_bool_expression_to_value_with_temporaries(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    match expression {
        Expr::Binary(binary) if bool_comparison_contains_call(binary, context) => {
            lower_bool_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Binary(binary) if bool_comparison_needs_temporaries(binary, context) => {
            lower_bool_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Binary(binary) if u8_comparison_is_lowerable(binary, context) => {
            lower_u8_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Binary(binary) if i32_comparison_needs_temporaries(binary, context) => {
            lower_i32_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Binary(binary) if usize_comparison_needs_temporaries(binary, context) => {
            lower_usize_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Call(call) => {
            let temporary = temporaries.next_bool()?;
            Ok(LoweredBoolValue {
                instructions: lower_bool_normal_call(call, temporary, context, temporaries)?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::Propagate(propagation) => {
            let temporary = temporaries.next_bool()?;
            Ok(LoweredBoolValue {
                instructions: lower_bool_fallible_expression_to_location(
                    &propagation.expression,
                    temporary,
                    context,
                    diagnostic_code,
                    propagating_failure_mode(context)?,
                )?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::Force(force) => {
            let temporary = temporaries.next_bool()?;
            Ok(LoweredBoolValue {
                instructions: lower_bool_fallible_expression_to_location(
                    &force.expression,
                    temporary,
                    context,
                    diagnostic_code,
                    FallibleFailureMode::Trap,
                )?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::Catch(catch) => {
            let temporary = temporaries.next_bool()?;
            Ok(LoweredBoolValue {
                instructions: lower_bool_fallible_expression_to_location(
                    &catch.expression,
                    temporary,
                    context,
                    diagnostic_code,
                    lower_catch_failure_mode(
                        catch,
                        context,
                        bool_destination_reserved_abi_words(temporary),
                    )?,
                )?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            let operand = lower_bool_expression_to_value_with_temporaries(
                &unary.operand,
                context,
                diagnostic_code,
                temporaries,
            )?;
            Ok(LoweredBoolValue {
                instructions: operand.instructions,
                value: BoolValue::Not(Box::new(operand.value)),
            })
        }
        Expr::Member(_) => {
            let temporary = temporaries.next_bool()?;
            Ok(LoweredBoolValue {
                instructions: lower_aggregate_bool_field_to_location(
                    expression,
                    temporary,
                    context,
                    diagnostic_code,
                    temporaries,
                )?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::Group(group) => lower_bool_expression_to_value_with_temporaries(
            &group.expression,
            context,
            diagnostic_code,
            temporaries,
        ),
        _ => Ok(LoweredBoolValue {
            instructions: Vec::new(),
            value: lower_bool_value(expression, context, diagnostic_code)?,
        }),
    }
}

fn lower_bool_comparison_to_value_with_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let operator = match binary.operator {
        BinaryOperator::Equal => BoolComparisonOperator::Equal,
        BinaryOperator::NotEqual => BoolComparisonOperator::NotEqual,
        _ => return Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    };

    let left = lower_bool_comparison_operand_to_value_with_temporaries(
        &binary.left,
        context,
        diagnostic_code,
        temporaries,
    )?;
    let right = lower_bool_comparison_operand_to_value_with_temporaries(
        &binary.right,
        context,
        diagnostic_code,
        temporaries,
    )?;

    let mut instructions = left.instructions;
    instructions.extend(right.instructions);
    Ok(LoweredBoolValue {
        instructions,
        value: BoolValue::BoolComparison {
            operator,
            left: Box::new(left.value),
            right: Box::new(right.value),
        },
    })
}

fn lower_bool_comparison_operand_to_value_with_temporaries(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let temporary = temporaries.next_bool()?;
            Ok(LoweredBoolValue {
                instructions: lower_bool_normal_call(call, temporary, context, temporaries)?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::Member(_) => {
            let temporary = temporaries.next_bool()?;
            Ok(LoweredBoolValue {
                instructions: lower_aggregate_bool_field_to_location(
                    expression,
                    temporary,
                    context,
                    diagnostic_code,
                    temporaries,
                )?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::BoolLiteral(_) | Expr::Identifier(_) => Ok(LoweredBoolValue {
            instructions: Vec::new(),
            value: lower_bool_comparison_operand(expression, context, diagnostic_code)?,
        }),
        Expr::Group(group) => lower_bool_comparison_operand_to_value_with_temporaries(
            &group.expression,
            context,
            diagnostic_code,
            temporaries,
        ),
        _ => Err(unsupported_bool_comparison_operand_diagnostic(
            diagnostic_code,
        )),
    }
}

fn lower_i32_comparison_to_value_with_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let operator = i32_comparison_operator(binary.operator, diagnostic_code)?;
    let left = lower_i32_expression_to_value(&binary.left, context, temporaries)?;
    let right = lower_i32_expression_to_value(&binary.right, context, temporaries)?;

    let mut instructions = left.instructions;
    instructions.extend(right.instructions);
    Ok(LoweredBoolValue {
        instructions,
        value: BoolValue::I32Comparison {
            operator,
            left: left.value,
            right: right.value,
        },
    })
}

fn lower_usize_comparison_to_value_with_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let operator = i32_comparison_operator(binary.operator, diagnostic_code)?;
    let left = lower_usize_expression_to_value(&binary.left, context, temporaries)?;
    let right = lower_usize_expression_to_value(&binary.right, context, temporaries)?;

    let mut instructions = left.instructions;
    instructions.extend(right.instructions);
    Ok(LoweredBoolValue {
        instructions,
        value: BoolValue::UsizeComparison {
            operator,
            left: left.value,
            right: right.value,
        },
    })
}

fn lower_u8_comparison_to_value_with_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let operator = i32_comparison_operator(binary.operator, diagnostic_code)?;
    let left = lower_u8_expression_to_value(&binary.left, context, temporaries)?;
    let right = lower_u8_expression_to_value(&binary.right, context, temporaries)?;

    let mut instructions = left.instructions;
    instructions.extend(right.instructions);
    Ok(LoweredBoolValue {
        instructions,
        value: BoolValue::I32Comparison {
            operator,
            left: I32Value::U8ZeroExtend(Box::new(left.value)),
            right: I32Value::U8ZeroExtend(Box::new(right.value)),
        },
    })
}

fn lower_u8_comparison_condition(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    let comparison = lower_u8_comparison_to_value_with_temporaries(
        binary,
        context,
        diagnostic_code,
        &mut temporaries,
    )?;
    if comparison.instructions.is_empty() {
        Ok(comparison.value)
    } else {
        Err(unsupported_bool_expression_diagnostic(diagnostic_code))
    }
}

fn lower_str_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<StrValue, Vec<Diagnostic>> {
    match expression {
        Expr::Identifier(identifier) => context
            .str_location(&identifier.name)
            .map(StrValue::Location)
            .ok_or_else(unsupported_str_expression_diagnostic),
        Expr::Member(member) => {
            let Expr::Identifier(identifier) = member.object.as_ref() else {
                return Err(unsupported_str_expression_diagnostic());
            };

            let location = match member.member.as_str() {
                "code" => context.error_code_location(&identifier.name),
                "message" => context.error_message_location(&identifier.name),
                _ => None,
            };

            location
                .map(StrValue::Location)
                .ok_or_else(unsupported_str_expression_diagnostic)
        }
        Expr::Group(group) => lower_str_value(&group.expression, context),
        _ => lower_str_literal(expression),
    }
}

fn lower_slice_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<SliceValue, Vec<Diagnostic>> {
    match expression {
        Expr::Identifier(identifier) => context
            .slice_location(&identifier.name)
            .map(SliceValue::Location)
            .ok_or_else(unsupported_slice_expression_diagnostic),
        Expr::Group(group) => lower_slice_value(&group.expression, context),
        _ => Err(unsupported_slice_expression_diagnostic()),
    }
}

pub(super) fn lower_i32_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<I32Value, Vec<Diagnostic>> {
    match expression {
        Expr::Call(_) => Err(unsupported_non_tail_call_diagnostic()),
        Expr::Identifier(identifier) => context
            .i32_location(&identifier.name)
            .map(I32Value::Location)
            .ok_or_else(unsupported_i32_expression_diagnostic),
        Expr::Group(group) => lower_i32_value(&group.expression, context),
        _ => lower_i32_literal(expression).map(I32Value::Const),
    }
}

pub(super) fn lower_u8_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<U8Value, Vec<Diagnostic>> {
    match expression {
        Expr::Call(_) => Err(unsupported_non_tail_call_diagnostic()),
        Expr::Identifier(identifier) => context
            .u8_location(&identifier.name)
            .map(U8Value::Location)
            .ok_or_else(unsupported_u8_expression_diagnostic),
        Expr::Group(group) => lower_u8_value(&group.expression, context),
        _ => lower_u8_literal(expression).map(U8Value::Const),
    }
}

pub(super) fn lower_usize_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<UsizeValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(_) => Err(unsupported_non_tail_call_diagnostic()),
        Expr::Identifier(identifier) => context
            .usize_location(&identifier.name)
            .map(UsizeValue::Location)
            .ok_or_else(unsupported_usize_expression_diagnostic),
        Expr::Group(group) => lower_usize_value(&group.expression, context),
        _ => lower_usize_literal(expression).map(UsizeValue::Const),
    }
}

fn lower_u8_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredU8Value, Vec<Diagnostic>> {
    let source =
        lower_byte_collection_expression_to_value(&expression.object, context, temporaries)?;
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;

    match source {
        LoweredByteCollectionValue::Str(source) => {
            let mut instructions = source.instructions;
            instructions.extend(index.instructions);
            Ok(LoweredU8Value {
                instructions,
                value: match source.value {
                    StrValue::StaticBytes(bytes) => U8Value::StaticStrIndex {
                        bytes,
                        index: index.value,
                    },
                    StrValue::Location(source) => U8Value::StrIndex {
                        source,
                        index: index.value,
                    },
                },
            })
        }
        LoweredByteCollectionValue::Slice(source) => {
            let mut instructions = source.instructions;
            let value = match source.value {
                SliceValue::Location(source) => U8Value::SliceIndex {
                    source,
                    index: index.value,
                },
                SliceValue::StrBytes(StrValue::StaticBytes(bytes)) => U8Value::StaticStrIndex {
                    bytes,
                    index: index.value,
                },
                SliceValue::StrBytes(StrValue::Location(source)) => U8Value::StrIndex {
                    source,
                    index: index.value,
                },
            };
            instructions.extend(index.instructions);
            Ok(LoweredU8Value {
                instructions,
                value,
            })
        }
    }
}

enum ByteCollectionKind {
    Str,
    Slice,
}

enum LoweredByteCollectionValue {
    Str(LoweredStrValue),
    Slice(LoweredSliceValue),
}

fn lower_byte_collection_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredByteCollectionValue, Vec<Diagnostic>> {
    match byte_collection_expression_kind(expression, context) {
        Some(ByteCollectionKind::Str) => {
            lower_str_expression_to_value(expression, context, temporaries)
                .map(LoweredByteCollectionValue::Str)
        }
        Some(ByteCollectionKind::Slice) => {
            lower_slice_expression_to_value(expression, context, temporaries)
                .map(LoweredByteCollectionValue::Slice)
        }
        None => Err(unsupported_u8_expression_diagnostic()),
    }
}

fn byte_collection_expression_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<ByteCollectionKind> {
    match expression {
        Expr::StringLiteral(_) => Some(ByteCollectionKind::Str),
        Expr::Identifier(identifier) => {
            if context.str_location(&identifier.name).is_some() {
                Some(ByteCollectionKind::Str)
            } else if context.slice_location(&identifier.name).is_some() {
                Some(ByteCollectionKind::Slice)
            } else {
                None
            }
        }
        Expr::Call(call) => {
            let (target, _call_name) = context.direct_call_target_and_name(call)?;
            match context.call_return_type(&target) {
                Some(Type::Str) => Some(ByteCollectionKind::Str),
                Some(Type::Slice { .. }) => Some(ByteCollectionKind::Slice),
                _ => None,
            }
        }
        Expr::Group(group) => byte_collection_expression_kind(&group.expression, context),
        _ => None,
    }
}

pub(super) fn lower_usize_expression_to_word(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    let lowered = lower_usize_expression_to_value(expression, context, &mut temporaries)?;
    Ok((lowered.instructions, lowered.value))
}

pub(super) fn lower_call_arguments_to_scalar_arguments(
    call: &CallExpr,
    target: &crate::ir::CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, Vec<ScalarArgument>), Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_call_arguments(call, target, callee_name, context, &mut temporaries)
}

pub(super) fn lower_call_arguments_to_scalar_arguments_with_temporaries(
    call: &CallExpr,
    target: &crate::ir::CallTarget,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, Vec<ScalarArgument>), Vec<Diagnostic>> {
    lower_call_arguments(call, target, callee_name, context, temporaries)
}

fn lower_builtin_len_call_to_value(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Option<Result<LoweredUsizeValue, Vec<Diagnostic>>> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };
    if member.member != "len" || !call.arguments.is_empty() {
        return None;
    }
    byte_collection_expression_kind(&member.object, context)?;

    Some(
        lower_byte_collection_expression_to_value(&member.object, context, temporaries).map(
            |source| match source {
                LoweredByteCollectionValue::Str(source) => LoweredUsizeValue {
                    instructions: source.instructions,
                    value: match source.value {
                        StrValue::StaticBytes(bytes) => UsizeValue::Const(bytes.len() as u64),
                        StrValue::Location(location) => UsizeValue::StrLen(location),
                    },
                },
                LoweredByteCollectionValue::Slice(source) => {
                    let value = match source.value {
                        SliceValue::Location(location) => UsizeValue::SliceLen(location),
                        SliceValue::StrBytes(StrValue::StaticBytes(bytes)) => {
                            UsizeValue::Const(bytes.len() as u64)
                        }
                        SliceValue::StrBytes(StrValue::Location(location)) => {
                            UsizeValue::StrLen(location)
                        }
                    };
                    LoweredUsizeValue {
                        instructions: source.instructions,
                        value,
                    }
                }
            },
        ),
    )
}

pub(super) fn lower_bool_value(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(_) => Err(unsupported_non_tail_call_diagnostic()),
        Expr::BoolLiteral(literal) => match literal.value.as_str() {
            "true" => Ok(BoolValue::Const(true)),
            "false" => Ok(BoolValue::Const(false)),
            _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
        },
        Expr::Identifier(identifier) => context
            .bool_location(&identifier.name)
            .map(BoolValue::Location)
            .ok_or_else(|| unsupported_bool_expression_diagnostic(diagnostic_code)),
        Expr::Unary(unary) => lower_bool_unary_value(unary, context, diagnostic_code),
        Expr::Binary(binary) => lower_bool_binary_value(binary, context, diagnostic_code),
        Expr::Group(group) => lower_bool_value(&group.expression, context, diagnostic_code),
        _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    }
}

fn lower_bool_unary_value(
    unary: &UnaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    match unary.operator {
        UnaryOperator::LogicalNot => Ok(BoolValue::Not(Box::new(lower_bool_value(
            &unary.operand,
            context,
            diagnostic_code,
        )?))),
        UnaryOperator::Negate | UnaryOperator::Move => {
            Err(unsupported_bool_expression_diagnostic(diagnostic_code))
        }
    }
}

fn lower_bool_binary_value(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    match binary.operator {
        BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => {
            lower_bool_logical_value(binary, context, diagnostic_code)
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual
            if expressions_are_lowerable_bool_comparison_operands(
                &binary.left,
                &binary.right,
                context,
            ) =>
        {
            lower_bool_comparison_condition(binary, context, diagnostic_code)
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual
            if expressions_are_lowerable_bool_values(&binary.left, &binary.right, context) =>
        {
            Err(unsupported_bool_comparison_operand_diagnostic(
                diagnostic_code,
            ))
        }
        _ if u8_comparison_is_lowerable(binary, context) => {
            lower_u8_comparison_condition(binary, context, diagnostic_code)
        }
        _ if expressions_are_lowerable_usize_values(&binary.left, &binary.right, context) => {
            lower_usize_comparison_condition(binary, context, diagnostic_code)
        }
        _ => lower_i32_comparison_condition(binary, context, diagnostic_code),
    }
}

fn lower_bool_logical_value(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    let operator = match binary.operator {
        BinaryOperator::LogicalAnd => BoolLogicalOperator::And,
        BinaryOperator::LogicalOr => BoolLogicalOperator::Or,
        _ => return Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    };

    Ok(BoolValue::Logical {
        operator,
        left: Box::new(lower_bool_value(&binary.left, context, diagnostic_code)?),
        right: Box::new(lower_bool_value(&binary.right, context, diagnostic_code)?),
    })
}

fn lower_bool_comparison_condition(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    let operator = match binary.operator {
        BinaryOperator::Equal => BoolComparisonOperator::Equal,
        BinaryOperator::NotEqual => BoolComparisonOperator::NotEqual,
        _ => return Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    };

    Ok(BoolValue::BoolComparison {
        operator,
        left: Box::new(lower_bool_comparison_operand(
            &binary.left,
            context,
            diagnostic_code,
        )?),
        right: Box::new(lower_bool_comparison_operand(
            &binary.right,
            context,
            diagnostic_code,
        )?),
    })
}

fn lower_bool_comparison_operand(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    match expression {
        Expr::BoolLiteral(literal) => match literal.value.as_str() {
            "true" => Ok(BoolValue::Const(true)),
            "false" => Ok(BoolValue::Const(false)),
            _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
        },
        Expr::Identifier(identifier) => context
            .bool_location(&identifier.name)
            .map(BoolValue::Location)
            .ok_or_else(|| unsupported_bool_expression_diagnostic(diagnostic_code)),
        Expr::Group(group) => {
            lower_bool_comparison_operand(&group.expression, context, diagnostic_code)
        }
        _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    }
}

fn lower_i32_comparison_condition(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    let operator = i32_comparison_operator(binary.operator, diagnostic_code)?;

    Ok(BoolValue::I32Comparison {
        operator,
        left: lower_i32_value(&binary.left, context)?,
        right: lower_i32_value(&binary.right, context)?,
    })
}

fn lower_usize_comparison_condition(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    let operator = i32_comparison_operator(binary.operator, diagnostic_code)?;

    Ok(BoolValue::UsizeComparison {
        operator,
        left: lower_usize_value(&binary.left, context)?,
        right: lower_usize_value(&binary.right, context)?,
    })
}

fn i32_comparison_operator(
    operator: BinaryOperator,
    diagnostic_code: &'static str,
) -> Result<I32ComparisonOperator, Vec<Diagnostic>> {
    match operator {
        BinaryOperator::Equal => Ok(I32ComparisonOperator::Equal),
        BinaryOperator::NotEqual => Ok(I32ComparisonOperator::NotEqual),
        BinaryOperator::Less => Ok(I32ComparisonOperator::Less),
        BinaryOperator::LessEqual => Ok(I32ComparisonOperator::LessEqual),
        BinaryOperator::Greater => Ok(I32ComparisonOperator::Greater),
        BinaryOperator::GreaterEqual => Ok(I32ComparisonOperator::GreaterEqual),
        _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    }
}

fn unsupported_i32_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower i32 literals, parameters, arithmetic or shift expressions, and direct tail calls",
    )]
}

fn unsupported_u8_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower u8 literals, parameters, locals, direct tail calls, and indexing into `&str`, `&[u8]`, or `&+[u8]`",
    )]
}

fn unsupported_usize_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower usize literals, parameters, arithmetic or shift expressions, and direct tail calls",
    )]
}

fn unsupported_str_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower string literals and `&str` parameters as `&str` values",
    )]
}

fn unsupported_slice_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower slice parameters and locals as slice values",
    )]
}

fn unsupported_non_tail_call_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower function calls in direct tail return position",
    )]
}

fn unsupported_bool_comparison_operand_diagnostic(
    diagnostic_code: &'static str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 can only lower bool equality/inequality operands that are bool literals or bool locals",
    )]
}

fn unsupported_bool_expression_diagnostic(diagnostic_code: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 can only lower bool literals, bool locals, bool operators, i32, u8, or usize comparisons, and bool equality/inequality over bool literals or bool locals",
    )]
}
