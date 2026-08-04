use super::*;

/// Lowers a safe borrow as its one-word address while retaining `Type::Borrow`
/// in the surrounding IR contract.
pub(in crate::ir::lower) fn lower_borrow_expression_to_location(
    expression: &Expr,
    destination: UsizeLocation,
    borrow_type: &Type,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Type::Borrow {
        is_readwrite,
        inner,
    } = borrow_type
    else {
        return Err(unsupported_non_tail_call_diagnostic());
    };
    let expression = match unwrap_group(expression) {
        Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
            unwrap_group(&unary.operand)
        }
        expression => expression,
    };
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
        _ => return Err(unsupported_non_tail_call_diagnostic()),
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
