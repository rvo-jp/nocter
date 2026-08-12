use super::field_values::{
    call_expression, lower_aggregate_call_field_value_to_location,
    lower_aggregate_fallible_call_field_value_to_location,
    lower_aggregate_member_field_value_to_location,
};
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_enum_field_value_to_location(
    expected_type: &AbiType,
    expression: &Expr,
    destination: AggregateLocation,
    offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let expected_layout = layout_of(expected_type).map_err(|_error| {
        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
    })?;
    if let Some(instructions) = lower_payload_enum_constructor_to_location_at_offset_with_progress(
        expression,
        expected_type,
        expected_layout,
        destination,
        offset,
        diagnostic_code,
        subject,
        resolved,
        context,
        temporaries,
        None,
    )? {
        return Ok(instructions);
    }

    match expression {
        Expr::Identifier(identifier) => lower_enum_local_field_value_to_location(
            &identifier.name,
            false,
            expected_layout,
            destination,
            offset,
            diagnostic_code,
            subject,
            context,
        ),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unwrap_enum_field_group(&unary.operand) else {
                return Err(unsupported_aggregate_struct_literal_diagnostic(
                    diagnostic_code,
                    subject,
                ));
            };
            lower_enum_local_field_value_to_location(
                &identifier.name,
                true,
                expected_layout,
                destination,
                offset,
                diagnostic_code,
                subject,
                context,
            )
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
                propagating_outcome_mode(&propagation.expression, context)?,
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
                OutcomeFailureMode::Trap,
            )
        }
        Expr::Catch(catch) => {
            let Some(call) = call_expression(&catch.expression) else {
                return Err(unsupported_aggregate_struct_literal_diagnostic(
                    diagnostic_code,
                    subject,
                ));
            };
            lower_aggregate_fallible_call_field_value_to_location_with(
                call,
                expected_layout,
                destination,
                offset,
                diagnostic_code,
                subject,
                context,
                temporaries,
                |source, success_type, context| {
                    let Some((_root_source, resolved)) = context.resolved_calls() else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    lower_value_catch_failure_mode(
                        catch,
                        context,
                        0,
                        None,
                        |result, context| {
                            lower_aggregate_return_expression_to_location(
                                result,
                                success_type,
                                source,
                                context.function_name(),
                                resolved,
                                context,
                            )
                        },
                        "enum payload `catch` fallback must produce the payload type or exit",
                    )
                },
            )
        }
        Expr::Otherwise(otherwise) => lower_aggregate_optional_otherwise_to_location(
            destination,
            offset,
            expected_layout,
            Some(expected_type),
            otherwise,
            context,
            || unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject),
        ),
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
        Expr::Group(group) => lower_enum_field_value_to_location(
            expected_type,
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

#[allow(clippy::too_many_arguments)]
fn lower_enum_local_field_value_to_location(
    name: &str,
    is_explicit_move: bool,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(source) = context.aggregate_local(name) else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    if source.layout != expected_layout
        || (!is_explicit_move && !source.is_copy)
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

fn unwrap_enum_field_group(mut expression: &Expr) -> &Expr {
    while let Expr::Group(group) = expression {
        expression = &group.expression;
    }
    expression
}
