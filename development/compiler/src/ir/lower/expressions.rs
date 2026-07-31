use super::aggregates::{
    aggregate_call_return_layout_from_resolved, aggregate_fields_from_type_expr,
    aggregate_type_layout, lower_aggregate_struct_literal_to_location_with_temporaries,
    push_aggregate_call_instruction, push_fallible_aggregate_call_instruction,
    supported_aggregate_copy_layout,
};
use super::bindings::{
    lower_aggregate_optional_otherwise_to_location, lower_assignment,
    lower_bool_optional_otherwise_to_location, lower_i32_optional_otherwise_to_location,
    lower_local_binding, lower_slice_optional_otherwise_to_location,
    lower_str_optional_otherwise_to_location, lower_u8_optional_otherwise_to_location,
    lower_usize_optional_otherwise_to_location,
};
use super::context::{AggregateDrop, AggregateFieldKind, LoweringContext};
use super::errors::{ErrorPayload, lower_error_payload};
use super::functions::{
    BranchPrologue, LoweredPayloadlessSwitchBody, LoweredSwitchBlock, LoweredSwitchCondition,
    append_scope_end_drops_before_exit, expression_contains_explicit_aggregate_move,
    expression_contains_explicit_aggregate_move_outside, lower_aggregate_drop_instructions,
    lower_aggregate_return_expression, lower_direct_aggregate_return_with_scope_drops,
    lower_never_expression_with_scope_drops, lower_scope_end_drops_for_locals_since,
    lower_value_return_with_scope_drops, mark_explicit_moves_in_expression,
    mark_lowered_statement_aggregate_uses, propagating_failure_mode,
    tag_only_if_is_as_control_flow, tag_only_switch_as_control_flow,
};
use super::literals::{
    lower_i32_literal, lower_str_literal, lower_u8_literal, lower_usize_literal,
};
use super::types::{
    return_type_expr_is_top_level_optional_with_resolver, scalar_or_view_type_from_type_expr,
    top_level_optional_success_abi_value_with_resolver,
};
mod aggregate_members;
mod byte_collections;
mod calls;
mod control_flow_values;
mod diagnostics;
mod fallible;
mod fixed_arrays;
mod predicates;
mod returns;
mod scalar_values;
mod statement_effects;
mod temporaries;
mod utility;

use aggregate_members::*;
use byte_collections::*;
use control_flow_values::*;
use diagnostics::*;
use fallible::*;
use fixed_arrays::*;
use returns::*;
use scalar_values::*;
use statement_effects::*;
use utility::*;

use crate::abi::{
    AbiType, ValueLayout, abi_value_from_type_expr, abi_value_from_type_expr_with_resolver,
    array_element_stride,
};
use crate::ast::{
    BinaryExpr, BinaryOperator, Block, CallExpr, CatchExpr, Expr, IfIsStmt, IfStmt, IndexExpr,
    Stmt, SwitchStmt, TypeConversionExpr, UnaryExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolComparisonOperator, BoolLocation, BoolLogicalOperator, BoolValue,
    FallibleFailureMode, I32ComparisonOperator, I32Location, I32Value, Instruction, ScalarArgument,
    SliceLocation, SliceValue, StrLocation, StrValue, Type, U8Location, U8Value, UsizeLocation,
    UsizeValue,
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
        Expr::Otherwise(_) => {
            lower_i32_optional_otherwise_to_location(expression, destination, context)?
                .ok_or_else(unsupported_i32_expression_diagnostic)
        }
        Expr::If(statement) => lower_i32_if_expression_to_location(statement, destination, context),
        Expr::IfIs(statement) => {
            lower_i32_if_is_expression_to_location(statement, destination, context)
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
        Expr::Otherwise(_) => {
            lower_u8_optional_otherwise_to_location(expression, destination, context)?
                .ok_or_else(unsupported_u8_expression_diagnostic)
        }
        Expr::If(statement) => lower_u8_if_expression_to_location(statement, destination, context),
        Expr::IfIs(statement) => {
            lower_u8_if_is_expression_to_location(statement, destination, context)
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
        Expr::Otherwise(_) => {
            lower_usize_optional_otherwise_to_location(expression, destination, context)?
                .ok_or_else(unsupported_usize_expression_diagnostic)
        }
        Expr::If(statement) => {
            lower_usize_if_expression_to_location(statement, destination, context)
        }
        Expr::IfIs(statement) => {
            lower_usize_if_is_expression_to_location(statement, destination, context)
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
        Expr::Otherwise(_) => {
            lower_str_optional_otherwise_to_location(expression, destination, context)?
                .ok_or_else(unsupported_str_expression_diagnostic)
        }
        Expr::If(statement) => lower_str_if_expression_to_location(statement, destination, context),
        Expr::IfIs(statement) => {
            lower_str_if_is_expression_to_location(statement, destination, context)
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
        Expr::Otherwise(_) => {
            lower_slice_optional_otherwise_to_location(expression, destination, context)?
                .ok_or_else(unsupported_slice_expression_diagnostic)
        }
        Expr::If(statement) => {
            lower_slice_if_expression_to_location(statement, destination, context)
        }
        Expr::IfIs(statement) => {
            lower_slice_if_is_expression_to_location(statement, destination, context)
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
        Expr::Otherwise(_) => {
            let temporary = temporaries.next_str()?;
            let expression_context = context
                .with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
            Ok(LoweredStrValue {
                instructions: lower_str_optional_otherwise_to_location(
                    expression,
                    temporary,
                    &expression_context,
                )?
                .ok_or_else(unsupported_str_expression_diagnostic)?,
                value: StrValue::Location(temporary),
            })
        }
        Expr::Match(statement) => {
            lower_str_match_expression_to_value(statement, context, temporaries)
        }
        Expr::If(statement) => lower_str_if_expression_to_value(statement, context, temporaries),
        Expr::IfIs(statement) => {
            lower_str_if_is_expression_to_value(statement, context, temporaries)
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
        Expr::Otherwise(_) => {
            let temporary = temporaries.next_slice()?;
            let expression_context = context
                .with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
            Ok(LoweredSliceValue {
                instructions: lower_slice_optional_otherwise_to_location(
                    expression,
                    temporary,
                    &expression_context,
                )?
                .ok_or_else(unsupported_slice_expression_diagnostic)?,
                value: SliceValue::Location(temporary),
            })
        }
        Expr::Match(statement) => {
            lower_slice_match_expression_to_value(statement, context, temporaries)
        }
        Expr::If(statement) => lower_slice_if_expression_to_value(statement, context, temporaries),
        Expr::IfIs(statement) => {
            lower_slice_if_is_expression_to_value(statement, context, temporaries)
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
        Expr::Otherwise(_) => {
            lower_bool_optional_otherwise_to_location(expression, destination, context)?
                .ok_or_else(|| unsupported_bool_expression_diagnostic(diagnostic_code))
        }
        Expr::If(statement) => {
            lower_bool_if_expression_to_location(statement, destination, context, diagnostic_code)
        }
        Expr::IfIs(statement) => lower_bool_if_is_expression_to_location(
            statement,
            destination,
            context,
            diagnostic_code,
        ),
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
        AggregateMemberRoot::OptionalCall(otherwise) => {
            lower_aggregate_optional_otherwise_member_field_access(
                otherwise,
                &access.field_path,
                context,
                temporaries,
            )
        }
    }
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
        AggregateMemberRoot::OptionalCall(otherwise) => {
            aggregate_optional_otherwise_member_field_kind(otherwise, &field_path, context)
        }
    })
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
        Expr::Otherwise(_) => {
            let temporary = temporaries.next_bool()?;
            let expression_context = context
                .with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
            Ok(LoweredBoolValue {
                instructions: lower_bool_optional_otherwise_to_location(
                    expression,
                    temporary,
                    &expression_context,
                )?
                .ok_or_else(|| unsupported_bool_expression_diagnostic(diagnostic_code))?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::If(statement) => {
            lower_bool_if_expression_to_value(statement, context, diagnostic_code, temporaries)
        }
        Expr::IfIs(statement) => {
            lower_bool_if_is_expression_to_value(statement, context, diagnostic_code, temporaries)
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
    pub(super) instructions: Vec<Instruction>,
    pub(super) source: AggregateLocation,
    pub(super) offset: u32,
    pub(super) element: AbiType,
    pub(super) out_of_bounds: bool,
    pub(super) is_readwrite: bool,
}

pub(super) struct FixedArrayElementIndexedAccess {
    pub(super) source: AggregateLocation,
    pub(super) base_offset: u32,
    pub(super) index: UsizeValue,
    pub(super) index_instructions: Vec<Instruction>,
    pub(super) length: u64,
    pub(super) stride: u32,
    pub(super) element: AbiType,
    pub(super) is_readwrite: bool,
}

pub(super) fn fixed_array_element_access(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    unsupported_diagnostic: impl Fn() -> Vec<Diagnostic> + Copy,
) -> Result<Option<FixedArrayElementAccess>, Vec<Diagnostic>> {
    let Some(metadata) =
        fixed_array_access_metadata(expression, context, temporaries, unsupported_diagnostic)?
    else {
        return Ok(None);
    };
    let Some(index) = fixed_array_constant_index_value(&expression.index) else {
        return Ok(None);
    };
    if index >= u128::from(metadata.length) {
        return Ok(Some(FixedArrayElementAccess {
            instructions: metadata.instructions,
            source: metadata.source,
            offset: 0,
            element: metadata.element,
            out_of_bounds: true,
            is_readwrite: metadata.is_readwrite,
        }));
    }

    let element_offset = u64::try_from(index)
        .ok()
        .and_then(|index| index.checked_mul(u64::from(metadata.stride)))
        .ok_or_else(unsupported_diagnostic)?;
    let offset = u64::from(metadata.base_offset)
        .checked_add(element_offset)
        .and_then(|offset| u32::try_from(offset).ok())
        .ok_or_else(unsupported_diagnostic)?;
    Ok(Some(FixedArrayElementAccess {
        instructions: metadata.instructions,
        source: metadata.source,
        offset,
        element: metadata.element,
        out_of_bounds: false,
        is_readwrite: metadata.is_readwrite,
    }))
}

pub(super) fn fixed_array_element_indexed_access(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    unsupported_diagnostic: impl Fn() -> Vec<Diagnostic> + Copy,
) -> Result<Option<FixedArrayElementIndexedAccess>, Vec<Diagnostic>> {
    if fixed_array_constant_index_value(&expression.index).is_some() {
        return Ok(None);
    }
    let Some(metadata) =
        fixed_array_access_metadata(expression, context, temporaries, unsupported_diagnostic)?
    else {
        return Ok(None);
    };
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;
    let mut index_instructions = metadata.instructions;
    index_instructions.extend(index.instructions);
    Ok(Some(FixedArrayElementIndexedAccess {
        source: metadata.source,
        base_offset: metadata.base_offset,
        index: index.value,
        index_instructions,
        length: metadata.length,
        stride: metadata.stride,
        element: metadata.element,
        is_readwrite: metadata.is_readwrite,
    }))
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
