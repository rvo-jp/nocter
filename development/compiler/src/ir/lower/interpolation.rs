//! Lowering for owned standard-library string interpolation.
//!
//! Targets come exclusively from the typecheck interpolation plan. This keeps
//! std declaration discovery and validation out of IR lowering.

use super::aggregates::{
    aggregate_fields_from_type_expr_with_resolver, aggregate_type_layout,
    push_aggregate_call_instruction, type_expr_is_copy_aggregate_value_with_resolver,
};
use super::context::LoweringContext;
use super::expressions::{
    TemporaryAllocator, lower_bool_expression_to_value, lower_i32_expression_to_word,
    lower_str_expression_to_value, lower_u8_expression_to_word, lower_usize_expression_to_word,
};
use super::functions::{
    lower_aggregate_drop_instructions_at_location, lower_aggregate_return_expression_to_location,
};
use crate::ast::{BindingStmt, Expr, InterpolatedStringExpr, InterpolatedStringPart};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BorrowArgument, BorrowSource, Instruction, ScalarArgument, StrValue,
    UsizeValue,
};
use crate::semantics::InterpolationInputKind;

pub(super) fn lower_interpolated_string_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::InterpolatedString(interpolated) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let ty = context
        .binding_type_expr(statement.name_span)
        .or_else(|| context.expression_type_expr(interpolated.span))
        .ok_or_else(|| interpolation_diagnostic("the result type fact is unavailable"))?;
    let value = context
        .abi_value_for_type_expr(&ty)
        .ok_or_else(|| interpolation_diagnostic("the owned String ABI layout is unavailable"))?;
    let (root_source, resolved) = context
        .resolved_calls()
        .ok_or_else(|| interpolation_diagnostic("resolution facts are unavailable"))?;
    let is_copy = type_expr_is_copy_aggregate_value_with_resolver(&ty, resolved, |source| {
        context.resolved_source(source)
    });
    let drop_kind = context.aggregate_drop_for_type_expr(&ty);
    let fields =
        aggregate_fields_from_type_expr_with_resolver(&ty, root_source, resolved, |source| {
            context.resolved_source(source)
        })
        .unwrap_or_default();
    let slot_index = context.define_aggregate_local(
        statement.name.clone(),
        value.layout,
        is_copy,
        drop_kind,
        fields,
    );
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: value.layout,
    }];
    instructions.extend(lower_interpolated_string_to_slot(
        interpolated,
        slot_index,
        context,
    )?);
    Ok(Some(instructions))
}

pub(in crate::ir::lower) fn lower_interpolated_string_return_to_location(
    interpolated: &InterpolatedStringExpr,
    destination: AggregateLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let plan = context
        .interpolation_plan(interpolated.span)
        .ok_or_else(|| interpolation_diagnostic("the trusted lowering plan is unavailable"))?;
    let constructor = context
        .runtime_callable_target(&plan.constructor)
        .ok_or_else(|| interpolation_diagnostic("the constructor target is unavailable"))?;
    let return_type = context
        .call_return_type(&constructor)
        .cloned()
        .ok_or_else(|| interpolation_diagnostic("the constructor ABI is unavailable"))?;
    let layout = aggregate_type_layout(&return_type)
        .ok_or_else(|| interpolation_diagnostic("the constructor does not return an aggregate"))?;
    let result_ty = context
        .expression_type_expr(interpolated.span)
        .ok_or_else(|| interpolation_diagnostic("the result type fact is unavailable"))?;
    let drop_kind = context
        .aggregate_drop_for_type_expr(&result_ty)
        .ok_or_else(|| interpolation_diagnostic("the result has no trusted drop glue"))?;
    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut local_context = context.clone();
    if !local_context.register_or_complete_temporary_aggregate_drop(slot_index, layout, drop_kind) {
        return Err(interpolation_diagnostic(
            "the result temporary drop state could not be registered",
        ));
    }
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    instructions.extend(lower_interpolated_string_to_slot(
        interpolated,
        slot_index,
        &local_context,
    )?);
    instructions.push(Instruction::CopyAggregate {
        destination,
        source: AggregateLocation::Slot(slot_index),
        layout,
    });
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_interpolated_string_to_slot(
    interpolated: &InterpolatedStringExpr,
    slot_index: usize,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let plan = context
        .interpolation_plan(interpolated.span)
        .ok_or_else(|| interpolation_diagnostic("the trusted lowering plan is unavailable"))?;
    if plan.parts.len() != interpolated.parts.len() {
        return Err(interpolation_diagnostic(
            "the lowering plan does not match the parsed parts",
        ));
    }
    let constructor = context
        .runtime_callable_target(&plan.constructor)
        .ok_or_else(|| interpolation_diagnostic("the constructor target is unavailable"))?;
    let return_type = context
        .call_return_type(&constructor)
        .cloned()
        .ok_or_else(|| interpolation_diagnostic("the constructor ABI is unavailable"))?;
    let layout = aggregate_type_layout(&return_type)
        .ok_or_else(|| interpolation_diagnostic("the constructor does not return an aggregate"))?;
    let mut instructions = Vec::new();
    push_aggregate_call_instruction(
        &mut instructions,
        &return_type,
        AggregateLocation::Slot(slot_index),
        constructor,
        vec![ScalarArgument::Usize(UsizeValue::Const(0))],
        layout,
    );

    for (part, planned) in interpolated.parts.iter().zip(plan.parts.iter()) {
        let formatter = context
            .runtime_callable_target(&planned.formatter)
            .ok_or_else(|| interpolation_diagnostic("a formatter target is unavailable"))?;
        let mut cleanup = Vec::new();
        let value = match (part, planned.input) {
            (InterpolatedStringPart::Text(text), InterpolationInputKind::Str) => {
                ScalarArgument::Str(StrValue::StaticBytes(text.value.as_bytes().to_vec()))
            }
            (InterpolatedStringPart::Expression(part), InterpolationInputKind::Str) => {
                let mut temporaries = TemporaryAllocator::new(context)?;
                let lowered =
                    lower_str_expression_to_value(&part.expression, context, &mut temporaries)?;
                instructions.extend(lowered.instructions);
                ScalarArgument::Str(lowered.value)
            }
            (InterpolatedStringPart::Expression(part), InterpolationInputKind::I32) => {
                let (lowered, value) = lower_i32_expression_to_word(&part.expression, context)?;
                instructions.extend(lowered);
                ScalarArgument::I32(value)
            }
            (InterpolatedStringPart::Expression(part), InterpolationInputKind::U8) => {
                let (lowered, value) = lower_u8_expression_to_word(&part.expression, context)?;
                instructions.extend(lowered);
                ScalarArgument::U8(value)
            }
            (InterpolatedStringPart::Expression(part), InterpolationInputKind::Usize) => {
                let (lowered, value) = lower_usize_expression_to_word(&part.expression, context)?;
                instructions.extend(lowered);
                ScalarArgument::Usize(value)
            }
            (InterpolatedStringPart::Expression(part), InterpolationInputKind::Bool) => {
                let lowered = lower_bool_expression_to_value(&part.expression, context, "E8015")?;
                instructions.extend(lowered.instructions);
                ScalarArgument::Bool(lowered.value)
            }
            (InterpolatedStringPart::Expression(part), InterpolationInputKind::String) => {
                let (source, mut lowered, mut drops) = lower_string_expression_to_borrow_source(
                    &part.expression,
                    &return_type,
                    context,
                )?;
                instructions.append(&mut lowered);
                cleanup.append(&mut drops);
                ScalarArgument::Borrow(BorrowArgument { source })
            }
            _ => {
                return Err(interpolation_diagnostic(
                    "a parsed part does not match its trusted lowering plan",
                ));
            }
        };
        instructions.push(Instruction::CallVoid {
            target: formatter,
            arguments: vec![
                ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(slot_index),
                }),
                value,
            ],
        });
        instructions.append(&mut cleanup);
    }

    Ok(instructions)
}

fn lower_string_expression_to_borrow_source(
    expression: &Expr,
    string_ir_type: &crate::ir::Type,
    context: &LoweringContext,
) -> Result<(BorrowSource, Vec<Instruction>, Vec<Instruction>), Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => {
            let local = context.aggregate_local(&identifier.name).ok_or_else(|| {
                interpolation_diagnostic("the String value has no aggregate storage")
            })?;
            Ok((
                BorrowSource::AggregateSlot(local.slot_index),
                Vec::new(),
                Vec::new(),
            ))
        }
        expression => {
            let ty = context
                .expression_type_expr(expression.span())
                .ok_or_else(|| interpolation_diagnostic("a String expression has no type fact"))?;
            let value = context.abi_value_for_type_expr(&ty).ok_or_else(|| {
                interpolation_diagnostic("a String expression has no aggregate ABI layout")
            })?;
            let drop_kind = context.aggregate_drop_for_type_expr(&ty).ok_or_else(|| {
                interpolation_diagnostic("a temporary String has no trusted drop glue")
            })?;
            let (_, resolved) = context
                .resolved_calls()
                .ok_or_else(|| interpolation_diagnostic("resolution facts are unavailable"))?;
            let mut temporaries = TemporaryAllocator::new(context)?;
            let slot_index = temporaries.next_aggregate_slot();
            let mut local_context = context.clone();
            if !local_context.register_or_complete_temporary_aggregate_drop(
                slot_index,
                value.layout,
                drop_kind.clone(),
            ) {
                return Err(interpolation_diagnostic(
                    "a temporary String drop state could not be registered",
                ));
            }
            let mut instructions = vec![Instruction::ReserveAggregateSlot {
                slot_index,
                layout: value.layout,
            }];
            instructions.extend(lower_aggregate_return_expression_to_location(
                expression,
                string_ir_type,
                AggregateLocation::Slot(slot_index),
                context.function_name(),
                resolved,
                &local_context,
            )?);
            let drops = lower_aggregate_drop_instructions_at_location(
                "temporary interpolation String",
                AggregateLocation::Slot(slot_index),
                0,
                value.layout,
                &drop_kind,
                context,
            )?;
            Ok((BorrowSource::AggregateSlot(slot_index), instructions, drops))
        }
    }
}

fn interpolation_diagnostic(detail: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8015",
        format!("IR cannot lower owned string interpolation: {detail}"),
    )]
}

fn unwrap_group(mut expression: &Expr) -> &Expr {
    while let Expr::Group(group) = expression {
        expression = &group.expression;
    }
    expression
}
