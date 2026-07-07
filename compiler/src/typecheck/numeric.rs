use super::model::Type;
use crate::ast::{Expr, LiteralExpr, UnaryExpr, UnaryOperator};

pub(super) fn is_integer_type(ty: &Type) -> bool {
    matches!(ty, Type::I32)
        || matches!(ty, Type::Primitive(name) if integer_type_max(name).is_some())
}

pub(super) fn is_signed_integer_type(ty: &Type) -> bool {
    matches!(ty, Type::I32)
        || matches!(ty, Type::Primitive(name) if signed_integer_type_min_abs(name).is_some())
}

pub(super) fn is_integer_literal_expr(expression: &Expr) -> bool {
    match expression {
        Expr::IntegerLiteral(_) => true,
        Expr::Unary(unary) if unary.operator == UnaryOperator::Negate => {
            is_integer_literal_expr(&unary.operand)
        }
        Expr::Group(group) => is_integer_literal_expr(&group.expression),
        _ => false,
    }
}

pub(super) fn is_negative_integer_literal_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Unary(unary) if unary.operator == UnaryOperator::Negate => {
            integer_literal_expr_value(&unary.operand).is_some()
        }
        Expr::Group(group) => is_negative_integer_literal_expr(&group.expression),
        _ => false,
    }
}

pub(super) fn integer_literal_fits_type(literal: &LiteralExpr, ty: &Type) -> bool {
    let Some(value) = integer_literal_value(&literal.value) else {
        return false;
    };
    let Some(max) = integer_type_max(&ty.display()) else {
        return false;
    };
    value <= max
}

pub(super) fn negative_integer_literal_fits_type(expression: &UnaryExpr, ty: &Type) -> bool {
    if !is_signed_integer_type(ty) {
        return false;
    }

    let Some(value) = integer_literal_expr_value(&expression.operand) else {
        return false;
    };
    let Some(min_abs) = signed_integer_type_min_abs(&ty.display()) else {
        return false;
    };
    value <= min_abs
}

pub(super) fn integer_literal_expr_value(expression: &Expr) -> Option<u128> {
    match expression {
        Expr::IntegerLiteral(literal) => integer_literal_value(&literal.value),
        Expr::Group(group) => integer_literal_expr_value(&group.expression),
        _ => None,
    }
}

pub(super) fn integer_type_max(name: &str) -> Option<u128> {
    match name {
        "i8" => Some(i8::MAX as u128),
        "i16" => Some(i16::MAX as u128),
        "i32" => Some(i32::MAX as u128),
        "i64" | "isize" => Some(i64::MAX as u128),
        "u8" => Some(u8::MAX as u128),
        "u16" => Some(u16::MAX as u128),
        "u32" => Some(u32::MAX as u128),
        "u64" | "usize" => Some(u64::MAX as u128),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct IntegerRange {
    pub(super) min: i128,
    pub(super) max: i128,
}

pub(super) fn integer_type_range(ty: &Type) -> Option<IntegerRange> {
    integer_type_range_by_name(&ty.display())
}

pub(super) fn integer_type_range_by_name(name: &str) -> Option<IntegerRange> {
    match name {
        "i8" => Some(IntegerRange {
            min: i8::MIN as i128,
            max: i8::MAX as i128,
        }),
        "i16" => Some(IntegerRange {
            min: i16::MIN as i128,
            max: i16::MAX as i128,
        }),
        "i32" => Some(IntegerRange {
            min: i32::MIN as i128,
            max: i32::MAX as i128,
        }),
        "i64" | "isize" => Some(IntegerRange {
            min: i64::MIN as i128,
            max: i64::MAX as i128,
        }),
        "u8" => Some(IntegerRange {
            min: 0,
            max: u8::MAX as i128,
        }),
        "u16" => Some(IntegerRange {
            min: 0,
            max: u16::MAX as i128,
        }),
        "u32" => Some(IntegerRange {
            min: 0,
            max: u32::MAX as i128,
        }),
        "u64" | "usize" => Some(IntegerRange {
            min: 0,
            max: u64::MAX as i128,
        }),
        _ => None,
    }
}

pub(super) fn signed_integer_type_min_abs(name: &str) -> Option<u128> {
    match name {
        "i8" => Some(i8::MAX as u128 + 1),
        "i16" => Some(i16::MAX as u128 + 1),
        "i32" => Some(i32::MAX as u128 + 1),
        "i64" | "isize" => Some(i64::MAX as u128 + 1),
        _ => None,
    }
}

pub(super) fn integer_literal_value(text: &str) -> Option<u128> {
    let (base, digits) = if let Some(rest) = text.strip_prefix("0x") {
        (16, rest)
    } else if let Some(rest) = text.strip_prefix("0b") {
        (2, rest)
    } else {
        (10, text)
    };
    let digits = digits.replace('_', "");
    u128::from_str_radix(&digits, base).ok()
}
