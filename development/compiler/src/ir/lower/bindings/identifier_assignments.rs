use super::*;

pub(super) fn lower_identifier_assignment(
    identifier: &crate::ast::IdentifierExpr,
    value: &Expr,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(destination) = context.i32_location(&identifier.name) {
        let I32Location::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        if let Some(instructions) =
            lower_i32_optional_otherwise_to_location(value, destination, context)?
        {
            return Ok(instructions);
        }
        return lower_i32_expression_to_location(value, destination, context);
    }

    if let Some(destination) = context.u8_location(&identifier.name) {
        let U8Location::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        if let Some(instructions) =
            lower_u8_optional_otherwise_to_location(value, destination, context)?
        {
            return Ok(instructions);
        }
        return lower_u8_expression_to_location(value, destination, context);
    }

    if let Some(destination) = context.usize_location(&identifier.name) {
        let UsizeLocation::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        if let Some(instructions) =
            lower_usize_optional_otherwise_to_location(value, destination, context)?
        {
            return Ok(instructions);
        }
        return lower_usize_expression_to_location(value, destination, context);
    }

    if let Some(destination) = context.bool_location(&identifier.name) {
        let BoolLocation::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        if let Some(instructions) =
            lower_bool_optional_otherwise_to_location(value, destination, context)?
        {
            return Ok(instructions);
        }
        return lower_bool_expression_to_location(value, destination, context, "E8008");
    }

    if let Some(destination) = context.str_location(&identifier.name) {
        let StrLocation::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        if let Some(instructions) =
            lower_str_optional_otherwise_to_location(value, destination, context)?
        {
            return Ok(instructions);
        }
        return lower_str_expression_to_location(value, destination, context);
    }

    if let Some(destination) = context.slice_location(&identifier.name) {
        let SliceLocation::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        if let Some(instructions) =
            lower_slice_optional_otherwise_to_location(value, destination, context)?
        {
            return Ok(instructions);
        }
        return lower_slice_expression_to_location(value, destination, context);
    }

    if let Some((slot_index, layout)) = context.aggregate_slot(&identifier.name) {
        let target_type = context.local_binding_type_expr_for_identifier(identifier);
        return lower_aggregate_assignment(
            slot_index,
            layout,
            target_type.as_ref(),
            value,
            context,
        );
    }

    Err(unsupported_assignment_diagnostic())
}
