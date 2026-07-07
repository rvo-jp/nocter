use crate::ast::{Expr, UnaryOperator};
use crate::diagnostics::Diagnostic;

pub(super) fn lower_i32_literal(expression: &Expr) -> Result<i32, Vec<Diagnostic>> {
    match expression {
        Expr::IntegerLiteral(literal) => parse_i32_literal(&literal.value),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Negate => {
            let value = lower_unsigned_integer_literal(&unary.operand)?;

            if value == (i32::MAX as u32) + 1 {
                Ok(i32::MIN)
            } else {
                i32::try_from(value)
                    .map(|value| -value)
                    .map_err(|_| integer_out_of_range_diagnostic())
            }
        }
        Expr::Group(group) => lower_i32_literal(&group.expression),
        _ => Err(vec![Diagnostic::error(
            "E8003",
            "IR v0 can only lower integer literal returns",
        )]),
    }
}

fn lower_unsigned_integer_literal(expression: &Expr) -> Result<u32, Vec<Diagnostic>> {
    match expression {
        Expr::IntegerLiteral(literal) => parse_u32_literal(&literal.value),
        Expr::Group(group) => lower_unsigned_integer_literal(&group.expression),
        _ => Err(vec![Diagnostic::error(
            "E8003",
            "IR v0 can only lower integer literal returns",
        )]),
    }
}

fn parse_i32_literal(text: &str) -> Result<i32, Vec<Diagnostic>> {
    let value = parse_u32_literal(text)?;
    i32::try_from(value).map_err(|_| integer_out_of_range_diagnostic())
}

fn parse_u32_literal(text: &str) -> Result<u32, Vec<Diagnostic>> {
    let (base, digits) = literal_base_and_digits(text);
    let digits = digits.replace('_', "");

    u32::from_str_radix(&digits, base).map_err(|_| integer_out_of_range_diagnostic())
}

fn literal_base_and_digits(text: &str) -> (u32, &str) {
    if let Some(digits) = text.strip_prefix("0x") {
        (16, digits)
    } else if let Some(digits) = text.strip_prefix("0b") {
        (2, digits)
    } else {
        (10, text)
    }
}

fn integer_out_of_range_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8003",
        "IR v0 integer literal return is outside the `i32` range",
    )]
}
