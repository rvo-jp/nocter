use super::aggregates::{
    aggregate_call_return_layout_from_resolved, aggregate_fields_from_type_expr,
    aggregate_fields_from_type_expr_with_resolver, aggregate_type_layout,
    lower_aggregate_array_literal_to_location, lower_aggregate_struct_literal_to_location,
    lower_aggregate_struct_literal_to_location_at_offset,
    lower_aggregate_struct_literal_to_location_with_temporaries, push_aggregate_call_instruction,
    push_fallible_aggregate_call_instruction, supported_aggregate_copy_layout,
    type_expr_is_copy_struct, type_expr_is_copy_struct_with_resolver,
};
use super::context::{AggregateFieldKind, DropGlue, LoweringContext, SliceTypeInfo};
use super::errors::lower_error_payload;
use super::expressions::{
    TemporaryAllocator, aggregate_call_field, aggregate_member_field_kind_from_member,
    expression_contains_interpolated_string, expression_is_lowerable_bool_binding,
    fixed_array_element_access, lower_aggregate_member_field_access,
    lower_bool_expression_to_location, lower_bool_expression_to_value,
    lower_bool_expression_to_value_with_temporaries, lower_call_arguments_to_scalar_arguments,
    lower_call_arguments_to_scalar_arguments_with_temporaries, lower_catch_failure_mode,
    lower_fallible_bool_normal_call, lower_fallible_i32_normal_call,
    lower_fallible_slice_normal_call, lower_fallible_str_normal_call,
    lower_fallible_u8_normal_call, lower_fallible_usize_normal_call,
    lower_i32_expression_to_location, lower_i32_expression_to_word,
    lower_i32_expression_to_word_with_temporaries, lower_macos_syscall_primitive_call_to_location,
    lower_pointer_address_expression_to_word, lower_slice_expression_to_location,
    lower_slice_expression_to_value, lower_str_expression_to_location,
    lower_str_expression_to_value, lower_u8_expression_to_location, lower_u8_expression_to_word,
    lower_u8_expression_to_word_with_temporaries, lower_usize_expression_to_location,
    lower_usize_expression_to_word, lower_usize_expression_to_word_with_temporaries,
    lower_void_expression_statement, push_store_slice_view_to_aggregate_field,
    push_store_str_view_to_aggregate_field,
};
use super::functions::{
    lower_drop_statement, lower_never_expression_with_scope_drops,
    lower_return_statement_with_scope_drops, lower_scope_end_drops_for_locals_since,
    propagating_failure_mode, replacement_drop_for_aggregate_slot,
};
use super::literals::{lower_u16_literal, lower_u32_literal};
use super::types::{
    return_type_expr_is_top_level_optional, scalar_or_view_type_from_type_expr,
    view_element_type_from_type_expr,
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
    AggregateLocation, BoolLocation, BorrowArgument, BorrowSource, FallibleFailureMode,
    I32Location, I32Value, Instruction, ScalarArgument, SliceElementIndex, SliceLocation,
    SliceValue, StrLocation, Type, U8Location, U8Value, UsizeLocation, UsizeValue,
};
use crate::typecheck::{TypecheckScalarViewKind, TypecheckSliceElementKind};

#[derive(Clone, Copy)]
pub(super) struct LoopControlContext<'a> {
    pub(super) loop_scope_mark: usize,
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
    if expression_contains_interpolated_string(&statement.initializer) {
        return Err(unsupported_interpolated_string_diagnostic());
    }

    if let Some(instructions) = lower_otherwise_scalar_binding(statement, context, loop_control)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_otherwise_aggregate_binding(statement, context, loop_control)?
    {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_aggregate_struct_literal_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_aggregate_array_literal_binding(statement, context)? {
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
        ScalarBindingKind::Bool => lower_bool_local_binding(statement, context),
        ScalarBindingKind::Str => lower_str_local_binding(statement, context),
        ScalarBindingKind::Slice(info) => lower_slice_local_binding(statement, context, info),
    }
}

fn optional_success_scalar_binding_kind(
    statement: &BindingStmt,
    success_type: &Type,
    context: &LoweringContext,
) -> Result<Option<ScalarBindingKind>, Vec<Diagnostic>> {
    let Some(ty) = &statement.ty else {
        return Ok(scalar_binding_kind_from_type(success_type));
    };

    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower annotated `otherwise` bindings without resolved type information",
        ));
    };
    Ok(match scalar_or_view_type_from_type_expr(ty, resolved) {
        Some(Type::I32) => Some(ScalarBindingKind::I32),
        Some(Type::U8) => Some(ScalarBindingKind::U8),
        Some(Type::Usize) => Some(ScalarBindingKind::Usize),
        Some(Type::Bool) => Some(ScalarBindingKind::Bool),
        Some(Type::Str) => Some(ScalarBindingKind::Str),
        Some(Type::Slice { .. }) => Some(ScalarBindingKind::Slice(slice_type_info_from_type_expr(
            ty, context,
        ))),
        _ => None,
    })
}

fn lower_otherwise_terminal_block(
    block: &Block,
    context: &mut LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(result) = &block.result {
        let mut instructions = Vec::new();
        for statement in &block.statements {
            instructions.extend(lower_otherwise_leading_statement(
                statement,
                context,
                loop_control,
            )?);
        }

        let Some(terminating_instructions) =
            lower_never_expression_with_scope_drops(result, context)?
        else {
            return Err(unsupported_binding_diagnostic(
                "IR v0 can only lower `otherwise` fallback blocks ending in `return`, `break`, `continue`, or a `never` expression",
            ));
        };
        instructions.extend(terminating_instructions);
        return Ok(instructions);
    }

    let Some((terminal, leading)) = block.statements.split_last() else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower empty `otherwise` fallback blocks",
        ));
    };

    let mut instructions = Vec::new();
    for statement in leading {
        instructions.extend(lower_otherwise_leading_statement(
            statement,
            context,
            loop_control,
        )?);
    }

    match terminal {
        Stmt::Return(statement) => {
            instructions.extend(lower_return_statement_with_scope_drops(
                statement, context, "E8008",
            )?);
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) =
                lower_never_expression_with_scope_drops(&statement.expression, context)?
            else {
                return Err(unsupported_binding_diagnostic(
                    "IR v0 can only lower `otherwise` fallback blocks ending in `return`, `break`, `continue`, or a `never` expression",
                ));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        Stmt::Break(_) => {
            instructions.extend(lower_otherwise_loop_control_statement(
                Instruction::Break,
                context,
                loop_control,
            )?);
            Ok(instructions)
        }
        Stmt::Continue(_) => {
            instructions.extend(lower_otherwise_loop_control_statement(
                Instruction::Continue,
                context,
                loop_control,
            )?);
            Ok(instructions)
        }
        _ => Err(unsupported_binding_diagnostic(
            "IR v0 can only lower `otherwise` fallback blocks ending in `return`, `break`, `continue`, or a `never` expression",
        )),
    }
}

fn lower_otherwise_leading_statement(
    statement: &Stmt,
    context: &mut LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => Ok(Vec::new()),
        Stmt::Binding(statement) => lower_local_binding_with_loop_control(
            statement,
            context,
            loop_control,
        ),
        Stmt::Assignment(statement) => lower_assignment(statement, context),
        Stmt::Drop(statement) => lower_drop_statement(statement, context),
        Stmt::Expression(statement) => {
            lower_void_expression_statement(&statement.expression, context)?.ok_or_else(|| {
                unsupported_binding_diagnostic(
                    "IR v0 can only lower `otherwise` leading expression statements that make effect-only calls",
                )
            })
        }
        _ => Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower this statement inside `otherwise` fallback blocks",
        )),
    }
}

fn lower_otherwise_loop_control_statement(
    instruction: Instruction,
    context: &mut LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(loop_control) = loop_control else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower `break` and `continue` inside `otherwise` fallback blocks when the binding is inside a nonterminal loop",
        ));
    };

    let mut instructions =
        lower_scope_end_drops_for_locals_since(context, loop_control.loop_scope_mark)?;
    if matches!(instruction, Instruction::Continue) {
        instructions.extend(loop_control.continue_instructions.iter().cloned());
    }
    instructions.push(instruction);
    Ok(instructions)
}

fn lower_otherwise_recover_or_handle_failure_mode<F>(
    fallback: &Block,
    context: &LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
    mut lower_result: F,
    unsupported_message: &'static str,
) -> Result<FallibleFailureMode, Vec<Diagnostic>>
where
    F: FnMut(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
{
    let mut fallback_context = context.clone();
    let local_mark = fallback_context.local_mark();

    if let Some(result) = &fallback.result {
        let mut instructions = Vec::new();
        for statement in &fallback.statements {
            instructions.extend(lower_otherwise_leading_statement(
                statement,
                &mut fallback_context,
                loop_control,
            )?);
        }

        if let Some(terminating_instructions) =
            lower_never_expression_with_scope_drops(result, &mut fallback_context)?
        {
            instructions.extend(terminating_instructions);
            return Ok(FallibleFailureMode::Handle { instructions });
        }

        instructions.extend(
            lower_result(result, &fallback_context)
                .map_err(|_| unsupported_binding_diagnostic(unsupported_message))?,
        );
        instructions.extend(lower_scope_end_drops_for_locals_since(
            &mut fallback_context,
            local_mark,
        )?);
        return Ok(FallibleFailureMode::Recover { instructions });
    }

    let instructions =
        lower_otherwise_terminal_block(fallback, &mut fallback_context, loop_control)?;
    Ok(FallibleFailureMode::Handle { instructions })
}

fn lower_otherwise_scalar_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Otherwise(otherwise) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return Ok(None);
    };

    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower `otherwise` bindings without resolved call information",
        ));
    };
    let Some(return_type) = context.call_return_type_expr(call) else {
        return Ok(None);
    };
    if !return_type_expr_is_top_level_optional(&return_type, resolved) {
        return Ok(None);
    }

    let Some((target, _call_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let Some(kind) =
        optional_success_scalar_binding_kind(statement, success_type.as_ref(), context)?
    else {
        return Ok(None);
    };
    lower_otherwise_scalar_call_binding(
        statement,
        call,
        &otherwise.fallback,
        kind,
        context,
        loop_control,
    )
    .map(Some)
}

fn lower_otherwise_scalar_call_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    fallback: &Block,
    kind: ScalarBindingKind,
    context: &mut LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let expression_context = context.with_reserved_local_abi_words(kind.abi_word_count());
    let mut temporaries = TemporaryAllocator::new(&expression_context)?;
    match kind {
        ScalarBindingKind::I32 => {
            let destination = context.next_i32_local_location()?;
            let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_i32_expression_to_location(expression, destination, context)
                },
                "IR v0 can only lower i32 `otherwise` fallback blocks that produce an i32 value or exit",
            )?;
            let instructions = lower_fallible_i32_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_i32_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::U8 => {
            let destination = context.next_u8_local_location()?;
            let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_u8_expression_to_location(expression, destination, context)
                },
                "IR v0 can only lower u8 `otherwise` fallback blocks that produce a u8 value or exit",
            )?;
            let instructions = lower_fallible_u8_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_u8_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::Usize => {
            let destination = context.next_usize_local_location()?;
            let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_usize_expression_to_location(expression, destination, context)
                },
                "IR v0 can only lower usize `otherwise` fallback blocks that produce a usize value or exit",
            )?;
            let instructions = lower_fallible_usize_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_usize_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::Bool => {
            let destination = context.next_bool_local_location()?;
            let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_bool_expression_to_location(expression, destination, context, "E8008")
                },
                "IR v0 can only lower bool `otherwise` fallback blocks that produce a bool value or exit",
            )?;
            let instructions = lower_fallible_bool_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_bool_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::Str => {
            let destination = context.next_str_local_location()?;
            let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_str_expression_to_location(expression, destination, context)
                },
                "IR v0 can only lower &str `otherwise` fallback blocks that produce a &str value or exit",
            )?;
            let instructions = lower_fallible_str_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_str_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::Slice(info) => {
            let destination = context.next_slice_local_location()?;
            let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_slice_expression_to_location(expression, destination, context)
                },
                "IR v0 can only lower slice `otherwise` fallback blocks that produce a slice value or exit",
            )?;
            let instructions = lower_fallible_slice_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_slice_local(
                statement.name.clone(),
                info.element_kind,
                info.element_type,
            );
            Ok(instructions)
        }
    }
}

fn lower_otherwise_aggregate_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Otherwise(otherwise) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return Ok(None);
    };

    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower aggregate `otherwise` bindings without resolved call information",
        ));
    };
    let Some(return_type) = context.call_return_type_expr(call) else {
        return Ok(None);
    };
    if !return_type_expr_is_top_level_optional(&return_type, resolved) {
        return Ok(None);
    }

    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let Some(layout) = aggregate_type_layout(success_type.as_ref()) else {
        return Ok(None);
    };
    validate_aggregate_binding_layout(layout)?;

    let is_copy = call_success_type_is_copy_struct(call, context);
    let drop_glue = call_success_drop_glue(call, context);
    let fields = call_success_aggregate_fields(call, context);
    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, drop_glue, fields);
    let failure_mode = lower_otherwise_aggregate_failure_mode(
        &otherwise.fallback,
        layout,
        AggregateLocation::Slot(slot_index),
        context,
        loop_control,
    )?;
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    let (mut argument_instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
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
    Ok(Some(instructions))
}

fn lower_otherwise_aggregate_failure_mode(
    fallback: &Block,
    layout: ValueLayout,
    destination: AggregateLocation,
    context: &LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<FallibleFailureMode, Vec<Diagnostic>> {
    lower_otherwise_recover_or_handle_failure_mode(
        fallback,
        context,
        loop_control,
        |expression, context| {
            lower_aggregate_member_value_assignment(destination, 0, layout, expression, context)
        },
        "IR v0 can only lower aggregate `otherwise` fallback blocks with supported aggregate values or exits",
    )
}

fn lower_aggregate_struct_literal_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::StructLiteral(literal) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Some((root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower aggregate struct literal bindings without resolved type information",
        ));
    };

    let value = abi_value_from_type_expr(&literal.ty, resolved).map_err(|_error| {
        unsupported_binding_diagnostic(
            "IR v0 can only lower local aggregate bindings whose initializer has an ABI layout",
        )
    })?;
    validate_aggregate_binding_layout(value.layout)?;

    let is_copy = type_expr_is_copy_struct(&literal.ty, resolved);
    let drop_glue = context.drop_glue_for_type_expr(&literal.ty);
    let fields =
        aggregate_fields_from_type_expr(&literal.ty, root_source, resolved).unwrap_or_default();
    let slot_index = context.define_aggregate_local(
        statement.name.clone(),
        value.layout,
        is_copy,
        drop_glue,
        fields,
    );
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: value.layout,
    }];
    instructions.extend(lower_aggregate_struct_literal_to_location(
        literal,
        value.layout,
        AggregateLocation::Slot(slot_index),
        "E8008",
        "local bindings",
        resolved,
        context,
    )?);
    Ok(Some(instructions))
}

fn lower_aggregate_array_literal_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::ArrayLiteral(literal) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower fixed array literal bindings without resolved type information",
        ));
    };
    let Some(ty) = context
        .binding_type_expr(statement.name_span)
        .or_else(|| statement.ty.clone())
    else {
        return Ok(None);
    };

    let value = abi_value_from_type_expr_with_resolver(&ty, resolved, |source| {
        context.resolved_source(source)
    })
    .map_err(|_error| {
        unsupported_binding_diagnostic(
            "IR v0 can only lower fixed array literal bindings whose type has an ABI layout",
        )
    })?;
    if !matches!(value.ty, AbiType::Array { .. }) {
        return Ok(None);
    }
    validate_aggregate_binding_layout(value.layout)?;

    let slot_index = context.define_aggregate_local(
        statement.name.clone(),
        value.layout,
        true,
        None,
        Vec::new(),
    );
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: value.layout,
    }];
    instructions.extend(lower_aggregate_array_literal_to_location(
        literal,
        &value.ty,
        value.layout,
        AggregateLocation::Slot(slot_index),
        "E8008",
        "local bindings",
        resolved,
        context,
    )?);
    Ok(Some(instructions))
}

fn lower_aggregate_call_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match unwrap_group(&statement.initializer) {
        Expr::Call(call) => lower_aggregate_normal_call_binding(statement, call, context),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Ok(None);
            };
            lower_aggregate_fallible_call_binding(
                statement,
                call,
                propagating_failure_mode(context)?,
                context,
            )
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Ok(None);
            };
            lower_aggregate_fallible_call_binding(
                statement,
                call,
                FallibleFailureMode::Trap,
                context,
            )
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Ok(None);
            };
            lower_aggregate_fallible_call_binding(
                statement,
                call,
                lower_catch_failure_mode(catch, context, 0)?,
                context,
            )
        }
        _ => Ok(None),
    }
}

fn lower_aggregate_normal_call_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if macos_syscall_primitive_call(call, context)
        && let Some(layout) = aggregate_call_return_layout_from_resolved(call, context)
    {
        validate_aggregate_binding_layout(layout)?;
        let is_copy = call_success_type_is_copy_struct(call, context);
        let drop_glue = call_success_drop_glue(call, context);
        let fields = call_success_aggregate_fields(call, context);
        let slot_index = context.define_aggregate_local(
            statement.name.clone(),
            layout,
            is_copy,
            drop_glue,
            fields,
        );
        let mut temporaries = TemporaryAllocator::new(context)?;
        let Some(mut syscall_instructions) = lower_macos_syscall_primitive_call_to_location(
            call,
            AggregateLocation::Slot(slot_index),
            layout,
            context,
            &mut temporaries,
        )?
        else {
            return Ok(None);
        };
        let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
        instructions.append(&mut syscall_instructions);
        return Ok(Some(instructions));
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
    validate_aggregate_binding_layout(layout)?;

    let is_copy = call_success_type_is_copy_struct(call, context);
    let drop_glue = call_success_drop_glue(call, context);
    let fields = call_success_aggregate_fields(call, context);
    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, drop_glue, fields);
    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    instructions.insert(0, Instruction::ReserveAggregateSlot { slot_index, layout });
    push_aggregate_call_instruction(
        &mut instructions,
        &return_type,
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
    );
    Ok(Some(instructions))
}

fn lower_aggregate_fallible_call_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    failure_mode: FallibleFailureMode,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };

    let Some(Type::Fallible(success)) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let Some(layout) = aggregate_type_layout(success.as_ref()) else {
        return Ok(None);
    };
    validate_aggregate_binding_layout(layout)?;

    let is_copy = call_success_type_is_copy_struct(call, context);
    let drop_glue = call_success_drop_glue(call, context);
    let fields = call_success_aggregate_fields(call, context);
    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, drop_glue, fields);
    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    instructions.insert(0, Instruction::ReserveAggregateSlot { slot_index, layout });
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success.as_ref(),
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
        failure_mode,
    );
    Ok(Some(instructions))
}

fn lower_aggregate_copy_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Some(source) = context.aggregate_local(&identifier.name) else {
        return Ok(None);
    };
    if !source.is_copy || !supported_aggregate_copy_layout(source.layout) {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate copy bindings from copy aggregate locals",
        ));
    }
    let Some(fields) = context.aggregate_local_fields(&identifier.name) else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower aggregate copy bindings without aggregate field metadata",
        ));
    };

    let slot_index = context.define_aggregate_local(
        statement.name.clone(),
        source.layout,
        source.is_copy,
        source.drop_glue.clone(),
        fields,
    );
    Ok(Some(vec![
        Instruction::ReserveAggregateSlot {
            slot_index,
            layout: source.layout,
        },
        Instruction::CopyAggregate {
            destination: AggregateLocation::Slot(slot_index),
            source: AggregateLocation::Slot(source.slot_index),
            layout: source.layout,
        },
    ]))
}

fn lower_aggregate_move_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Unary(unary) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    if unary.operator != UnaryOperator::Move {
        return Ok(None);
    }
    let Expr::Identifier(identifier) = unwrap_group(&unary.operand) else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate move bindings from `move name` initializers",
        ));
    };
    let Some(source) = context.aggregate_local(&identifier.name) else {
        return Ok(None);
    };
    if !supported_aggregate_copy_layout(source.layout) {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate move bindings for supported aggregate layouts",
        ));
    }
    let Some(fields) = context.aggregate_local_fields(&identifier.name) else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower aggregate move bindings without aggregate field metadata",
        ));
    };

    let slot_index = context.define_aggregate_local(
        statement.name.clone(),
        source.layout,
        source.is_copy,
        source.drop_glue.clone(),
        fields,
    );
    Ok(Some(vec![
        Instruction::ReserveAggregateSlot {
            slot_index,
            layout: source.layout,
        },
        Instruction::CopyAggregate {
            destination: AggregateLocation::Slot(slot_index),
            source: AggregateLocation::Slot(source.slot_index),
            layout: source.layout,
        },
    ]))
}

fn lower_aggregate_member_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Member(member) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };

    match aggregate_member_binding_path(member, context)? {
        Some((AggregateMemberBindingRoot::Identifier(identifier_name), field_path)) => {
            lower_aggregate_local_member_binding(statement, identifier_name, &field_path, context)
        }
        Some((AggregateMemberBindingRoot::Call(call), field_path)) => {
            lower_aggregate_call_member_binding(statement, call, &field_path, context)
        }
        Some((AggregateMemberBindingRoot::FallibleCall(call, failure_mode), field_path)) => {
            lower_aggregate_fallible_call_member_binding(
                statement,
                call,
                &field_path,
                failure_mode,
                context,
            )
        }
        None => Ok(None),
    }
}

fn lower_aggregate_local_member_binding(
    statement: &BindingStmt,
    identifier_name: &str,
    field_path: &str,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(field) = context.aggregate_field(identifier_name, field_path) else {
        return Ok(None);
    };
    let source = field.source;
    let source_offset = field.offset;
    let is_copy = field.is_copy;
    let AggregateFieldKind::Aggregate { layout, fields } = field.kind else {
        return Ok(None);
    };
    if !is_copy || !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate member bindings from copy aggregate fields",
        ));
    }

    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, None, fields);
    Ok(Some(vec![
        Instruction::ReserveAggregateSlot { slot_index, layout },
        Instruction::CopyAggregateRange {
            destination: AggregateLocation::Slot(slot_index),
            destination_offset: 0,
            source,
            source_offset,
            layout,
        },
    ]))
}

fn lower_aggregate_call_member_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    field_path: &str,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };
    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let Some(source_layout) = aggregate_type_layout(&return_type) else {
        return Ok(None);
    };
    let Some(field) = aggregate_call_field(call, field_path, context) else {
        return Ok(None);
    };
    let source_offset = field.offset;
    let is_copy = field.is_copy;
    let AggregateFieldKind::Aggregate { layout, fields } = field.kind else {
        return Ok(None);
    };
    if !is_copy
        || !supported_aggregate_copy_layout(layout)
        || !supported_aggregate_copy_layout(source_layout)
    {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate member bindings from copy aggregate fields",
        ));
    }

    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, None, fields);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let source_slot = temporaries.next_aggregate_slot();
    let mut instructions = vec![
        Instruction::ReserveAggregateSlot { slot_index, layout },
        Instruction::ReserveAggregateSlot {
            slot_index: source_slot,
            layout: source_layout,
        },
    ];
    let (mut argument_instructions, arguments) =
        lower_call_arguments_to_scalar_arguments_with_temporaries(
            call,
            &target,
            &call_name,
            context,
            &mut temporaries,
        )?;
    instructions.append(&mut argument_instructions);
    push_aggregate_call_instruction(
        &mut instructions,
        &return_type,
        AggregateLocation::Slot(source_slot),
        target,
        arguments,
        source_layout,
    );
    instructions.push(Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(slot_index),
        destination_offset: 0,
        source: AggregateLocation::Slot(source_slot),
        source_offset,
        layout,
    });
    Ok(Some(instructions))
}

fn lower_aggregate_fallible_call_member_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    field_path: &str,
    failure_mode: FallibleFailureMode,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let Some(source_layout) = aggregate_type_layout(success_type.as_ref()) else {
        return Ok(None);
    };
    let Some(field) = aggregate_call_field(call, field_path, context) else {
        return Ok(None);
    };
    let source_offset = field.offset;
    let is_copy = field.is_copy;
    let AggregateFieldKind::Aggregate { layout, fields } = field.kind else {
        return Ok(None);
    };
    if !is_copy
        || !supported_aggregate_copy_layout(layout)
        || !supported_aggregate_copy_layout(source_layout)
    {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate member bindings from copy fallible aggregate fields",
        ));
    }

    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, None, fields);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let source_slot = temporaries.next_aggregate_slot();
    let mut instructions = vec![
        Instruction::ReserveAggregateSlot { slot_index, layout },
        Instruction::ReserveAggregateSlot {
            slot_index: source_slot,
            layout: source_layout,
        },
    ];
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
        AggregateLocation::Slot(source_slot),
        target,
        arguments,
        source_layout,
        failure_mode,
    );
    instructions.push(Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(slot_index),
        destination_offset: 0,
        source: AggregateLocation::Slot(source_slot),
        source_offset,
        layout,
    });
    Ok(Some(instructions))
}

fn lower_aggregate_slice_index_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Index(index) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Some(element) = copy_aggregate_slice_index_element(index, context) else {
        return Ok(None);
    };

    let slot_index = context.define_aggregate_local(
        statement.name.clone(),
        element.layout,
        true,
        element.drop_glue,
        element.fields,
    );
    let mut temporaries = TemporaryAllocator::new(context)?;
    let lowered_slice = lower_slice_expression_to_value(&index.object, context, &mut temporaries)?;
    let SliceValue::Location(source) = lowered_slice.value else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate slice index bindings from slice locations",
        ));
    };
    let (index_instructions, element_index) =
        lower_usize_expression_to_word_with_temporaries(&index.index, context, &mut temporaries)?;

    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: element.layout,
    }];
    instructions.extend(lowered_slice.instructions);
    instructions.extend(index_instructions);
    let element_index =
        materialize_slice_aggregate_index(&mut instructions, element_index, &mut temporaries)?;
    instructions.push(Instruction::CopySliceElementToAggregate {
        destination: AggregateLocation::Slot(slot_index),
        source,
        index: element_index,
        layout: element.layout,
    });
    Ok(Some(instructions))
}

enum AggregateMemberBindingRoot<'a> {
    Identifier(&'a str),
    Call(&'a CallExpr),
    FallibleCall(&'a CallExpr, FallibleFailureMode),
}

struct CopyAggregateSliceElement {
    layout: ValueLayout,
    fields: Vec<super::context::AggregateField>,
    drop_glue: Option<DropGlue>,
}

fn copy_aggregate_slice_index_element(
    index: &IndexExpr,
    context: &LoweringContext,
) -> Option<CopyAggregateSliceElement> {
    let element_ty = slice_index_element_type_expr(index, context)?;
    let (root_source, resolved) = context.resolved_calls()?;
    if !type_expr_is_copy_struct_with_resolver(&element_ty, resolved, |source| {
        context.resolved_source(source)
    }) {
        return None;
    }

    let value = abi_value_from_type_expr_with_resolver(&element_ty, resolved, |source| {
        context.resolved_source(source)
    })
    .ok()?;
    if !matches!(value.ty, AbiType::Struct(_)) || !supported_aggregate_copy_layout(value.layout) {
        return None;
    }
    let fields = aggregate_fields_from_type_expr_with_resolver(
        &element_ty,
        root_source,
        resolved,
        |source| context.resolved_source(source),
    )?;
    Some(CopyAggregateSliceElement {
        layout: value.layout,
        fields,
        drop_glue: context.drop_glue_for_type_expr(&element_ty),
    })
}

fn slice_index_element_type_expr(index: &IndexExpr, context: &LoweringContext) -> Option<TypeExpr> {
    slice_target_element_type_expr(&index.object, context)
}

fn slice_target_element_type_expr(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<TypeExpr> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => context.slice_element_type_expr(&identifier.name).cloned(),
        Expr::Member(member) => match aggregate_member_field_kind_from_member(member, context)
            .ok()
            .flatten()
        {
            Some(AggregateFieldKind::Slice(info)) => info.element_type,
            _ => None,
        },
        Expr::Call(call) => {
            let return_type = context.call_return_type_expr(call)?;
            slice_element_type_expr_from_type_expr(&return_type, context)
        }
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return None;
            };
            let TypeExpr::Fallible(fallible) = context.call_return_type_expr(call)? else {
                return None;
            };
            slice_element_type_expr_from_type_expr(&fallible.success, context)
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return None;
            };
            let TypeExpr::Fallible(fallible) = context.call_return_type_expr(call)? else {
                return None;
            };
            slice_element_type_expr_from_type_expr(&fallible.success, context)
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return None;
            };
            let TypeExpr::Fallible(fallible) = context.call_return_type_expr(call)? else {
                return None;
            };
            slice_element_type_expr_from_type_expr(&fallible.success, context)
        }
        Expr::Group(group) => slice_target_element_type_expr(&group.expression, context),
        _ => None,
    }
}

fn slice_element_type_expr_from_type_expr(
    ty: &TypeExpr,
    context: &LoweringContext,
) -> Option<TypeExpr> {
    let (_root_source, resolved) = context.resolved_calls()?;
    slice_element_type_expr_from_type_expr_with_resolved(ty, resolved, context)
}

fn slice_element_type_expr_from_type_expr_with_resolved(
    ty: &TypeExpr,
    resolved: &crate::resolve::ResolveOutput,
    context: &LoweringContext,
) -> Option<TypeExpr> {
    match ty {
        TypeExpr::Borrow(borrow) => {
            let TypeExpr::View(view) = borrow.inner.as_ref() else {
                return None;
            };
            Some(*view.element.clone())
        }
        TypeExpr::Reference(reference) => {
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            let target_resolved = context
                .resolved_source(target.span().source)
                .unwrap_or(resolved);
            slice_element_type_expr_from_type_expr_with_resolved(target, target_resolved, context)
        }
        _ => None,
    }
}

fn aggregate_member_binding_path<'a>(
    member: &'a MemberExpr,
    context: &LoweringContext,
) -> Result<Option<(AggregateMemberBindingRoot<'a>, String)>, Vec<Diagnostic>> {
    let Some((root, mut fields)) = aggregate_member_binding_root_and_path(&member.object, context)?
    else {
        return Ok(None);
    };
    fields.push(member.member.as_str());
    Ok(Some((root, fields.join("."))))
}

fn aggregate_member_binding_root_and_path<'a>(
    expression: &'a Expr,
    context: &LoweringContext,
) -> Result<Option<(AggregateMemberBindingRoot<'a>, Vec<&'a str>)>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Ok(Some((
            AggregateMemberBindingRoot::Identifier(&identifier.name),
            Vec::new(),
        ))),
        Expr::Call(call) => Ok(Some((AggregateMemberBindingRoot::Call(call), Vec::new()))),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Ok(None);
            };
            Ok(Some((
                AggregateMemberBindingRoot::FallibleCall(call, propagating_failure_mode(context)?),
                Vec::new(),
            )))
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Ok(None);
            };
            Ok(Some((
                AggregateMemberBindingRoot::FallibleCall(call, FallibleFailureMode::Trap),
                Vec::new(),
            )))
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Ok(None);
            };
            Ok(Some((
                AggregateMemberBindingRoot::FallibleCall(
                    call,
                    lower_catch_failure_mode(catch, context, 0)?,
                ),
                Vec::new(),
            )))
        }
        Expr::Member(member) => {
            let Some((root, mut fields)) =
                aggregate_member_binding_root_and_path(&member.object, context)?
            else {
                return Ok(None);
            };
            fields.push(member.member.as_str());
            Ok(Some((root, fields)))
        }
        _ => Ok(None),
    }
}

fn validate_aggregate_binding_layout(
    layout: crate::abi::ValueLayout,
) -> Result<(), Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate call bindings with non-empty ABI layouts",
        ));
    }
    Ok(())
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

fn expression_is_pointer_address_value(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Call(call) => matches!(
            context.primitive_name_for_call(call),
            Some("from_addr" | "from_ref" | "from_ref_mut")
        ),
        Expr::Group(group) => expression_is_pointer_address_value(&group.expression, context),
        _ => false,
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
            lower_identifier_assignment(identifier, &statement.value, context)
        }
        Expr::Member(member) => lower_aggregate_field_assignment(member, &statement.value, context),
        Expr::Index(index) => lower_index_assignment(index, &statement.value, context),
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

fn lower_compound_assignment(
    statement: &AssignmentStmt,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(&statement.target) {
        Expr::Identifier(identifier) => {
            lower_compound_identifier_assignment(statement, identifier, context)
        }
        Expr::Member(member) => {
            lower_compound_aggregate_field_assignment(statement, member, context)
        }
        Expr::Index(index) => lower_compound_slice_index_assignment(statement, index, context),
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

fn lower_compound_identifier_assignment(
    statement: &AssignmentStmt,
    identifier: &crate::ast::IdentifierExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(destination) = context.i32_location(&identifier.name) {
        let I32Location::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        let (mut instructions, right) = lower_i32_expression_to_word(&statement.value, context)?;
        instructions.push(i32_compound_assignment_instruction(
            statement.operator,
            destination,
            right,
        )?);
        return Ok(instructions);
    }

    if let Some(destination) = context.usize_location(&identifier.name) {
        let UsizeLocation::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        let (mut instructions, right) = lower_usize_expression_to_word(&statement.value, context)?;
        instructions.push(usize_compound_assignment_instruction(
            statement.operator,
            destination,
            right,
        )?);
        return Ok(instructions);
    }

    if let Some(destination) = context.u8_location(&identifier.name) {
        let U8Location::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        let (mut instructions, right) = lower_u8_expression_to_word(&statement.value, context)?;
        instructions.push(u8_compound_assignment_instruction(
            statement.operator,
            destination,
            right,
        )?);
        return Ok(instructions);
    }

    Err(unsupported_assignment_diagnostic())
}

fn lower_compound_aggregate_field_assignment(
    statement: &AssignmentStmt,
    target: &MemberExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((identifier_name, field_path)) = aggregate_assignment_target_path(target) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(field) = context.aggregate_field(identifier_name, &field_path) else {
        return Err(unsupported_assignment_diagnostic());
    };
    if !field.is_readwrite {
        return Err(unsupported_assignment_diagnostic());
    }
    match field.kind {
        AggregateFieldKind::I32 => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let (mut instructions, right) = lower_i32_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            let current = temporaries.next_i32()?;
            instructions.push(Instruction::LoadAggregateI32 {
                destination: current,
                source: field.source,
                offset: field.offset,
            });
            instructions.push(i32_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreAggregateI32 {
                destination: field.source,
                offset: field.offset,
                value: I32Value::Location(current),
            });
            Ok(instructions)
        }
        AggregateFieldKind::Usize => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let (mut instructions, right) = lower_usize_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            let current = temporaries.next_usize()?;
            instructions.push(Instruction::LoadAggregateUsize {
                destination: current,
                source: field.source,
                offset: field.offset,
            });
            instructions.push(usize_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreAggregateUsize {
                destination: field.source,
                offset: field.offset,
                value: UsizeValue::Location(current),
            });
            Ok(instructions)
        }
        AggregateFieldKind::U8 => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let (mut instructions, right) = lower_u8_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            let current = temporaries.next_u8()?;
            instructions.push(Instruction::LoadAggregateU8 {
                destination: current,
                source: field.source,
                offset: field.offset,
            });
            instructions.push(u8_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreAggregateU8 {
                destination: field.source,
                offset: field.offset,
                value: U8Value::Location(current),
            });
            Ok(instructions)
        }
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

fn lower_compound_slice_index_assignment(
    statement: &AssignmentStmt,
    target: &IndexExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let element_kind = slice_index_assignment_element_kind(&target.object, context);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let lowered_slice = lower_slice_expression_to_value(&target.object, context, &mut temporaries)?;
    let SliceValue::Location(destination) = lowered_slice.value else {
        return Err(unsupported_assignment_diagnostic());
    };
    let (index_instructions, index) =
        lower_usize_expression_to_word_with_temporaries(&target.index, context, &mut temporaries)?;
    let mut instructions = lowered_slice.instructions;
    instructions.extend(index_instructions);
    let index =
        materialize_slice_index_assignment_index(&mut instructions, index, &mut temporaries)?;

    match element_kind {
        TypecheckSliceElementKind::I32 => {
            let (value_instructions, right) = lower_i32_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            instructions.extend(value_instructions);
            let current = temporaries.next_i32()?;
            instructions.push(Instruction::SetI32 {
                destination: current,
                value: I32Value::SliceIndex {
                    source: destination,
                    index: index.clone(),
                },
            });
            instructions.push(i32_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreI32ToSliceIndex {
                destination,
                index,
                value: I32Value::Location(current),
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::Usize => {
            let (value_instructions, right) = lower_usize_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            instructions.extend(value_instructions);
            let current = temporaries.next_usize()?;
            instructions.push(Instruction::SetUsize {
                destination: current,
                value: UsizeValue::SliceIndex {
                    source: destination,
                    index: Box::new(index.clone()),
                },
            });
            instructions.push(usize_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreUsizeToSliceIndex {
                destination,
                index,
                value: UsizeValue::Location(current),
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::U8 => {
            let (value_instructions, right) = lower_u8_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            instructions.extend(value_instructions);
            let current = temporaries.next_u8()?;
            instructions.push(Instruction::SetU8 {
                destination: current,
                value: U8Value::SliceIndex {
                    source: destination,
                    index: index.clone(),
                },
            });
            instructions.push(u8_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreU8ToSliceIndex {
                destination,
                index,
                value: U8Value::Location(current),
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::Bool
        | TypecheckSliceElementKind::Str
        | TypecheckSliceElementKind::Other => Err(unsupported_assignment_diagnostic()),
    }
}

fn i32_compound_assignment_instruction(
    operator: AssignmentOperator,
    destination: I32Location,
    right: I32Value,
) -> Result<Instruction, Vec<Diagnostic>> {
    let left = I32Value::Location(destination);
    match operator {
        AssignmentOperator::AddAssign => Ok(Instruction::AddI32 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::SubtractAssign => Ok(Instruction::SubtractI32 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::MultiplyAssign => Ok(Instruction::MultiplyI32 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::DivideAssign => Ok(Instruction::DivideI32 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::RemainderAssign => Ok(Instruction::RemainderI32 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::Assign => Err(unsupported_assignment_diagnostic()),
    }
}

fn usize_compound_assignment_instruction(
    operator: AssignmentOperator,
    destination: UsizeLocation,
    right: UsizeValue,
) -> Result<Instruction, Vec<Diagnostic>> {
    let left = UsizeValue::Location(destination);
    match operator {
        AssignmentOperator::AddAssign => Ok(Instruction::AddUsize {
            destination,
            left,
            right,
        }),
        AssignmentOperator::SubtractAssign => Ok(Instruction::SubtractUsize {
            destination,
            left,
            right,
        }),
        AssignmentOperator::MultiplyAssign => Ok(Instruction::MultiplyUsize {
            destination,
            left,
            right,
        }),
        AssignmentOperator::DivideAssign => Ok(Instruction::DivideUsize {
            destination,
            left,
            right,
        }),
        AssignmentOperator::RemainderAssign => Ok(Instruction::RemainderUsize {
            destination,
            left,
            right,
        }),
        AssignmentOperator::Assign => Err(unsupported_assignment_diagnostic()),
    }
}

fn u8_compound_assignment_instruction(
    operator: AssignmentOperator,
    destination: U8Location,
    right: U8Value,
) -> Result<Instruction, Vec<Diagnostic>> {
    let left = U8Value::Location(destination);
    match operator {
        AssignmentOperator::AddAssign => Ok(Instruction::AddU8 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::SubtractAssign => Ok(Instruction::SubtractU8 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::MultiplyAssign => Ok(Instruction::MultiplyU8 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::DivideAssign => Ok(Instruction::DivideU8 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::RemainderAssign => Ok(Instruction::RemainderU8 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::Assign => Err(unsupported_assignment_diagnostic()),
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

fn lower_identifier_assignment(
    identifier: &crate::ast::IdentifierExpr,
    value: &Expr,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(destination) = context.i32_location(&identifier.name) {
        let I32Location::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        return lower_i32_expression_to_location(value, destination, context);
    }

    if let Some(destination) = context.u8_location(&identifier.name) {
        let U8Location::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        return lower_u8_expression_to_location(value, destination, context);
    }

    if let Some(destination) = context.usize_location(&identifier.name) {
        let UsizeLocation::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        return lower_usize_expression_to_location(value, destination, context);
    }

    if let Some(destination) = context.bool_location(&identifier.name) {
        let BoolLocation::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        return lower_bool_expression_to_location(value, destination, context, "E8008");
    }

    if let Some(destination) = context.str_location(&identifier.name) {
        let StrLocation::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        return lower_str_expression_to_location(value, destination, context);
    }

    if let Some(destination) = context.slice_location(&identifier.name) {
        let SliceLocation::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        return lower_slice_expression_to_location(value, destination, context);
    }

    if let Some((slot_index, layout)) = context.aggregate_slot(&identifier.name) {
        let target_type = context.local_binding_type_expr_for_identifier(identifier);
        return lower_aggregate_assignment(
            slot_index,
            layout,
            target_type.as_ref(),
            value,
            context,
        );
    }

    Err(unsupported_assignment_diagnostic())
}

fn lower_index_assignment(
    target: &IndexExpr,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_fixed_array_index_assignment(target, value, context)? {
        return Ok(instructions);
    }
    lower_slice_index_assignment(target, value, context)
}

fn lower_fixed_array_index_assignment(
    target: &IndexExpr,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(access) =
        fixed_array_element_access(target, context, unsupported_assignment_diagnostic)?
    else {
        return Ok(None);
    };

    let mut temporaries = TemporaryAllocator::new(context)?;
    match access.element {
        AbiType::I32 => {
            let (mut instructions, value) =
                lower_i32_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            if access.out_of_bounds {
                instructions.push(Instruction::Trap);
            } else {
                instructions.push(Instruction::StoreAggregateI32 {
                    destination: access.source,
                    offset: access.offset,
                    value,
                });
            }
            Ok(Some(instructions))
        }
        AbiType::U8 => {
            let (mut instructions, value) =
                lower_u8_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            if access.out_of_bounds {
                instructions.push(Instruction::Trap);
            } else {
                instructions.push(Instruction::StoreAggregateU8 {
                    destination: access.source,
                    offset: access.offset,
                    value,
                });
            }
            Ok(Some(instructions))
        }
        AbiType::Usize => {
            let (mut instructions, value) =
                lower_usize_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            if access.out_of_bounds {
                instructions.push(Instruction::Trap);
            } else {
                instructions.push(Instruction::StoreAggregateUsize {
                    destination: access.source,
                    offset: access.offset,
                    value,
                });
            }
            Ok(Some(instructions))
        }
        AbiType::Bool => {
            let mut lowered = lower_bool_expression_to_value_with_temporaries(
                value,
                context,
                "E8008",
                &mut temporaries,
            )?;
            if access.out_of_bounds {
                lowered.instructions.push(Instruction::Trap);
            } else {
                lowered.instructions.push(Instruction::StoreAggregateBool {
                    destination: access.source,
                    offset: access.offset,
                    value: lowered.value,
                });
            }
            Ok(Some(lowered.instructions))
        }
        AbiType::StrView => {
            let mut lowered = lower_str_expression_to_value(value, context, &mut temporaries)?;
            if access.out_of_bounds {
                lowered.instructions.push(Instruction::Trap);
            } else {
                push_store_str_view_to_aggregate_field(
                    &mut lowered.instructions,
                    access.source,
                    access.offset,
                    lowered.value,
                    &mut temporaries,
                    unsupported_assignment_diagnostic,
                )?;
            }
            Ok(Some(lowered.instructions))
        }
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

fn lower_slice_index_assignment(
    target: &IndexExpr,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let element_kind = slice_index_assignment_element_kind(&target.object, context);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let lowered_slice = lower_slice_expression_to_value(&target.object, context, &mut temporaries)?;
    let SliceValue::Location(destination) = lowered_slice.value else {
        return Err(unsupported_assignment_diagnostic());
    };
    let (index_instructions, index) =
        lower_usize_expression_to_word_with_temporaries(&target.index, context, &mut temporaries)?;
    let mut instructions = lowered_slice.instructions;
    instructions.extend(index_instructions);
    let index =
        materialize_slice_index_assignment_index(&mut instructions, index, &mut temporaries)?;

    match element_kind {
        TypecheckSliceElementKind::U8 => {
            let (value_instructions, value) =
                lower_u8_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            instructions.extend(value_instructions);
            instructions.push(Instruction::StoreU8ToSliceIndex {
                destination,
                index,
                value,
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::I32 => {
            let (value_instructions, value) =
                lower_i32_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            instructions.extend(value_instructions);
            instructions.push(Instruction::StoreI32ToSliceIndex {
                destination,
                index,
                value,
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::Usize => {
            let (value_instructions, value) =
                lower_usize_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            instructions.extend(value_instructions);
            instructions.push(Instruction::StoreUsizeToSliceIndex {
                destination,
                index,
                value,
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::Bool => {
            let mut lowered = lower_bool_expression_to_value_with_temporaries(
                value,
                context,
                "E8008",
                &mut temporaries,
            )?;
            instructions.append(&mut lowered.instructions);
            instructions.push(Instruction::StoreBoolToSliceIndex {
                destination,
                index,
                value: lowered.value,
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::Str => {
            let mut lowered = lower_str_expression_to_value(value, context, &mut temporaries)?;
            instructions.append(&mut lowered.instructions);
            instructions.push(Instruction::StoreStrToSliceIndex {
                destination,
                index,
                value: lowered.value,
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::Other => lower_copy_aggregate_slice_index_assignment(
            target,
            value,
            destination,
            index,
            instructions,
            context,
            &mut temporaries,
        ),
    }
}

fn lower_copy_aggregate_slice_index_assignment(
    target: &IndexExpr,
    value: &Expr,
    destination: SliceLocation,
    index: UsizeValue,
    mut instructions: Vec<Instruction>,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(element) = copy_aggregate_slice_index_element(target, context) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let source_slot = temporaries.next_aggregate_slot();
    instructions.push(Instruction::ReserveAggregateSlot {
        slot_index: source_slot,
        layout: element.layout,
    });
    instructions.extend(lower_copy_aggregate_value_to_slot_with_temporaries(
        source_slot,
        element.layout,
        value,
        context,
        temporaries,
    )?);
    let index = materialize_slice_aggregate_index(&mut instructions, index, temporaries)?;
    instructions.push(Instruction::CopyAggregateToSliceElement {
        destination,
        index,
        source: AggregateLocation::Slot(source_slot),
        layout: element.layout,
    });
    Ok(instructions)
}

fn lower_copy_aggregate_value_to_slot_with_temporaries(
    slot_index: usize,
    layout: ValueLayout,
    value: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(value) {
        Expr::StructLiteral(literal) => {
            lower_aggregate_struct_literal_to_location_with_temporaries(
                literal,
                layout,
                AggregateLocation::Slot(slot_index),
                "E8008",
                "slice index assignments",
                context
                    .resolved_calls()
                    .map(|(_root_source, resolved)| resolved)
                    .ok_or_else(unsupported_assignment_diagnostic)?,
                context,
                temporaries,
            )
        }
        Expr::Identifier(identifier) => {
            lower_aggregate_copy_assignment(slot_index, layout, &identifier.name, context)
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unwrap_group(&unary.operand) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_move_assignment(slot_index, layout, &identifier.name, context)
        }
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

fn materialize_slice_index_assignment_index(
    instructions: &mut Vec<Instruction>,
    value: UsizeValue,
    temporaries: &mut TemporaryAllocator,
) -> Result<UsizeValue, Vec<Diagnostic>> {
    match value {
        UsizeValue::Const(_) | UsizeValue::Location(_) => Ok(value),
        _ => {
            let destination = temporaries.next_usize()?;
            instructions.push(Instruction::SetUsize { destination, value });
            Ok(UsizeValue::Location(destination))
        }
    }
}

fn materialize_slice_aggregate_index(
    instructions: &mut Vec<Instruction>,
    value: UsizeValue,
    temporaries: &mut TemporaryAllocator,
) -> Result<SliceElementIndex, Vec<Diagnostic>> {
    match value {
        UsizeValue::Const(value) => Ok(SliceElementIndex::Const(value)),
        UsizeValue::Location(location) => Ok(SliceElementIndex::Location(location)),
        value => {
            let destination = temporaries.next_usize()?;
            instructions.push(Instruction::SetUsize { destination, value });
            Ok(SliceElementIndex::Location(destination))
        }
    }
}

fn slice_index_assignment_element_kind(
    object: &Expr,
    context: &LoweringContext,
) -> TypecheckSliceElementKind {
    match unwrap_group(object) {
        Expr::Identifier(identifier) => context
            .slice_element_kind(&identifier.name)
            .unwrap_or(TypecheckSliceElementKind::Other),
        Expr::Call(call) => call_return_slice_element_kind(call, context)
            .unwrap_or(TypecheckSliceElementKind::Other),
        Expr::Member(member) => match aggregate_member_field_kind_from_member(member, context)
            .ok()
            .flatten()
        {
            Some(AggregateFieldKind::Slice(info)) => info.element_kind,
            _ => TypecheckSliceElementKind::Other,
        },
        Expr::Propagate(propagation) => slice_index_assignment_fallible_element_kind(
            unwrap_group(&propagation.expression),
            context,
        ),
        Expr::Force(force) => {
            slice_index_assignment_fallible_element_kind(unwrap_group(&force.expression), context)
        }
        Expr::Catch(catch) => {
            slice_index_assignment_fallible_element_kind(unwrap_group(&catch.expression), context)
        }
        Expr::Group(group) => slice_index_assignment_element_kind(&group.expression, context),
        _ => TypecheckSliceElementKind::Other,
    }
}

fn slice_index_assignment_fallible_element_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> TypecheckSliceElementKind {
    let Expr::Call(call) = expression else {
        return TypecheckSliceElementKind::Other;
    };
    call_success_slice_element_kind(call, context).unwrap_or(TypecheckSliceElementKind::Other)
}

fn lower_aggregate_field_assignment(
    target: &MemberExpr,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((identifier_name, field_path)) = aggregate_assignment_target_path(target) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(field) = context.aggregate_field(identifier_name, &field_path) else {
        return Err(unsupported_assignment_diagnostic());
    };
    if !field.is_readwrite {
        return Err(unsupported_assignment_diagnostic());
    }
    let destination = field.source;
    let offset = field.offset;
    let field_is_copy = field.is_copy;
    let field_drop_glue = field.drop_glue.clone();
    match field.kind {
        AggregateFieldKind::I32 => {
            let (mut instructions, value) = lower_i32_expression_to_word(value, context)?;
            instructions.push(Instruction::StoreAggregateI32 {
                destination,
                offset,
                value,
            });
            Ok(instructions)
        }
        AggregateFieldKind::U16 => Ok(vec![Instruction::StoreAggregateU16 {
            destination,
            offset,
            value: lower_u16_literal(value)?,
        }]),
        AggregateFieldKind::U32 => Ok(vec![Instruction::StoreAggregateU32 {
            destination,
            offset,
            value: lower_u32_literal(value)?,
        }]),
        AggregateFieldKind::U8 => {
            let (mut instructions, value) = lower_u8_expression_to_word(value, context)?;
            instructions.push(Instruction::StoreAggregateU8 {
                destination,
                offset,
                value,
            });
            Ok(instructions)
        }
        AggregateFieldKind::Usize => {
            let (mut instructions, value) = match lower_usize_expression_to_word(value, context) {
                Ok(lowered) => lowered,
                Err(_) if expression_is_pointer_address_value(value, context) => {
                    let mut temporaries = TemporaryAllocator::new(context)?;
                    lower_pointer_address_expression_to_word(value, context, &mut temporaries)?
                }
                Err(diagnostics) => return Err(diagnostics),
            };
            instructions.push(Instruction::StoreAggregateUsize {
                destination,
                offset,
                value,
            });
            Ok(instructions)
        }
        AggregateFieldKind::Bool => {
            let mut lowered = lower_bool_expression_to_value(value, context, "E8008")?;
            lowered.instructions.push(Instruction::StoreAggregateBool {
                destination,
                offset,
                value: lowered.value,
            });
            Ok(lowered.instructions)
        }
        AggregateFieldKind::Str => {
            lower_str_aggregate_field_assignment(value, destination, offset, context)
        }
        AggregateFieldKind::Slice(_) => {
            lower_slice_aggregate_field_assignment(value, destination, offset, context)
        }
        AggregateFieldKind::Aggregate { layout, .. } => {
            if field_is_copy {
                lower_aggregate_member_value_assignment(destination, offset, layout, value, context)
            } else {
                lower_aggregate_member_replacement_assignment(
                    destination,
                    offset,
                    layout,
                    field_drop_glue,
                    value,
                    context,
                )
            }
        }
    }
}

fn lower_str_aggregate_field_assignment(
    value: &Expr,
    destination: AggregateLocation,
    offset: u32,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    let mut lowered = lower_str_expression_to_value(value, context, &mut temporaries)?;
    push_store_str_view_to_aggregate_field(
        &mut lowered.instructions,
        destination,
        offset,
        lowered.value,
        &mut temporaries,
        unsupported_assignment_diagnostic,
    )?;
    Ok(lowered.instructions)
}

fn lower_slice_aggregate_field_assignment(
    value: &Expr,
    destination: AggregateLocation,
    offset: u32,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    let mut lowered = lower_slice_expression_to_value(value, context, &mut temporaries)?;
    push_store_slice_view_to_aggregate_field(
        &mut lowered.instructions,
        destination,
        offset,
        lowered.value,
        &mut temporaries,
        unsupported_assignment_diagnostic,
    )?;
    Ok(lowered.instructions)
}

fn lower_aggregate_member_value_assignment(
    destination: AggregateLocation,
    destination_offset: u32,
    layout: ValueLayout,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_assignment_diagnostic());
    }

    match unwrap_group(value) {
        Expr::StructLiteral(literal) => {
            let Some((_root_source, resolved)) = context.resolved_calls() else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_struct_literal_to_location_at_offset(
                literal,
                layout,
                destination,
                destination_offset,
                "E8008",
                "assignments",
                resolved,
                context,
            )
        }
        Expr::Identifier(identifier) => {
            let Some(source) = context.aggregate_local(&identifier.name) else {
                return Err(unsupported_assignment_diagnostic());
            };
            if source.layout != layout || !source.is_copy {
                return Err(unsupported_assignment_diagnostic());
            }
            Ok(vec![Instruction::CopyAggregateRange {
                destination,
                destination_offset,
                source: AggregateLocation::Slot(source.slot_index),
                source_offset: 0,
                layout,
            }])
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unwrap_group(&unary.operand) else {
                return Err(unsupported_assignment_diagnostic());
            };
            let Some(source) = context.aggregate_local(&identifier.name) else {
                return Err(unsupported_assignment_diagnostic());
            };
            if source.layout != layout {
                return Err(unsupported_assignment_diagnostic());
            }
            Ok(vec![Instruction::CopyAggregateRange {
                destination,
                destination_offset,
                source: AggregateLocation::Slot(source.slot_index),
                source_offset: 0,
                layout,
            }])
        }
        Expr::Call(call) => lower_aggregate_call_member_value_assignment(
            destination,
            destination_offset,
            layout,
            call,
            context,
        ),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_fallible_call_member_value_assignment(
                destination,
                destination_offset,
                layout,
                call,
                propagating_failure_mode(context)?,
                context,
            )
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_fallible_call_member_value_assignment(
                destination,
                destination_offset,
                layout,
                call,
                FallibleFailureMode::Trap,
                context,
            )
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_fallible_call_member_value_assignment(
                destination,
                destination_offset,
                layout,
                call,
                lower_catch_failure_mode(catch, context, 0)?,
                context,
            )
        }
        Expr::Member(_) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let access = lower_aggregate_member_field_access(value, context, &mut temporaries)?
                .ok_or_else(unsupported_assignment_diagnostic)?;
            let source_location = access.source;
            let source_offset = access.offset;
            let source_is_copy = access.is_copy;
            let AggregateFieldKind::Aggregate {
                layout: source_layout,
                ..
            } = access.kind
            else {
                return Err(unsupported_assignment_diagnostic());
            };
            if source_layout != layout || !source_is_copy {
                return Err(unsupported_assignment_diagnostic());
            }
            let mut instructions = access.instructions;
            instructions.push(Instruction::CopyAggregateRange {
                destination,
                destination_offset,
                source: source_location,
                source_offset,
                layout,
            });
            Ok(instructions)
        }
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

fn lower_aggregate_member_replacement_assignment(
    destination: AggregateLocation,
    destination_offset: u32,
    layout: ValueLayout,
    drop_glue: Option<DropGlue>,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_assignment_diagnostic());
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let replacement_slot = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index: replacement_slot,
        layout,
    }];
    instructions.extend(lower_aggregate_member_value_assignment(
        AggregateLocation::Slot(replacement_slot),
        0,
        layout,
        value,
        context,
    )?);
    if let Some(drop_instruction) = replacement_drop_for_aggregate_field(
        destination,
        destination_offset,
        layout,
        drop_glue,
        context,
    )? {
        instructions.push(drop_instruction);
    }
    instructions.push(Instruction::CopyAggregateRange {
        destination,
        destination_offset,
        source: AggregateLocation::Slot(replacement_slot),
        source_offset: 0,
        layout,
    });
    Ok(instructions)
}

fn replacement_drop_for_aggregate_field(
    destination: AggregateLocation,
    destination_offset: u32,
    layout: ValueLayout,
    drop_glue: Option<DropGlue>,
    context: &LoweringContext,
) -> Result<Option<Instruction>, Vec<Diagnostic>> {
    let Some(drop_glue) = drop_glue else {
        return Ok(None);
    };
    let Some(parameter_types) = context.call_parameter_types(&drop_glue.target) else {
        return Err(unsupported_assignment_diagnostic());
    };
    if parameter_types.len() != 1
        || !drop_parameter_matches_aggregate_layout(&parameter_types[0], layout)
    {
        return Err(unsupported_assignment_diagnostic());
    }

    let source = borrow_source_for_aggregate_field(destination, destination_offset)?;
    Ok(Some(Instruction::CallVoid {
        target: drop_glue.target,
        arguments: vec![ScalarArgument::Borrow(BorrowArgument { source })],
    }))
}

fn borrow_source_for_aggregate_field(
    destination: AggregateLocation,
    offset: u32,
) -> Result<BorrowSource, Vec<Diagnostic>> {
    match destination {
        AggregateLocation::Slot(slot_index) => {
            Ok(BorrowSource::AggregateSlotField { slot_index, offset })
        }
        AggregateLocation::Parameter(parameter_index) => {
            Ok(BorrowSource::AggregateParameterField {
                parameter_index,
                offset,
            })
        }
        AggregateLocation::Return
        | AggregateLocation::DirectReturn
        | AggregateLocation::DirectParameter { .. } => Err(unsupported_assignment_diagnostic()),
    }
}

fn drop_parameter_matches_aggregate_layout(parameter_type: &Type, layout: ValueLayout) -> bool {
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

fn lower_aggregate_call_member_value_assignment(
    destination: AggregateLocation,
    destination_offset: u32,
    layout: ValueLayout,
    call: &CallExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(callee_layout) = aggregate_type_layout(&return_type) else {
        return Err(unsupported_assignment_diagnostic());
    };
    if callee_layout != layout {
        return Err(unsupported_assignment_diagnostic());
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let source_slot = temporaries.next_aggregate_slot();
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
    push_aggregate_call_instruction(
        &mut instructions,
        &return_type,
        AggregateLocation::Slot(source_slot),
        target,
        arguments,
        layout,
    );
    instructions.push(Instruction::CopyAggregateRange {
        destination,
        destination_offset,
        source: AggregateLocation::Slot(source_slot),
        source_offset: 0,
        layout,
    });
    Ok(instructions)
}

fn lower_aggregate_fallible_call_member_value_assignment(
    destination: AggregateLocation,
    destination_offset: u32,
    layout: ValueLayout,
    call: &CallExpr,
    failure_mode: FallibleFailureMode,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(Type::Fallible(success)) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(callee_layout) = aggregate_type_layout(success.as_ref()) else {
        return Err(unsupported_assignment_diagnostic());
    };
    if callee_layout != layout {
        return Err(unsupported_assignment_diagnostic());
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let source_slot = temporaries.next_aggregate_slot();
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
        success.as_ref(),
        AggregateLocation::Slot(source_slot),
        target,
        arguments,
        layout,
        failure_mode,
    );
    instructions.push(Instruction::CopyAggregateRange {
        destination,
        destination_offset,
        source: AggregateLocation::Slot(source_slot),
        source_offset: 0,
        layout,
    });
    Ok(instructions)
}

fn aggregate_assignment_target_path(target: &MemberExpr) -> Option<(&str, String)> {
    let (identifier_name, mut fields) = aggregate_assignment_root_and_path(&target.object)?;
    fields.push(target.member.as_str());
    Some((identifier_name, fields.join(".")))
}

fn aggregate_assignment_root_and_path(expression: &Expr) -> Option<(&str, Vec<&str>)> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Some((&identifier.name, Vec::new())),
        Expr::Member(member) => {
            let (identifier_name, mut fields) = aggregate_assignment_root_and_path(&member.object)?;
            fields.push(member.member.as_str());
            Some((identifier_name, fields))
        }
        _ => None,
    }
}

fn lower_aggregate_assignment(
    slot_index: usize,
    layout: ValueLayout,
    target_type: Option<&TypeExpr>,
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if aggregate_assignment_moves_from_slot(expression, slot_index, context) {
        return Err(unsupported_assignment_diagnostic());
    }

    let replacement_drop = replacement_drop_for_aggregate_slot(slot_index, context)?;
    if replacement_drop.is_empty() {
        return lower_aggregate_assignment_to_slot(
            slot_index,
            layout,
            target_type,
            expression,
            context,
        );
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let replacement_slot = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index: replacement_slot,
        layout,
    }];
    instructions.extend(lower_aggregate_assignment_to_slot(
        replacement_slot,
        layout,
        target_type,
        expression,
        context,
    )?);
    instructions.extend(replacement_drop);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::Slot(slot_index),
        source: AggregateLocation::Slot(replacement_slot),
        layout,
    });
    Ok(instructions)
}

fn aggregate_assignment_moves_from_slot(
    expression: &Expr,
    destination_slot: usize,
    context: &LoweringContext,
) -> bool {
    let Expr::Unary(unary) = unwrap_group(expression) else {
        return false;
    };
    if unary.operator != UnaryOperator::Move {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group(&unary.operand) else {
        return false;
    };
    context
        .aggregate_slot(&identifier.name)
        .is_some_and(|(slot_index, _layout)| slot_index == destination_slot)
}

fn lower_aggregate_assignment_to_slot(
    slot_index: usize,
    layout: ValueLayout,
    target_type: Option<&TypeExpr>,
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::ArrayLiteral(literal) => lower_aggregate_array_literal_assignment(
            slot_index,
            layout,
            target_type,
            literal,
            context,
        ),
        Expr::StructLiteral(literal) => {
            lower_aggregate_struct_literal_assignment(slot_index, layout, literal, context)
        }
        Expr::Call(call) => lower_aggregate_call_assignment(slot_index, layout, call, context),
        Expr::Identifier(identifier) => {
            lower_aggregate_copy_assignment(slot_index, layout, &identifier.name, context)
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unwrap_group(&unary.operand) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_move_assignment(slot_index, layout, &identifier.name, context)
        }
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_fallible_call_assignment(
                slot_index,
                layout,
                call,
                propagating_failure_mode(context)?,
                context,
            )
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_fallible_call_assignment(
                slot_index,
                layout,
                call,
                FallibleFailureMode::Trap,
                context,
            )
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_fallible_call_assignment(
                slot_index,
                layout,
                call,
                lower_catch_failure_mode(catch, context, 0)?,
                context,
            )
        }
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

fn lower_aggregate_copy_assignment(
    destination_slot: usize,
    destination_layout: ValueLayout,
    source_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(source) = context.aggregate_local(source_name) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(destination) = context.aggregate_local_by_slot(destination_slot) else {
        return Err(unsupported_assignment_diagnostic());
    };
    if source.layout != destination_layout
        || destination.layout != destination_layout
        || !source.is_copy
        || !destination.is_copy
        || !supported_aggregate_copy_layout(destination_layout)
    {
        return Err(unsupported_assignment_diagnostic());
    }

    Ok(vec![Instruction::CopyAggregate {
        destination: AggregateLocation::Slot(destination_slot),
        source: AggregateLocation::Slot(source.slot_index),
        layout: destination_layout,
    }])
}

fn lower_aggregate_move_assignment(
    destination_slot: usize,
    destination_layout: ValueLayout,
    source_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(source) = context.aggregate_local(source_name) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let destination = context.aggregate_local_by_slot(destination_slot);
    if source.slot_index == destination_slot
        || source.layout != destination_layout
        || destination.is_some_and(|destination| destination.layout != destination_layout)
        || !supported_aggregate_copy_layout(destination_layout)
    {
        return Err(unsupported_assignment_diagnostic());
    }

    Ok(vec![Instruction::CopyAggregate {
        destination: AggregateLocation::Slot(destination_slot),
        source: AggregateLocation::Slot(source.slot_index),
        layout: destination_layout,
    }])
}

fn lower_aggregate_struct_literal_assignment(
    slot_index: usize,
    layout: ValueLayout,
    literal: &crate::ast::StructLiteralExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_assignment_diagnostic());
    };

    lower_aggregate_struct_literal_to_location(
        literal,
        layout,
        AggregateLocation::Slot(slot_index),
        "E8008",
        "assignments",
        resolved,
        context,
    )
}

fn lower_aggregate_array_literal_assignment(
    slot_index: usize,
    layout: ValueLayout,
    target_type: Option<&TypeExpr>,
    literal: &crate::ast::ArrayLiteralExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(ty) = target_type else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let value = abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
        context.resolved_source(source)
    })
    .map_err(|_error| unsupported_assignment_diagnostic())?;
    if !matches!(value.ty, AbiType::Array { .. }) || value.layout != layout {
        return Err(unsupported_assignment_diagnostic());
    }

    lower_aggregate_array_literal_to_location(
        literal,
        &value.ty,
        layout,
        AggregateLocation::Slot(slot_index),
        "E8008",
        "assignments",
        resolved,
        context,
    )
}

fn lower_aggregate_call_assignment(
    slot_index: usize,
    layout: ValueLayout,
    call: &CallExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if macos_syscall_primitive_call(call, context) {
        let mut temporaries = TemporaryAllocator::new(context)?;
        if let Some(instructions) = lower_macos_syscall_primitive_call_to_location(
            call,
            AggregateLocation::Slot(slot_index),
            layout,
            context,
            &mut temporaries,
        )? {
            return Ok(instructions);
        }
    }

    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_assignment_diagnostic());
    };

    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(callee_layout) = aggregate_type_layout(&return_type) else {
        return Err(unsupported_assignment_diagnostic());
    };
    if callee_layout != layout {
        return Err(unsupported_assignment_diagnostic());
    }

    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    push_aggregate_call_instruction(
        &mut instructions,
        &return_type,
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
    );
    Ok(instructions)
}

fn lower_aggregate_fallible_call_assignment(
    slot_index: usize,
    layout: ValueLayout,
    call: &CallExpr,
    failure_mode: FallibleFailureMode,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_assignment_diagnostic());
    };

    let Some(Type::Fallible(success)) = context.call_return_type(&target) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(callee_layout) = aggregate_type_layout(success.as_ref()) else {
        return Err(unsupported_assignment_diagnostic());
    };
    if callee_layout != layout {
        return Err(unsupported_assignment_diagnostic());
    }

    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success.as_ref(),
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
        failure_mode,
    );
    Ok(instructions)
}

fn lower_error_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some((root_source, resolved)) = context.resolved_calls() else {
        return Ok(None);
    };

    let payload_context = context.with_reserved_local_abi_words(4);
    let Some(payload) = lower_error_payload(
        &statement.initializer,
        resolved,
        root_source,
        Some(&payload_context),
    )?
    else {
        return Ok(None);
    };

    let (code, message) = context.define_error_local(statement.name.clone())?;
    Ok(Some(payload.into_store_instructions(code, message)))
}

fn lower_i32_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_i32_local_location()?;
    let instructions =
        lower_i32_expression_to_location(&statement.initializer, destination, context)?;
    context.define_i32_local(statement.name.clone());
    Ok(instructions)
}

fn lower_u8_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_u8_local_location()?;
    let instructions =
        lower_u8_expression_to_location(&statement.initializer, destination, context)?;
    context.define_u8_local(statement.name.clone());
    Ok(instructions)
}

fn lower_usize_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_usize_local_location()?;
    let instructions =
        lower_usize_expression_to_location(&statement.initializer, destination, context)?;
    context.define_usize_local(statement.name.clone());
    Ok(instructions)
}

fn lower_bool_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_bool_local_location()?;
    let instructions =
        lower_bool_expression_to_location(&statement.initializer, destination, context, "E8008")?;
    context.define_bool_local(statement.name.clone());
    Ok(instructions)
}

fn lower_str_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_str_local_location()?;
    let instructions =
        lower_str_expression_to_location(&statement.initializer, destination, context)?;
    context.define_str_local(statement.name.clone());
    Ok(instructions)
}

fn lower_slice_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
    info: SliceTypeInfo,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_slice_local_location()?;
    let instructions =
        lower_slice_expression_to_location(&statement.initializer, destination, context)?;
    context.define_slice_local(statement.name.clone(), info.element_kind, info.element_type);
    Ok(instructions)
}

fn scalar_binding_kind(
    statement: &BindingStmt,
    context: &LoweringContext,
) -> Result<ScalarBindingKind, Vec<Diagnostic>> {
    match &statement.ty {
        Some(ty) => {
            let Some((_root_source, resolved)) = context.resolved_calls() else {
                return Err(unsupported_binding_diagnostic(
                    "IR v0 cannot lower annotated local bindings without resolved type information",
                ));
            };
            match scalar_or_view_type_from_type_expr(ty, resolved) {
                Some(Type::I32) => Ok(ScalarBindingKind::I32),
                Some(Type::U8) => Ok(ScalarBindingKind::U8),
                Some(Type::Usize) => Ok(ScalarBindingKind::Usize),
                Some(Type::Bool) => Ok(ScalarBindingKind::Bool),
                Some(Type::Str) => Ok(ScalarBindingKind::Str),
                Some(Type::Slice { .. }) => Ok(ScalarBindingKind::Slice(
                    slice_type_info_from_type_expr(ty, context),
                )),
                _ => Err(unsupported_binding_diagnostic(
                    "IR v0 can only lower local bindings annotated as `i32`, `u8`, `usize`, `bool`, `&str`, `&[T]`, `&+[T]`, or aliases to those types",
                )),
            }
        }
        None => {
            if let Some(kind) = context.binding_scalar_view_kind(statement.name_span) {
                return Ok(scalar_binding_kind_from_typecheck_kind(
                    kind,
                    slice_type_info_from_expression(&statement.initializer, context),
                ));
            }
            Ok(
                expression_is_lowerable_bool_binding(&statement.initializer, context)
                    .then_some(ScalarBindingKind::Bool)
                    .or_else(|| {
                        expression_is_bool_returning_call(&statement.initializer, context)
                            .then_some(ScalarBindingKind::Bool)
                    })
                    .or_else(|| expression_scalar_binding_kind(&statement.initializer, context))
                    .unwrap_or(ScalarBindingKind::I32),
            )
        }
    }
}

fn scalar_binding_kind_from_typecheck_kind(
    kind: TypecheckScalarViewKind,
    slice_info: Option<SliceTypeInfo>,
) -> ScalarBindingKind {
    match kind {
        TypecheckScalarViewKind::I32 => ScalarBindingKind::I32,
        TypecheckScalarViewKind::U8 => ScalarBindingKind::U8,
        TypecheckScalarViewKind::Usize => ScalarBindingKind::Usize,
        TypecheckScalarViewKind::Bool => ScalarBindingKind::Bool,
        TypecheckScalarViewKind::Str => ScalarBindingKind::Str,
        TypecheckScalarViewKind::Slice(element_kind) => {
            ScalarBindingKind::Slice(slice_info.unwrap_or(SliceTypeInfo {
                element_kind,
                element_type: None,
            }))
        }
    }
}

fn slice_type_info_from_type_expr(ty: &TypeExpr, context: &LoweringContext) -> SliceTypeInfo {
    let element_type = slice_element_type_expr_from_type_expr(ty, context);
    let element_kind = element_type
        .as_ref()
        .map(|element_type| slice_element_kind_from_element_type_expr(element_type, context))
        .unwrap_or_else(|| slice_element_kind_from_type_expr(ty, context));
    SliceTypeInfo {
        element_kind,
        element_type,
    }
}

fn slice_type_info_from_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<SliceTypeInfo> {
    let element_type = slice_target_element_type_expr(expression, context)?;
    Some(SliceTypeInfo {
        element_kind: slice_element_kind_from_element_type_expr(&element_type, context),
        element_type: Some(element_type),
    })
}

fn slice_type_info_from_call_return(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<SliceTypeInfo> {
    let return_type = context.call_return_type_expr(call)?;
    Some(slice_type_info_from_type_expr(&return_type, context))
}

fn slice_type_info_from_call_success(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<SliceTypeInfo> {
    let TypeExpr::Fallible(fallible) = context.call_return_type_expr(call)? else {
        return None;
    };
    Some(slice_type_info_from_type_expr(&fallible.success, context))
}

fn slice_type_info_from_kind(element_kind: TypecheckSliceElementKind) -> SliceTypeInfo {
    SliceTypeInfo {
        element_kind,
        element_type: None,
    }
}

fn expression_scalar_binding_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    match unwrap_group(expression) {
        Expr::Call(call) => call_return_scalar_binding_kind(call, context),
        Expr::Propagate(propagation) => fallible_call_success_scalar_binding_kind(
            unwrap_group(&propagation.expression),
            context,
        ),
        Expr::Force(force) => {
            fallible_call_success_scalar_binding_kind(unwrap_group(&force.expression), context)
        }
        Expr::Catch(catch) => {
            fallible_call_success_scalar_binding_kind(unwrap_group(&catch.expression), context)
        }
        Expr::Member(member) if context.payloadless_enum_variant_tag(member).is_some() => {
            Some(ScalarBindingKind::U8)
        }
        _ => None,
    }
}

fn call_return_scalar_binding_kind(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    if let Some(kind) = primitive_call_scalar_binding_kind(call, context) {
        return Some(kind);
    }

    let (target, _call_name) = context.direct_call_target_and_name(call)?;
    scalar_binding_kind_from_call_return_type(call, context.call_return_type(&target)?, context)
}

fn fallible_call_success_scalar_binding_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    let Expr::Call(call) = expression else {
        return None;
    };
    if let Some(kind) = primitive_call_fallible_success_scalar_binding_kind(call, context) {
        return Some(kind);
    }

    let (target, _call_name) = context.direct_call_target_and_name(call)?;
    let Type::Fallible(success) = context.call_return_type(&target)? else {
        return None;
    };
    scalar_binding_kind_from_call_success_type(call, success, context)
}

fn primitive_call_scalar_binding_kind(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    match context.primitive_name_for_call(call)? {
        "addr" | "from_addr" | "from_ref" | "from_ref_mut" | "pointee_size" => {
            Some(ScalarBindingKind::Usize)
        }
        "str_from_raw_parts" => Some(ScalarBindingKind::Str),
        "bytes_from_str" => Some(ScalarBindingKind::Slice(slice_type_info_from_kind(
            TypecheckSliceElementKind::U8,
        ))),
        "slice_from_raw_parts"
        | "slice_from_raw_parts_mut"
        | "slice_from_raw_parts_value"
        | "slice_from_raw_parts_value_mut" => Some(ScalarBindingKind::Slice(
            slice_type_info_from_call_return(call, context).unwrap_or_else(|| {
                slice_type_info_from_kind(
                    call_return_slice_element_kind(call, context)
                        .unwrap_or(TypecheckSliceElementKind::Other),
                )
            }),
        )),
        _ => None,
    }
}

fn primitive_call_fallible_success_scalar_binding_kind(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    match context.primitive_name_for_call(call)? {
        "open_read_raw" => Some(ScalarBindingKind::I32),
        "read_bytes_raw" => Some(ScalarBindingKind::Usize),
        _ => None,
    }
}

fn scalar_binding_kind_from_type(ty: &Type) -> Option<ScalarBindingKind> {
    match ty {
        Type::I32 => Some(ScalarBindingKind::I32),
        Type::U8 => Some(ScalarBindingKind::U8),
        Type::Usize => Some(ScalarBindingKind::Usize),
        Type::Bool => Some(ScalarBindingKind::Bool),
        Type::Str => Some(ScalarBindingKind::Str),
        Type::Slice { .. } => Some(ScalarBindingKind::Slice(slice_type_info_from_kind(
            TypecheckSliceElementKind::Other,
        ))),
        _ => None,
    }
}

fn scalar_binding_kind_from_call_return_type(
    call: &CallExpr,
    ty: &Type,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    match ty {
        Type::Slice { .. } => Some(ScalarBindingKind::Slice(
            slice_type_info_from_call_return(call, context).unwrap_or_else(|| {
                slice_type_info_from_kind(
                    call_return_slice_element_kind(call, context)
                        .unwrap_or(TypecheckSliceElementKind::Other),
                )
            }),
        )),
        _ => scalar_binding_kind_from_type(ty),
    }
}

fn scalar_binding_kind_from_call_success_type(
    call: &CallExpr,
    ty: &Type,
    context: &LoweringContext,
) -> Option<ScalarBindingKind> {
    match ty {
        Type::Slice { .. } => Some(ScalarBindingKind::Slice(
            slice_type_info_from_call_success(call, context).unwrap_or_else(|| {
                slice_type_info_from_kind(
                    call_success_slice_element_kind(call, context)
                        .unwrap_or(TypecheckSliceElementKind::Other),
                )
            }),
        )),
        _ => scalar_binding_kind_from_type(ty),
    }
}

fn slice_element_kind_from_type_expr(
    ty: &TypeExpr,
    context: &LoweringContext,
) -> TypecheckSliceElementKind {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return TypecheckSliceElementKind::Other;
    };
    slice_element_kind_from_type(view_element_type_from_type_expr(ty, resolved))
}

fn slice_element_kind_from_element_type_expr(
    ty: &TypeExpr,
    context: &LoweringContext,
) -> TypecheckSliceElementKind {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return TypecheckSliceElementKind::Other;
    };
    slice_element_kind_from_type(scalar_or_view_type_from_type_expr(ty, resolved))
}

fn call_return_slice_element_kind(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<TypecheckSliceElementKind> {
    let (_root_source, resolved) = context.resolved_calls()?;
    let return_type = context.call_return_type_expr(call)?;
    Some(slice_element_kind_from_type(
        view_element_type_from_type_expr(&return_type, resolved),
    ))
}

fn call_success_slice_element_kind(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<TypecheckSliceElementKind> {
    let (_root_source, resolved) = context.resolved_calls()?;
    let return_type = context.call_return_type_expr(call)?;
    let TypeExpr::Fallible(fallible) = return_type else {
        return None;
    };
    Some(slice_element_kind_from_type(
        view_element_type_from_type_expr(&fallible.success, resolved),
    ))
}

fn slice_element_kind_from_type(ty: Option<Type>) -> TypecheckSliceElementKind {
    match ty {
        Some(Type::I32) => TypecheckSliceElementKind::I32,
        Some(Type::U8) => TypecheckSliceElementKind::U8,
        Some(Type::Usize) => TypecheckSliceElementKind::Usize,
        Some(Type::Bool) => TypecheckSliceElementKind::Bool,
        Some(Type::Str) => TypecheckSliceElementKind::Str,
        _ => TypecheckSliceElementKind::Other,
    }
}

fn expression_is_bool_returning_call(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Call(call) => {
            let Some((target, _call_name)) = context.direct_call_target_and_name(call) else {
                return false;
            };
            context.call_return_type(&target) == Some(&Type::Bool)
        }
        Expr::Unary(unary) => {
            unary.operator == UnaryOperator::LogicalNot
                && expression_is_bool_returning_call(&unary.operand, context)
        }
        Expr::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr
            ) =>
        {
            expression_is_bool_returning_call(&binary.left, context)
                && expression_is_bool_returning_call(&binary.right, context)
                || expression_is_lowerable_bool_binding(&binary.left, context)
                    && expression_is_bool_returning_call(&binary.right, context)
                || expression_is_bool_returning_call(&binary.left, context)
                    && expression_is_lowerable_bool_binding(&binary.right, context)
        }
        Expr::Group(group) => expression_is_bool_returning_call(&group.expression, context),
        _ => false,
    }
}

fn call_success_type_is_copy_struct(call: &CallExpr, context: &LoweringContext) -> bool {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return false;
    };
    let Some(return_type) = context.call_return_type_expr(call) else {
        return false;
    };
    type_expr_is_copy_struct(&return_type, resolved)
}

fn call_success_aggregate_fields(
    call: &CallExpr,
    context: &LoweringContext,
) -> Vec<super::context::AggregateField> {
    let Some((root_source, resolved)) = context.resolved_calls() else {
        return Vec::new();
    };
    let Some(return_type) = context.call_return_type_expr(call) else {
        return Vec::new();
    };
    aggregate_fields_from_type_expr(&return_type, root_source, resolved).unwrap_or_default()
}

fn call_success_drop_glue(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<super::context::DropGlue> {
    let return_type = context.call_return_type_expr(call)?;
    context.drop_glue_for_type_expr(&return_type)
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

fn unsupported_binding_diagnostic(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error("E8008", message)]
}

fn unsupported_assignment_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        "IR v0 can only lower simple `=` assignment to scalar local bindings, supported read-write slice elements, scalar aggregate fields, aggregate slots, copy aggregate fields, or drop-aware aggregate field replacement",
    )]
}

fn unsupported_interpolated_string_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        "IR v0 cannot lower interpolated string construction until explicit std/string allocation and std/fmt.append_* lowering are implemented",
    )]
}

enum ScalarBindingKind {
    I32,
    U8,
    Usize,
    Bool,
    Str,
    Slice(SliceTypeInfo),
}

impl ScalarBindingKind {
    fn abi_word_count(&self) -> usize {
        match self {
            Self::I32 | Self::U8 | Self::Usize | Self::Bool => 1,
            Self::Str | Self::Slice(_) => 2,
        }
    }
}
