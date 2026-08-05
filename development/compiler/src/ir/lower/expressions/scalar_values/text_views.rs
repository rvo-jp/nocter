use super::*;

pub(in crate::ir::lower::expressions) fn materialize_computed_str_value(
    value: StrValue,
    instructions: &mut Vec<Instruction>,
    temporaries: &mut TemporaryAllocator,
) -> Result<StrValue, Vec<Diagnostic>> {
    match value {
        StrValue::ProcessArg { .. }
        | StrValue::ProcessEnvironmentName { .. }
        | StrValue::ProcessEnvironmentValue { .. }
        | StrValue::SliceIndex { .. } => {
            let temporary = temporaries.next_str()?;
            instructions.push(Instruction::SetStr {
                destination: temporary,
                value,
            });
            Ok(StrValue::Location(temporary))
        }
        StrValue::StaticBytes(_) | StrValue::Location(_) => Ok(value),
    }
}

pub(in crate::ir::lower::expressions) fn lower_str_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<StrValue, Vec<Diagnostic>> {
    match expression {
        Expr::Identifier(identifier) => context
            .str_location(&identifier.name)
            .map(StrValue::Location)
            .ok_or_else(unsupported_str_expression_diagnostic),
        Expr::Member(member) => {
            let Expr::Identifier(identifier) = member.object.as_ref() else {
                return Err(unsupported_str_expression_diagnostic());
            };

            let location = match member.member.as_str() {
                "code" => context.error_code_location(&identifier.name),
                "message" => context.error_message_location(&identifier.name),
                _ => None,
            };

            location
                .map(StrValue::Location)
                .ok_or_else(unsupported_str_expression_diagnostic)
        }
        Expr::Group(group) => lower_str_value(&group.expression, context),
        _ => lower_str_literal(expression),
    }
}

pub(in crate::ir::lower::expressions) fn lower_slice_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<SliceValue, Vec<Diagnostic>> {
    match expression {
        Expr::Identifier(identifier) => context
            .slice_location(&identifier.name)
            .map(SliceValue::Location)
            .ok_or_else(unsupported_slice_expression_diagnostic),
        Expr::Group(group) => lower_slice_value(&group.expression, context),
        _ => Err(unsupported_slice_expression_diagnostic()),
    }
}
