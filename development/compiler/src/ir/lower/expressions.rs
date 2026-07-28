use super::aggregates::{
    aggregate_call_return_layout_from_resolved, aggregate_fields_from_type_expr,
    aggregate_type_layout, lower_aggregate_struct_literal_to_location_with_temporaries,
    push_aggregate_call_instruction, push_fallible_aggregate_call_instruction,
    supported_aggregate_copy_layout,
};
use super::bindings::{lower_assignment, lower_local_binding};
use super::context::{AggregateFieldKind, DropGlue, LoweringContext};
use super::errors::{ErrorPayload, lower_error_payload};
use super::functions::{
    append_scope_end_drops_before_exit, expression_contains_explicit_aggregate_move,
    expression_contains_explicit_aggregate_move_outside, lower_aggregate_return_expression,
    lower_direct_aggregate_return_with_scope_drops, lower_never_expression_with_scope_drops,
    lower_scope_end_drops_for_locals_since, lower_value_return_with_scope_drops,
    mark_explicit_moves_in_expression, mark_lowered_statement_aggregate_uses,
    payloadless_if_is_as_if_statement, payloadless_switch_as_if_statement,
    propagating_failure_mode,
};
use super::literals::{
    lower_i32_literal, lower_str_literal, lower_u8_literal, lower_usize_literal,
};
use super::types::scalar_or_view_type_from_type_expr;
mod calls;
mod predicates;
mod temporaries;

use crate::abi::{
    AbiType, ValueLayout, abi_value_from_type_expr, abi_value_from_type_expr_with_resolver,
    array_element_stride,
};
use crate::ast::{
    BinaryExpr, BinaryOperator, Block, CallExpr, CatchExpr, Expr, IfStmt, IndexExpr, Stmt,
    SwitchStmt, TypeConversionExpr, UnaryExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolComparisonOperator, BoolLocation, BoolLogicalOperator, BoolValue,
    BorrowArgument, BorrowSource, FallibleFailureMode, I32ComparisonOperator, I32Location,
    I32Value, Instruction, ScalarArgument, SliceLocation, SliceValue, StrLocation, StrValue, Type,
    U8Location, U8Value, UsizeLocation, UsizeValue,
};
use crate::literals::decode_integer_literal_value;
pub(super) use calls::lower_macos_syscall_primitive_call_to_location;
pub(super) use calls::lower_pointer_address_expression_to_word;
pub(super) use calls::primitive_trap_call;
use calls::{
    call_arguments_require_stack, is_tail_call_stack_pointer_argument,
    lower_addr_primitive_call_to_location, lower_addr_primitive_call_to_word,
    lower_arg_count_raw_primitive_call_to_word, lower_arg_raw_primitive_call_to_value,
    lower_bool_normal_call, lower_call_arguments, lower_close_fd_raw_primitive_call,
    lower_copy_ptr_to_ptr_primitive_call, lower_copy_str_to_ptr_primitive_call,
    lower_direct_tail_call, lower_exit_raw_primitive_call, lower_fallible_void_normal_call,
    lower_from_ref_primitive_call_to_location, lower_from_ref_primitive_call_to_word,
    lower_i32_normal_call, lower_pointee_size_primitive_call_to_word,
    lower_slice_from_raw_parts_primitive_call_to_location, lower_slice_normal_call,
    lower_store_u8_to_ptr_primitive_call, lower_store_value_to_ptr_primitive_call,
    lower_str_bytes_primitive_call_to_location, lower_str_bytes_primitive_call_to_value,
    lower_str_from_raw_parts_primitive_call_to_location, lower_str_normal_call,
    lower_u8_normal_call, lower_usize_normal_call, lower_void_normal_call, primitive_addr_call,
    primitive_arg_count_raw_call, primitive_arg_raw_call, primitive_bytes_from_str_call,
    primitive_close_fd_raw_call, primitive_copy_ptr_to_ptr_call, primitive_copy_str_to_ptr_call,
    primitive_exit_raw_call, primitive_from_ref_call, primitive_pointee_size_call,
    primitive_slice_from_raw_parts_call, primitive_store_u8_to_ptr_call,
    primitive_store_value_to_ptr_call, primitive_str_from_raw_parts_call,
    primitive_write_bytes_raw_call, primitive_write_text_raw_call,
};
pub(super) use calls::{
    lower_fallible_bool_normal_call, lower_fallible_i32_normal_call,
    lower_fallible_slice_normal_call, lower_fallible_str_normal_call,
    lower_fallible_u8_normal_call, lower_fallible_usize_normal_call,
};
use predicates::{
    bool_comparison_contains_call, bool_comparison_needs_temporaries,
    expressions_are_lowerable_bool_comparison_operands, expressions_are_lowerable_bool_values,
    expressions_are_lowerable_usize_values, i32_comparison_needs_temporaries,
    is_i32_binary_operator, is_u8_binary_operator, is_usize_binary_operator,
    str_comparison_is_lowerable, u8_comparison_is_lowerable, usize_comparison_needs_temporaries,
};
pub(super) use predicates::{
    expression_contains_interpolated_string, expression_is_lowerable_bool_binding,
    short_circuit_bool_expression_needs_branch,
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
        Expr::If(statement) => lower_i32_if_expression_to_location(statement, destination, context),
        Expr::IfIs(statement) => {
            let if_statement = payloadless_if_is_as_if_statement(statement, context, "E8008")?;
            lower_i32_if_expression_to_location(&if_statement, destination, context)
        }
        Expr::Match(statement) => lower_i32_match_expression_to_location(
            statement,
            destination,
            context,
            i32_destination_reserved_abi_words(destination),
        ),
        Expr::Unary(unary) if i32_unary_negate_requires_runtime(unary) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_i32_negate_expression_to_location_with_temporaries(
                unary,
                destination,
                context,
                &mut temporaries,
            )
        }
        Expr::Binary(binary) if is_i32_binary_operator(binary.operator) => {
            lower_i32_binary_expression_to_location(binary, destination, context)
        }
        Expr::Index(index) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered = lower_i32_index_expression_to_value(index, context, &mut temporaries)?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetI32 {
                destination,
                value: lowered.value,
            });
            Ok(instructions)
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
        Expr::If(statement) => lower_u8_if_expression_to_location(statement, destination, context),
        Expr::IfIs(statement) => {
            let if_statement = payloadless_if_is_as_if_statement(statement, context, "E8008")?;
            lower_u8_if_expression_to_location(&if_statement, destination, context)
        }
        Expr::Match(statement) => lower_u8_match_expression_to_location(
            statement,
            destination,
            context,
            u8_destination_reserved_abi_words(destination),
        ),
        Expr::Binary(binary) if is_u8_binary_operator(binary.operator) => {
            lower_u8_binary_expression_to_location(binary, destination, context)
        }
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
        Expr::Member(member) => {
            if let Some(tag) = context.payloadless_enum_variant_tag(member) {
                return Ok(vec![Instruction::SetU8 {
                    destination,
                    value: U8Value::Const(tag),
                }]);
            }
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
                return lower_addr_primitive_call_to_location(
                    call,
                    destination,
                    context,
                    &mut temporaries,
                );
            }
            if context.primitive_name_for_call(call) == Some("from_addr") {
                let (mut instructions, value) = lower_pointer_address_expression_to_word(
                    expression,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::SetUsize { destination, value });
                return Ok(instructions);
            }
            if primitive_from_ref_call(call, context) {
                return lower_from_ref_primitive_call_to_location(
                    call,
                    destination,
                    context,
                    &mut temporaries,
                );
            }
            if primitive_pointee_size_call(call, context) {
                let (mut instructions, value) =
                    lower_pointee_size_primitive_call_to_word(call, context, &mut temporaries)?;
                instructions.push(Instruction::SetUsize { destination, value });
                return Ok(instructions);
            }
            if primitive_arg_count_raw_call(call, context) {
                let (mut instructions, value) = lower_arg_count_raw_primitive_call_to_word(call)?;
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
        Expr::If(statement) => {
            lower_usize_if_expression_to_location(statement, destination, context)
        }
        Expr::IfIs(statement) => {
            let if_statement = payloadless_if_is_as_if_statement(statement, context, "E8008")?;
            lower_usize_if_expression_to_location(&if_statement, destination, context)
        }
        Expr::Match(statement) => lower_usize_match_expression_to_location(
            statement,
            destination,
            context,
            usize_destination_reserved_abi_words(destination),
        ),
        Expr::Binary(binary) if is_usize_binary_operator(binary.operator) => {
            lower_usize_binary_expression_to_location(binary, destination, context)
        }
        Expr::Index(index) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered = lower_usize_index_expression_to_value(index, context, &mut temporaries)?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetUsize {
                destination,
                value: lowered.value,
            });
            Ok(instructions)
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
            if primitive_arg_raw_call(call, context) {
                let (mut instructions, value) =
                    lower_arg_raw_primitive_call_to_value(call, context, &mut temporaries)?;
                instructions.push(Instruction::SetStr { destination, value });
                return Ok(instructions);
            }
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
        Expr::If(statement) => lower_str_if_expression_to_location(statement, destination, context),
        Expr::IfIs(statement) => {
            let if_statement = payloadless_if_is_as_if_statement(statement, context, "E8008")?;
            lower_str_if_expression_to_location(&if_statement, destination, context)
        }
        Expr::Match(statement) => lower_str_match_expression_to_location(
            statement,
            destination,
            context,
            str_destination_reserved_abi_words(destination),
        ),
        Expr::Group(group) => {
            lower_str_expression_to_location(&group.expression, destination, context)
        }
        _ => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let value = lower_str_expression_to_value(expression, context, &mut temporaries)?;
            let mut instructions = value.instructions;
            instructions.push(Instruction::SetStr {
                destination,
                value: value.value,
            });
            Ok(instructions)
        }
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
            if primitive_slice_from_raw_parts_call(call, context) {
                return lower_slice_from_raw_parts_primitive_call_to_location(
                    call,
                    destination,
                    context,
                    &mut temporaries,
                );
            }
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
        Expr::If(statement) => {
            lower_slice_if_expression_to_location(statement, destination, context)
        }
        Expr::IfIs(statement) => {
            let if_statement = payloadless_if_is_as_if_statement(statement, context, "E8008")?;
            lower_slice_if_expression_to_location(&if_statement, destination, context)
        }
        Expr::Match(statement) => lower_slice_match_expression_to_location(
            statement,
            destination,
            context,
            slice_destination_reserved_abi_words(destination),
        ),
        Expr::Group(group) => {
            lower_slice_expression_to_location(&group.expression, destination, context)
        }
        _ => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let value = lower_slice_expression_to_value(expression, context, &mut temporaries)?;
            let mut instructions = value.instructions;
            instructions.push(Instruction::SetSlice {
                destination,
                value: value.value,
            });
            Ok(instructions)
        }
    }
}

fn lower_i32_if_expression_to_location(
    statement: &IfStmt,
    destination: I32Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_expression_to_location(statement, context, |expression, branch_context| {
        lower_i32_expression_to_location(expression, destination, branch_context)
    })
}

fn lower_u8_if_expression_to_location(
    statement: &IfStmt,
    destination: U8Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_expression_to_location(statement, context, |expression, branch_context| {
        lower_u8_expression_to_location(expression, destination, branch_context)
    })
}

fn lower_usize_if_expression_to_location(
    statement: &IfStmt,
    destination: UsizeLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_expression_to_location(statement, context, |expression, branch_context| {
        lower_usize_expression_to_location(expression, destination, branch_context)
    })
}

fn lower_bool_if_expression_to_location(
    statement: &IfStmt,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_expression_to_location(statement, context, |expression, branch_context| {
        lower_bool_expression_to_location(expression, destination, branch_context, diagnostic_code)
    })
}

fn lower_str_if_expression_to_location(
    statement: &IfStmt,
    destination: StrLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_expression_to_location(statement, context, |expression, branch_context| {
        lower_str_expression_to_location(expression, destination, branch_context)
    })
}

fn lower_slice_if_expression_to_location(
    statement: &IfStmt,
    destination: SliceLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_expression_to_location(statement, context, |expression, branch_context| {
        lower_slice_expression_to_location(expression, destination, branch_context)
    })
}

fn lower_i32_match_expression_to_location(
    statement: &SwitchStmt,
    destination: I32Location,
    context: &LoweringContext,
    reserved_local_abi_words: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_match_expression_to_location(
        statement,
        context,
        reserved_local_abi_words,
        "E8008",
        |if_statement, switch_context| {
            lower_i32_if_expression_to_location(if_statement, destination, switch_context)
        },
    )
}

fn lower_u8_match_expression_to_location(
    statement: &SwitchStmt,
    destination: U8Location,
    context: &LoweringContext,
    reserved_local_abi_words: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_match_expression_to_location(
        statement,
        context,
        reserved_local_abi_words,
        "E8008",
        |if_statement, switch_context| {
            lower_u8_if_expression_to_location(if_statement, destination, switch_context)
        },
    )
}

fn lower_usize_match_expression_to_location(
    statement: &SwitchStmt,
    destination: UsizeLocation,
    context: &LoweringContext,
    reserved_local_abi_words: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_match_expression_to_location(
        statement,
        context,
        reserved_local_abi_words,
        "E8008",
        |if_statement, switch_context| {
            lower_usize_if_expression_to_location(if_statement, destination, switch_context)
        },
    )
}

fn lower_bool_match_expression_to_location(
    statement: &SwitchStmt,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    reserved_local_abi_words: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_match_expression_to_location(
        statement,
        context,
        reserved_local_abi_words,
        diagnostic_code,
        |if_statement, switch_context| {
            lower_bool_if_expression_to_location(
                if_statement,
                destination,
                switch_context,
                diagnostic_code,
            )
        },
    )
}

fn lower_str_match_expression_to_location(
    statement: &SwitchStmt,
    destination: StrLocation,
    context: &LoweringContext,
    reserved_local_abi_words: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_match_expression_to_location(
        statement,
        context,
        reserved_local_abi_words,
        "E8008",
        |if_statement, switch_context| {
            lower_str_if_expression_to_location(if_statement, destination, switch_context)
        },
    )
}

fn lower_slice_match_expression_to_location(
    statement: &SwitchStmt,
    destination: SliceLocation,
    context: &LoweringContext,
    reserved_local_abi_words: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_match_expression_to_location(
        statement,
        context,
        reserved_local_abi_words,
        "E8008",
        |if_statement, switch_context| {
            lower_slice_if_expression_to_location(if_statement, destination, switch_context)
        },
    )
}

fn lower_match_expression_to_location(
    statement: &SwitchStmt,
    context: &LoweringContext,
    reserved_local_abi_words: usize,
    diagnostic_code: &'static str,
    lower_if: impl FnOnce(&IfStmt, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut switch_context = context.with_reserved_local_abi_words(reserved_local_abi_words);
    let switch =
        payloadless_switch_as_if_statement(statement, &mut switch_context, diagnostic_code)?;
    let mut instructions = switch.leading_instructions;
    instructions.extend(lower_if(&switch.if_statement, &switch_context)?);
    Ok(instructions)
}

fn lower_i32_if_expression_to_value(
    statement: &IfStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredI32Value, Vec<Diagnostic>> {
    let temporary = temporaries.next_i32()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredI32Value {
        instructions: lower_i32_if_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: I32Value::Location(temporary),
    })
}

fn lower_u8_if_expression_to_value(
    statement: &IfStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredU8Value, Vec<Diagnostic>> {
    let temporary = temporaries.next_u8()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredU8Value {
        instructions: lower_u8_if_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: U8Value::Location(temporary),
    })
}

fn lower_usize_if_expression_to_value(
    statement: &IfStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_usize()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredUsizeValue {
        instructions: lower_usize_if_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: UsizeValue::Location(temporary),
    })
}

fn lower_bool_if_expression_to_value(
    statement: &IfStmt,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_bool()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredBoolValue {
        instructions: lower_bool_if_expression_to_location(
            statement,
            temporary,
            &expression_context,
            diagnostic_code,
        )?,
        value: BoolValue::Location(temporary),
    })
}

fn lower_str_if_expression_to_value(
    statement: &IfStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_str()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredStrValue {
        instructions: lower_str_if_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: StrValue::Location(temporary),
    })
}

fn lower_slice_if_expression_to_value(
    statement: &IfStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredSliceValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_slice()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredSliceValue {
        instructions: lower_slice_if_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: SliceValue::Location(temporary),
    })
}

fn lower_i32_match_expression_to_value(
    statement: &SwitchStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredI32Value, Vec<Diagnostic>> {
    let temporary = temporaries.next_i32()?;
    Ok(LoweredI32Value {
        instructions: lower_i32_match_expression_to_location(
            statement,
            temporary,
            context,
            temporaries.reserved_local_abi_words(context)?,
        )?,
        value: I32Value::Location(temporary),
    })
}

fn lower_u8_match_expression_to_value(
    statement: &SwitchStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredU8Value, Vec<Diagnostic>> {
    let temporary = temporaries.next_u8()?;
    Ok(LoweredU8Value {
        instructions: lower_u8_match_expression_to_location(
            statement,
            temporary,
            context,
            temporaries.reserved_local_abi_words(context)?,
        )?,
        value: U8Value::Location(temporary),
    })
}

fn lower_usize_match_expression_to_value(
    statement: &SwitchStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_usize()?;
    Ok(LoweredUsizeValue {
        instructions: lower_usize_match_expression_to_location(
            statement,
            temporary,
            context,
            temporaries.reserved_local_abi_words(context)?,
        )?,
        value: UsizeValue::Location(temporary),
    })
}

fn lower_bool_match_expression_to_value(
    statement: &SwitchStmt,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_bool()?;
    Ok(LoweredBoolValue {
        instructions: lower_bool_match_expression_to_location(
            statement,
            temporary,
            context,
            diagnostic_code,
            temporaries.reserved_local_abi_words(context)?,
        )?,
        value: BoolValue::Location(temporary),
    })
}

fn lower_str_match_expression_to_value(
    statement: &SwitchStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_str()?;
    Ok(LoweredStrValue {
        instructions: lower_str_match_expression_to_location(
            statement,
            temporary,
            context,
            temporaries.reserved_local_abi_words(context)?,
        )?,
        value: StrValue::Location(temporary),
    })
}

fn lower_slice_match_expression_to_value(
    statement: &SwitchStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredSliceValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_slice()?;
    Ok(LoweredSliceValue {
        instructions: lower_slice_match_expression_to_location(
            statement,
            temporary,
            context,
            temporaries.reserved_local_abi_words(context)?,
        )?,
        value: SliceValue::Location(temporary),
    })
}

fn lower_if_expression_to_location(
    statement: &IfStmt,
    context: &LoweringContext,
    lower_result: impl Fn(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_value_control_expression_diagnostic());
    };
    if expression_contains_explicit_aggregate_move(&statement.condition, context) {
        return Err(unsupported_value_control_expression_diagnostic());
    }

    let condition = lower_bool_expression_to_value(&statement.condition, context, "E8008")?;
    let mut instructions = condition.instructions;
    instructions.push(Instruction::If {
        condition: condition.value,
        then_instructions: lower_value_control_block_to_location(
            &statement.then_block,
            context,
            &lower_result,
        )?,
        else_instructions: lower_value_control_block_to_location(
            else_block,
            context,
            &lower_result,
        )?,
    });
    Ok(instructions)
}

fn lower_value_control_block_to_location(
    block: &Block,
    context: &LoweringContext,
    lower_result: &impl Fn(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(result) = block.result.as_deref() else {
        return Err(unsupported_value_control_expression_diagnostic());
    };
    let mut branch_context = context.clone();
    let local_mark = branch_context.local_mark();
    let (mut instructions, ends_execution) =
        lower_value_control_leading_statements(&block.statements, &mut branch_context, local_mark)?;
    if ends_execution {
        return Ok(instructions);
    }
    if expression_contains_explicit_aggregate_move_outside(result, &branch_context, local_mark) {
        return Err(unsupported_value_control_expression_diagnostic());
    }
    instructions.extend(lower_result(result, &branch_context)?);
    mark_explicit_moves_in_expression(result, &mut branch_context);
    instructions.extend(lower_scope_end_drops_for_locals_since(
        &mut branch_context,
        local_mark,
    )?);
    Ok(instructions)
}

fn lower_value_control_leading_statements(
    statements: &[Stmt],
    context: &mut LoweringContext,
    local_mark: usize,
) -> Result<(Vec<Instruction>, bool), Vec<Diagnostic>> {
    let mut instructions = Vec::new();

    for statement in statements {
        match statement {
            Stmt::Import(_) | Stmt::FromImport(_) => {}
            Stmt::Binding(statement) => {
                if expression_contains_explicit_aggregate_move_outside(
                    &statement.initializer,
                    context,
                    local_mark,
                ) {
                    return Err(unsupported_value_control_expression_diagnostic());
                }
                instructions.extend(lower_local_binding(statement, context)?);
            }
            Stmt::Assignment(statement) => {
                if expression_contains_explicit_aggregate_move_outside(
                    &statement.value,
                    context,
                    local_mark,
                ) {
                    return Err(unsupported_value_control_expression_diagnostic());
                }
                instructions.extend(lower_assignment(statement, context)?);
            }
            Stmt::Expression(statement) => {
                if expression_contains_explicit_aggregate_move_outside(
                    &statement.expression,
                    context,
                    local_mark,
                ) {
                    return Err(unsupported_value_control_expression_diagnostic());
                }
                if let Some(terminating_instructions) =
                    lower_never_expression_with_scope_drops(&statement.expression, context)?
                {
                    instructions.extend(terminating_instructions);
                    mark_explicit_moves_in_expression(&statement.expression, context);
                    return Ok((instructions, true));
                }
                let Some(void_instructions) =
                    lower_void_expression_statement(&statement.expression, context)?
                else {
                    return Err(unsupported_value_control_expression_diagnostic());
                };
                instructions.extend(void_instructions);
            }
            Stmt::Drop(_)
            | Stmt::Return(_)
            | Stmt::If(_)
            | Stmt::IfIs(_)
            | Stmt::Switch(_)
            | Stmt::ForRange(_)
            | Stmt::While(_)
            | Stmt::Loop(_)
            | Stmt::Break(_)
            | Stmt::Continue(_) => return Err(unsupported_value_control_expression_diagnostic()),
        }
        mark_lowered_statement_aggregate_uses(statement, context);
    }

    Ok((instructions, false))
}

fn unsupported_value_control_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        "IR v0 can only lower value control expressions with `else`, a final expression in every branch, and supported leading statements",
    )]
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
            if primitive_copy_ptr_to_ptr_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                return lower_copy_ptr_to_ptr_primitive_call(call, context, &mut temporaries)
                    .map(Some);
            }
            if primitive_store_u8_to_ptr_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                return lower_store_u8_to_ptr_primitive_call(call, context, &mut temporaries)
                    .map(Some);
            }
            if primitive_store_value_to_ptr_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                return lower_store_value_to_ptr_primitive_call(call, context, &mut temporaries)
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

            let mut temporaries = TemporaryAllocator::new(context)?;
            match context.call_return_type(&target) {
                Some(Type::Void) => lower_void_normal_call(call, context, &mut temporaries),
                Some(Type::I32) => {
                    let destination = temporaries.next_i32()?;
                    lower_i32_normal_call(call, destination, context, &mut temporaries)
                }
                Some(Type::U8) => {
                    let destination = temporaries.next_u8()?;
                    lower_u8_normal_call(call, destination, context, &mut temporaries)
                }
                Some(Type::Usize) => {
                    let destination = temporaries.next_usize()?;
                    lower_usize_normal_call(call, destination, context, &mut temporaries)
                }
                Some(Type::Bool) => {
                    let destination = temporaries.next_bool()?;
                    lower_bool_normal_call(call, destination, context, &mut temporaries)
                }
                Some(Type::Str) => {
                    let destination = temporaries.next_str()?;
                    lower_str_normal_call(call, destination, context, &mut temporaries)
                }
                Some(Type::Slice { .. }) => {
                    let destination = temporaries.next_slice()?;
                    lower_slice_normal_call(call, destination, context, &mut temporaries)
                }
                Some(Type::Aggregate { .. } | Type::DirectAggregate { .. }) => {
                    lower_aggregate_normal_call_statement(call, context, &mut temporaries)
                }
                _ => return Ok(None),
            }
            .map(Some)
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
            lower_catch_failure_mode(
                catch,
                context,
                discarded_fallible_statement_reserved_abi_words(&catch.expression, context)
                    .unwrap_or(0),
            )?,
        ),
        Expr::StructLiteral(literal) => {
            lower_aggregate_struct_literal_statement(literal, context).map(Some)
        }
        _ => Ok(None),
    }
}

fn lower_aggregate_struct_literal_statement(
    literal: &crate::ast::StructLiteralExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_aggregate_literal_statement_diagnostic());
    };
    let value = abi_value_from_type_expr(&literal.ty, resolved)
        .map_err(|_error| unsupported_aggregate_literal_statement_diagnostic())?;
    if !supported_aggregate_copy_layout(value.layout) {
        return Err(unsupported_aggregate_literal_statement_diagnostic());
    }

    let drop_glue = context.drop_glue_for_type_expr(&literal.ty);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: value.layout,
    }];
    instructions.extend(lower_aggregate_struct_literal_to_location_with_temporaries(
        literal,
        value.layout,
        AggregateLocation::Slot(slot_index),
        "E8007",
        "expression statements",
        resolved,
        context,
        &mut temporaries,
    )?);
    append_discarded_aggregate_drop(
        &mut instructions,
        drop_glue,
        value.layout,
        slot_index,
        context,
    )?;
    Ok(instructions)
}

fn lower_fallible_void_expression_statement(
    expression: &Expr,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if primitive_write_text_raw_call(call, context)
                || primitive_write_bytes_raw_call(call, context)
            {
                let mut temporaries = TemporaryAllocator::new(context)?;
                return lower_fallible_void_normal_call(
                    call,
                    context,
                    &mut temporaries,
                    failure_mode,
                )
                .map(Some);
            }

            let Some((target, _call_name)) = context.direct_call_target_and_name(call) else {
                return Ok(None);
            };
            let Some(Type::Fallible(success_type)) = context.call_return_type(&target).cloned()
            else {
                return Ok(None);
            };

            let mut temporaries = TemporaryAllocator::new(context)?;
            match success_type.as_ref() {
                Type::Void => {
                    lower_fallible_void_normal_call(call, context, &mut temporaries, failure_mode)
                }
                Type::I32 => {
                    let destination = temporaries.next_i32()?;
                    lower_fallible_i32_normal_call(
                        call,
                        destination,
                        context,
                        &mut temporaries,
                        failure_mode,
                    )
                }
                Type::U8 => {
                    let destination = temporaries.next_u8()?;
                    lower_fallible_u8_normal_call(
                        call,
                        destination,
                        context,
                        &mut temporaries,
                        failure_mode,
                    )
                }
                Type::Usize => {
                    let destination = temporaries.next_usize()?;
                    lower_fallible_usize_normal_call(
                        call,
                        destination,
                        context,
                        &mut temporaries,
                        failure_mode,
                    )
                }
                Type::Bool => {
                    let destination = temporaries.next_bool()?;
                    lower_fallible_bool_normal_call(
                        call,
                        destination,
                        context,
                        &mut temporaries,
                        failure_mode,
                    )
                }
                Type::Str => {
                    let destination = temporaries.next_str()?;
                    lower_fallible_str_normal_call(
                        call,
                        destination,
                        context,
                        &mut temporaries,
                        failure_mode,
                    )
                }
                Type::Slice { .. } => {
                    let destination = temporaries.next_slice()?;
                    lower_fallible_slice_normal_call(
                        call,
                        destination,
                        context,
                        &mut temporaries,
                        failure_mode,
                    )
                }
                Type::Aggregate { .. } | Type::DirectAggregate { .. } => {
                    lower_aggregate_fallible_call_statement(
                        call,
                        success_type.as_ref(),
                        context,
                        &mut temporaries,
                        failure_mode,
                    )
                }
                _ => return Ok(None),
            }
            .map(Some)
        }
        Expr::Group(group) => {
            lower_fallible_void_expression_statement(&group.expression, context, failure_mode)
        }
        _ => Ok(None),
    }
}

fn lower_aggregate_normal_call_statement(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(return_type_expr) = context.call_return_type_expr(call) else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    let drop_glue = context.drop_glue_for_type_expr(&return_type_expr);
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    let Some(layout) = aggregate_type_layout(&return_type) else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    if !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    }

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    if macos_syscall_primitive_call(call, context) {
        let Some(mut syscall_instructions) = lower_macos_syscall_primitive_call_to_location(
            call,
            AggregateLocation::Slot(slot_index),
            layout,
            context,
            temporaries,
        )?
        else {
            return Err(unsupported_aggregate_call_statement_diagnostic());
        };
        instructions.append(&mut syscall_instructions);
    } else {
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
    }
    append_discarded_aggregate_drop(&mut instructions, drop_glue, layout, slot_index, context)?;
    Ok(instructions)
}

fn lower_aggregate_fallible_call_statement(
    call: &CallExpr,
    success_type: &Type,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(return_type_expr) = context.call_return_type_expr(call) else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    let drop_glue = context.drop_glue_for_type_expr(&return_type_expr);
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    let Some(layout) = aggregate_type_layout(success_type) else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    if !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    }

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    let (mut argument_instructions, arguments) =
        lower_call_arguments(call, &target, &call_name, context, temporaries)?;
    instructions.append(&mut argument_instructions);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success_type,
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
        failure_mode,
    );
    append_discarded_aggregate_drop(&mut instructions, drop_glue, layout, slot_index, context)?;
    Ok(instructions)
}

fn append_discarded_aggregate_drop(
    instructions: &mut Vec<Instruction>,
    drop_glue: Option<DropGlue>,
    layout: ValueLayout,
    slot_index: usize,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(drop_glue) = drop_glue else {
        return Ok(());
    };
    let Some(parameter_types) = context.call_parameter_types(&drop_glue.target) else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    if parameter_types.len() != 1
        || !drop_parameter_matches_aggregate_slot(&parameter_types[0], layout)
    {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    }

    instructions.push(Instruction::CallVoid {
        target: drop_glue.target,
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(slot_index),
        })],
    });
    Ok(())
}

fn drop_parameter_matches_aggregate_slot(parameter_type: &Type, layout: ValueLayout) -> bool {
    let Type::Borrow {
        is_readwrite: true,
        inner,
    } = parameter_type
    else {
        return false;
    };

    match inner.as_ref() {
        Type::Aggregate {
            layout: parameter_layout,
        }
        | Type::DirectAggregate {
            layout: parameter_layout,
            ..
        } => *parameter_layout == layout,
        _ => false,
    }
}

fn unsupported_aggregate_call_statement_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        "IR v0 cannot lower discarded aggregate call statement",
    )]
}

fn unsupported_aggregate_literal_statement_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        "IR v0 cannot lower discarded aggregate literal statement",
    )]
}

fn discarded_fallible_statement_reserved_abi_words(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<usize> {
    match expression {
        Expr::Call(call) if primitive_write_text_raw_call(call, context) => Some(0),
        Expr::Call(call) if primitive_write_bytes_raw_call(call, context) => Some(0),
        Expr::Call(call) => {
            let (target, _call_name) = context.direct_call_target_and_name(call)?;
            let Type::Fallible(success_type) = context.call_return_type(&target)? else {
                return None;
            };
            discarded_fallible_success_reserved_abi_words(success_type.as_ref())
        }
        Expr::Group(group) => {
            discarded_fallible_statement_reserved_abi_words(&group.expression, context)
        }
        _ => None,
    }
}

fn discarded_fallible_success_reserved_abi_words(success_type: &Type) -> Option<usize> {
    match success_type {
        Type::Void => Some(0),
        Type::I32 | Type::U8 | Type::Usize | Type::Bool => Some(1),
        Type::Str | Type::Slice { .. } => Some(2),
        _ => None,
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
                && context.function_returns_optional()
                && expression_is_none_literal(expression)
            {
                instructions.extend(append_scope_end_drops_before_exit(
                    vec![Instruction::ReturnOptionalNone],
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

            if let Some(expression) = &statement.expression
                && matches!(success_type, Type::DirectAggregate { .. })
                && !context.pending_aggregate_drops().is_empty()
            {
                let Some((_root_source, resolved)) = context.resolved_calls() else {
                    return Err(unsupported_catch_block_diagnostic());
                };
                let function_name = context.function_name().to_string();
                instructions.extend(lower_direct_aggregate_return_with_scope_drops(
                    expression,
                    &success_type,
                    &function_return_type,
                    &function_name,
                    resolved,
                    context,
                )?);
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
                (Type::Aggregate { .. } | Type::DirectAggregate { .. }, Some(expression)) => {
                    let Some((_root_source, resolved)) = context.resolved_calls() else {
                        return Err(unsupported_catch_block_diagnostic());
                    };
                    let function_name = context.function_name().to_string();
                    lower_aggregate_return_expression(
                        expression,
                        &success_type,
                        &function_name,
                        resolved,
                        context,
                    )
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
                | (Type::Aggregate { .. }, None)
                | (Type::DirectAggregate { .. }, None)
                | (Type::Borrow { .. }, _)
                | (Type::Error, _)
                | (Type::Never, None) => Err(unsupported_catch_block_diagnostic()),
                (Type::Fallible(_), _) => Err(unsupported_catch_block_diagnostic()),
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
            Stmt::Import(_) | Stmt::FromImport(_) => {}
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
    payload.into_return_instructions()
}

fn expression_is_none_literal(expression: &Expr) -> bool {
    matches!(unwrap_group(expression), Expr::NoneLiteral(_))
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
        "IR v0 can only lower catch blocks containing leading scalar local bindings, scalar assignments, or effect-only call statements followed by `return`",
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
        Expr::If(statement) => lower_i32_if_expression_to_value(statement, context, temporaries),
        Expr::IfIs(statement) => {
            let if_statement = payloadless_if_is_as_if_statement(statement, context, "E8008")?;
            lower_i32_if_expression_to_value(&if_statement, context, temporaries)
        }
        Expr::Match(statement) => {
            lower_i32_match_expression_to_value(statement, context, temporaries)
        }
        Expr::Unary(unary) if i32_unary_negate_requires_runtime(unary) => {
            let destination = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_i32_negate_expression_to_location_with_temporaries(
                    unary,
                    destination,
                    context,
                    temporaries,
                )?,
                value: I32Value::Location(destination),
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
        Expr::Index(index) => lower_i32_index_expression_to_value(index, context, temporaries),
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

fn lower_i32_negate_expression_to_location_with_temporaries(
    unary: &UnaryExpr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let right = lower_i32_expression_to_value(&unary.operand, context, temporaries)?;
    let mut instructions = right.instructions;
    instructions.push(Instruction::SubtractI32 {
        destination,
        left: I32Value::Const(0),
        right: right.value,
    });
    Ok(instructions)
}

fn i32_unary_negate_requires_runtime(unary: &UnaryExpr) -> bool {
    unary.operator == UnaryOperator::Negate
        && !expression_is_unsigned_integer_literal(&unary.operand)
}

fn expression_is_unsigned_integer_literal(expression: &Expr) -> bool {
    match expression {
        Expr::IntegerLiteral(_) => true,
        Expr::Group(group) => expression_is_unsigned_integer_literal(&group.expression),
        _ => false,
    }
}

pub(super) fn lower_i32_expression_to_word(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, I32Value), Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_i32_expression_to_word_with_temporaries(expression, context, &mut temporaries)
}

pub(super) fn lower_i32_expression_to_word_with_temporaries(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, I32Value), Vec<Diagnostic>> {
    let lowered = lower_i32_expression_to_value(expression, context, temporaries)?;
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
        Expr::If(statement) => lower_u8_if_expression_to_value(statement, context, temporaries),
        Expr::IfIs(statement) => {
            let if_statement = payloadless_if_is_as_if_statement(statement, context, "E8008")?;
            lower_u8_if_expression_to_value(&if_statement, context, temporaries)
        }
        Expr::Match(statement) => {
            lower_u8_match_expression_to_value(statement, context, temporaries)
        }
        Expr::Binary(binary) if is_u8_binary_operator(binary.operator) => {
            let temporary = temporaries.next_u8()?;
            Ok(LoweredU8Value {
                instructions: lower_u8_binary_expression_to_location_with_temporaries(
                    binary,
                    temporary,
                    context,
                    temporaries,
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
        Expr::Member(member) => {
            if let Some(tag) = context.payloadless_enum_variant_tag(member) {
                return Ok(LoweredU8Value {
                    instructions: Vec::new(),
                    value: U8Value::Const(tag),
                });
            }
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

fn lower_u8_binary_expression_to_location(
    binary: &BinaryExpr,
    destination: U8Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_u8_binary_expression_to_location_with_temporaries(
        binary,
        destination,
        context,
        &mut temporaries,
    )
}

fn lower_u8_binary_expression_to_location_with_temporaries(
    binary: &BinaryExpr,
    destination: U8Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let left = lower_u8_expression_to_value(&binary.left, context, temporaries)?;
    let right = lower_u8_expression_to_value(&binary.right, context, temporaries)?;
    let mut instructions = left.instructions;
    instructions.extend(right.instructions);
    instructions.push(u8_binary_instruction(
        binary.operator,
        destination,
        left.value,
        right.value,
    )?);
    Ok(instructions)
}

pub(super) fn lower_u8_expression_to_word(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, U8Value), Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_u8_expression_to_word_with_temporaries(expression, context, &mut temporaries)
}

pub(super) fn lower_u8_expression_to_word_with_temporaries(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, U8Value), Vec<Diagnostic>> {
    let lowered = lower_u8_expression_to_value(expression, context, temporaries)?;
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
            if context.primitive_name_for_call(call) == Some("from_addr") {
                let (instructions, value) =
                    lower_pointer_address_expression_to_word(expression, context, temporaries)?;
                return Ok(LoweredUsizeValue {
                    instructions,
                    value,
                });
            }
            if primitive_from_ref_call(call, context) {
                let (instructions, value) =
                    lower_from_ref_primitive_call_to_word(call, context, temporaries)?;
                return Ok(LoweredUsizeValue {
                    instructions,
                    value,
                });
            }
            if primitive_pointee_size_call(call, context) {
                let (instructions, value) =
                    lower_pointee_size_primitive_call_to_word(call, context, temporaries)?;
                return Ok(LoweredUsizeValue {
                    instructions,
                    value,
                });
            }
            if primitive_arg_count_raw_call(call, context) {
                let (instructions, value) = lower_arg_count_raw_primitive_call_to_word(call)?;
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
        Expr::If(statement) => lower_usize_if_expression_to_value(statement, context, temporaries),
        Expr::IfIs(statement) => {
            let if_statement = payloadless_if_is_as_if_statement(statement, context, "E8008")?;
            lower_usize_if_expression_to_value(&if_statement, context, temporaries)
        }
        Expr::Match(statement) => {
            lower_usize_match_expression_to_value(statement, context, temporaries)
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
        Expr::Index(index) => lower_usize_index_expression_to_value(index, context, temporaries),
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

pub(super) fn lower_str_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if primitive_arg_raw_call(call, context) {
                let (instructions, value) =
                    lower_arg_raw_primitive_call_to_value(call, context, temporaries)?;
                return Ok(LoweredStrValue {
                    instructions,
                    value,
                });
            }
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
        Expr::Match(statement) => {
            lower_str_match_expression_to_value(statement, context, temporaries)
        }
        Expr::If(statement) => lower_str_if_expression_to_value(statement, context, temporaries),
        Expr::IfIs(statement) => {
            let if_statement = payloadless_if_is_as_if_statement(statement, context, "E8008")?;
            lower_str_if_expression_to_value(&if_statement, context, temporaries)
        }
        Expr::Member(_) => lower_aggregate_str_field_to_value(expression, context, temporaries),
        Expr::Index(index) => lower_str_index_expression_to_value(index, context, temporaries),
        Expr::Group(group) => {
            lower_str_expression_to_value(&group.expression, context, temporaries)
        }
        _ => Ok(LoweredStrValue {
            instructions: Vec::new(),
            value: lower_str_value(expression, context)?,
        }),
    }
}

pub(super) fn lower_slice_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredSliceValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if primitive_slice_from_raw_parts_call(call, context) {
                let temporary = temporaries.next_slice()?;
                let instructions = lower_slice_from_raw_parts_primitive_call_to_location(
                    call,
                    temporary,
                    context,
                    temporaries,
                )?;
                return Ok(LoweredSliceValue {
                    instructions,
                    value: SliceValue::Location(temporary),
                });
            }
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
        Expr::Match(statement) => {
            lower_slice_match_expression_to_value(statement, context, temporaries)
        }
        Expr::If(statement) => lower_slice_if_expression_to_value(statement, context, temporaries),
        Expr::IfIs(statement) => {
            let if_statement = payloadless_if_is_as_if_statement(statement, context, "E8008")?;
            lower_slice_if_expression_to_value(&if_statement, context, temporaries)
        }
        Expr::Member(_) => lower_aggregate_slice_field_to_value(expression, context, temporaries),
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

fn u8_binary_instruction(
    operator: BinaryOperator,
    destination: U8Location,
    left: U8Value,
    right: U8Value,
) -> Result<Instruction, Vec<Diagnostic>> {
    match operator {
        BinaryOperator::Add => Ok(Instruction::AddU8 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Subtract => Ok(Instruction::SubtractU8 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Multiply => Ok(Instruction::MultiplyU8 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Divide => Ok(Instruction::DivideU8 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Remainder => Ok(Instruction::RemainderU8 {
            destination,
            left,
            right,
        }),
        BinaryOperator::ShiftLeft => Ok(Instruction::ShiftLeftU8 {
            destination,
            left,
            right,
        }),
        BinaryOperator::ShiftRight => Ok(Instruction::ShiftRightU8 {
            destination,
            left,
            right,
        }),
        _ => Err(unsupported_u8_expression_diagnostic()),
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
        Expr::Call(call) if primitive_exit_raw_call(call, context) => {
            lower_exit_raw_primitive_call(call, context).map(Some)
        }
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
                let mut instructions = lower_addr_primitive_call_to_location(
                    call,
                    UsizeLocation::Return,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if context.primitive_name_for_call(call) == Some("from_addr") {
                let (mut instructions, value) = lower_pointer_address_expression_to_word(
                    expression,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value,
                });
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if primitive_from_ref_call(call, context) {
                let mut instructions = lower_from_ref_primitive_call_to_location(
                    call,
                    UsizeLocation::Return,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if primitive_pointee_size_call(call, context) {
                let (mut instructions, value) =
                    lower_pointee_size_primitive_call_to_word(call, context, &mut temporaries)?;
                instructions.push(Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value,
                });
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if primitive_arg_count_raw_call(call, context) {
                let (mut instructions, value) = lower_arg_count_raw_primitive_call_to_word(call)?;
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
            if primitive_arg_raw_call(call, context) {
                let (mut instructions, value) =
                    lower_arg_raw_primitive_call_to_value(call, context, &mut temporaries)?;
                instructions.push(Instruction::SetStr {
                    destination: StrLocation::Return,
                    value,
                });
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
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
            if primitive_slice_from_raw_parts_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                let mut instructions = lower_slice_from_raw_parts_primitive_call_to_location(
                    call,
                    SliceLocation::Return,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
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
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            if let Some(value) =
                lower_builtin_is_empty_call_to_value(call, context, &mut temporaries)
            {
                let lowered = value?;
                let mut instructions = lowered.instructions;
                instructions.push(Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: lowered.value,
                });
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }

            lower_direct_tail_call(call, context)
        }
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
        Expr::Binary(binary) if short_circuit_bool_expression_needs_branch(binary, context) => {
            lower_short_circuit_bool_expression_to_location(
                binary,
                destination,
                context,
                diagnostic_code,
            )
        }
        Expr::Binary(binary) if str_comparison_is_lowerable(binary, context) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let comparison = lower_str_comparison_to_value_with_temporaries(
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
        Expr::Binary(binary) if usize_comparison_needs_temporaries(binary, context) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let comparison = lower_usize_comparison_to_value_with_temporaries(
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
            if let Some(value) =
                lower_builtin_is_empty_call_to_value(call, context, &mut temporaries)
            {
                let lowered = value?;
                let mut instructions = lowered.instructions;
                instructions.push(Instruction::SetBool {
                    destination,
                    value: lowered.value,
                });
                return Ok(instructions);
            }
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
        Expr::If(statement) => {
            lower_bool_if_expression_to_location(statement, destination, context, diagnostic_code)
        }
        Expr::IfIs(statement) => {
            let if_statement =
                payloadless_if_is_as_if_statement(statement, context, diagnostic_code)?;
            lower_bool_if_expression_to_location(
                &if_statement,
                destination,
                context,
                diagnostic_code,
            )
        }
        Expr::Match(statement) => lower_bool_match_expression_to_location(
            statement,
            destination,
            context,
            diagnostic_code,
            bool_destination_reserved_abi_words(destination),
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
        Expr::Index(index) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered = lower_bool_index_expression_to_value(
                index,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: lowered.value,
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

fn lower_short_circuit_bool_expression_to_value_with_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_bool()?;
    Ok(LoweredBoolValue {
        instructions: lower_short_circuit_bool_expression_to_location_with_temporaries(
            binary,
            temporary,
            context,
            diagnostic_code,
            temporaries,
        )?,
        value: BoolValue::Location(temporary),
    })
}

fn lower_short_circuit_bool_expression_to_location_with_temporaries(
    binary: &BinaryExpr,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_bool_expression_to_branch_with_temporaries(
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
        temporaries,
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
        && short_circuit_bool_expression_needs_branch(binary, context)
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

fn lower_bool_expression_to_branch_with_temporaries(
    expression: &Expr,
    then_instructions: Vec<Instruction>,
    else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Expr::Binary(binary) = unwrap_group(expression)
        && short_circuit_bool_expression_needs_branch(binary, context)
    {
        return lower_short_circuit_bool_expression_to_branch_with_temporaries(
            binary,
            then_instructions,
            else_instructions,
            context,
            diagnostic_code,
            temporaries,
        );
    }

    let condition = lower_bool_expression_to_value_with_temporaries(
        expression,
        context,
        diagnostic_code,
        temporaries,
    )?;
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

fn lower_short_circuit_bool_expression_to_branch_with_temporaries(
    binary: &BinaryExpr,
    then_instructions: Vec<Instruction>,
    else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match binary.operator {
        BinaryOperator::LogicalAnd => {
            let left = lower_bool_expression_to_value_with_temporaries(
                &binary.left,
                context,
                diagnostic_code,
                temporaries,
            )?;
            let then_instructions = lower_bool_expression_to_branch_with_temporaries(
                &binary.right,
                then_instructions,
                else_instructions.clone(),
                context,
                diagnostic_code,
                temporaries,
            )?;
            let mut instructions = left.instructions;
            instructions.push(Instruction::If {
                condition: left.value,
                then_instructions,
                else_instructions,
            });
            Ok(instructions)
        }
        BinaryOperator::LogicalOr => {
            let left = lower_bool_expression_to_value_with_temporaries(
                &binary.left,
                context,
                diagnostic_code,
                temporaries,
            )?;
            let else_instructions = lower_bool_expression_to_branch_with_temporaries(
                &binary.right,
                then_instructions.clone(),
                else_instructions,
                context,
                diagnostic_code,
                temporaries,
            )?;
            let mut instructions = left.instructions;
            instructions.push(Instruction::If {
                condition: left.value,
                then_instructions,
                else_instructions,
            });
            Ok(instructions)
        }
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

fn lower_aggregate_str_field_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    let access = lower_aggregate_member_field_access(expression, context, temporaries)?
        .filter(|access| access.kind == AggregateFieldKind::Str)
        .ok_or_else(unsupported_str_expression_diagnostic)?;
    let temporary = temporaries.next_str()?;
    let StrLocation::Local(index) = temporary else {
        unreachable!("temporary str locations are local pairs");
    };
    let len_index = index
        .checked_add(1)
        .ok_or_else(unsupported_str_expression_diagnostic)?;
    let len_offset = access
        .offset
        .checked_add(8)
        .ok_or_else(unsupported_str_expression_diagnostic)?;
    let mut instructions = access.instructions;
    instructions.push(Instruction::LoadAggregateUsize {
        destination: UsizeLocation::Local(index),
        source: access.source,
        offset: access.offset,
    });
    instructions.push(Instruction::LoadAggregateUsize {
        destination: UsizeLocation::Local(len_index),
        source: access.source,
        offset: len_offset,
    });
    Ok(LoweredStrValue {
        instructions,
        value: StrValue::Location(temporary),
    })
}

fn lower_aggregate_slice_field_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredSliceValue, Vec<Diagnostic>> {
    let access = lower_aggregate_member_field_access(expression, context, temporaries)?
        .filter(|access| matches!(access.kind, AggregateFieldKind::Slice(_)))
        .ok_or_else(unsupported_slice_expression_diagnostic)?;
    let temporary = temporaries.next_slice()?;
    let SliceLocation::Local(index) = temporary else {
        unreachable!("temporary slice locations are local pairs");
    };
    let len_index = index
        .checked_add(1)
        .ok_or_else(unsupported_slice_expression_diagnostic)?;
    let len_offset = access
        .offset
        .checked_add(8)
        .ok_or_else(unsupported_slice_expression_diagnostic)?;
    let mut instructions = access.instructions;
    instructions.push(Instruction::LoadAggregateUsize {
        destination: UsizeLocation::Local(index),
        source: access.source,
        offset: access.offset,
    });
    instructions.push(Instruction::LoadAggregateUsize {
        destination: UsizeLocation::Local(len_index),
        source: access.source,
        offset: len_offset,
    });
    Ok(LoweredSliceValue {
        instructions,
        value: SliceValue::Location(temporary),
    })
}

pub(super) fn push_store_str_view_to_aggregate_field(
    instructions: &mut Vec<Instruction>,
    destination: AggregateLocation,
    offset: u32,
    value: StrValue,
    temporaries: &mut TemporaryAllocator,
    unsupported_diagnostic: impl Fn() -> Vec<Diagnostic>,
) -> Result<(), Vec<Diagnostic>> {
    let temporary = temporaries.next_str()?;
    let StrLocation::Local(index) = temporary else {
        unreachable!("temporary str locations are local pairs");
    };
    let len_index = index.checked_add(1).ok_or_else(&unsupported_diagnostic)?;
    let len_offset = offset.checked_add(8).ok_or_else(unsupported_diagnostic)?;

    instructions.push(Instruction::SetStr {
        destination: temporary,
        value,
    });
    instructions.push(Instruction::StoreAggregateUsize {
        destination,
        offset,
        value: UsizeValue::Location(UsizeLocation::Local(index)),
    });
    instructions.push(Instruction::StoreAggregateUsize {
        destination,
        offset: len_offset,
        value: UsizeValue::Location(UsizeLocation::Local(len_index)),
    });
    Ok(())
}

pub(super) fn push_store_slice_view_to_aggregate_field(
    instructions: &mut Vec<Instruction>,
    destination: AggregateLocation,
    offset: u32,
    value: SliceValue,
    temporaries: &mut TemporaryAllocator,
    unsupported_diagnostic: impl Fn() -> Vec<Diagnostic>,
) -> Result<(), Vec<Diagnostic>> {
    let temporary = temporaries.next_slice()?;
    let SliceLocation::Local(index) = temporary else {
        unreachable!("temporary slice locations are local pairs");
    };
    let len_index = index.checked_add(1).ok_or_else(&unsupported_diagnostic)?;
    let len_offset = offset.checked_add(8).ok_or_else(unsupported_diagnostic)?;

    instructions.push(Instruction::SetSlice {
        destination: temporary,
        value,
    });
    instructions.push(Instruction::StoreAggregateUsize {
        destination,
        offset,
        value: UsizeValue::Location(UsizeLocation::Local(index)),
    });
    instructions.push(Instruction::StoreAggregateUsize {
        destination,
        offset: len_offset,
        value: UsizeValue::Location(UsizeLocation::Local(len_index)),
    });
    Ok(())
}

pub(super) struct LoweredAggregateFieldAccess {
    pub(super) instructions: Vec<Instruction>,
    pub(super) source: AggregateLocation,
    pub(super) offset: u32,
    pub(super) kind: AggregateFieldKind,
    pub(super) is_readwrite: bool,
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
                is_readwrite: field.is_readwrite,
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

fn aggregate_member_field_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Option<AggregateFieldKind>, Vec<Diagnostic>> {
    let Expr::Member(member) = unwrap_group(expression) else {
        return Ok(None);
    };
    aggregate_member_field_kind_from_member(member, context)
}

pub(super) fn aggregate_member_field_kind_from_member(
    member: &crate::ast::MemberExpr,
    context: &LoweringContext,
) -> Result<Option<AggregateFieldKind>, Vec<Diagnostic>> {
    let Some((root, mut fields)) = aggregate_member_root_and_path(&member.object, context)? else {
        return Ok(None);
    };
    fields.push(member.member.as_str());
    let field_path = fields.join(".");
    Ok(match root {
        AggregateMemberRoot::Identifier(identifier_name) => context
            .aggregate_field(identifier_name, &field_path)
            .map(|field| field.kind),
        AggregateMemberRoot::Call(call) => {
            aggregate_call_member_field_kind(call, &field_path, context)
        }
        AggregateMemberRoot::FallibleCall(call, _) => {
            aggregate_fallible_call_member_field_kind(call, &field_path, context)
        }
    })
}

fn aggregate_call_member_field_kind(
    call: &CallExpr,
    member_name: &str,
    context: &LoweringContext,
) -> Option<AggregateFieldKind> {
    if macos_syscall_primitive_call(call, context)
        && let Some(layout) = aggregate_call_return_layout_from_resolved(call, context)
    {
        if !supported_aggregate_copy_layout(layout) {
            return None;
        }
        return aggregate_call_field(call, member_name, context).map(|field| field.kind);
    }

    let (target, _) = context.direct_call_target_and_name(call)?;
    let layout = aggregate_type_layout(context.call_return_type(&target)?)?;
    if !supported_aggregate_copy_layout(layout) {
        return None;
    }
    aggregate_call_field(call, member_name, context).map(|field| field.kind)
}

fn aggregate_fallible_call_member_field_kind(
    call: &CallExpr,
    member_name: &str,
    context: &LoweringContext,
) -> Option<AggregateFieldKind> {
    let (target, _) = context.direct_call_target_and_name(call)?;
    let Type::Fallible(success_type) = context.call_return_type(&target)? else {
        return None;
    };
    let layout = aggregate_type_layout(success_type.as_ref())?;
    if !supported_aggregate_copy_layout(layout) {
        return None;
    }
    aggregate_call_field(call, member_name, context).map(|field| field.kind)
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
            is_readwrite: false,
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
        is_readwrite: false,
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
        is_readwrite: false,
        is_copy: true,
    }))
}

pub(super) fn aggregate_call_field(
    call: &CallExpr,
    member_name: &str,
    context: &LoweringContext,
) -> Option<super::context::AggregateField> {
    let (root_source, resolved) = context.resolved_calls()?;
    let return_type = context.call_return_type_expr(call)?;
    aggregate_fields_from_type_expr(&return_type, root_source, resolved)?
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

pub(super) fn lower_bool_expression_to_value_with_temporaries(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    match expression {
        Expr::Binary(binary) if short_circuit_bool_expression_needs_branch(binary, context) => {
            lower_short_circuit_bool_expression_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Binary(binary) if str_comparison_is_lowerable(binary, context) => {
            lower_str_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
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
            if let Some(value) = lower_builtin_is_empty_call_to_value(call, context, temporaries) {
                return value;
            }

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
        Expr::If(statement) => {
            lower_bool_if_expression_to_value(statement, context, diagnostic_code, temporaries)
        }
        Expr::IfIs(statement) => {
            let if_statement =
                payloadless_if_is_as_if_statement(statement, context, diagnostic_code)?;
            lower_bool_if_expression_to_value(&if_statement, context, diagnostic_code, temporaries)
        }
        Expr::Match(statement) => {
            lower_bool_match_expression_to_value(statement, context, diagnostic_code, temporaries)
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
        Expr::Index(index) => {
            lower_bool_index_expression_to_value(index, context, diagnostic_code, temporaries)
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
    lower_bool_expression_to_value_with_temporaries(
        expression,
        context,
        diagnostic_code,
        temporaries,
    )
}

fn lower_str_comparison_to_value_with_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let operator = str_comparison_operator(binary.operator, diagnostic_code)?;
    let left = lower_str_expression_to_value(&binary.left, context, temporaries)?;
    let right = lower_str_expression_to_value(&binary.right, context, temporaries)?;

    let mut instructions = left.instructions;
    let left = materialize_computed_str_value(left.value, &mut instructions, temporaries)?;
    instructions.extend(right.instructions);
    let right = materialize_computed_str_value(right.value, &mut instructions, temporaries)?;
    Ok(LoweredBoolValue {
        instructions,
        value: BoolValue::StrComparison {
            operator,
            left,
            right,
        },
    })
}

fn materialize_computed_str_value(
    value: StrValue,
    instructions: &mut Vec<Instruction>,
    temporaries: &mut TemporaryAllocator,
) -> Result<StrValue, Vec<Diagnostic>> {
    match value {
        StrValue::ProcessArg { .. } | StrValue::SliceIndex { .. } => {
            let temporary = temporaries.next_str()?;
            instructions.push(Instruction::SetStr {
                destination: temporary,
                value,
            });
            Ok(StrValue::Location(temporary))
        }
        StrValue::StaticBytes(_) | StrValue::Location(_) => Ok(value),
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
        Expr::Member(member) => context
            .payloadless_enum_variant_tag(member)
            .map(U8Value::Const)
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

pub(super) struct FixedArrayElementAccess {
    pub(super) source: AggregateLocation,
    pub(super) offset: u32,
    pub(super) element: AbiType,
    pub(super) out_of_bounds: bool,
}

struct FixedArrayElementIndexedAccess {
    source: AggregateLocation,
    base_offset: u32,
    index: UsizeValue,
    index_instructions: Vec<Instruction>,
    length: u64,
    stride: u32,
    element: AbiType,
}

struct FixedArrayAccessMetadata {
    source: AggregateLocation,
    length: u64,
    stride: u32,
    element: AbiType,
}

pub(super) fn fixed_array_element_access(
    expression: &IndexExpr,
    context: &LoweringContext,
    unsupported_diagnostic: impl Fn() -> Vec<Diagnostic> + Copy,
) -> Result<Option<FixedArrayElementAccess>, Vec<Diagnostic>> {
    let Some(metadata) = fixed_array_access_metadata(expression, context, unsupported_diagnostic)?
    else {
        return Ok(None);
    };
    let Some(index) = fixed_array_constant_index_value(&expression.index) else {
        return Ok(None);
    };
    if index >= u128::from(metadata.length) {
        return Ok(Some(FixedArrayElementAccess {
            source: metadata.source,
            offset: 0,
            element: metadata.element,
            out_of_bounds: true,
        }));
    }

    let offset = u64::try_from(index)
        .ok()
        .and_then(|index| index.checked_mul(u64::from(metadata.stride)))
        .and_then(|offset| u32::try_from(offset).ok())
        .ok_or_else(unsupported_diagnostic)?;
    Ok(Some(FixedArrayElementAccess {
        source: metadata.source,
        offset,
        element: metadata.element,
        out_of_bounds: false,
    }))
}

fn fixed_array_element_indexed_access(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    unsupported_diagnostic: impl Fn() -> Vec<Diagnostic> + Copy,
) -> Result<Option<FixedArrayElementIndexedAccess>, Vec<Diagnostic>> {
    if fixed_array_constant_index_value(&expression.index).is_some() {
        return Ok(None);
    }
    let Some(metadata) = fixed_array_access_metadata(expression, context, unsupported_diagnostic)?
    else {
        return Ok(None);
    };
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;
    Ok(Some(FixedArrayElementIndexedAccess {
        source: metadata.source,
        base_offset: 0,
        index: index.value,
        index_instructions: index.instructions,
        length: metadata.length,
        stride: metadata.stride,
        element: metadata.element,
    }))
}

fn fixed_array_access_metadata(
    expression: &IndexExpr,
    context: &LoweringContext,
    unsupported_diagnostic: impl Fn() -> Vec<Diagnostic> + Copy,
) -> Result<Option<FixedArrayAccessMetadata>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = unwrap_group(&expression.object) else {
        return Ok(None);
    };
    let Some(local) = context.aggregate_local(&identifier.name) else {
        return Ok(None);
    };
    let Some(ty) = context.local_binding_type_expr_for_identifier(identifier) else {
        return Ok(None);
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_diagnostic());
    };
    let value = abi_value_from_type_expr_with_resolver(&ty, resolved, |source| {
        context.resolved_source(source)
    })
    .map_err(|_error| unsupported_diagnostic())?;
    let AbiType::Array { element, length } = &value.ty else {
        return Ok(None);
    };
    if value.layout != local.layout {
        return Err(unsupported_diagnostic());
    }
    let stride = array_element_stride(element).map_err(|_error| unsupported_diagnostic())?;
    let stride = u32::try_from(stride).map_err(|_error| unsupported_diagnostic())?;
    Ok(Some(FixedArrayAccessMetadata {
        source: AggregateLocation::Slot(local.slot_index),
        length: *length,
        stride,
        element: element.as_ref().clone(),
    }))
}

fn fixed_array_constant_index_value(expression: &Expr) -> Option<u128> {
    match unwrap_group(expression) {
        Expr::IntegerLiteral(literal) => decode_integer_literal_value(&literal.value),
        _ => None,
    }
}

fn lower_i32_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredI32Value, Vec<Diagnostic>> {
    if let Some(lowered) =
        lower_fixed_array_i32_index_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }
    if let Some(lowered) =
        lower_fixed_array_i32_indexed_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }
    lower_i32_slice_index_expression_to_value(expression, context, temporaries)
}

fn lower_fixed_array_i32_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredI32Value>, Vec<Diagnostic>> {
    let Some(access) =
        fixed_array_element_access(expression, context, unsupported_i32_expression_diagnostic)?
    else {
        return Ok(None);
    };
    if access.element != AbiType::I32 {
        return Ok(None);
    }
    if access.out_of_bounds {
        return Ok(Some(LoweredI32Value {
            instructions: vec![Instruction::Trap],
            value: I32Value::Const(0),
        }));
    }

    let temporary = temporaries.next_i32()?;
    Ok(Some(LoweredI32Value {
        instructions: vec![Instruction::LoadAggregateI32 {
            destination: temporary,
            source: access.source,
            offset: access.offset,
        }],
        value: I32Value::Location(temporary),
    }))
}

fn lower_fixed_array_i32_indexed_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredI32Value>, Vec<Diagnostic>> {
    let Some(access) = fixed_array_element_indexed_access(
        expression,
        context,
        temporaries,
        unsupported_i32_expression_diagnostic,
    )?
    else {
        return Ok(None);
    };
    if access.element != AbiType::I32 {
        return Ok(None);
    }

    let temporary = temporaries.next_i32()?;
    let mut instructions = access.index_instructions;
    instructions.push(Instruction::LoadAggregateI32Indexed {
        destination: temporary,
        source: access.source,
        base_offset: access.base_offset,
        index: access.index,
        length: access.length,
        stride: access.stride,
    });
    Ok(Some(LoweredI32Value {
        instructions,
        value: I32Value::Location(temporary),
    }))
}

fn lower_u8_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredU8Value, Vec<Diagnostic>> {
    if let Some(lowered) =
        lower_fixed_array_u8_index_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }
    if let Some(lowered) =
        lower_fixed_array_u8_indexed_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }

    let source =
        lower_byte_collection_expression_to_value(&expression.object, context, temporaries)?;
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;

    match source {
        LoweredByteCollectionValue::Str(source) => {
            let mut instructions = source.instructions;
            instructions.extend(index.instructions);
            let value = match source.value {
                StrValue::StaticBytes(bytes) => U8Value::StaticStrIndex {
                    bytes,
                    index: index.value,
                },
                StrValue::Location(source) => U8Value::StrIndex {
                    source,
                    index: index.value,
                },
                value @ (StrValue::ProcessArg { .. } | StrValue::SliceIndex { .. }) => {
                    let source = temporaries.next_str()?;
                    instructions.push(Instruction::SetStr {
                        destination: source,
                        value,
                    });
                    U8Value::StrIndex {
                        source,
                        index: index.value,
                    }
                }
            };
            Ok(LoweredU8Value {
                instructions,
                value,
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
                SliceValue::StrBytes(
                    value @ (StrValue::ProcessArg { .. } | StrValue::SliceIndex { .. }),
                ) => {
                    let source = temporaries.next_str()?;
                    instructions.push(Instruction::SetStr {
                        destination: source,
                        value,
                    });
                    U8Value::StrIndex {
                        source,
                        index: index.value,
                    }
                }
            };
            instructions.extend(index.instructions);
            Ok(LoweredU8Value {
                instructions,
                value,
            })
        }
    }
}

fn lower_fixed_array_u8_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredU8Value>, Vec<Diagnostic>> {
    let Some(access) =
        fixed_array_element_access(expression, context, unsupported_u8_expression_diagnostic)?
    else {
        return Ok(None);
    };
    if access.element != AbiType::U8 {
        return Ok(None);
    }
    if access.out_of_bounds {
        return Ok(Some(LoweredU8Value {
            instructions: vec![Instruction::Trap],
            value: U8Value::Const(0),
        }));
    }

    let temporary = temporaries.next_u8()?;
    Ok(Some(LoweredU8Value {
        instructions: vec![Instruction::LoadAggregateU8 {
            destination: temporary,
            source: access.source,
            offset: access.offset,
        }],
        value: U8Value::Location(temporary),
    }))
}

fn lower_fixed_array_u8_indexed_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredU8Value>, Vec<Diagnostic>> {
    let Some(access) = fixed_array_element_indexed_access(
        expression,
        context,
        temporaries,
        unsupported_u8_expression_diagnostic,
    )?
    else {
        return Ok(None);
    };
    if access.element != AbiType::U8 {
        return Ok(None);
    }

    let temporary = temporaries.next_u8()?;
    let mut instructions = access.index_instructions;
    instructions.push(Instruction::LoadAggregateU8Indexed {
        destination: temporary,
        source: access.source,
        base_offset: access.base_offset,
        index: access.index,
        length: access.length,
        stride: access.stride,
    });
    Ok(Some(LoweredU8Value {
        instructions,
        value: U8Value::Location(temporary),
    }))
}

fn lower_usize_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    if let Some(lowered) =
        lower_fixed_array_usize_index_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }
    if let Some(lowered) =
        lower_fixed_array_usize_indexed_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }
    lower_usize_slice_index_expression_to_value(expression, context, temporaries)
}

fn lower_fixed_array_usize_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredUsizeValue>, Vec<Diagnostic>> {
    let Some(access) =
        fixed_array_element_access(expression, context, unsupported_usize_expression_diagnostic)?
    else {
        return Ok(None);
    };
    if access.element != AbiType::Usize {
        return Ok(None);
    }
    if access.out_of_bounds {
        return Ok(Some(LoweredUsizeValue {
            instructions: vec![Instruction::Trap],
            value: UsizeValue::Const(0),
        }));
    }

    let temporary = temporaries.next_usize()?;
    Ok(Some(LoweredUsizeValue {
        instructions: vec![Instruction::LoadAggregateUsize {
            destination: temporary,
            source: access.source,
            offset: access.offset,
        }],
        value: UsizeValue::Location(temporary),
    }))
}

fn lower_fixed_array_usize_indexed_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredUsizeValue>, Vec<Diagnostic>> {
    let Some(access) = fixed_array_element_indexed_access(
        expression,
        context,
        temporaries,
        unsupported_usize_expression_diagnostic,
    )?
    else {
        return Ok(None);
    };
    if access.element != AbiType::Usize {
        return Ok(None);
    }

    let temporary = temporaries.next_usize()?;
    let mut instructions = access.index_instructions;
    instructions.push(Instruction::LoadAggregateUsizeIndexed {
        destination: temporary,
        source: access.source,
        base_offset: access.base_offset,
        index: access.index,
        length: access.length,
        stride: access.stride,
    });
    Ok(Some(LoweredUsizeValue {
        instructions,
        value: UsizeValue::Location(temporary),
    }))
}

fn lower_usize_slice_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    let source = lower_slice_expression_to_value(&expression.object, context, temporaries)?;
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;
    let mut instructions = source.instructions;
    instructions.extend(index.instructions);

    let SliceValue::Location(source) = source.value else {
        return Err(unsupported_usize_expression_diagnostic());
    };

    Ok(LoweredUsizeValue {
        instructions,
        value: UsizeValue::SliceIndex {
            source,
            index: Box::new(index.value),
        },
    })
}

fn lower_i32_slice_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredI32Value, Vec<Diagnostic>> {
    let source = lower_slice_expression_to_value(&expression.object, context, temporaries)?;
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;
    let mut instructions = source.instructions;
    instructions.extend(index.instructions);

    let SliceValue::Location(source) = source.value else {
        return Err(unsupported_i32_expression_diagnostic());
    };

    Ok(LoweredI32Value {
        instructions,
        value: I32Value::SliceIndex {
            source,
            index: index.value,
        },
    })
}

fn lower_bool_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    if let Some(lowered) = lower_fixed_array_bool_index_expression_to_value(
        expression,
        context,
        diagnostic_code,
        temporaries,
    )? {
        return Ok(lowered);
    }
    if let Some(lowered) = lower_fixed_array_bool_indexed_expression_to_value(
        expression,
        context,
        diagnostic_code,
        temporaries,
    )? {
        return Ok(lowered);
    }
    lower_bool_slice_index_expression_to_value(expression, context, diagnostic_code, temporaries)
}

fn lower_fixed_array_bool_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredBoolValue>, Vec<Diagnostic>> {
    let Some(access) = fixed_array_element_access(expression, context, || {
        unsupported_bool_expression_diagnostic(diagnostic_code)
    })?
    else {
        return Ok(None);
    };
    if access.element != AbiType::Bool {
        return Ok(None);
    }
    if access.out_of_bounds {
        return Ok(Some(LoweredBoolValue {
            instructions: vec![Instruction::Trap],
            value: BoolValue::Const(false),
        }));
    }

    let temporary = temporaries.next_bool()?;
    Ok(Some(LoweredBoolValue {
        instructions: vec![Instruction::LoadAggregateBool {
            destination: temporary,
            source: access.source,
            offset: access.offset,
        }],
        value: BoolValue::Location(temporary),
    }))
}

fn lower_fixed_array_bool_indexed_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredBoolValue>, Vec<Diagnostic>> {
    let Some(access) =
        fixed_array_element_indexed_access(expression, context, temporaries, || {
            unsupported_bool_expression_diagnostic(diagnostic_code)
        })?
    else {
        return Ok(None);
    };
    if access.element != AbiType::Bool {
        return Ok(None);
    }

    let temporary = temporaries.next_bool()?;
    let mut instructions = access.index_instructions;
    instructions.push(Instruction::LoadAggregateBoolIndexed {
        destination: temporary,
        source: access.source,
        base_offset: access.base_offset,
        index: access.index,
        length: access.length,
        stride: access.stride,
    });
    Ok(Some(LoweredBoolValue {
        instructions,
        value: BoolValue::Location(temporary),
    }))
}

fn lower_bool_slice_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let source = lower_slice_expression_to_value(&expression.object, context, temporaries)?;
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;
    let mut instructions = source.instructions;
    instructions.extend(index.instructions);

    let SliceValue::Location(source) = source.value else {
        return Err(unsupported_bool_expression_diagnostic(diagnostic_code));
    };

    Ok(LoweredBoolValue {
        instructions,
        value: BoolValue::SliceIndex {
            source,
            index: index.value,
        },
    })
}

fn lower_str_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    if let Some(lowered) =
        lower_fixed_array_str_index_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }
    if let Some(lowered) =
        lower_fixed_array_str_indexed_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }
    lower_str_slice_index_expression_to_value(expression, context, temporaries)
}

fn lower_fixed_array_str_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredStrValue>, Vec<Diagnostic>> {
    let Some(access) =
        fixed_array_element_access(expression, context, unsupported_str_expression_diagnostic)?
    else {
        return Ok(None);
    };
    if access.element != AbiType::StrView {
        return Ok(None);
    }
    if access.out_of_bounds {
        return Ok(Some(LoweredStrValue {
            instructions: vec![Instruction::Trap],
            value: StrValue::StaticBytes(Vec::new()),
        }));
    }

    let temporary = temporaries.next_str()?;
    let StrLocation::Local(index) = temporary else {
        unreachable!("temporary str locations are local pairs");
    };
    let len_index = index
        .checked_add(1)
        .ok_or_else(unsupported_str_expression_diagnostic)?;
    let len_offset = access
        .offset
        .checked_add(8)
        .ok_or_else(unsupported_str_expression_diagnostic)?;
    Ok(Some(LoweredStrValue {
        instructions: vec![
            Instruction::LoadAggregateUsize {
                destination: UsizeLocation::Local(index),
                source: access.source,
                offset: access.offset,
            },
            Instruction::LoadAggregateUsize {
                destination: UsizeLocation::Local(len_index),
                source: access.source,
                offset: len_offset,
            },
        ],
        value: StrValue::Location(temporary),
    }))
}

fn lower_fixed_array_str_indexed_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredStrValue>, Vec<Diagnostic>> {
    let Some(access) = fixed_array_element_indexed_access(
        expression,
        context,
        temporaries,
        unsupported_str_expression_diagnostic,
    )?
    else {
        return Ok(None);
    };
    if access.element != AbiType::StrView {
        return Ok(None);
    }

    let temporary = temporaries.next_str()?;
    let StrLocation::Local(index) = temporary else {
        unreachable!("temporary str locations are local pairs");
    };
    let len_index = index
        .checked_add(1)
        .ok_or_else(unsupported_str_expression_diagnostic)?;
    let len_base_offset = access
        .base_offset
        .checked_add(8)
        .ok_or_else(unsupported_str_expression_diagnostic)?;
    let mut instructions = access.index_instructions;
    instructions.push(Instruction::LoadAggregateUsizeIndexed {
        destination: UsizeLocation::Local(index),
        source: access.source,
        base_offset: access.base_offset,
        index: access.index.clone(),
        length: access.length,
        stride: access.stride,
    });
    instructions.push(Instruction::LoadAggregateUsizeIndexed {
        destination: UsizeLocation::Local(len_index),
        source: access.source,
        base_offset: len_base_offset,
        index: access.index,
        length: access.length,
        stride: access.stride,
    });
    Ok(Some(LoweredStrValue {
        instructions,
        value: StrValue::Location(temporary),
    }))
}

fn lower_str_slice_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    let source = lower_slice_expression_to_value(&expression.object, context, temporaries)?;
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;
    let mut instructions = source.instructions;
    instructions.extend(index.instructions);

    let SliceValue::Location(source) = source.value else {
        return Err(unsupported_str_expression_diagnostic());
    };

    Ok(LoweredStrValue {
        instructions,
        value: StrValue::SliceIndex {
            source,
            index: index.value,
        },
    })
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
        Expr::Call(call) => byte_collection_call_kind(call, context),
        Expr::Member(_) => match aggregate_member_field_kind(expression, context)
            .ok()
            .flatten()?
        {
            AggregateFieldKind::Str => Some(ByteCollectionKind::Str),
            AggregateFieldKind::Slice(_) => Some(ByteCollectionKind::Slice),
            _ => None,
        },
        Expr::Propagate(propagation) => {
            fallible_byte_collection_expression_kind(&propagation.expression, context)
        }
        Expr::Force(force) => fallible_byte_collection_expression_kind(&force.expression, context),
        Expr::Catch(catch) => fallible_byte_collection_expression_kind(&catch.expression, context),
        Expr::Group(group) => byte_collection_expression_kind(&group.expression, context),
        _ => None,
    }
}

fn byte_collection_call_kind(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<ByteCollectionKind> {
    if primitive_arg_raw_call(call, context) {
        return Some(ByteCollectionKind::Str);
    }
    if primitive_str_from_raw_parts_call(call, context) {
        return Some(ByteCollectionKind::Str);
    }
    if primitive_bytes_from_str_call(call, context)
        || primitive_slice_from_raw_parts_call(call, context)
    {
        return Some(ByteCollectionKind::Slice);
    }

    let (target, _call_name) = context.direct_call_target_and_name(call)?;
    byte_collection_kind_from_type(context.call_return_type(&target)?)
}

fn fallible_byte_collection_expression_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<ByteCollectionKind> {
    let Expr::Call(call) = unwrap_group(expression) else {
        return None;
    };
    let (target, _call_name) = context.direct_call_target_and_name(call)?;
    let Type::Fallible(success) = context.call_return_type(&target)? else {
        return None;
    };
    byte_collection_kind_from_type(success)
}

fn byte_collection_kind_from_type(ty: &Type) -> Option<ByteCollectionKind> {
    match ty {
        Type::Str => Some(ByteCollectionKind::Str),
        Type::Slice { .. } => Some(ByteCollectionKind::Slice),
        _ => None,
    }
}

pub(super) fn lower_usize_expression_to_word(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_usize_expression_to_word_with_temporaries(expression, context, &mut temporaries)
}

pub(super) fn lower_usize_expression_to_word_with_temporaries(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    let lowered = lower_usize_expression_to_value(expression, context, temporaries)?;
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

    Some(lower_byte_collection_len_expression_to_value(
        &member.object,
        context,
        temporaries,
    ))
}

fn lower_builtin_is_empty_call_to_value(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Option<Result<LoweredBoolValue, Vec<Diagnostic>>> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };
    if member.member != "is_empty" || !call.arguments.is_empty() {
        return None;
    }
    byte_collection_expression_kind(&member.object, context)?;

    Some(
        lower_byte_collection_len_expression_to_value(&member.object, context, temporaries).map(
            |source| LoweredBoolValue {
                instructions: source.instructions,
                value: BoolValue::UsizeComparison {
                    operator: I32ComparisonOperator::Equal,
                    left: source.value,
                    right: UsizeValue::Const(0),
                },
            },
        ),
    )
}

fn lower_byte_collection_len_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    match lower_byte_collection_expression_to_value(expression, context, temporaries)? {
        LoweredByteCollectionValue::Str(source) => {
            let mut instructions = source.instructions;
            let value = match source.value {
                StrValue::StaticBytes(bytes) => UsizeValue::Const(bytes.len() as u64),
                StrValue::Location(location) => UsizeValue::StrLen(location),
                value @ (StrValue::ProcessArg { .. } | StrValue::SliceIndex { .. }) => {
                    let temporary = temporaries.next_str()?;
                    instructions.push(Instruction::SetStr {
                        destination: temporary,
                        value,
                    });
                    UsizeValue::StrLen(temporary)
                }
            };
            Ok(LoweredUsizeValue {
                instructions,
                value,
            })
        }
        LoweredByteCollectionValue::Slice(source) => {
            let mut instructions = source.instructions;
            let value = match source.value {
                SliceValue::Location(location) => UsizeValue::SliceLen(location),
                SliceValue::StrBytes(StrValue::StaticBytes(bytes)) => {
                    UsizeValue::Const(bytes.len() as u64)
                }
                SliceValue::StrBytes(StrValue::Location(location)) => UsizeValue::StrLen(location),
                SliceValue::StrBytes(
                    value @ (StrValue::ProcessArg { .. } | StrValue::SliceIndex { .. }),
                ) => {
                    let temporary = temporaries.next_str()?;
                    instructions.push(Instruction::SetStr {
                        destination: temporary,
                        value,
                    });
                    UsizeValue::StrLen(temporary)
                }
            };
            Ok(LoweredUsizeValue {
                instructions,
                value,
            })
        }
    }
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
            lower_bool_comparison_condition(binary, context, diagnostic_code)
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual
            if str_comparison_is_lowerable(binary, context) =>
        {
            lower_str_comparison_condition(binary, context, diagnostic_code)
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
    lower_bool_value(expression, context, diagnostic_code)
}

fn lower_str_comparison_condition(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    let operator = str_comparison_operator(binary.operator, diagnostic_code)?;
    Ok(BoolValue::StrComparison {
        operator,
        left: lower_str_value(&binary.left, context)?,
        right: lower_str_value(&binary.right, context)?,
    })
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

fn str_comparison_operator(
    operator: BinaryOperator,
    diagnostic_code: &'static str,
) -> Result<BoolComparisonOperator, Vec<Diagnostic>> {
    match operator {
        BinaryOperator::Equal => Ok(BoolComparisonOperator::Equal),
        BinaryOperator::NotEqual => Ok(BoolComparisonOperator::NotEqual),
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
        "IR v0 can only lower usize literals, parameters, locals, arithmetic or shift expressions, slice indexing, len calls, and direct tail calls",
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

fn unsupported_bool_expression_diagnostic(diagnostic_code: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 can only lower bool literals, bool locals, bool operators, i32, u8, usize comparisons, and bool equality/inequality over lowerable bool values",
    )]
}
