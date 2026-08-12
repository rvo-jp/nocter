use super::*;

/// Lowers a safe borrow as its one-word address while retaining `Type::Borrow`
/// in the surrounding IR contract.
pub(in crate::ir::lower) fn lower_borrow_expression_to_location(
    expression: &Expr,
    destination: UsizeLocation,
    borrow_type: &Type,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(lowered) = lower_borrow_coercion_to_location(expression, destination, context) {
        return lowered;
    }
    let Type::Borrow {
        is_readwrite,
        inner,
    } = borrow_type
    else {
        return Err(unsupported_borrow_expression_diagnostic());
    };
    let expression = match unwrap_group(expression) {
        Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
            unwrap_group(&unary.operand)
        }
        expression => expression,
    };
    if let Expr::TypeConversion(conversion) = expression {
        let source_type = context
            .conversion_plan(conversion.span)
            .and_then(|plan| context.ir_type_for_type_expr(&plan.source_ty))
            .unwrap_or_else(|| borrow_type.clone());
        return lower_borrow_expression_to_location(
            &conversion.expression,
            destination,
            &source_type,
            context,
        );
    }
    if let Expr::Propagate(propagation) = expression {
        return lower_outcome_borrow_expression(
            &propagation.expression,
            destination,
            context,
            propagating_outcome_mode(&propagation.expression, context)?,
        );
    }
    if let Expr::Force(force) = expression {
        return lower_outcome_borrow_expression(
            &force.expression,
            destination,
            context,
            OutcomeFailureMode::Trap,
        );
    }
    if let Expr::Catch(catch) = expression {
        return lower_outcome_borrow_expression(
            &catch.expression,
            destination,
            context,
            lower_value_catch_failure_mode(
                catch,
                context,
                usize_destination_reserved_abi_words(destination),
                None,
                |result, context| {
                    lower_borrow_expression_to_location(result, destination, borrow_type, context)
                },
                "native lowering can only lower borrow `catch` fallback blocks that produce a matching borrow or exit",
            )?,
        );
    }
    if let Expr::If(statement) = expression {
        return lower_borrow_if_expression_to_location(
            statement,
            destination,
            borrow_type,
            context,
        );
    }
    if let Expr::IfIs(statement) = expression {
        return lower_borrow_if_is_expression_to_location(
            statement,
            destination,
            borrow_type,
            context,
        );
    }
    if let Expr::Match(statement) = expression {
        return lower_borrow_match_expression_to_location(
            statement,
            destination,
            borrow_type,
            context,
        );
    }
    if let Expr::Call(call) = expression {
        let mut temporaries = TemporaryAllocator::new(context)?;
        return lower_borrow_normal_call(call, destination, context, &mut temporaries);
    }
    if let Expr::Identifier(identifier) = expression
        && let Some(field) = context.closure_capture_field(&identifier.name)
        && let AggregateFieldKind::Borrow {
            is_readwrite: field_is_readwrite,
            inner: field_inner,
        } = &field.kind
        && field_inner == inner.as_ref()
        && (!*is_readwrite || *field_is_readwrite)
    {
        return Ok(vec![Instruction::LoadAggregateUsize {
            destination,
            source: field.source,
            offset: field.offset,
        }]);
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let source_expression = match expression {
        Expr::Borrow(borrow) if borrow.is_readwrite == *is_readwrite => &borrow.expression,
        Expr::Identifier(_) => expression,
        _ => return Err(unsupported_borrow_expression_diagnostic()),
    };
    let (mut instructions, source) = lower_borrow_source_from_expression(
        source_expression,
        inner,
        *is_readwrite,
        borrow_type,
        "borrow value",
        context,
        &mut temporaries,
    )?;
    instructions.push(Instruction::SetUsizeFromBorrow {
        destination,
        source,
    });
    Ok(instructions)
}

fn lower_outcome_borrow_expression(
    expression: &Expr,
    destination: UsizeLocation,
    context: &LoweringContext,
    failure_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_stored_outcome_expression(
        expression,
        crate::ir::ComposedOutcomeDestination::Borrow(destination),
        context,
        failure_mode.clone(),
    )? {
        return Ok(instructions);
    }
    let Expr::Call(call) = unwrap_group(expression) else {
        return Err(unsupported_borrow_expression_diagnostic());
    };
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_fallible_borrow_normal_call(call, destination, context, &mut temporaries, failure_mode)
}
