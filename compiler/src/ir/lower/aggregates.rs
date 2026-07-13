use super::context::{AggregateField, AggregateFieldKind, LoweringContext};
use super::expressions::{
    lower_bool_expression_to_value, lower_i32_expression_to_word, lower_u8_expression_to_word,
    lower_usize_expression_to_word,
};
use crate::abi::{AbiType, ValueLayout, abi_value_from_type_expr, layout_of, layout_struct};
use crate::ast::{Expr, StructLiteralExpr, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{AggregateLocation, Instruction, UsizeValue};
use crate::resolve::ResolveOutput;
use std::collections::HashMap;

pub(super) fn supported_aggregate_copy_layout(layout: ValueLayout) -> bool {
    matches!(layout.size % 8, 0 | 1 | 4)
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
    let struct_layout = layout_struct(&fields).map_err(|_error| {
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
        let offset = u32::try_from(field_layout.offset).map_err(|_error| {
            unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
        })?;
        let mut field_instructions = lower_aggregate_field_to_location(
            field_type,
            &field.value,
            destination,
            offset,
            diagnostic_code,
            subject,
            resolved,
            context,
        )?;
        instructions.append(&mut field_instructions);
    }

    Ok(instructions)
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
            let Expr::StructLiteral(literal) = expression else {
                return Err(unsupported_aggregate_struct_literal_diagnostic(
                    diagnostic_code,
                    subject,
                ));
            };
            let actual = abi_value_from_type_expr(&literal.ty, resolved).map_err(|_error| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
            let expected_layout = layout_of(field_type).map_err(|_error| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
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
            )
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
        )?);
    }
    Ok(instructions)
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
            "IR v0 can only lower aggregate {subject} when the expression is a struct literal with scalar integer, bool, or `std/ptr.from_addr` pointer fields"
        ),
    )]
}
