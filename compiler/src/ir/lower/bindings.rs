use super::aggregates::{
    aggregate_call_return_layout_from_resolved, aggregate_fields_from_type_expr,
    aggregate_type_layout, lower_aggregate_struct_literal_to_location,
    lower_aggregate_struct_literal_to_location_at_offset, push_aggregate_call_instruction,
    push_fallible_aggregate_call_instruction, supported_aggregate_copy_layout,
    type_expr_is_copy_struct,
};
use super::context::{AggregateFieldKind, DropGlue, LoweringContext};
use super::errors::lower_error_payload;
use super::expressions::{
    TemporaryAllocator, aggregate_call_field, expression_contains_interpolated_string,
    expression_is_lowerable_bool_binding, lower_aggregate_member_field_access,
    lower_bool_expression_to_location, lower_bool_expression_to_value,
    lower_call_arguments_to_scalar_arguments,
    lower_call_arguments_to_scalar_arguments_with_temporaries, lower_catch_failure_mode,
    lower_fallible_bool_normal_call, lower_fallible_i32_normal_call,
    lower_fallible_slice_normal_call, lower_fallible_str_normal_call,
    lower_fallible_u8_normal_call, lower_fallible_usize_normal_call,
    lower_i32_expression_to_location, lower_i32_expression_to_word,
    lower_i32_expression_to_word_with_temporaries, lower_macos_syscall_primitive_call_to_location,
    lower_pointer_address_expression_to_word, lower_slice_expression_to_location,
    lower_str_expression_to_location, lower_u8_expression_to_location, lower_u8_expression_to_word,
    lower_usize_expression_to_location, lower_usize_expression_to_word,
    lower_usize_expression_to_word_with_temporaries, lower_void_expression_statement,
};
use super::functions::{
    lower_drop_statement, lower_never_expression_with_scope_drops,
    lower_return_statement_with_scope_drops, propagating_failure_mode,
    replacement_drop_for_aggregate_slot,
};
use super::literals::{lower_u16_literal, lower_u32_literal};
use super::types::{
    return_type_expr_is_top_level_optional, scalar_or_view_type_from_type_expr,
    view_element_type_from_type_expr,
};
use crate::abi::{ValueLayout, abi_value_from_type_expr};
use crate::ast::{
    AssignmentOperator, AssignmentStmt, BinaryOperator, BindingStmt, Block, CallExpr, Expr,
    MemberExpr, Stmt, TypeExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolLocation, BorrowArgument, BorrowSource, FallibleFailureMode,
    I32Location, I32Value, Instruction, ScalarArgument, SliceLocation, StrLocation, Type,
    U8Location, UsizeLocation, UsizeValue,
};
use crate::typecheck::{TypecheckScalarViewKind, TypecheckSliceElementKind};

pub(super) fn lower_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if expression_contains_interpolated_string(&statement.initializer) {
        return Err(unsupported_interpolated_string_diagnostic());
    }

    if let Some(instructions) = lower_optional_let_else_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_optional_default_scalar_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_optional_default_aggregate_binding(statement, context)? {
        return Ok(instructions);
    }

    if statement.else_block.is_some() {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower optional `let ... else` or `var ... else` bindings",
        ));
    }

    if let Some(instructions) = lower_aggregate_struct_literal_binding(statement, context)? {
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

    if let Some(instructions) = lower_error_local_binding(statement, context)? {
        return Ok(instructions);
    }

    match scalar_binding_kind(statement, context)? {
        ScalarBindingKind::I32 => lower_i32_local_binding(statement, context),
        ScalarBindingKind::U8 => lower_u8_local_binding(statement, context),
        ScalarBindingKind::Usize => lower_usize_local_binding(statement, context),
        ScalarBindingKind::Bool => lower_bool_local_binding(statement, context),
        ScalarBindingKind::Str => lower_str_local_binding(statement, context),
        ScalarBindingKind::Slice(element_kind) => {
            lower_slice_local_binding(statement, context, element_kind)
        }
    }
}

fn lower_optional_let_else_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Ok(None);
    };

    let Expr::Call(call) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };

    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower optional `let ... else` bindings without resolved call information",
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
    let failure_mode = lower_optional_let_else_failure_mode(else_block, context)?;

    if let Some(kind) =
        optional_let_else_scalar_binding_kind(statement, success_type.as_ref(), context)?
    {
        return lower_optional_let_else_scalar_call_binding(
            statement,
            call,
            kind,
            failure_mode,
            context,
        )
        .map(Some);
    }

    if aggregate_type_layout(success_type.as_ref()).is_some() {
        return lower_aggregate_fallible_call_binding(statement, call, failure_mode, context);
    }

    Ok(None)
}

fn optional_let_else_scalar_binding_kind(
    statement: &BindingStmt,
    success_type: &Type,
    context: &LoweringContext,
) -> Result<Option<ScalarBindingKind>, Vec<Diagnostic>> {
    let Some(ty) = &statement.ty else {
        return Ok(scalar_binding_kind_from_type(success_type));
    };

    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower annotated optional `let ... else` bindings without resolved type information",
        ));
    };
    Ok(match scalar_or_view_type_from_type_expr(ty, resolved) {
        Some(Type::I32) => Some(ScalarBindingKind::I32),
        Some(Type::U8) => Some(ScalarBindingKind::U8),
        Some(Type::Usize) => Some(ScalarBindingKind::Usize),
        Some(Type::Bool) => Some(ScalarBindingKind::Bool),
        Some(Type::Str) => Some(ScalarBindingKind::Str),
        Some(Type::Slice { .. }) => Some(ScalarBindingKind::Slice(
            slice_element_kind_from_type_expr(ty, context),
        )),
        _ => None,
    })
}

fn lower_optional_let_else_failure_mode(
    else_block: &Block,
    context: &LoweringContext,
) -> Result<FallibleFailureMode, Vec<Diagnostic>> {
    let mut else_context = context.clone();
    let instructions = lower_optional_let_else_block(else_block, &mut else_context)?;
    Ok(FallibleFailureMode::Handle { instructions })
}

fn lower_optional_let_else_block(
    block: &Block,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((terminal, leading)) = block.statements.split_last() else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower empty optional `let ... else` blocks",
        ));
    };

    let mut instructions = Vec::new();
    for statement in leading {
        instructions.extend(lower_optional_let_else_leading_statement(
            statement, context,
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
                    "IR v0 can only lower optional `let ... else` blocks ending in `return` or a `never` expression",
                ));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(unsupported_binding_diagnostic(
            "IR v0 can only lower optional `let ... else` blocks ending in `return` or a `never` expression",
        )),
    }
}

fn lower_optional_let_else_leading_statement(
    statement: &Stmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match statement {
        Stmt::Binding(statement) => lower_local_binding(statement, context),
        Stmt::Assignment(statement) => lower_assignment(statement, context),
        Stmt::Drop(statement) => lower_drop_statement(statement, context),
        Stmt::Expression(statement) => {
            lower_void_expression_statement(&statement.expression, context)?.ok_or_else(|| {
                unsupported_binding_diagnostic(
                    "IR v0 can only lower optional `let ... else` leading expression statements that make effect-only calls",
                )
            })
        }
        _ => Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower this statement inside optional `let ... else` blocks",
        )),
    }
}

fn lower_optional_let_else_scalar_call_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    kind: ScalarBindingKind,
    failure_mode: FallibleFailureMode,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    match kind {
        ScalarBindingKind::I32 => {
            let destination = context.next_i32_local_location()?;
            let instructions = lower_fallible_i32_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_i32_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::U8 => {
            let destination = context.next_u8_local_location()?;
            let instructions = lower_fallible_u8_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_u8_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::Usize => {
            let destination = context.next_usize_local_location()?;
            let instructions = lower_fallible_usize_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_usize_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::Bool => {
            let destination = context.next_bool_local_location()?;
            let instructions = lower_fallible_bool_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_bool_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::Str => {
            let destination = context.next_str_local_location()?;
            let instructions = lower_fallible_str_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_str_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::Slice(element_kind) => {
            let destination = context.next_slice_local_location()?;
            let instructions = lower_fallible_slice_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_slice_local(statement.name.clone(), element_kind);
            Ok(instructions)
        }
    }
}

fn lower_optional_default_scalar_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::OptionalDefault(default) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Expr::Call(call) = unwrap_group(&default.value) else {
        return Ok(None);
    };

    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower optional default bindings without resolved call information",
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
        optional_let_else_scalar_binding_kind(statement, success_type.as_ref(), context)?
    else {
        return Ok(None);
    };
    lower_optional_default_scalar_call_binding(statement, call, &default.default, kind, context)
        .map(Some)
}

fn lower_optional_default_scalar_call_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    fallback: &Expr,
    kind: ScalarBindingKind,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let expression_context = context.with_reserved_local_abi_words(kind.abi_word_count());
    let mut temporaries = TemporaryAllocator::new(&expression_context)?;
    match kind {
        ScalarBindingKind::I32 => {
            let destination = context.next_i32_local_location()?;
            let failure_mode = FallibleFailureMode::Recover {
                instructions: lower_i32_expression_to_location(
                    fallback,
                    destination,
                    &expression_context,
                )?,
            };
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
            let failure_mode = FallibleFailureMode::Recover {
                instructions: lower_u8_expression_to_location(
                    fallback,
                    destination,
                    &expression_context,
                )?,
            };
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
            let failure_mode = FallibleFailureMode::Recover {
                instructions: lower_usize_expression_to_location(
                    fallback,
                    destination,
                    &expression_context,
                )?,
            };
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
            let failure_mode = FallibleFailureMode::Recover {
                instructions: lower_bool_expression_to_location(
                    fallback,
                    destination,
                    &expression_context,
                    "E8008",
                )?,
            };
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
            let failure_mode = FallibleFailureMode::Recover {
                instructions: lower_str_expression_to_location(
                    fallback,
                    destination,
                    &expression_context,
                )?,
            };
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
        ScalarBindingKind::Slice(element_kind) => {
            let destination = context.next_slice_local_location()?;
            let failure_mode = FallibleFailureMode::Recover {
                instructions: lower_slice_expression_to_location(
                    fallback,
                    destination,
                    &expression_context,
                )?,
            };
            let instructions = lower_fallible_slice_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_slice_local(statement.name.clone(), element_kind);
            Ok(instructions)
        }
    }
}

fn lower_optional_default_aggregate_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::OptionalDefault(default) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Expr::Call(call) = unwrap_group(&default.value) else {
        return Ok(None);
    };

    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower optional default aggregate bindings without resolved call information",
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
    let failure_mode = lower_optional_default_aggregate_recover_failure_mode(
        &default.default,
        layout,
        AggregateLocation::Slot(slot_index),
        context,
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

fn lower_optional_default_aggregate_recover_failure_mode(
    fallback: &Expr,
    layout: ValueLayout,
    destination: AggregateLocation,
    context: &LoweringContext,
) -> Result<FallibleFailureMode, Vec<Diagnostic>> {
    let instructions = lower_aggregate_member_value_assignment(
        destination,
        0,
        layout,
        fallback,
        context,
    )
    .map_err(|_| {
        unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate optional default bindings with supported aggregate value fallbacks",
        )
    })?;
    Ok(FallibleFailureMode::Recover { instructions })
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

enum AggregateMemberBindingRoot<'a> {
    Identifier(&'a str),
    Call(&'a CallExpr),
    FallibleCall(&'a CallExpr, FallibleFailureMode),
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
        Expr::Call(call) => context.primitive_name_for_call(call) == Some("from_addr"),
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
        _ => Err(unsupported_assignment_diagnostic()),
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

pub(super) fn assignment_targets_readwrite_aggregate_field(
    statement: &AssignmentStmt,
    context: &LoweringContext,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Expr::Member(member) = unwrap_group(&statement.target) else {
        return false;
    };
    let Some((identifier_name, field_path)) = aggregate_assignment_target_path(member) else {
        return false;
    };
    context
        .aggregate_field(identifier_name, &field_path)
        .is_some_and(|field| field.is_readwrite)
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
        return lower_aggregate_assignment(slot_index, layout, value, context);
    }

    Err(unsupported_assignment_diagnostic())
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
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if aggregate_assignment_moves_from_slot(expression, slot_index, context) {
        return Err(unsupported_assignment_diagnostic());
    }

    let replacement_drop = replacement_drop_for_aggregate_slot(slot_index, context)?;
    if replacement_drop.is_empty() {
        return lower_aggregate_assignment_to_slot(slot_index, layout, expression, context);
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
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
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
    element_kind: TypecheckSliceElementKind,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_slice_local_location()?;
    let instructions =
        lower_slice_expression_to_location(&statement.initializer, destination, context)?;
    context.define_slice_local(statement.name.clone(), element_kind);
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
                    slice_element_kind_from_type_expr(ty, context),
                )),
                _ => Err(unsupported_binding_diagnostic(
                    "IR v0 can only lower local bindings annotated as `i32`, `u8`, `usize`, `bool`, `&str`, `&[T]`, `&+[T]`, or aliases to those types",
                )),
            }
        }
        None => Ok(context
            .binding_scalar_view_kind(statement.name_span)
            .map(scalar_binding_kind_from_typecheck_kind)
            .or_else(|| {
                expression_is_lowerable_bool_binding(&statement.initializer, context)
                    .then_some(ScalarBindingKind::Bool)
            })
            .or_else(|| {
                expression_is_bool_returning_call(&statement.initializer, context)
                    .then_some(ScalarBindingKind::Bool)
            })
            .or_else(|| expression_scalar_binding_kind(&statement.initializer, context))
            .unwrap_or(ScalarBindingKind::I32)),
    }
}

fn scalar_binding_kind_from_typecheck_kind(kind: TypecheckScalarViewKind) -> ScalarBindingKind {
    match kind {
        TypecheckScalarViewKind::I32 => ScalarBindingKind::I32,
        TypecheckScalarViewKind::U8 => ScalarBindingKind::U8,
        TypecheckScalarViewKind::Usize => ScalarBindingKind::Usize,
        TypecheckScalarViewKind::Bool => ScalarBindingKind::Bool,
        TypecheckScalarViewKind::Str => ScalarBindingKind::Str,
        TypecheckScalarViewKind::Slice(element_kind) => ScalarBindingKind::Slice(element_kind),
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
        "addr" | "pointee_size" => Some(ScalarBindingKind::Usize),
        "str_from_raw_parts" => Some(ScalarBindingKind::Str),
        "bytes_from_str" => Some(ScalarBindingKind::Slice(TypecheckSliceElementKind::U8)),
        "slice_from_raw_parts"
        | "slice_from_raw_parts_mut"
        | "slice_from_raw_parts_value"
        | "slice_from_raw_parts_value_mut" => Some(ScalarBindingKind::Slice(
            call_return_slice_element_kind(call, context)
                .unwrap_or(TypecheckSliceElementKind::Other),
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
        Type::Slice { .. } => Some(ScalarBindingKind::Slice(TypecheckSliceElementKind::Other)),
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
            call_return_slice_element_kind(call, context)
                .unwrap_or(TypecheckSliceElementKind::Other),
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
            call_success_slice_element_kind(call, context)
                .unwrap_or(TypecheckSliceElementKind::Other),
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
        "IR v0 can only lower simple `=` assignment to scalar local bindings, scalar aggregate fields, aggregate slots, copy aggregate fields, or drop-aware aggregate field replacement",
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
    Slice(TypecheckSliceElementKind),
}

impl ScalarBindingKind {
    fn abi_word_count(&self) -> usize {
        match self {
            Self::I32 | Self::U8 | Self::Usize | Self::Bool => 1,
            Self::Str | Self::Slice(_) => 2,
        }
    }
}
