use super::*;
use crate::ir::lower::typed_literals::lower_typed_literal_to_location;

pub(super) fn lower_typed_literal_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let span = match unwrap_group(&statement.initializer) {
        Expr::TypedSequenceLiteral(literal) => literal.span,
        Expr::TypedStringLiteral(literal) => literal.span,
        _ => return Ok(None),
    };
    let return_type_expr = context.expression_type_expr(span).ok_or_else(|| {
        unsupported_binding_diagnostic("typed literal result has no concrete type fact")
    })?;
    let value = context
        .abi_value_for_type_expr(&return_type_expr)
        .ok_or_else(|| {
            unsupported_binding_diagnostic("typed literal result has no aggregate ABI layout")
        })?;
    if !matches!(
        value.ty,
        AbiType::Struct(_) | AbiType::Enum(_) | AbiType::Array { .. }
    ) {
        return Err(unsupported_binding_diagnostic(
            "typed literal target must lower to a nominal aggregate",
        ));
    }
    let layout = value.layout;
    validate_aggregate_binding_layout(layout)?;
    let is_copy = type_expr_is_copy_aggregate_value_with_resolver(
        &return_type_expr,
        context
            .resolved_calls()
            .map(|(_, resolved)| resolved)
            .ok_or_else(|| {
                unsupported_binding_diagnostic("typed literal resolution facts are unavailable")
            })?,
        |source| context.resolved_source(source),
    );
    let drop_kind = context.aggregate_drop_for_type_expr(&return_type_expr);
    let fields = context
        .resolved_calls()
        .and_then(|(root_source, resolved)| {
            aggregate_fields_from_type_expr_with_resolver(
                &return_type_expr,
                root_source,
                resolved,
                |source| context.resolved_source(source),
            )
        })
        .unwrap_or_default();
    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, drop_kind, fields);

    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    instructions.extend(
        lower_typed_literal_to_location(
            &statement.initializer,
            AggregateLocation::Slot(slot_index),
            context,
        )?
        .ok_or_else(|| unsupported_binding_diagnostic("expected a typed literal expression"))?,
    );
    Ok(Some(instructions))
}
