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
    TemporaryAllocator, lower_borrow_source_from_expression, lower_str_expression_to_value,
};
use super::functions::{
    lower_aggregate_drop_instructions_at_location, lower_aggregate_return_expression_to_location,
};
use crate::ast::{BindingStmt, Expr, InterpolatedStringExpr, InterpolatedStringPart};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BorrowArgument, BorrowSource, Instruction, ScalarArgument, StrValue, Type,
    UsizeValue,
};
use crate::typecheck::TypecheckInterpolationPart;

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
        let formatter_plan = &planned.formatter;
        let formatter = context
            .protocol_method_target(formatter_plan)
            .ok_or_else(|| interpolation_diagnostic("a formatter target is unavailable"))?;
        let (mut receiver_instructions, receiver, cleanup) =
            lower_format_receiver(part, planned, &formatter, context)?;
        instructions.append(&mut receiver_instructions);
        instructions.push(Instruction::CallVoid {
            target: formatter,
            arguments: vec![
                receiver,
                ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(slot_index),
                }),
            ],
        });
        instructions.extend(cleanup);
    }

    Ok(instructions)
}

fn lower_format_receiver(
    part: &InterpolatedStringPart,
    planned: &TypecheckInterpolationPart,
    formatter: &crate::ir::CallTarget,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, ScalarArgument, Vec<Instruction>), Vec<Diagnostic>> {
    let parameters = context
        .call_parameter_types(formatter)
        .ok_or_else(|| interpolation_diagnostic("the formatter ABI is unavailable"))?;
    let [receiver_type, output_type] = parameters else {
        return Err(interpolation_diagnostic(
            "the formatter ABI does not contain receiver and output parameters",
        ));
    };
    if !matches!(
        output_type,
        Type::Borrow {
            is_readwrite: true,
            inner,
        } if matches!(inner.as_ref(), Type::Aggregate { .. } | Type::DirectAggregate { .. })
    ) {
        return Err(interpolation_diagnostic(
            "the formatter output parameter is not a readwrite String borrow",
        ));
    }

    match (part, receiver_type) {
        (InterpolatedStringPart::Text(text), Type::Str) => Ok((
            Vec::new(),
            ScalarArgument::Str(StrValue::StaticBytes(text.value.as_bytes().to_vec())),
            Vec::new(),
        )),
        (InterpolatedStringPart::Expression(part), Type::Str) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered =
                lower_str_expression_to_value(&part.expression, context, &mut temporaries)?;
            Ok((
                lowered.instructions,
                ScalarArgument::Str(lowered.value),
                Vec::new(),
            ))
        }
        (
            InterpolatedStringPart::Expression(part),
            Type::Borrow {
                is_readwrite: false,
                inner,
            },
        ) => {
            if matches!(
                inner.as_ref(),
                Type::Aggregate { .. } | Type::DirectAggregate { .. }
            ) && !format_receiver_is_stable_place(&part.expression)
            {
                return lower_temporary_aggregate_format_receiver(
                    &part.expression,
                    &planned.accepted_type,
                    inner,
                    context,
                );
            }
            let mut temporaries = TemporaryAllocator::new(context)?;
            let parameter_type = Type::Borrow {
                is_readwrite: false,
                inner: inner.clone(),
            };
            let (instructions, source) = lower_borrow_source_from_expression(
                &part.expression,
                inner,
                false,
                &parameter_type,
                &planned.formatter.target_name,
                context,
                &mut temporaries,
            )?;
            Ok((
                instructions,
                ScalarArgument::Borrow(BorrowArgument { source }),
                Vec::new(),
            ))
        }
        _ => Err(interpolation_diagnostic(
            "a parsed part does not match its resolved formatter receiver",
        )),
    }
}

fn lower_temporary_aggregate_format_receiver(
    expression: &Expr,
    accepted_type: &crate::ast::TypeExpr,
    receiver_type: &Type,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, ScalarArgument, Vec<Instruction>), Vec<Diagnostic>> {
    let layout = match receiver_type {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => {
            return Err(interpolation_diagnostic(
                "a temporary format receiver has no aggregate layout",
            ));
        }
    };
    let (_, resolved) = context
        .resolved_calls()
        .ok_or_else(|| interpolation_diagnostic("resolution facts are unavailable"))?;
    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let drop_kind = context.aggregate_drop_for_type_expr(accepted_type);
    let mut local_context = context.clone();
    if let Some(drop_kind) = &drop_kind
        && !local_context.register_or_complete_temporary_aggregate_drop(
            slot_index,
            layout,
            drop_kind.clone(),
        )
    {
        return Err(interpolation_diagnostic(
            "a temporary format receiver drop state could not be registered",
        ));
    }
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    instructions.extend(lower_aggregate_return_expression_to_location(
        expression,
        receiver_type,
        AggregateLocation::Slot(slot_index),
        context.function_name(),
        resolved,
        &local_context,
    )?);
    let cleanup = if let Some(drop_kind) = &drop_kind {
        lower_aggregate_drop_instructions_at_location(
            "temporary interpolation format receiver",
            AggregateLocation::Slot(slot_index),
            0,
            layout,
            drop_kind,
            context,
        )?
    } else {
        Vec::new()
    };
    Ok((
        instructions,
        ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(slot_index),
        }),
        cleanup,
    ))
}

fn format_receiver_is_stable_place(expression: &Expr) -> bool {
    matches!(
        unwrap_group(expression),
        Expr::Identifier(_) | Expr::Member(_) | Expr::Index(_)
    )
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
