use super::*;

pub(in crate::ir::lower) fn lower_aggregate_return_expression_to_location(
    expression: &Expr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Expr::InterpolatedString(interpolated) = expression {
        return crate::ir::lower::interpolation::lower_interpolated_string_return_to_location(
            interpolated,
            destination,
            context,
        );
    }
    if matches!(
        expression,
        Expr::TypedSequenceLiteral(_) | Expr::TypedStringLiteral(_)
    ) {
        return crate::ir::lower::typed_literals::lower_typed_literal_to_location(
            expression,
            destination,
            context,
        )?
        .ok_or_else(|| unsupported_aggregate_return_diagnostic(function_name));
    }
    match expression {
        Expr::StructLiteral(literal) => lower_aggregate_struct_literal_return_to_location(
            literal,
            return_type,
            destination,
            function_name,
            resolved,
            context,
        ),
        Expr::ArrayLiteral(literal) => lower_aggregate_array_literal_return_to_location(
            literal,
            return_type,
            destination,
            function_name,
            resolved,
            context,
        ),
        Expr::Call(call) => {
            if let Some(instructions) = lower_payload_enum_constructor_return_to_location(
                expression,
                return_type,
                destination,
                function_name,
                resolved,
                context,
            )? {
                return Ok(instructions);
            }
            lower_aggregate_call_return_to_location(
                call,
                return_type,
                destination,
                function_name,
                context,
            )
        }
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            lower_aggregate_fallible_call_return_to_location(
                call,
                return_type,
                destination,
                function_name,
                context,
                propagating_failure_mode(context)?,
            )
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            lower_aggregate_fallible_call_return_to_location(
                call,
                return_type,
                destination,
                function_name,
                context,
                FallibleFailureMode::Trap,
            )
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            lower_aggregate_fallible_call_return_to_location(
                call,
                return_type,
                destination,
                function_name,
                context,
                lower_catch_failure_mode(catch, context, 0)?,
            )
        }
        Expr::Otherwise(otherwise) => lower_aggregate_otherwise_return_to_location(
            otherwise,
            return_type,
            destination,
            function_name,
            resolved,
            context,
        ),
        Expr::Identifier(identifier) => lower_aggregate_local_return_to_location(
            &identifier.name,
            AggregateValueUse::ImplicitCopy,
            return_type,
            destination,
            function_name,
            context,
        ),
        Expr::Member(_) => {
            if let Some(instructions) = lower_payload_enum_constructor_return_to_location(
                expression,
                return_type,
                destination,
                function_name,
                resolved,
                context,
            )? {
                return Ok(instructions);
            }
            lower_aggregate_member_return_to_location(
                expression,
                return_type,
                destination,
                function_name,
                context,
            )
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unary.operand.as_ref() else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            lower_aggregate_local_return_to_location(
                &identifier.name,
                AggregateValueUse::ExplicitMove,
                return_type,
                destination,
                function_name,
                context,
            )
        }
        Expr::Group(group) => lower_aggregate_return_expression_to_location(
            &group.expression,
            return_type,
            destination,
            function_name,
            resolved,
            context,
        ),
        _ => Err(unsupported_aggregate_return_diagnostic(function_name)),
    }
}
