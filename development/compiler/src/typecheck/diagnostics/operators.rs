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
            "operator `{}` cannot compare `{}` with `{}` because no accessible equality operation accepts both operands",
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
        "declare `operator (&self == other: &Self): bool`, satisfy its operand types directly, or use one readonly coercion per operand"
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

pub(in crate::typecheck) fn type_conversion_not_supported_diagnostic(
    sources: &SourceMap,
    expression: &TypeConversionExpr,
    source: &Type,
    target: &Type,
    rejection: crate::typecheck::conversions::ConversionRejection,
) -> Diagnostic {
    let (message, help) = match rejection {
        crate::typecheck::conversions::ConversionRejection::MissingSourceBorrow => (
            format!(
                "`as` conversion from `{}` to `{}` requires an explicit source borrow",
                source.display(),
                target.display()
            ),
            "borrow the source with `&` or `&+` before applying `as`",
        ),
        crate::typecheck::conversions::ConversionRejection::RequiresReadwriteBorrow => (
            format!(
                "`as` conversion from `{}` to `{}` requires a readwrite source borrow",
                source.display(),
                target.display()
            ),
            "borrow a writable place with `&+` before applying `as`",
        ),
        crate::typecheck::conversions::ConversionRejection::InaccessibleCoercion => (
            format!(
                "`as` conversion from `{}` to `{}` selects a coercion that is not accessible here",
                source.display(),
                target.display()
            ),
            "make the coercion entry `pub` in the source type's defining module",
        ),
        crate::typecheck::conversions::ConversionRejection::Unsupported => (
            format!(
                "`as` conversion from `{}` to `{}` is neither lossless integer conversion nor an accessible borrow coercion",
                source.display(),
                target.display()
            ),
            "use a lossless integer target or declare an exact type-owned borrow coercion",
        ),
    };
    let mut diagnostic = Diagnostic::error("E0355", message);
    diagnostic.primary_span = sources.span_to_json(expression.as_span).ok().map(Box::new);
    diagnostic.help = Some(help.to_string());
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
            "operator `{}` cannot order `{}` with `{}` because no accessible strict ordering accepts both operands",
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
        "use matching integers, declare `operator (&self < other: &Self): bool`, or use one explicit readonly coercion per operand"
            .to_string(),
    );
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

pub(in crate::typecheck) fn move_operand_must_be_binding_diagnostic(
    sources: &SourceMap,
    expression: &UnaryExpr,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0370",
        "`move` requires a local binding or parameter binding name",
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operand.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("write `move name` with a binding name as the operand".to_string());
    diagnostic
}

pub(in crate::typecheck) fn move_operand_not_move_only_diagnostic(
    sources: &SourceMap,
    expression: &UnaryExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0394",
        format!(
            "`move` cannot transfer `{}` because the operand is not a move-only owned binding",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operand.span())
        .ok()
        .map(Box::new);
    diagnostic.help =
        Some("remove `move` for copy values, or use a non-copy owned binding".to_string());
    diagnostic
}
