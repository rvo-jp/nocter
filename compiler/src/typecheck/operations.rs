use super::arrays::array_length_matches;
use super::copyability::non_copy_struct_type_name;
use super::diagnostics::{
    arithmetic_operand_type_mismatch_diagnostic, equality_operand_type_mismatch_diagnostic,
    logical_not_operand_type_mismatch_diagnostic, logical_operand_type_mismatch_diagnostic,
    move_operand_must_be_binding_diagnostic, move_operand_not_move_only_diagnostic,
    negative_shift_count_diagnostic, numeric_negate_operand_type_mismatch_diagnostic,
    optional_default_non_optional_diagnostic, optional_default_type_mismatch_diagnostic,
    ordered_comparison_operand_type_mismatch_diagnostic, shift_operand_type_mismatch_diagnostic,
    type_conversion_not_lossless_diagnostic,
};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment, same_known_type};
use super::numeric::{
    integer_literal_expr_value, integer_literal_fits_type, integer_type_range,
    is_integer_literal_expr, is_integer_type, is_negative_integer_literal_expr,
    is_signed_integer_type, negative_integer_literal_fits_type,
};
use super::type_expr::type_expr_to_type_in_environment;
use super::variants::{enum_variant_expression_is_assignable, types_are_same_payloadless_enum};
use crate::ast::{
    BinaryExpr, BinaryOperator, Expr, OptionalDefaultExpr, TypeConversionExpr, UnaryExpr,
    UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

pub(super) fn check_binary_expression(
    sources: &SourceMap,
    expression: &BinaryExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let left_type = expression_type(&expression.left, resolved, environment);
    let right_type = expression_type(&expression.right, resolved, environment);

    if left_type.is_unknown_or_unresolved() || right_type.is_unknown_or_unresolved() {
        return;
    }

    match expression.operator {
        BinaryOperator::Add
        | BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Remainder => {
            if !arithmetic_operands_match(
                &left_type,
                &expression.left,
                &right_type,
                &expression.right,
                resolved,
                environment,
            ) {
                diagnostics.push(arithmetic_operand_type_mismatch_diagnostic(
                    sources,
                    expression,
                    &left_type,
                    &right_type,
                ));
            }
        }
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
            if !shift_operands_match(&left_type, &right_type) {
                diagnostics.push(shift_operand_type_mismatch_diagnostic(
                    sources,
                    expression,
                    &left_type,
                    &right_type,
                ));
            } else if is_negative_integer_literal_expr(&expression.right) {
                diagnostics.push(negative_shift_count_diagnostic(sources, expression));
            }
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            if !equality_operands_match(
                &left_type,
                &expression.left,
                &right_type,
                &expression.right,
                resolved,
                environment,
            ) {
                diagnostics.push(equality_operand_type_mismatch_diagnostic(
                    sources,
                    expression,
                    &left_type,
                    &right_type,
                ));
            }
        }
        BinaryOperator::Less
        | BinaryOperator::LessEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterEqual => {
            if !ordered_comparison_operands_match(
                &left_type,
                &expression.left,
                &right_type,
                &expression.right,
                resolved,
                environment,
            ) {
                diagnostics.push(ordered_comparison_operand_type_mismatch_diagnostic(
                    sources,
                    expression,
                    &left_type,
                    &right_type,
                ));
            }
        }
        BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => {
            if !logical_operands_match(&left_type, &right_type) {
                diagnostics.push(logical_operand_type_mismatch_diagnostic(
                    sources,
                    expression,
                    &left_type,
                    &right_type,
                ));
            }
        }
    }
}

pub(super) fn check_unary_expression(
    sources: &SourceMap,
    expression: &UnaryExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let operand_type = expression_type(&expression.operand, resolved, environment);
    if operand_type.is_unknown_or_unresolved() {
        return;
    }

    match expression.operator {
        UnaryOperator::LogicalNot => {
            if !is_bool_type(&operand_type) {
                diagnostics.push(logical_not_operand_type_mismatch_diagnostic(
                    sources,
                    expression,
                    &operand_type,
                ));
            }
        }
        UnaryOperator::Negate => {
            if !is_signed_integer_type(&operand_type) {
                diagnostics.push(numeric_negate_operand_type_mismatch_diagnostic(
                    sources,
                    expression,
                    &operand_type,
                ));
            }
        }
        UnaryOperator::Move => {
            if !matches!(expression.operand.as_ref(), Expr::Identifier(_)) {
                diagnostics.push(move_operand_must_be_binding_diagnostic(sources, expression));
            } else if non_copy_struct_type_name(&operand_type, resolved).is_none() {
                diagnostics.push(move_operand_not_move_only_diagnostic(
                    sources,
                    expression,
                    &operand_type,
                ));
            }
        }
    }
}

pub(super) fn check_type_conversion_expression(
    sources: &SourceMap,
    expression: &TypeConversionExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let source_type = expression_type(&expression.expression, resolved, environment);
    let target_type = type_expr_to_type_in_environment(&expression.ty, resolved, environment);
    if source_type.is_unknown_or_unresolved() || target_type.is_unknown_or_unresolved() {
        return;
    }
    if target_type.first_unsized_part().is_some() {
        return;
    }

    if !is_lossless_integer_conversion(
        &source_type,
        &expression.expression,
        &target_type,
        resolved,
        environment,
    ) {
        diagnostics.push(type_conversion_not_lossless_diagnostic(
            sources,
            expression,
            &source_type,
            &target_type,
        ));
    }
}

pub(super) fn check_optional_default_expression(
    sources: &SourceMap,
    expression: &OptionalDefaultExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let value_type = expression_type(&expression.value, resolved, environment);
    if value_type.is_unknown_or_unresolved() {
        return;
    }

    let Type::Optional(payload_type) = value_type else {
        diagnostics.push(optional_default_non_optional_diagnostic(
            sources,
            expression,
            &value_type,
        ));
        return;
    };

    let default_type = expression_type(&expression.default, resolved, environment);
    if default_type.is_unknown_or_unresolved() {
        return;
    }

    if !is_expression_assignable(&payload_type, &expression.default, resolved, environment) {
        diagnostics.push(optional_default_type_mismatch_diagnostic(
            sources,
            expression,
            &payload_type,
            &default_type,
        ));
    }
}

pub(super) fn is_assignable(expected: &Type, actual: &Type) -> bool {
    if actual == &Type::Never {
        return true;
    }

    match (expected, actual) {
        (Type::Optional(_), Type::None) => true,
        (Type::Optional(expected_inner), Type::Optional(actual_inner)) => {
            is_assignable(expected_inner, actual_inner)
        }
        (Type::Optional(inner), actual) => is_assignable(inner, actual),
        _ => expected == actual,
    }
}

pub(super) fn is_expression_assignable(
    expected: &Type,
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    if matches!(expected, Type::Unknown | Type::Unresolved(_)) {
        return true;
    }

    match (expected, expression) {
        (Type::Optional(_), Expr::NoneLiteral(_)) => true,
        (Type::Optional(inner), _) => {
            let actual = expression_type(expression, resolved, environment);
            is_assignable(expected, &actual)
                || is_expression_assignable(inner, expression, resolved, environment)
        }
        (_, Expr::IntegerLiteral(literal)) if is_integer_type(expected) => {
            integer_literal_fits_type(literal, expected)
        }
        (_, Expr::Unary(unary))
            if unary.operator == UnaryOperator::Negate
                && integer_literal_expr_value(&unary.operand).is_some() =>
        {
            negative_integer_literal_fits_type(unary, expected)
        }
        (Type::Array { element, length }, Expr::ArrayLiteral(literal)) => {
            array_length_matches(length, literal.elements.len())
                && literal.elements.iter().all(|element_expr| {
                    is_expression_assignable(element, element_expr, resolved, environment)
                })
        }
        (_, Expr::Group(group)) => {
            is_expression_assignable(expected, &group.expression, resolved, environment)
        }
        (_, _) => {
            enum_variant_expression_is_assignable(expected, expression, resolved, environment)
                .unwrap_or_else(|| {
                    let actual = expression_type(expression, resolved, environment);
                    is_assignable(expected, &actual)
                })
        }
    }
}

pub(super) fn is_bool_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(name) if name == "bool")
}

fn is_str_type(ty: &Type) -> bool {
    matches!(ty, Type::Str)
}

fn equality_operands_match(
    left_type: &Type,
    left: &Expr,
    right_type: &Type,
    right: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    if is_bool_type(left_type) || is_bool_type(right_type) {
        return is_bool_type(left_type) && is_bool_type(right_type);
    }

    if is_str_type(left_type) || is_str_type(right_type) {
        return is_str_type(left_type) && is_str_type(right_type);
    }

    if types_are_same_payloadless_enum(left_type, right_type, resolved) {
        return true;
    }

    integer_operands_match(left_type, left, right_type, right, resolved, environment)
}

fn arithmetic_operands_match(
    left_type: &Type,
    left: &Expr,
    right_type: &Type,
    right: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    is_integer_type(left_type)
        && is_integer_type(right_type)
        && integer_operands_match(left_type, left, right_type, right, resolved, environment)
}

fn ordered_comparison_operands_match(
    left_type: &Type,
    left: &Expr,
    right_type: &Type,
    right: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    is_integer_type(left_type)
        && is_integer_type(right_type)
        && integer_operands_match(left_type, left, right_type, right, resolved, environment)
}

fn shift_operands_match(left_type: &Type, right_type: &Type) -> bool {
    is_integer_type(left_type) && is_integer_type(right_type)
}

fn is_lossless_integer_conversion(
    source_type: &Type,
    source: &Expr,
    target_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    if !is_integer_type(target_type) {
        return false;
    }

    if is_integer_literal_expr(source) {
        return is_expression_assignable(target_type, source, resolved, environment);
    }

    let Some(source_range) = integer_type_range(source_type) else {
        return false;
    };
    let Some(target_range) = integer_type_range(target_type) else {
        return false;
    };

    target_range.min <= source_range.min && source_range.max <= target_range.max
}

pub(super) fn integer_operands_match(
    left_type: &Type,
    left: &Expr,
    right_type: &Type,
    right: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    (is_integer_type(left_type)
        && is_integer_type(right_type)
        && same_known_type(left_type, right_type))
        || (is_integer_type(left_type)
            && is_integer_literal_expr(right)
            && is_expression_assignable(left_type, right, resolved, environment))
        || (is_integer_type(right_type)
            && is_integer_literal_expr(left)
            && is_expression_assignable(right_type, left, resolved, environment))
}

fn logical_operands_match(left_type: &Type, right_type: &Type) -> bool {
    is_bool_type(left_type) && is_bool_type(right_type)
}

pub(super) fn binary_expression_type(
    expression: &BinaryExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    match expression.operator {
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::Less
        | BinaryOperator::LessEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterEqual
        | BinaryOperator::LogicalAnd
        | BinaryOperator::LogicalOr => Type::Primitive("bool".to_string()),
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
            shift_expression_type(expression, resolved, environment)
        }
        BinaryOperator::Add
        | BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Remainder => {
            arithmetic_expression_type(expression, resolved, environment)
        }
    }
}

fn shift_expression_type(
    expression: &BinaryExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let left_type = expression_type(&expression.left, resolved, environment);
    if is_integer_type(&left_type) {
        left_type
    } else {
        Type::Unknown
    }
}

fn arithmetic_expression_type(
    expression: &BinaryExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let left_type = expression_type(&expression.left, resolved, environment);
    let right_type = expression_type(&expression.right, resolved, environment);

    if left_type.is_unknown_or_unresolved() || right_type.is_unknown_or_unresolved() {
        return Type::Unknown;
    }

    if is_integer_type(&left_type)
        && is_integer_type(&right_type)
        && same_known_type(&left_type, &right_type)
    {
        return left_type;
    }

    if is_integer_type(&left_type)
        && is_integer_literal_expr(&expression.right)
        && is_expression_assignable(&left_type, &expression.right, resolved, environment)
    {
        return left_type;
    }

    if is_integer_type(&right_type)
        && is_integer_literal_expr(&expression.left)
        && is_expression_assignable(&right_type, &expression.left, resolved, environment)
    {
        return right_type;
    }

    Type::Unknown
}
