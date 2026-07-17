use crate::ast::{Expr, UnaryOperator};
use crate::diagnostics::Diagnostic;
use crate::ir::StrValue;
use crate::literals::decode_string_literal_bytes;

pub(super) fn lower_str_literal(expression: &Expr) -> Result<StrValue, Vec<Diagnostic>> {
    match expression {
        Expr::StringLiteral(literal) => decode_string_literal_bytes(&literal.value)
            .map(StrValue::StaticBytes)
            .map_err(|message| vec![Diagnostic::error("E8003", message)]),
        Expr::Group(group) => lower_str_literal(&group.expression),
        _ => Err(vec![Diagnostic::error(
            "E8003",
            "IR v0 can only lower string literals as `&str` values",
        )]),
    }
}

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

pub(super) fn lower_usize_literal(expression: &Expr) -> Result<u64, Vec<Diagnostic>> {
    match expression {
        Expr::IntegerLiteral(literal) => parse_u64_literal(&literal.value),
        Expr::Group(group) => lower_usize_literal(&group.expression),
        _ => Err(vec![Diagnostic::error(
            "E8003",
            "IR v0 can only lower integer literal returns",
        )]),
    }
}

pub(super) fn lower_u8_literal(expression: &Expr) -> Result<u8, Vec<Diagnostic>> {
    match expression {
        Expr::IntegerLiteral(literal) => parse_u8_literal(&literal.value),
        Expr::Group(group) => lower_u8_literal(&group.expression),
        _ => Err(vec![Diagnostic::error(
            "E8003",
            "IR v0 can only lower integer literal returns",
        )]),
    }
}

pub(super) fn lower_u16_literal(expression: &Expr) -> Result<u16, Vec<Diagnostic>> {
    match expression {
        Expr::IntegerLiteral(literal) => parse_u16_literal(&literal.value),
        Expr::Group(group) => lower_u16_literal(&group.expression),
        _ => Err(vec![Diagnostic::error(
            "E8003",
            "IR v0 can only lower integer literal returns",
        )]),
    }
}

pub(super) fn lower_u32_literal(expression: &Expr) -> Result<u32, Vec<Diagnostic>> {
    lower_unsigned_integer_literal(expression)
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

fn parse_u16_literal(text: &str) -> Result<u16, Vec<Diagnostic>> {
    let value = parse_u32_literal(text)?;
    u16::try_from(value).map_err(|_| integer_out_of_range_diagnostic())
}

fn parse_u64_literal(text: &str) -> Result<u64, Vec<Diagnostic>> {
    let (base, digits) = literal_base_and_digits(text);
    let digits = digits.replace('_', "");

    u64::from_str_radix(&digits, base).map_err(|_| integer_out_of_range_diagnostic())
}

fn parse_u8_literal(text: &str) -> Result<u8, Vec<Diagnostic>> {
    let (base, digits) = literal_base_and_digits(text);
    let digits = digits.replace('_', "");

    u8::from_str_radix(&digits, base).map_err(|_| integer_out_of_range_diagnostic())
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
