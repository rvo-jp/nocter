use super::*;
use crate::ast::UnaryOperator;

pub(super) fn lower_stored_outcome_argument(
    expression: &Expr,
    parameter_type: Option<&TypeExpr>,
    context: &LoweringContext,
) -> Result<ScalarArgument, Vec<Diagnostic>> {
    let expression = match expression.without_groups() {
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            unary.operand.without_groups()
        }
        expression => expression,
    };
    let Expr::Identifier(identifier) = expression.without_groups() else {
        return Err(vec![Diagnostic::error(
            "E8006",
            "stored outcome arguments must currently name a stored outcome value",
        )]);
    };
    let local = context.outcome_local(&identifier.name).ok_or_else(|| {
        vec![Diagnostic::error(
            "E8006",
            "stored outcome argument does not resolve to an outcome slot",
        )]
    })?;
    if !local.is_live {
        return Err(vec![Diagnostic::error(
            "E8006",
            "stored outcome argument was already moved",
        )]);
    }
    if let Some(parameter_type) = parameter_type
        && let Some((_root_source, resolved)) = context.resolved_calls()
    {
        let shape =
            crate::outcomes::outcome_shape_with_resolver(parameter_type, resolved, |source| {
                context.resolved_source(source)
            });
        let payload = abi_value_from_type_expr_with_resolver(&shape.payload, resolved, |source| {
            context.resolved_source(source)
        })
        .ok();
        if payload
            .and_then(|payload| shape.storage_layout(payload.layout))
            .is_some_and(|storage| storage != local.storage)
        {
            return Err(vec![Diagnostic::error(
                "E8006",
                "stored outcome argument layout does not match its parameter",
            )]);
        }
    }

    let source = AggregateArgumentSource::Slot(local.slot_index);
    if local.storage.layout.size <= crate::abi::DIRECT_VALUE_MAX_SIZE {
        let words = local
            .storage
            .layout
            .size
            .div_ceil(crate::abi::ABI_WORD_SIZE) as usize;
        Ok(ScalarArgument::AggregateDirect(DirectAggregateArgument {
            source,
            layout: local.storage.layout,
            words,
        }))
    } else {
        Ok(ScalarArgument::AggregateIndirect(AggregateArgument {
            source,
        }))
    }
}
