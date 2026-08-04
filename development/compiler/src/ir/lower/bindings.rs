use super::aggregates::{
    ArrayInitializationProgress, PayloadInitializationProgress, StructInitializationProgress,
    aggregate_call_return_layout_from_resolved, aggregate_fields_from_type_expr,
    aggregate_fields_from_type_expr_with_resolver, aggregate_type_layout,
    array_literal_requires_runtime_progress, lower_aggregate_array_literal_to_location,
    lower_aggregate_array_literal_to_location_at_offset,
    lower_aggregate_array_literal_to_location_with_progress,
    lower_aggregate_struct_literal_to_location,
    lower_aggregate_struct_literal_to_location_at_offset,
    lower_aggregate_struct_literal_to_location_at_offset_with_temporaries,
    lower_aggregate_struct_literal_to_location_with_progress,
    lower_aggregate_struct_literal_to_location_with_temporaries, lower_outcome_field_to_location,
    lower_payload_enum_constructor_to_location,
    lower_payload_enum_constructor_to_location_with_progress,
    payload_enum_constructor_member_and_arguments, push_aggregate_call_instruction,
    push_fallible_aggregate_call_instruction, supported_aggregate_copy_layout,
    type_expr_is_copy_aggregate_value_with_resolver, type_expr_is_copy_struct_with_resolver,
};
use super::context::{
    AggregateDrop, AggregateFieldKind, DropGlue, LoweringContext, OutcomeDrop, OutcomeLocal,
    SliceTypeInfo,
};
use super::errors::lower_error_payload;
use super::expressions::{
    TemporaryAllocator, aggregate_call_field, aggregate_member_field_kind_from_member,
    expression_is_lowerable_bool_binding, fixed_array_element_access,
    fixed_array_element_indexed_access, lower_aggregate_member_field_access,
    lower_bool_closure_capture_assignment, lower_bool_expression_to_location,
    lower_bool_expression_to_value, lower_bool_expression_to_value_with_temporaries,
    lower_borrow_expression_to_location, lower_call_arguments_to_scalar_arguments,
    lower_call_arguments_to_scalar_arguments_with_temporaries, lower_catch_failure_mode,
    lower_composed_outcome_call, lower_fallible_bool_normal_call,
    lower_fallible_borrow_normal_call, lower_fallible_i32_normal_call,
    lower_fallible_slice_normal_call, lower_fallible_str_normal_call,
    lower_fallible_u8_normal_call, lower_fallible_usize_normal_call,
    lower_i32_closure_capture_assignment, lower_i32_expression_to_location,
    lower_i32_expression_to_word, lower_i32_expression_to_word_with_temporaries,
    lower_macos_syscall_primitive_call_to_location, lower_pointer_address_expression_to_word,
    lower_slice_expression_to_location, lower_slice_expression_to_value,
    lower_str_expression_to_location, lower_str_expression_to_value,
    lower_u8_closure_capture_assignment, lower_u8_expression_to_location,
    lower_u8_expression_to_word, lower_u8_expression_to_word_with_temporaries,
    lower_usize_closure_capture_assignment, lower_usize_expression_to_location,
    lower_usize_expression_to_word, lower_usize_expression_to_word_with_temporaries,
    lower_void_expression_statement, primitive_take_value_at_ptr_call,
    push_store_slice_view_to_aggregate_field, push_store_str_view_to_aggregate_field,
};
use super::functions::{
    lower_aggregate_drop_instructions, lower_aggregate_drop_instructions_at_location,
    lower_aggregate_return_expression_to_location, lower_drop_statement,
    lower_never_expression_with_scope_drops, lower_return_statement_with_scope_drops,
    lower_scope_end_drops_for_locals_since, replacement_drop_for_aggregate_slot,
};
use super::interpolation::lower_interpolated_string_binding;
use super::literals::{
    lower_i8_literal, lower_i16_literal, lower_i64_literal, lower_u16_literal, lower_u32_literal,
    lower_u64_literal,
};
use super::outcome_propagation::{
    propagating_outcome_mode, propagating_outcome_mode_for_layer,
    stored_optional_propagation_instructions,
};
use super::regions::CleanupScopeMark;
use super::types::{
    parameter_type_from_type_expr_with_resolver, return_type_expr_is_top_level_optional,
    return_type_from_type_expr_with_resolver, scalar_or_view_type_from_type_expr,
    top_level_optional_success_abi_value_with_resolver, view_element_type_from_type_expr,
};
use crate::abi::{
    AbiType, ValueLayout, abi_value_from_type_expr, abi_value_from_type_expr_with_resolver,
};
use crate::ast::{
    AssignmentOperator, AssignmentStmt, BinaryOperator, BindingStmt, Block, CallExpr, Expr,
    IndexExpr, MemberExpr, Stmt, TypeExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolLocation, BoolValue, ComposedOutcomeDestination, I32Location, I32Value,
    Instruction, OutcomeFailureMode, SliceElementIndex, SliceLocation, SliceValue, StrLocation,
    StrValue, Type, U8Location, U8Value, UsizeLocation, UsizeValue,
};
use crate::outcomes::{OutcomeLayer, outcome_shape_with_resolver};
use crate::resolve::ResolveOutput;
use crate::typecheck::{TypecheckScalarViewKind, TypecheckSliceElementKind};

mod aggregate_bindings;
mod aggregate_field_assignments;
mod aggregate_slot_assignments;
mod compound_assignments;
mod diagnostics;
mod identifier_assignments;
mod index_assignments;
mod optional_assignments;
mod otherwise_bindings;
mod outcome_aggregate_values;
mod outcome_values;
mod payload_field_assignments;
mod pointer_take_bindings;
mod scalar_bindings;
mod slice_types;
mod typed_literals;
mod utility;

use aggregate_bindings::*;
use aggregate_field_assignments::*;
use aggregate_slot_assignments::*;
use compound_assignments::*;
use diagnostics::*;
use identifier_assignments::*;
use index_assignments::*;
use optional_assignments::*;
pub(in crate::ir::lower) use otherwise_bindings::lower_otherwise_recover_or_handle_failure_mode;
use otherwise_bindings::*;
use outcome_aggregate_values::*;
use outcome_values::*;
use payload_field_assignments::*;
use pointer_take_bindings::*;
use scalar_bindings::*;
use slice_types::*;
use typed_literals::*;
use utility::*;

#[derive(Clone, Copy)]
pub(super) struct LoopControlContext<'a> {
    pub(super) scope_mark: CleanupScopeMark,
    pub(super) continue_instructions: &'a [Instruction],
}

pub(super) fn lower_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_local_binding_with_loop_control(statement, context, None)
}

pub(super) fn lower_local_binding_with_loop_control(
    statement: &BindingStmt,
    context: &mut LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_interpolated_string_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_outcome_local_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_direct_outcome_local_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_stored_outcome_aggregate_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_otherwise_scalar_binding(statement, context, loop_control)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_otherwise_aggregate_binding(statement, context, loop_control)?
    {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_typed_literal_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_closure_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_aggregate_struct_literal_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_aggregate_array_literal_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_payload_enum_constructor_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_pointer_take_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_aggregate_call_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_aggregate_copy_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_aggregate_move_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_aggregate_member_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_aggregate_slice_index_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_error_local_binding(statement, context)? {
        return Ok(instructions);
    }

    match scalar_binding_kind(statement, context)? {
        ScalarBindingKind::I32 => lower_i32_local_binding(statement, context),
        ScalarBindingKind::U8 => lower_u8_local_binding(statement, context),
        ScalarBindingKind::Usize => lower_usize_local_binding(statement, context),
        ScalarBindingKind::Borrow {
            is_readwrite,
            inner,
        } => lower_borrow_local_binding(statement, is_readwrite, inner, context),
        ScalarBindingKind::Bool => lower_bool_local_binding(statement, context),
        ScalarBindingKind::Str => lower_str_local_binding(statement, context),
        ScalarBindingKind::Slice(info) => lower_slice_local_binding(statement, context, info),
    }
}

pub(super) fn lower_assignment(
    statement: &AssignmentStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if statement.operator != AssignmentOperator::Assign {
        return lower_compound_assignment(statement, context);
    }

    match unwrap_group(&statement.target) {
        Expr::Identifier(identifier) => {
            lower_outcome_assignment(identifier, &statement.value, context)?.map_or_else(
                || lower_identifier_assignment(identifier, &statement.value, context),
                Ok,
            )
        }
        Expr::Member(member) => lower_aggregate_field_assignment(member, &statement.value, context),
        Expr::Index(index) => lower_index_assignment(index, &statement.value, context),
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

pub(super) fn assignment_targets_readwrite_aggregate_field(
    statement: &AssignmentStmt,
    context: &LoweringContext,
) -> bool {
    let Expr::Member(member) = unwrap_group(&statement.target) else {
        return false;
    };
    let Some((identifier_name, field_path)) = aggregate_assignment_target_path(member) else {
        return false;
    };
    let Some(field) = context.aggregate_field(identifier_name, &field_path) else {
        return false;
    };
    if !field.is_readwrite {
        return false;
    }

    match statement.operator {
        AssignmentOperator::Assign => true,
        AssignmentOperator::AddAssign
        | AssignmentOperator::SubtractAssign
        | AssignmentOperator::MultiplyAssign
        | AssignmentOperator::DivideAssign
        | AssignmentOperator::RemainderAssign => {
            matches!(
                field.kind,
                AggregateFieldKind::I32 | AggregateFieldKind::U8 | AggregateFieldKind::Usize
            )
        }
    }
}

pub(super) fn assignment_targets_direct_slice_index(
    statement: &AssignmentStmt,
    context: &LoweringContext,
) -> bool {
    let Expr::Index(index) = unwrap_group(&statement.target) else {
        return false;
    };
    if !index.object.is_direct_slice_index_assignment_object() {
        return false;
    }

    match statement.operator {
        AssignmentOperator::Assign => true,
        AssignmentOperator::AddAssign
        | AssignmentOperator::SubtractAssign
        | AssignmentOperator::MultiplyAssign
        | AssignmentOperator::DivideAssign
        | AssignmentOperator::RemainderAssign => matches!(
            slice_index_assignment_element_kind(&index.object, context),
            TypecheckSliceElementKind::I32
                | TypecheckSliceElementKind::U8
                | TypecheckSliceElementKind::Usize
        ),
    }
}

pub(super) fn lower_i32_optional_otherwise_to_location(
    value: &Expr,
    destination: I32Location,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_stored_optional_otherwise(
        value,
        ComposedOutcomeDestination::I32(destination),
        context,
        move |expression, context| {
            lower_i32_expression_to_location(expression, destination, context)
        },
        "IR can only lower i32 stored `otherwise` fallbacks that produce i32 or exit",
    )? {
        return Ok(Some(instructions));
    }
    let Some((call, fallback)) = direct_optional_otherwise_call(value, context)? else {
        return Ok(None);
    };
    let fallback_destination = destination;
    let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
        fallback,
        context,
        None,
        move |expression, context| {
            lower_i32_expression_to_location(expression, fallback_destination, context)
        },
        "native lowering can only lower i32 `otherwise` fallback blocks that produce an i32 value or exit",
    )?;
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_fallible_i32_normal_call(call, destination, context, &mut temporaries, failure_mode)
        .map(Some)
}

pub(super) fn lower_u8_optional_otherwise_to_location(
    value: &Expr,
    destination: U8Location,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_stored_optional_otherwise(
        value,
        ComposedOutcomeDestination::U8(destination),
        context,
        move |expression, context| {
            lower_u8_expression_to_location(expression, destination, context)
        },
        "IR can only lower u8 stored `otherwise` fallbacks that produce u8 or exit",
    )? {
        return Ok(Some(instructions));
    }
    let Some((call, fallback)) = direct_optional_otherwise_call(value, context)? else {
        return Ok(None);
    };
    let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
        fallback,
        context,
        None,
        move |expression, context| {
            lower_u8_expression_to_location(expression, destination, context)
        },
        "native lowering can only lower u8 `otherwise` fallback blocks that produce a u8 value or exit",
    )?;
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_fallible_u8_normal_call(call, destination, context, &mut temporaries, failure_mode)
        .map(Some)
}

pub(super) fn lower_usize_optional_otherwise_to_location(
    value: &Expr,
    destination: UsizeLocation,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_stored_optional_otherwise(
        value,
        ComposedOutcomeDestination::Usize(destination),
        context,
        move |expression, context| {
            lower_usize_expression_to_location(expression, destination, context)
        },
        "IR can only lower usize stored `otherwise` fallbacks that produce usize or exit",
    )? {
        return Ok(Some(instructions));
    }
    let Some((call, fallback)) = direct_optional_otherwise_call(value, context)? else {
        return Ok(None);
    };
    let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
        fallback,
        context,
        None,
        move |expression, context| {
            lower_usize_expression_to_location(expression, destination, context)
        },
        "native lowering can only lower usize `otherwise` fallback blocks that produce a usize value or exit",
    )?;
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_fallible_usize_normal_call(call, destination, context, &mut temporaries, failure_mode)
        .map(Some)
}

pub(super) fn lower_bool_optional_otherwise_to_location(
    value: &Expr,
    destination: BoolLocation,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_stored_optional_otherwise(
        value,
        ComposedOutcomeDestination::Bool(destination),
        context,
        move |expression, context| {
            lower_bool_expression_to_location(expression, destination, context, "E8008")
        },
        "IR can only lower bool stored `otherwise` fallbacks that produce bool or exit",
    )? {
        return Ok(Some(instructions));
    }
    let Some((call, fallback)) = direct_optional_otherwise_call(value, context)? else {
        return Ok(None);
    };
    let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
        fallback,
        context,
        None,
        move |expression, context| {
            lower_bool_expression_to_location(expression, destination, context, "E8008")
        },
        "native lowering can only lower bool `otherwise` fallback blocks that produce a bool value or exit",
    )?;
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_fallible_bool_normal_call(call, destination, context, &mut temporaries, failure_mode)
        .map(Some)
}

pub(super) fn lower_str_optional_otherwise_to_location(
    value: &Expr,
    destination: StrLocation,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_stored_optional_otherwise(
        value,
        ComposedOutcomeDestination::Str(destination),
        context,
        move |expression, context| {
            lower_str_expression_to_location(expression, destination, context)
        },
        "IR can only lower &str stored `otherwise` fallbacks that produce &str or exit",
    )? {
        return Ok(Some(instructions));
    }
    let Some((call, fallback)) = direct_optional_otherwise_call(value, context)? else {
        return Ok(None);
    };
    let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
        fallback,
        context,
        None,
        move |expression, context| {
            lower_str_expression_to_location(expression, destination, context)
        },
        "native lowering can only lower &str `otherwise` fallback blocks that produce a &str value or exit",
    )?;
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_fallible_str_normal_call(call, destination, context, &mut temporaries, failure_mode)
        .map(Some)
}

pub(super) fn lower_slice_optional_otherwise_to_location(
    value: &Expr,
    destination: SliceLocation,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_stored_optional_otherwise(
        value,
        ComposedOutcomeDestination::Slice(destination),
        context,
        move |expression, context| {
            lower_slice_expression_to_location(expression, destination, context)
        },
        "IR can only lower slice stored `otherwise` fallbacks that produce a slice or exit",
    )? {
        return Ok(Some(instructions));
    }
    let Some((call, fallback)) = direct_optional_otherwise_call(value, context)? else {
        return Ok(None);
    };
    let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
        fallback,
        context,
        None,
        move |expression, context| {
            lower_slice_expression_to_location(expression, destination, context)
        },
        "native lowering can only lower slice `otherwise` fallback blocks that produce a slice value or exit",
    )?;
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_fallible_slice_normal_call(call, destination, context, &mut temporaries, failure_mode)
        .map(Some)
}

pub(in crate::ir::lower) fn lower_aggregate_optional_otherwise_to_location(
    destination: AggregateLocation,
    destination_offset: u32,
    layout: ValueLayout,
    expected_abi_type: Option<&AbiType>,
    otherwise: &crate::ast::OtherwiseExpr,
    context: &LoweringContext,
    unsupported_diagnostic: impl Fn() -> Vec<Diagnostic>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_diagnostic());
    };
    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return Err(unsupported_diagnostic());
    };
    let Some(return_type) = context.call_return_type_expr(call) else {
        return Err(unsupported_diagnostic());
    };
    if !return_type_expr_is_top_level_optional(&return_type, resolved) {
        return Err(unsupported_diagnostic());
    }
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_diagnostic());
    };
    let Some(Type::Optional(success_type)) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_diagnostic());
    };
    let Some(callee_layout) = aggregate_type_layout(success_type.as_ref()) else {
        return Err(unsupported_diagnostic());
    };
    if callee_layout != layout {
        return Err(unsupported_diagnostic());
    }

    if destination_offset == 0 {
        let failure_mode = lower_otherwise_aggregate_failure_mode(
            &otherwise.fallback,
            layout,
            expected_abi_type,
            destination,
            resolved,
            context,
            None,
            &unsupported_diagnostic,
        )?;
        let (mut argument_instructions, arguments) =
            lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
        let mut instructions = Vec::new();
        instructions.append(&mut argument_instructions);
        push_fallible_aggregate_call_instruction(
            &mut instructions,
            success_type.as_ref(),
            destination,
            target,
            arguments,
            layout,
            failure_mode,
        );
        return Ok(instructions);
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let source_slot = temporaries.next_aggregate_slot();
    let call_destination = AggregateLocation::Slot(source_slot);
    let failure_mode = lower_otherwise_aggregate_failure_mode(
        &otherwise.fallback,
        layout,
        expected_abi_type,
        call_destination,
        resolved,
        context,
        None,
        &unsupported_diagnostic,
    )?;
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index: source_slot,
        layout,
    }];
    let (mut argument_instructions, arguments) =
        lower_call_arguments_to_scalar_arguments_with_temporaries(
            call,
            &target,
            &call_name,
            context,
            &mut temporaries,
        )?;
    instructions.append(&mut argument_instructions);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success_type.as_ref(),
        call_destination,
        target,
        arguments,
        layout,
        failure_mode,
    );
    instructions.push(Instruction::CopyAggregateRange {
        destination,
        destination_offset,
        source: call_destination,
        source_offset: 0,
        layout,
    });
    Ok(instructions)
}
