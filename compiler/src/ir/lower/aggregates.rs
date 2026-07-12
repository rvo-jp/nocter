use super::context::LoweringContext;
use super::expressions::lower_usize_expression_to_word;
use crate::abi::{AbiType, ValueLayout, abi_value_from_type_expr, layout_struct};
use crate::ast::{Expr, StructLiteralExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{AggregateLocation, Instruction, UsizeValue};
use crate::resolve::ResolveOutput;
use std::collections::HashMap;

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
    if !value.is_indirect() || value.layout != expected_layout {
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
        .map(|(field, layout)| (field.name.as_str(), (&field.ty, layout.offset)))
        .collect::<HashMap<_, _>>();

    let mut instructions = Vec::new();
    for field in &literal.fields {
        let Some((field_type, offset)) = field_layouts.get(field.name.as_str()) else {
            return Err(unsupported_aggregate_struct_literal_diagnostic(
                diagnostic_code,
                subject,
            ));
        };
        let offset = u32::try_from(*offset).map_err(|_error| {
            unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
        })?;
        let (mut field_instructions, value) = lower_aggregate_word_field_value(
            field_type,
            &field.value,
            diagnostic_code,
            subject,
            context,
        )?;
        instructions.append(&mut field_instructions);
        instructions.push(Instruction::StoreAggregateUsize {
            destination,
            offset,
            value,
        });
    }

    Ok(instructions)
}

fn lower_aggregate_word_field_value(
    field_type: &AbiType,
    expression: &Expr,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    match field_type {
        AbiType::Usize => lower_usize_expression_to_word(expression, context),
        AbiType::Pointer => {
            lower_aggregate_pointer_field_value(expression, diagnostic_code, subject, context)
        }
        _ => Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        )),
    }
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
            "IR v0 can only lower aggregate {subject} when the expression is an indirect struct literal with `usize` fields or `std/ptr.from_addr` pointer fields"
        ),
    )]
}
