use super::context::{
    AggregateField, AggregateFieldKind, LoweringContext, drop_glue_for_type_expr,
};
use super::expressions::{
    TemporaryAllocator, lower_aggregate_member_field_access, lower_bool_expression_to_value,
    lower_call_arguments_to_scalar_arguments_with_temporaries, lower_catch_failure_mode,
    lower_i32_expression_to_word, lower_macos_syscall_primitive_call_to_location,
    lower_slice_expression_to_value, lower_str_expression_to_value, lower_u8_expression_to_word,
    lower_usize_expression_to_word, push_store_slice_view_to_aggregate_field,
    push_store_str_view_to_aggregate_field,
};
use super::functions::propagating_failure_mode;
use super::literals::{lower_u16_literal, lower_u32_literal};
use super::types::view_element_type_from_type_expr;
use crate::abi::{AbiType, ValueLayout, abi_value_from_type_expr, layout_of, layout_struct};
use crate::ast::{
    CallExpr, Expr, StructLiteralExpr, TypeExpr, UnaryOperator, substitute_type_expr_parameters,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, CallTarget, FallibleFailureMode, Instruction, ScalarArgument, Type,
    UsizeValue,
};
use crate::resolve::{ResolveOutput, StructFieldSignature, TypeSymbol, TypeSymbolKind};
use crate::source::SourceId;
use crate::typecheck::TypecheckSliceElementKind;
use std::collections::{HashMap, HashSet};

pub(super) fn supported_aggregate_copy_layout(layout: ValueLayout) -> bool {
    layout.size > 0
}

pub(super) fn aggregate_type_layout(ty: &Type) -> Option<ValueLayout> {
    match ty {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => Some(*layout),
        _ => None,
    }
}

pub(super) fn aggregate_call_return_layout_from_resolved(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<ValueLayout> {
    let (_root_source, resolved) = context.resolved_calls()?;
    let return_type = context.call_return_type_expr(call)?;
    let value = abi_value_from_type_expr(&return_type, resolved).ok()?;
    if matches!(value.ty, AbiType::Struct(_)) {
        Some(value.layout)
    } else {
        None
    }
}

pub(super) fn type_expr_is_copy_struct(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    type_expr_is_copy_struct_inner(ty, resolved, &mut HashSet::new())
}

fn type_expr_is_copy_struct_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        TypeExpr::Reference(reference) => {
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return false;
            };
            if symbol.generic_arity > 0 {
                return false;
            }
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
                TypeSymbolKind::Enum | TypeSymbolKind::Interface => false,
            }
        }
        TypeExpr::Generic(generic) => {
            let Some(symbol) = resolved.type_symbol_by_reference_name(&generic.name) else {
                return false;
            };
            let Some(substitutions) = generic_type_expr_substitutions(symbol, ty) else {
                return false;
            };
            match symbol.kind {
                TypeSymbolKind::Struct => symbol.is_copy,
                TypeSymbolKind::Alias => {
                    if !resolving_names.insert(symbol.canonical_name.clone()) {
                        return false;
                    }
                    let is_copy = symbol.alias_target.as_ref().is_some_and(|target| {
                        let target = substitute_type_expr_parameters(target, &substitutions);
                        type_expr_is_copy_struct_inner(&target, resolved, resolving_names)
                    });
                    resolving_names.remove(&symbol.canonical_name);
                    is_copy
                }
                TypeSymbolKind::Enum | TypeSymbolKind::Interface => false,
            }
        }
        TypeExpr::Fallible(fallible) => {
            type_expr_is_copy_struct_inner(&fallible.success, resolved, resolving_names)
        }
        TypeExpr::Optional(optional) => {
            type_expr_is_copy_struct_inner(&optional.inner, resolved, resolving_names)
        }
        _ => false,
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
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Option<Vec<AggregateField>> {
    let ty = match ty {
        TypeExpr::Fallible(fallible) => &fallible.success,
        TypeExpr::Optional(optional) => &optional.inner,
        _ => ty,
    };
    let value = abi_value_from_type_expr(ty, resolved).ok()?;
    let AbiType::Struct(fields) = value.ty else {
        return Some(Vec::new());
    };
    let struct_layout = layout_struct(&fields).ok()?;
    let source_fields = struct_field_signatures_from_type_expr(ty, resolved)?;
    if fields.len() != source_fields.len() {
        return None;
    }

    let mut aggregate_fields = Vec::new();
    for ((field, layout), source_field) in fields
        .iter()
        .zip(struct_layout.fields.iter())
        .zip(source_fields.iter())
    {
        collect_aggregate_fields(
            &field.name,
            &field.ty,
            Some(&source_field.ty),
            layout.offset,
            root_source,
            resolved,
            &mut aggregate_fields,
        )?;
    }
    Some(aggregate_fields)
}

fn struct_field_signatures_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<Vec<StructFieldSignature>> {
    match ty {
        TypeExpr::Reference(reference) => {
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
            if symbol.generic_arity > 0 {
                return None;
            }
            match symbol.kind {
                TypeSymbolKind::Struct => Some(symbol.fields.clone()),
                TypeSymbolKind::Alias => {
                    let target = symbol.alias_target.as_ref()?;
                    struct_field_signatures_from_type_expr(target, resolved)
                }
                TypeSymbolKind::Enum | TypeSymbolKind::Interface => None,
            }
        }
        TypeExpr::Generic(generic) => {
            let symbol = resolved.type_symbol_by_reference_name(&generic.name)?;
            let substitutions = generic_type_expr_substitutions(symbol, ty)?;
            match symbol.kind {
                TypeSymbolKind::Struct => Some(
                    symbol
                        .fields
                        .iter()
                        .cloned()
                        .map(|mut field| {
                            field.ty = substitute_type_expr_parameters(&field.ty, &substitutions);
                            field
                        })
                        .collect(),
                ),
                TypeSymbolKind::Alias => {
                    let target = symbol.alias_target.as_ref()?;
                    let target = substitute_type_expr_parameters(target, &substitutions);
                    struct_field_signatures_from_type_expr(&target, resolved)
                }
                TypeSymbolKind::Enum | TypeSymbolKind::Interface => None,
            }
        }
        TypeExpr::Fallible(fallible) => {
            struct_field_signatures_from_type_expr(&fallible.success, resolved)
        }
        TypeExpr::Optional(optional) => {
            struct_field_signatures_from_type_expr(&optional.inner, resolved)
        }
        _ => None,
    }
}

fn generic_type_expr_substitutions(
    symbol: &TypeSymbol,
    ty: &TypeExpr,
) -> Option<HashMap<String, TypeExpr>> {
    let TypeExpr::Generic(generic) = ty else {
        return None;
    };
    if symbol.generic_arity != generic.arguments.len() {
        return None;
    }
    Some(
        symbol
            .generic_parameters
            .iter()
            .cloned()
            .zip(generic.arguments.iter().cloned())
            .collect(),
    )
}

fn collect_aggregate_fields(
    name: &str,
    ty: &AbiType,
    source_ty: Option<&TypeExpr>,
    base_offset: u64,
    root_source: SourceId,
    resolved: &ResolveOutput,
    aggregate_fields: &mut Vec<AggregateField>,
) -> Option<()> {
    if let Some(kind) = aggregate_field_kind_from_abi_type(ty, source_ty, resolved) {
        let offset = u32::try_from(base_offset).ok()?;
        aggregate_fields.push(AggregateField {
            name: name.to_string(),
            offset,
            kind,
            is_copy: true,
            drop_glue: None,
        });
        return Some(());
    }

    let AbiType::Struct(fields) = ty else {
        return Some(());
    };
    let struct_layout = layout_struct(fields).ok()?;
    let offset = u32::try_from(base_offset).ok()?;
    let mut nested_fields = Vec::new();
    let nested_source_fields = if let Some(source_ty) = source_ty {
        let source_fields = struct_field_signatures_from_type_expr(source_ty, resolved)?;
        if fields.len() != source_fields.len() {
            return None;
        }
        Some(source_fields)
    } else {
        None
    };
    for (index, (field, layout)) in fields.iter().zip(struct_layout.fields.iter()).enumerate() {
        let nested_source_ty = nested_source_fields
            .as_ref()
            .and_then(|source_fields| source_fields.get(index))
            .map(|field| &field.ty);
        collect_aggregate_fields(
            &field.name,
            &field.ty,
            nested_source_ty,
            layout.offset,
            root_source,
            resolved,
            &mut nested_fields,
        )?;
    }
    aggregate_fields.push(AggregateField {
        name: name.to_string(),
        offset,
        kind: AggregateFieldKind::Aggregate {
            layout: ValueLayout::new(struct_layout.size, struct_layout.align),
            fields: nested_fields,
        },
        is_copy: source_ty.is_some_and(|ty| type_expr_is_copy_struct(ty, resolved)),
        drop_glue: source_ty.and_then(|ty| drop_glue_for_type_expr(ty, root_source, resolved)),
    });

    for (index, (field, layout)) in fields.iter().zip(struct_layout.fields.iter()).enumerate() {
        let offset = base_offset.checked_add(layout.offset)?;
        let nested_source_ty = nested_source_fields
            .as_ref()
            .and_then(|source_fields| source_fields.get(index))
            .map(|field| &field.ty);
        collect_aggregate_fields(
            &format!("{name}.{}", field.name),
            &field.ty,
            nested_source_ty,
            offset,
            root_source,
            resolved,
            aggregate_fields,
        )?;
    }
    Some(())
}

fn aggregate_field_kind_from_abi_type(
    ty: &AbiType,
    source_ty: Option<&TypeExpr>,
    resolved: &ResolveOutput,
) -> Option<AggregateFieldKind> {
    match ty {
        AbiType::I32 => Some(AggregateFieldKind::I32),
        AbiType::U16 => Some(AggregateFieldKind::U16),
        AbiType::U32 => Some(AggregateFieldKind::U32),
        AbiType::U8 => Some(AggregateFieldKind::U8),
        AbiType::Bool => Some(AggregateFieldKind::Bool),
        AbiType::U64 | AbiType::Usize | AbiType::Pointer => Some(AggregateFieldKind::Usize),
        AbiType::StrView => Some(AggregateFieldKind::Str),
        AbiType::SliceView => Some(AggregateFieldKind::Slice(
            source_ty
                .and_then(|ty| view_element_type_from_type_expr(ty, resolved))
                .map(typecheck_slice_element_kind_from_type)
                .unwrap_or(TypecheckSliceElementKind::Other),
        )),
        _ => None,
    }
}

fn typecheck_slice_element_kind_from_type(ty: Type) -> TypecheckSliceElementKind {
    match ty {
        Type::I32 => TypecheckSliceElementKind::I32,
        Type::U8 => TypecheckSliceElementKind::U8,
        Type::Usize => TypecheckSliceElementKind::Usize,
        Type::Bool => TypecheckSliceElementKind::Bool,
        Type::Str => TypecheckSliceElementKind::Str,
        _ => TypecheckSliceElementKind::Other,
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
        AbiType::U16 => {
            validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;
            Ok(vec![Instruction::StoreAggregateU16 {
                destination,
                offset,
                value: lower_u16_literal(expression)?,
            }])
        }
        AbiType::U32 => {
            validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;
            Ok(vec![Instruction::StoreAggregateU32 {
                destination,
                offset,
                value: lower_u32_literal(expression)?,
            }])
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
        AbiType::StrView => lower_str_view_field_to_location(
            expression,
            destination,
            offset,
            diagnostic_code,
            subject,
            context,
            temporaries,
        ),
        AbiType::SliceView => lower_slice_view_field_to_location(
            expression,
            destination,
            offset,
            diagnostic_code,
            subject,
            context,
            temporaries,
        ),
        AbiType::Pointer => {
            let (mut instructions, value) = lower_aggregate_pointer_field_value(
                expression,
                diagnostic_code,
                subject,
                context,
                temporaries,
            )?;
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
                Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
                    let Expr::Identifier(identifier) = unary.operand.as_ref() else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    let Some(source) = context.aggregate_local(&identifier.name) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    if source.layout != expected_layout
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
                        propagating_failure_mode(context)?,
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
                Expr::Catch(catch) => {
                    let Some(call) = call_expression(&catch.expression) else {
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
                        lower_catch_failure_mode(catch, context, 0)?,
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

fn lower_str_view_field_to_location(
    expression: &Expr,
    destination: AggregateLocation,
    offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut lowered = lower_str_expression_to_value(expression, context, temporaries)?;
    push_store_str_view_to_aggregate_field(
        &mut lowered.instructions,
        destination,
        offset,
        lowered.value,
        temporaries,
        || unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject),
    )?;
    Ok(lowered.instructions)
}

fn lower_slice_view_field_to_location(
    expression: &Expr,
    destination: AggregateLocation,
    offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut lowered = lower_slice_expression_to_value(expression, context, temporaries)?;
    push_store_slice_view_to_aggregate_field(
        &mut lowered.instructions,
        destination,
        offset,
        lowered.value,
        temporaries,
        || unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject),
    )?;
    Ok(lowered.instructions)
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
    if macos_syscall_primitive_call(call, context)
        && let Some(layout) = aggregate_call_return_layout_from_resolved(call, context)
        && layout == expected_layout
    {
        if destination_offset != 0 {
            let source_slot = temporaries.next_aggregate_slot();
            let Some(mut instructions) = lower_macos_syscall_primitive_call_to_location(
                call,
                AggregateLocation::Slot(source_slot),
                expected_layout,
                context,
                temporaries,
            )?
            else {
                return Err(unsupported_aggregate_struct_literal_diagnostic(
                    diagnostic_code,
                    subject,
                ));
            };
            let mut staged = vec![Instruction::ReserveAggregateSlot {
                slot_index: source_slot,
                layout,
            }];
            staged.append(&mut instructions);
            staged.push(Instruction::CopyAggregateRange {
                destination,
                destination_offset,
                source: AggregateLocation::Slot(source_slot),
                source_offset: 0,
                layout,
            });
            return Ok(staged);
        }
        let Some(instructions) = lower_macos_syscall_primitive_call_to_location(
            call,
            destination,
            expected_layout,
            context,
            temporaries,
        )?
        else {
            return Err(unsupported_aggregate_struct_literal_diagnostic(
                diagnostic_code,
                subject,
            ));
        };
        return Ok(instructions);
    }

    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
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
            &call_name,
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
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
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
            &call_name,
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
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    match expression {
        Expr::Call(call)
            if context.primitive_name_for_call(call) == Some("from_addr")
                && call.arguments.len() == 1 =>
        {
            lower_usize_expression_to_word(&call.arguments[0], context)
        }
        Expr::Member(_) => {
            let access = lower_aggregate_member_field_access(expression, context, temporaries)?
                .filter(|access| access.kind == AggregateFieldKind::Usize)
                .ok_or_else(|| {
                    unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
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
        Expr::Group(group) => lower_aggregate_pointer_field_value(
            &group.expression,
            diagnostic_code,
            subject,
            context,
            temporaries,
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
            "IR v0 can only lower aggregate {subject} from struct literals whose fields are supported scalar/view values (u8, u16, u32, bool, i32, usize/u64, pointer, &str, or slice views), nested struct literals, copy aggregate values, aggregate calls, or aggregate member values"
        ),
    )]
}
