use super::aggregates::{
    aggregate_fields_from_type_expr, lower_aggregate_struct_literal_to_location,
    lower_aggregate_struct_literal_to_location_at_offset, supported_aggregate_copy_layout,
};
use super::context::{AggregateFieldKind, LoweringContext};
use super::expressions::{
    TemporaryAllocator, aggregate_call_field, expression_contains_interpolated_string,
    expression_is_lowerable_bool_binding, expression_is_unsupported_bool_comparison_binding,
    lower_aggregate_member_field_access, lower_bool_expression_to_location,
    lower_bool_expression_to_value, lower_call_arguments_to_scalar_arguments,
    lower_call_arguments_to_scalar_arguments_with_temporaries, lower_i32_expression_to_location,
    lower_i32_expression_to_word, lower_slice_expression_to_location,
    lower_str_expression_to_location, lower_u8_expression_to_location, lower_u8_expression_to_word,
    lower_usize_expression_to_location, lower_usize_expression_to_word,
};
use crate::abi::{ValueLayout, abi_value_from_type_expr};
use crate::ast::{
    AssignmentOperator, AssignmentStmt, BinaryOperator, BindingStmt, CallExpr, Expr, MemberExpr,
    TypeExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolLocation, FallibleFailureMode, I32Location, Instruction, SliceLocation,
    StrLocation, Type, U8Location, UsizeLocation,
};
use crate::resolve::{ResolveOutput, TypeSymbolKind};
use std::collections::HashSet;

pub(super) fn lower_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if statement.else_block.is_some() {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower optional `let ... else` or `var ... else` bindings",
        ));
    }

    if expression_contains_interpolated_string(&statement.initializer) {
        return Err(unsupported_interpolated_string_diagnostic());
    }

    if let Some(instructions) = lower_aggregate_struct_literal_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_aggregate_call_binding(statement, context)? {
        return Ok(instructions);
    }

    if let Some(instructions) = lower_aggregate_member_binding(statement, context)? {
        return Ok(instructions);
    }

    match scalar_binding_kind(statement, context)? {
        ScalarBindingKind::I32 => lower_i32_local_binding(statement, context),
        ScalarBindingKind::U8 => lower_u8_local_binding(statement, context),
        ScalarBindingKind::Usize => lower_usize_local_binding(statement, context),
        ScalarBindingKind::Bool => lower_bool_local_binding(statement, context),
        ScalarBindingKind::Str => lower_str_local_binding(statement, context),
        ScalarBindingKind::Slice => lower_slice_local_binding(statement, context),
    }
}

fn lower_aggregate_struct_literal_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::StructLiteral(literal) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
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
    let fields = aggregate_fields_from_type_expr(&literal.ty, resolved).unwrap_or_default();
    let slot_index =
        context.define_aggregate_local(statement.name.clone(), value.layout, is_copy, fields);
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
                FallibleFailureMode::Propagate,
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
        _ => Ok(None),
    }
}

fn lower_aggregate_normal_call_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Ok(None);
    };

    let target = context.call_target(call, &identifier.name);
    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let layout = match &return_type {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Ok(None),
    };
    validate_aggregate_binding_layout(layout)?;

    let is_copy = call_success_type_is_copy_struct(call, context);
    let fields = call_success_aggregate_fields(call, context);
    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, fields);
    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &identifier.name, context)?;
    instructions.insert(0, Instruction::ReserveAggregateSlot { slot_index, layout });
    match return_type {
        Type::Aggregate { .. } => {
            instructions.push(Instruction::CallAggregate {
                destination: AggregateLocation::Slot(slot_index),
                target,
                arguments,
            });
        }
        Type::DirectAggregate { .. } => {
            instructions.push(Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(slot_index),
                target,
                arguments,
                layout,
            });
        }
        _ => unreachable!("aggregate call binding requires aggregate return type"),
    }
    Ok(Some(instructions))
}

fn lower_aggregate_fallible_call_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    failure_mode: FallibleFailureMode,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Ok(None);
    };

    let target = context.call_target(call, &identifier.name);
    let Some(Type::Fallible(success)) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let layout = match success.as_ref() {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Ok(None),
    };
    validate_aggregate_binding_layout(layout)?;

    let is_copy = call_success_type_is_copy_struct(call, context);
    let fields = call_success_aggregate_fields(call, context);
    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, fields);
    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &identifier.name, context)?;
    instructions.insert(0, Instruction::ReserveAggregateSlot { slot_index, layout });
    match success.as_ref() {
        Type::Aggregate { .. } => {
            instructions.push(Instruction::CallFallibleAggregate {
                destination: AggregateLocation::Slot(slot_index),
                target,
                arguments,
                failure_mode,
            });
        }
        Type::DirectAggregate { .. } => {
            instructions.push(Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(slot_index),
                target,
                arguments,
                layout,
                failure_mode,
            });
        }
        _ => unreachable!("fallible aggregate binding requires aggregate success type"),
    }
    Ok(Some(instructions))
}

fn lower_aggregate_member_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Member(member) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };

    match aggregate_member_binding_path(member) {
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
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, fields);
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
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Ok(None);
    };
    let target = context.call_target(call, &identifier.name);
    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let source_layout = match &return_type {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Ok(None),
    };
    let Some(field) = aggregate_call_field(call, field_path, context) else {
        return Ok(None);
    };
    let source_offset = field.offset;
    let AggregateFieldKind::Aggregate { layout, fields } = field.kind else {
        return Ok(None);
    };
    if !supported_aggregate_copy_layout(layout) || !supported_aggregate_copy_layout(source_layout) {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate member bindings from supported aggregate fields",
        ));
    }

    let is_copy = call_success_type_is_copy_struct(call, context);
    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, fields);
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
            &identifier.name,
            context,
            &mut temporaries,
        )?;
    instructions.append(&mut argument_instructions);
    match return_type {
        Type::Aggregate { .. } => {
            instructions.push(Instruction::CallAggregate {
                destination: AggregateLocation::Slot(source_slot),
                target,
                arguments,
            });
        }
        Type::DirectAggregate { .. } => {
            instructions.push(Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(source_slot),
                target,
                arguments,
                layout: source_layout,
            });
        }
        _ => unreachable!("aggregate member binding requires aggregate call return type"),
    }
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
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Ok(None);
    };
    let target = context.call_target(call, &identifier.name);
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let source_layout = match success_type.as_ref() {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Ok(None),
    };
    let Some(field) = aggregate_call_field(call, field_path, context) else {
        return Ok(None);
    };
    let source_offset = field.offset;
    let AggregateFieldKind::Aggregate { layout, fields } = field.kind else {
        return Ok(None);
    };
    if !supported_aggregate_copy_layout(layout) || !supported_aggregate_copy_layout(source_layout) {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate member bindings from supported fallible aggregate fields",
        ));
    }

    let is_copy = call_success_type_is_copy_struct(call, context);
    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, fields);
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
            &identifier.name,
            context,
            &mut temporaries,
        )?;
    instructions.append(&mut argument_instructions);
    match success_type.as_ref() {
        Type::Aggregate { .. } => {
            instructions.push(Instruction::CallFallibleAggregate {
                destination: AggregateLocation::Slot(source_slot),
                target,
                arguments,
                failure_mode,
            });
        }
        Type::DirectAggregate { .. } => {
            instructions.push(Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(source_slot),
                target,
                arguments,
                layout: source_layout,
                failure_mode,
            });
        }
        _ => unreachable!("fallible aggregate member binding requires aggregate success type"),
    }
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

fn aggregate_member_binding_path(
    member: &MemberExpr,
) -> Option<(AggregateMemberBindingRoot<'_>, String)> {
    let (root, mut fields) = aggregate_member_binding_root_and_path(&member.object)?;
    fields.push(member.member.as_str());
    Some((root, fields.join(".")))
}

fn aggregate_member_binding_root_and_path<'a>(
    expression: &'a Expr,
) -> Option<(AggregateMemberBindingRoot<'a>, Vec<&'a str>)> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Some((
            AggregateMemberBindingRoot::Identifier(&identifier.name),
            Vec::new(),
        )),
        Expr::Call(call) => Some((AggregateMemberBindingRoot::Call(call), Vec::new())),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return None;
            };
            Some((
                AggregateMemberBindingRoot::FallibleCall(call, FallibleFailureMode::Propagate),
                Vec::new(),
            ))
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return None;
            };
            Some((
                AggregateMemberBindingRoot::FallibleCall(call, FallibleFailureMode::Trap),
                Vec::new(),
            ))
        }
        Expr::Member(member) => {
            let (root, mut fields) = aggregate_member_binding_root_and_path(&member.object)?;
            fields.push(member.member.as_str());
            Some((root, fields))
        }
        _ => None,
    }
}

fn validate_aggregate_binding_layout(
    layout: crate::abi::ValueLayout,
) -> Result<(), Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate call bindings whose final ABI word is 1, 2, 4, or 8 bytes",
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

pub(super) fn lower_assignment(
    statement: &AssignmentStmt,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if statement.operator != AssignmentOperator::Assign {
        return Err(unsupported_assignment_diagnostic());
    }

    match unwrap_group(&statement.target) {
        Expr::Identifier(identifier) => {
            lower_identifier_assignment(identifier, &statement.value, context)
        }
        Expr::Member(member) => lower_aggregate_field_assignment(member, &statement.value, context),
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

fn lower_identifier_assignment(
    identifier: &crate::ast::IdentifierExpr,
    value: &Expr,
    context: &LoweringContext,
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
            let (mut instructions, value) = lower_usize_expression_to_word(value, context)?;
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
            lower_aggregate_member_value_assignment(destination, offset, layout, value, context)
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
                FallibleFailureMode::Propagate,
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

fn lower_aggregate_call_member_value_assignment(
    destination: AggregateLocation,
    destination_offset: u32,
    layout: ValueLayout,
    call: &CallExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let target = context.call_target(call, &identifier.name);
    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let callee_layout = match &return_type {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Err(unsupported_assignment_diagnostic()),
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
            &identifier.name,
            context,
            &mut temporaries,
        )?;
    instructions.append(&mut argument_instructions);
    match return_type {
        Type::Aggregate { .. } => {
            instructions.push(Instruction::CallAggregate {
                destination: AggregateLocation::Slot(source_slot),
                target,
                arguments,
            });
        }
        Type::DirectAggregate { .. } => {
            instructions.push(Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(source_slot),
                target,
                arguments,
                layout,
            });
        }
        _ => unreachable!("aggregate field assignment requires aggregate call return type"),
    }
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
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let target = context.call_target(call, &identifier.name);
    let Some(Type::Fallible(success)) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let callee_layout = match success.as_ref() {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Err(unsupported_assignment_diagnostic()),
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
            &identifier.name,
            context,
            &mut temporaries,
        )?;
    instructions.append(&mut argument_instructions);
    match success.as_ref() {
        Type::Aggregate { .. } => {
            instructions.push(Instruction::CallFallibleAggregate {
                destination: AggregateLocation::Slot(source_slot),
                target,
                arguments,
                failure_mode,
            });
        }
        Type::DirectAggregate { .. } => {
            instructions.push(Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(source_slot),
                target,
                arguments,
                layout,
                failure_mode,
            });
        }
        _ => unreachable!("fallible aggregate field assignment requires aggregate success type"),
    }
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

fn aggregate_assignment_root_and_path<'a>(expression: &'a Expr) -> Option<(&'a str, Vec<&'a str>)> {
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
    match unwrap_group(expression) {
        Expr::StructLiteral(literal) => {
            lower_aggregate_struct_literal_assignment(slot_index, layout, literal, context)
        }
        Expr::Call(call) => lower_aggregate_call_assignment(slot_index, layout, call, context),
        Expr::Identifier(identifier) => {
            lower_aggregate_copy_assignment(slot_index, layout, &identifier.name, context)
        }
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_fallible_call_assignment(
                slot_index,
                layout,
                call,
                FallibleFailureMode::Propagate,
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
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_assignment_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let callee_layout = match &return_type {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Err(unsupported_assignment_diagnostic()),
    };
    if callee_layout != layout {
        return Err(unsupported_assignment_diagnostic());
    }

    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &identifier.name, context)?;
    match return_type {
        Type::Aggregate { .. } => {
            instructions.push(Instruction::CallAggregate {
                destination: AggregateLocation::Slot(slot_index),
                target,
                arguments,
            });
        }
        Type::DirectAggregate { .. } => {
            instructions.push(Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(slot_index),
                target,
                arguments,
                layout,
            });
        }
        _ => unreachable!("aggregate call assignment requires aggregate return type"),
    }
    Ok(instructions)
}

fn lower_aggregate_fallible_call_assignment(
    slot_index: usize,
    layout: ValueLayout,
    call: &CallExpr,
    failure_mode: FallibleFailureMode,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_assignment_diagnostic());
    };

    let target = context.call_target(call, &identifier.name);
    let Some(Type::Fallible(success)) = context.call_return_type(&target) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let callee_layout = match success.as_ref() {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Err(unsupported_assignment_diagnostic()),
    };
    if callee_layout != layout {
        return Err(unsupported_assignment_diagnostic());
    }

    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &identifier.name, context)?;
    match success.as_ref() {
        Type::Aggregate { .. } => {
            instructions.push(Instruction::CallFallibleAggregate {
                destination: AggregateLocation::Slot(slot_index),
                target,
                arguments,
                failure_mode,
            });
        }
        Type::DirectAggregate { .. } => {
            instructions.push(Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(slot_index),
                target,
                arguments,
                layout,
                failure_mode,
            });
        }
        _ => unreachable!("fallible aggregate assignment requires aggregate success type"),
    }
    Ok(instructions)
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
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_slice_local_location()?;
    let instructions =
        lower_slice_expression_to_location(&statement.initializer, destination, context)?;
    context.define_slice_local(statement.name.clone());
    Ok(instructions)
}

fn scalar_binding_kind(
    statement: &BindingStmt,
    context: &LoweringContext,
) -> Result<ScalarBindingKind, Vec<Diagnostic>> {
    match &statement.ty {
        Some(ty) if is_i32_type(ty) => Ok(ScalarBindingKind::I32),
        Some(ty) if is_u8_type(ty) => Ok(ScalarBindingKind::U8),
        Some(ty) if is_usize_type(ty) => Ok(ScalarBindingKind::Usize),
        Some(ty) if is_bool_type(ty) => Ok(ScalarBindingKind::Bool),
        Some(ty) if is_str_type(ty) => Ok(ScalarBindingKind::Str),
        Some(ty) if is_u8_slice_type(ty) => Ok(ScalarBindingKind::Slice),
        Some(_) => Err(unsupported_binding_diagnostic(
            "IR v0 can only lower local bindings annotated as `i32`, `u8`, `usize`, `bool`, `&str`, `&[u8]`, or `&+[u8]`",
        )),
        None if expression_is_lowerable_bool_binding(&statement.initializer, context) => {
            Ok(ScalarBindingKind::Bool)
        }
        None if expression_is_bool_returning_call(&statement.initializer, context) => {
            Ok(ScalarBindingKind::Bool)
        }
        None if expression_is_unsupported_bool_comparison_binding(
            &statement.initializer,
            context,
        ) =>
        {
            Ok(ScalarBindingKind::Bool)
        }
        None => Ok(ScalarBindingKind::I32),
    }
}

fn expression_is_bool_returning_call(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Call(call) => {
            let Expr::Identifier(identifier) = call.callee.as_ref() else {
                return false;
            };
            context.call_return_type(&context.call_target(call, &identifier.name))
                == Some(&Type::Bool)
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

fn is_i32_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Reference(reference) if reference.name == "i32")
}

fn is_u8_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Reference(reference) if reference.name == "u8")
}

fn is_usize_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Reference(reference) if reference.name == "usize")
}

fn is_bool_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Reference(reference) if reference.name == "bool")
}

fn is_str_type(ty: &TypeExpr) -> bool {
    matches!(
        ty,
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite
                && matches!(borrow.inner.as_ref(), TypeExpr::Reference(reference) if reference.name == "str")
    )
}

fn is_u8_slice_type(ty: &TypeExpr) -> bool {
    matches!(
        ty,
        TypeExpr::Borrow(borrow)
            if matches!(
                borrow.inner.as_ref(),
                TypeExpr::View(view)
                    if !view.is_readwrite
                        && matches!(view.element.as_ref(), TypeExpr::Reference(reference) if reference.name == "u8")
            )
    )
}

fn call_success_type_is_copy_struct(call: &CallExpr, context: &LoweringContext) -> bool {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return false;
    };
    let Some(signature) = resolved.call_signature_for_call(call) else {
        return false;
    };
    type_expr_is_copy_struct(&signature.return_type, resolved)
}

fn call_success_aggregate_fields(
    call: &CallExpr,
    context: &LoweringContext,
) -> Vec<super::context::AggregateField> {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Vec::new();
    };
    let Some(signature) = resolved.call_signature_for_call(call) else {
        return Vec::new();
    };
    aggregate_fields_from_type_expr(&signature.return_type, resolved).unwrap_or_default()
}

fn type_expr_is_copy_struct(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    type_expr_is_copy_struct_inner(ty, resolved, &mut HashSet::new())
}

fn type_expr_is_copy_struct_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        TypeExpr::Reference(reference) => {
            let Some(symbol) = resolved.type_symbol_by_name(&reference.name) else {
                return false;
            };
            match symbol.kind {
                TypeSymbolKind::Struct => symbol.is_copy,
                TypeSymbolKind::Alias => {
                    if !resolving_names.insert(symbol.canonical_name.clone()) {
                        return false;
                    }
                    let is_copy = symbol.alias_target.as_ref().is_some_and(|target| {
                        type_expr_is_copy_struct_inner(target, resolved, resolving_names)
                    });
                    resolving_names.remove(&symbol.canonical_name);
                    is_copy
                }
                TypeSymbolKind::Enum | TypeSymbolKind::Trait => false,
            }
        }
        TypeExpr::Fallible(fallible) => {
            type_expr_is_copy_struct_inner(&fallible.success, resolved, resolving_names)
        }
        _ => false,
    }
}

fn unsupported_binding_diagnostic(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error("E8008", message)]
}

fn unsupported_assignment_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        "IR v0 can only lower simple `=` assignment to scalar local bindings or aggregate slots",
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
    Slice,
}
