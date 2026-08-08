use crate::ast::{Expr, UnaryOperator};
use crate::diagnostics::Diagnostic;
use crate::integer::IntegerType;
use crate::ir::StrValue;
use crate::literals::{decode_byte_literal, decode_string_literal_bytes};

pub(super) fn lower_str_literal(expression: &Expr) -> Result<StrValue, Vec<Diagnostic>> {
    match expression {
        Expr::StringLiteral(literal) => decode_string_literal_bytes(&literal.value)
            .map(StrValue::StaticBytes)
            .map_err(|message| vec![Diagnostic::error("E8003", message)]),
        Expr::Group(group) => lower_str_literal(&group.expression),
        _ => Err(vec![Diagnostic::error(
            "E8003",
            "native lowering can only lower string literals as `&str` values",
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
            "native lowering can only lower integer literal returns",
        )]),
    }
}

pub(super) fn lower_usize_literal(expression: &Expr) -> Result<u64, Vec<Diagnostic>> {
    match expression {
        Expr::IntegerLiteral(literal) => parse_u64_literal(&literal.value),
        Expr::Group(group) => lower_usize_literal(&group.expression),
        _ => Err(vec![Diagnostic::error(
            "E8003",
            "native lowering can only lower integer literal returns",
        )]),
    }
}

pub(super) fn lower_u8_literal(expression: &Expr) -> Result<u8, Vec<Diagnostic>> {
    match expression {
        Expr::IntegerLiteral(literal) => parse_u8_literal(&literal.value),
        Expr::ByteLiteral(literal) => decode_byte_literal(&literal.value)
            .map_err(|message| vec![Diagnostic::error("E8003", message)]),
        Expr::Group(group) => lower_u8_literal(&group.expression),
        _ => Err(vec![Diagnostic::error(
            "E8003",
            "native lowering can only lower u8 literal values",
        )]),
    }
}

pub(super) fn lower_u16_literal(expression: &Expr) -> Result<u16, Vec<Diagnostic>> {
    match expression {
        Expr::IntegerLiteral(literal) => parse_u16_literal(&literal.value),
        Expr::Group(group) => lower_u16_literal(&group.expression),
        Expr::TypeConversion(conversion) => lower_u16_literal(&conversion.expression),
        _ => Err(vec![Diagnostic::error(
            "E8003",
            "native lowering can only lower integer literal returns",
        )]),
    }
}

pub(super) fn lower_i8_literal(expression: &Expr) -> Result<i8, Vec<Diagnostic>> {
    let value = lower_signed_integer_literal(expression, i8::MAX as u64, (i8::MAX as u64) + 1)?;
    i8::try_from(value).map_err(|_| typed_integer_out_of_range_diagnostic())
}

pub(super) fn lower_i16_literal(expression: &Expr) -> Result<i16, Vec<Diagnostic>> {
    let value = lower_signed_integer_literal(expression, i16::MAX as u64, (i16::MAX as u64) + 1)?;
    i16::try_from(value).map_err(|_| typed_integer_out_of_range_diagnostic())
}

pub(super) fn lower_i64_literal(expression: &Expr) -> Result<i64, Vec<Diagnostic>> {
    lower_signed_integer_literal(expression, i64::MAX as u64, (i64::MAX as u64) + 1)
}

pub(super) fn lower_u64_literal(expression: &Expr) -> Result<u64, Vec<Diagnostic>> {
    lower_unsigned_u64_literal(expression)
}

pub(super) fn lower_integer_literal_word(
    expression: &Expr,
    kind: IntegerType,
) -> Result<u64, Vec<Diagnostic>> {
    let bits = match kind {
        IntegerType::I8 => lower_i8_literal(expression)? as i64 as u64,
        IntegerType::I16 => lower_i16_literal(expression)? as i64 as u64,
        IntegerType::I32 => lower_i32_literal(expression)? as i64 as u64,
        IntegerType::I64 | IntegerType::Isize => lower_i64_literal(expression)? as u64,
        IntegerType::U8 => u64::from(lower_u8_literal(expression)?),
        IntegerType::U16 => u64::from(lower_u16_literal(expression)?),
        IntegerType::U32 => u64::from(lower_u32_literal(expression)?),
        IntegerType::U64 | IntegerType::Usize => lower_u64_literal(expression)?,
    };
    Ok(kind.canonical_word(bits))
}

pub(super) fn lower_u32_literal(expression: &Expr) -> Result<u32, Vec<Diagnostic>> {
    lower_unsigned_integer_literal(expression)
}

fn lower_signed_integer_literal(
    expression: &Expr,
    positive_limit: u64,
    negative_limit: u64,
) -> Result<i64, Vec<Diagnostic>> {
    match expression {
        Expr::IntegerLiteral(literal) => {
            let value = parse_u64_literal(&literal.value)?;
            if value > positive_limit {
                return Err(typed_integer_out_of_range_diagnostic());
            }
            Ok(value as i64)
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::Negate => {
            let value = lower_unsigned_u64_literal(&unary.operand)?;
            if value > negative_limit {
                return Err(typed_integer_out_of_range_diagnostic());
            }
            if value == (i64::MAX as u64) + 1 {
                Ok(i64::MIN)
            } else {
                Ok(-(value as i64))
            }
        }
        Expr::Group(group) => {
            lower_signed_integer_literal(&group.expression, positive_limit, negative_limit)
        }
        Expr::TypeConversion(conversion) => {
            lower_signed_integer_literal(&conversion.expression, positive_limit, negative_limit)
        }
        _ => Err(typed_integer_literal_diagnostic()),
    }
}

fn lower_unsigned_u64_literal(expression: &Expr) -> Result<u64, Vec<Diagnostic>> {
    match expression {
        Expr::IntegerLiteral(literal) => parse_u64_literal(&literal.value),
        Expr::Group(group) => lower_unsigned_u64_literal(&group.expression),
        Expr::TypeConversion(conversion) => lower_unsigned_u64_literal(&conversion.expression),
        _ => Err(typed_integer_literal_diagnostic()),
    }
}

fn lower_unsigned_integer_literal(expression: &Expr) -> Result<u32, Vec<Diagnostic>> {
    match expression {
        Expr::IntegerLiteral(literal) => parse_u32_literal(&literal.value),
        Expr::Group(group) => lower_unsigned_integer_literal(&group.expression),
        Expr::TypeConversion(conversion) => lower_unsigned_integer_literal(&conversion.expression),
        _ => Err(vec![Diagnostic::error(
            "E8003",
            "native lowering can only lower integer literal returns",
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
        "native lowering integer literal return is outside the `i32` range",
    )]
}

fn typed_integer_literal_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8003",
        "native lowering expected an integer literal value",
    )]
}

fn typed_integer_out_of_range_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8003",
        "native lowering integer literal is outside the target type range",
    )]
}
