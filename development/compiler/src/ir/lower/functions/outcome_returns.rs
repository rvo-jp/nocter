use super::*;

pub(super) fn lower_stored_outcome_return(
    expression: &Expr,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let (name, explicitly_moved) = match unwrap_group(expression) {
        Expr::Identifier(identifier) => (identifier.name.as_str(), false),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unwrap_group(&unary.operand) else {
                return Ok(None);
            };
            (identifier.name.as_str(), true)
        }
        _ => return Ok(None),
    };
    let Some(local) = context.outcome_local(name) else {
        return Ok(None);
    };
    if !local.is_live {
        return Err(vec![Diagnostic::error(
            "E8008",
            "cannot return a moved stored outcome",
        )]);
    }
    let Some(return_type) = context.function_return_type_expr() else {
        return Ok(None);
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Ok(None);
    };
    let shape = outcome_shape_with_resolver(return_type, resolved, |source| {
        context.resolved_source(source)
    });
    if shape.layers.is_empty() || !shape.is_supported_callable_shape() {
        return Ok(None);
    }
    let payload = abi_value_from_type_expr_with_resolver(&shape.payload, resolved, |source| {
        context.resolved_source(source)
    })
    .map_err(|error| {
        vec![Diagnostic::error(
            "E8008",
            format!("cannot lay out stored return payload: {error:?}"),
        )]
    })?;
    let Some(storage) = shape.storage_layout(payload.layout) else {
        return Ok(None);
    };
    if storage != local.storage {
        return Err(vec![Diagnostic::error(
            "E8008",
            "stored outcome return has a different shape from the callable result",
        )]);
    }
    if explicitly_moved || !local.is_copy {
        context.mark_outcome_local_moved(name);
    }
    let return_instruction = Instruction::ReturnStoredOutcome {
        source: AggregateLocation::Slot(local.slot_index),
        storage: local.storage,
        payload_type: local.payload_type,
    };
    append_scope_end_drops_before_exit(vec![return_instruction], context).map(Some)
}
