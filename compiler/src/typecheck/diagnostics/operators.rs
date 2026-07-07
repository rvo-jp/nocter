use super::{BinaryExpr, Diagnostic, SourceMap, Type, TypeConversionExpr, UnaryExpr};

pub(in crate::typecheck) fn equality_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0347",
        format!(
            "operator `{}` compares `{}` with `{}`, but equality operands must use the same supported equality type",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some(
        "compare `bool`, integer, `str`, or supported payloadless enum values of the same type"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn arithmetic_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0352",
        format!(
            "operator `{}` combines `{}` with `{}`, but integer arithmetic requires matching integer operands",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use integer operands with the same type".to_string());
    diagnostic
}

pub(in crate::typecheck) fn shift_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0353",
        format!(
            "operator `{}` shifts `{}` by `{}`, but shift operands must be integer values",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("shift an integer value by an integer count".to_string());
    diagnostic
}

pub(in crate::typecheck) fn negative_shift_count_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0354",
        format!(
            "operator `{}` uses a negative shift count",
            expression.operator.spelling()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.right.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a non-negative shift count".to_string());
    diagnostic
}

pub(in crate::typecheck) fn type_conversion_not_lossless_diagnostic(
    sources: &SourceMap,
    expression: &TypeConversionExpr,
    source: &Type,
    target: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0355",
        format!(
            "`as` conversion from `{}` to `{}` is not a lossless integer conversion",
            source.display(),
            target.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(expression.as_span).ok().map(Box::new);
    diagnostic.help = Some(
        "use `as` only when every source value can be represented by the target type".to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn ordered_comparison_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0348",
        format!(
            "operator `{}` compares `{}` with `{}`, but ordered comparison requires matching integer operands",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("compare integer values with the same type".to_string());
    diagnostic
}

pub(in crate::typecheck) fn logical_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0349",
        format!(
            "operator `{}` combines `{}` with `{}`, but logical operators require `bool` operands",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use `bool` expressions on both sides".to_string());
    diagnostic
}

pub(in crate::typecheck) fn logical_not_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &UnaryExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0350",
        format!(
            "operator `{}` uses `{}`, but logical not requires a `bool` operand",
            expression.operator.spelling(),
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a `bool` expression after `!`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn numeric_negate_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &UnaryExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0351",
        format!(
            "operator `{}` uses `{}`, but numeric negation requires a signed integer operand",
            expression.operator.spelling(),
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a signed integer value after `-`".to_string());
    diagnostic
}
