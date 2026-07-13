use super::context::{AggregateField, AggregateFieldKind, LoweringContext};
use super::expressions::{
    TemporaryAllocator, lower_aggregate_member_field_access, lower_bool_expression_to_value,
    lower_call_arguments_to_scalar_arguments_with_temporaries, lower_i32_expression_to_word,
    lower_u8_expression_to_word, lower_usize_expression_to_word,
};
use crate::abi::{AbiType, ValueLayout, abi_value_from_type_expr, layout_of, layout_struct};
use crate::ast::{CallExpr, Expr, StructLiteralExpr, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, CallTarget, FallibleFailureMode, Instruction, ScalarArgument, Type,
    UsizeValue,
};
use crate::resolve::ResolveOutput;
use std::collections::HashMap;

pub(super) fn supported_aggregate_copy_layout(layout: ValueLayout) -> bool {
    layout.size > 0
}

pub(super) fn aggregate_type_layout(ty: &Type) -> Option<ValueLayout> {
    match ty {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => Some(*layout),
        _ => None,
    }
}

pub(super) fn aggregate_call_instruction(
    return_type: &Type,
    destination: AggregateLocation,
    target: CallTarget,
    arguments: Vec<ScalarArgument>,
    layout: ValueLayout,
) -> Instruction {
    match return_type {
        Type::Aggregate { .. } => Instruction::CallAggregate {
            destination,
            target,
            arguments,
        },
        Type::DirectAggregate { .. } => Instruction::CallDirectAggregate {
            destination,
            target,
            arguments,
            layout,
        },
        _ => unreachable!("aggregate call instruction requires aggregate return type"),
    }
}

pub(super) fn push_aggregate_call_instruction(
    instructions: &mut Vec<Instruction>,
    return_type: &Type,
    destination: AggregateLocation,
    target: CallTarget,
    arguments: Vec<ScalarArgument>,
    layout: ValueLayout,
) {
    instructions.push(aggregate_call_instruction(
        return_type,
        destination,
        target,
        arguments,
        layout,
    ));
}

pub(super) fn fallible_aggregate_call_instruction(
    success_type: &Type,
    destination: AggregateLocation,
    target: CallTarget,
    arguments: Vec<ScalarArgument>,
    layout: ValueLayout,
    failure_mode: FallibleFailureMode,
) -> Instruction {
    match success_type {
        Type::Aggregate { .. } => Instruction::CallFallibleAggregate {
            destination,
            target,
            arguments,
            failure_mode,
        },
        Type::DirectAggregate { .. } => Instruction::CallFallibleDirectAggregate {
            destination,
            target,
            arguments,
            layout,
            failure_mode,
        },
        _ => unreachable!("fallible aggregate call instruction requires aggregate success type"),
    }
}

pub(super) fn push_fallible_aggregate_call_instruction(
    instructions: &mut Vec<Instruction>,
    success_type: &Type,
    destination: AggregateLocation,
    target: CallTarget,
    arguments: Vec<ScalarArgument>,
    layout: ValueLayout,
    failure_mode: FallibleFailureMode,
) {
    instructions.push(fallible_aggregate_call_instruction(
        success_type,
        destination,
        target,
        arguments,
        layout,
        failure_mode,
    ));
}

pub(super) fn lower_aggregate_struct_literal_to_location(
    literal: &StructLiteralExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_aggregate_struct_literal_to_location_at_offset(
        literal,
        expected_layout,
        destination,
        0,
        diagnostic_code,
        subject,
        resolved,
        context,
    )
}

pub(super) fn lower_aggregate_struct_literal_to_location_with_temporaries(
    literal: &StructLiteralExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_aggregate_struct_literal_to_location_at_offset_with_temporaries(
        literal,
        expected_layout,
        destination,
        0,
        diagnostic_code,
        subject,
        resolved,
        context,
        temporaries,
    )
}

pub(super) fn lower_aggregate_struct_literal_to_location_at_offset(
    literal: &StructLiteralExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    base_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_aggregate_struct_literal_to_location_at_offset_with_temporaries(
        literal,
        expected_layout,
        destination,
        base_offset,
        diagnostic_code,
        subject,
        resolved,
        context,
        &mut temporaries,
    )
}

fn lower_aggregate_struct_literal_to_location_at_offset_with_temporaries(
    literal: &StructLiteralExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    base_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let value = abi_value_from_type_expr(&literal.ty, resolved).map_err(|_error| {
        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
    })?;
    if value.layout != expected_layout {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

    let AbiType::Struct(fields) = value.ty else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    lower_aggregate_struct_fields_to_location(
        &fields,
        literal,
        destination,
        base_offset,
        diagnostic_code,
        subject,
        resolved,
        context,
        temporaries,
    )
}

pub(super) fn aggregate_fields_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<Vec<AggregateField>> {
    let ty = match ty {
        TypeExpr::Fallible(fallible) => &fallible.success,
        _ => ty,
    };
    let value = abi_value_from_type_expr(ty, resolved).ok()?;
    let AbiType::Struct(fields) = value.ty else {
        return Some(Vec::new());
    };
    let struct_layout = layout_struct(&fields).ok()?;

    let mut aggregate_fields = Vec::new();
    for (field, layout) in fields.iter().zip(struct_layout.fields.iter()) {
        collect_aggregate_fields(&field.name, &field.ty, layout.offset, &mut aggregate_fields)?;
    }
    Some(aggregate_fields)
}

fn collect_aggregate_fields(
    name: &str,
    ty: &AbiType,
    base_offset: u64,
    aggregate_fields: &mut Vec<AggregateField>,
) -> Option<()> {
    if let Some(kind) = aggregate_field_kind_from_abi_type(ty) {
        let offset = u32::try_from(base_offset).ok()?;
        aggregate_fields.push(AggregateField {
            name: name.to_string(),
            offset,
            kind,
        });
        return Some(());
    }

    let AbiType::Struct(fields) = ty else {
        return Some(());
    };
    let struct_layout = layout_struct(fields).ok()?;
    let offset = u32::try_from(base_offset).ok()?;
    let mut nested_fields = Vec::new();
    for (field, layout) in fields.iter().zip(struct_layout.fields.iter()) {
        collect_aggregate_fields(&field.name, &field.ty, layout.offset, &mut nested_fields)?;
    }
    aggregate_fields.push(AggregateField {
        name: name.to_string(),
        offset,
        kind: AggregateFieldKind::Aggregate {
            layout: ValueLayout::new(struct_layout.size, struct_layout.align),
            fields: nested_fields,
        },
    });

    for (field, layout) in fields.iter().zip(struct_layout.fields.iter()) {
        let offset = base_offset.checked_add(layout.offset)?;
        collect_aggregate_fields(
            &format!("{name}.{}", field.name),
            &field.ty,
            offset,
            aggregate_fields,
        )?;
    }
    Some(())
}

fn aggregate_field_kind_from_abi_type(ty: &AbiType) -> Option<AggregateFieldKind> {
    match ty {
        AbiType::I32 => Some(AggregateFieldKind::I32),
        AbiType::U8 => Some(AggregateFieldKind::U8),
        AbiType::Bool => Some(AggregateFieldKind::Bool),
        AbiType::U64 | AbiType::Usize | AbiType::Pointer => Some(AggregateFieldKind::Usize),
        _ => None,
    }
}

fn lower_aggregate_field_to_location(
    field_type: &AbiType,
    expression: &Expr,
    destination: AggregateLocation,
    offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match field_type {
        AbiType::U64 | AbiType::Usize => {
            let (mut instructions, value) = lower_usize_expression_to_word(expression, context)?;
            instructions.push(Instruction::StoreAggregateUsize {
                destination,
                offset,
                value,
            });
            Ok(instructions)
        }
        AbiType::I32 => {
            validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;
            let (mut instructions, value) = lower_i32_expression_to_word(expression, context)?;
            instructions.push(Instruction::StoreAggregateI32 {
                destination,
                offset,
                value,
            });
            Ok(instructions)
        }
        AbiType::U8 => {
            validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;
            let (mut instructions, value) = lower_u8_expression_to_word(expression, context)?;
            instructions.push(Instruction::StoreAggregateU8 {
                destination,
                offset,
                value,
            });
            Ok(instructions)
        }
        AbiType::Bool => {
            validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;
            let mut lowered = lower_bool_expression_to_value(expression, context, diagnostic_code)?;
            lowered.instructions.push(Instruction::StoreAggregateBool {
                destination,
                offset,
                value: lowered.value,
            });
            Ok(lowered.instructions)
        }
        AbiType::Pointer => {
            let (mut instructions, value) =
                lower_aggregate_pointer_field_value(expression, diagnostic_code, subject, context)?;
            instructions.push(Instruction::StoreAggregateUsize {
                destination,
                offset,
                value,
            });
            Ok(instructions)
        }
        AbiType::Struct(fields) => {
            let expected_layout = layout_of(field_type).map_err(|_error| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;

            match expression {
                Expr::StructLiteral(literal) => {
                    let actual =
                        abi_value_from_type_expr(&literal.ty, resolved).map_err(|_error| {
                            unsupported_aggregate_struct_literal_diagnostic(
                                diagnostic_code,
                                subject,
                            )
                        })?;
                    if actual.layout != expected_layout {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    }
                    lower_aggregate_struct_fields_to_location(
                        fields,
                        literal,
                        destination,
                        offset,
                        diagnostic_code,
                        subject,
                        resolved,
                        context,
                        temporaries,
                    )
                }
                Expr::Identifier(identifier) => {
                    let Some(source) = context.aggregate_local(&identifier.name) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    if source.layout != expected_layout
                        || !source.is_copy
                        || !supported_aggregate_copy_layout(expected_layout)
                    {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    }
                    Ok(vec![Instruction::CopyAggregateRange {
                        destination,
                        destination_offset: offset,
                        source: AggregateLocation::Slot(source.slot_index),
                        source_offset: 0,
                        layout: expected_layout,
                    }])
                }
                Expr::Call(call) => lower_aggregate_call_field_value_to_location(
                    call,
                    expected_layout,
                    destination,
                    offset,
                    diagnostic_code,
                    subject,
                    context,
                    temporaries,
                ),
                Expr::Propagate(propagation) => {
                    let Some(call) = call_expression(&propagation.expression) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    lower_aggregate_fallible_call_field_value_to_location(
                        call,
                        expected_layout,
                        destination,
                        offset,
                        diagnostic_code,
                        subject,
                        context,
                        temporaries,
                        FallibleFailureMode::Propagate,
                    )
                }
                Expr::Force(force) => {
                    let Some(call) = call_expression(&force.expression) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    lower_aggregate_fallible_call_field_value_to_location(
                        call,
                        expected_layout,
                        destination,
                        offset,
                        diagnostic_code,
                        subject,
                        context,
                        temporaries,
                        FallibleFailureMode::Trap,
                    )
                }
                Expr::Member(_) => lower_aggregate_member_field_value_to_location(
                    expression,
                    expected_layout,
                    destination,
                    offset,
                    diagnostic_code,
                    subject,
                    context,
                    temporaries,
                ),
                Expr::Group(group) => lower_aggregate_field_to_location(
                    field_type,
                    &group.expression,
                    destination,
                    offset,
                    diagnostic_code,
                    subject,
                    resolved,
                    context,
                    temporaries,
                ),
                _ => Err(unsupported_aggregate_struct_literal_diagnostic(
                    diagnostic_code,
                    subject,
                )),
            }
        }
        _ => Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        )),
    }
}

fn lower_aggregate_struct_fields_to_location(
    fields: &[crate::abi::AbiField],
    literal: &StructLiteralExpr,
    destination: AggregateLocation,
    base_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let struct_layout = layout_struct(fields).map_err(|_error| {
        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
    })?;
    let field_layouts = fields
        .iter()
        .zip(struct_layout.fields.iter())
        .map(|(field, layout)| (field.name.as_str(), (&field.ty, layout)))
        .collect::<HashMap<_, _>>();

    let mut instructions = Vec::new();
    for field in &literal.fields {
        let Some((field_type, field_layout)) = field_layouts.get(field.name.as_str()) else {
            return Err(unsupported_aggregate_struct_literal_diagnostic(
                diagnostic_code,
                subject,
            ));
        };
        let nested_offset = u32::try_from(field_layout.offset)
            .ok()
            .and_then(|offset| base_offset.checked_add(offset))
            .ok_or_else(|| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
        instructions.extend(lower_aggregate_field_to_location(
            field_type,
            &field.value,
            destination,
            nested_offset,
            diagnostic_code,
            subject,
            resolved,
            context,
            temporaries,
        )?);
    }
    Ok(instructions)
}

fn lower_aggregate_call_field_value_to_location(
    call: &CallExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    destination_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    let target = context.call_target(call, &identifier.name);
    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    let Some(layout) = aggregate_type_layout(&return_type) else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    if layout != expected_layout || !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

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
            temporaries,
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

fn lower_aggregate_fallible_call_field_value_to_location(
    call: &CallExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    destination_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    let target = context.call_target(call, &identifier.name);
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    let Some(layout) = aggregate_type_layout(success_type.as_ref()) else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    if layout != expected_layout || !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

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
            temporaries,
        )?;
    instructions.append(&mut argument_instructions);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success_type.as_ref(),
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

fn lower_aggregate_member_field_value_to_location(
    expression: &Expr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    destination_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(access) = lower_aggregate_member_field_access(expression, context, temporaries)?
    else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    let AggregateFieldKind::Aggregate { layout, .. } = access.kind else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    if layout != expected_layout || !access.is_copy || !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

    let mut instructions = access.instructions;
    instructions.push(Instruction::CopyAggregateRange {
        destination,
        destination_offset,
        source: access.source,
        source_offset: access.offset,
        layout,
    });
    Ok(instructions)
}

fn call_expression(expression: &Expr) -> Option<&CallExpr> {
    match expression {
        Expr::Call(call) => Some(call),
        Expr::Group(group) => call_expression(&group.expression),
        _ => None,
    }
}

fn validate_direct_aggregate_field_store(
    destination: AggregateLocation,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<(), Vec<Diagnostic>> {
    if matches!(destination, AggregateLocation::DirectReturn) {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }
    Ok(())
}

fn lower_aggregate_pointer_field_value(
    expression: &Expr,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    match expression {
        Expr::Call(call)
            if context.primitive_name_for_call(call) == Some("from_addr")
                && call.arguments.len() == 1 =>
        {
            lower_usize_expression_to_word(&call.arguments[0], context)
        }
        Expr::Group(group) => lower_aggregate_pointer_field_value(
            &group.expression,
            diagnostic_code,
            subject,
            context,
        ),
        _ => Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        )),
    }
}

pub(super) fn unsupported_aggregate_struct_literal_diagnostic(
    diagnostic_code: &'static str,
    subject: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        format!(
            "IR v0 can only lower aggregate {subject} from struct literals whose fields are supported scalar values, nested struct literals, copy aggregate values, aggregate calls, or aggregate member values"
        ),
    )]
}
