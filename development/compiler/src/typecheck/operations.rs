use super::arrays::array_length_matches;
use super::calls::{infer_generic_substitutions, resolved_call_signature};
use super::copyability::non_copy_owned_type_kind;
use super::diagnostics::{
    arithmetic_operand_type_mismatch_diagnostic, equality_operand_type_mismatch_diagnostic,
    logical_not_operand_type_mismatch_diagnostic, logical_operand_type_mismatch_diagnostic,
    move_operand_must_be_binding_diagnostic, move_operand_not_move_only_diagnostic,
    negative_shift_count_diagnostic, numeric_negate_operand_type_mismatch_diagnostic,
    ordered_comparison_operand_type_mismatch_diagnostic,
    otherwise_fallback_type_mismatch_diagnostic, otherwise_non_optional_diagnostic,
    shift_operand_type_mismatch_diagnostic, type_conversion_not_lossless_diagnostic,
};
use super::environments::{environment_for_if_is_binding, environment_for_switch_arm};
use super::expressions::{block_result_environment, block_result_type, expression_type};
use super::model::{Type, TypeEnvironment, same_known_type};
use super::numeric::{
    integer_literal_expr_value, integer_literal_fits_type, integer_type_range,
    is_integer_literal_expr, is_integer_type, is_negative_integer_literal_expr,
    is_signed_integer_type, negative_integer_literal_fits_type,
};
use super::type_expr::{
    infer_type_expr_substitutions, type_expr_to_type_in_environment,
    type_expr_to_type_with_substitutions,
};
use super::variants::{
    enum_variant_expression_is_assignable, switch_statement_covers_all_variants,
    types_are_same_payloadless_enum,
};
use crate::ast::{
    AssignmentOperator, BinaryExpr, BinaryOperator, Expr, OtherwiseExpr, TypeConversionExpr,
    UnaryExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;
use std::collections::HashSet;

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
            if !shift_operands_match(
                &left_type,
                &right_type,
                &expression.right,
                resolved,
                environment,
            ) {
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
            } else if non_copy_owned_type_kind(&operand_type, resolved).is_none()
                && !matches!(operand_type, Type::Parameter(_))
            {
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

pub(super) fn check_otherwise_expression(
    sources: &SourceMap,
    expression: &OtherwiseExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let value_type = expression_type(&expression.value, resolved, environment);
    if value_type.is_unknown_or_unresolved() {
        return;
    }

    let Type::Optional(payload_type) = value_type else {
        diagnostics.push(otherwise_non_optional_diagnostic(
            sources,
            expression,
            &value_type,
        ));
        return;
    };

    let fallback_type = block_result_type(&expression.fallback, resolved, environment);
    if fallback_type.is_unknown_or_unresolved() {
        return;
    }

    if !block_result_is_assignable(&payload_type, &expression.fallback, resolved, environment) {
        diagnostics.push(otherwise_fallback_type_mismatch_diagnostic(
            sources,
            expression,
            &payload_type,
            &fallback_type,
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
        (_, Expr::TypedSequenceLiteral(_) | Expr::TypedStringLiteral(_)) => {
            let actual = super::literals::literal_expression_type_with_expected(
                expression,
                Some(expected),
                resolved,
                environment,
            );
            is_assignable(expected, &actual)
        }
        (Type::Optional(_), Expr::NoneLiteral(_)) => true,
        (Type::Optional(inner), _) => {
            let actual = expression_type(expression, resolved, environment);
            is_assignable(expected, &actual)
                || is_expression_assignable(inner, expression, resolved, environment)
        }
        (Type::Fallible { success, .. }, _) => {
            let actual = expression_type(expression, resolved, environment);
            is_assignable(expected, &actual)
                || is_expression_assignable(success, expression, resolved, environment)
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
        (_, Expr::If(statement)) => {
            if_expression_is_assignable(expected, statement, resolved, environment)
        }
        (_, Expr::IfIs(statement)) => {
            if_is_expression_is_assignable(expected, statement, resolved, environment)
        }
        (_, Expr::Match(statement)) => {
            match_expression_is_assignable(expected, statement, resolved, environment)
        }
        (_, Expr::Call(call))
            if generic_call_return_is_assignable(expected, call, resolved, environment) =>
        {
            true
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

fn if_expression_is_assignable(
    expected: &Type,
    statement: &crate::ast::IfStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let Some(else_block) = &statement.else_block else {
        return expected == &Type::Void
            && block_result_is_assignable(expected, &statement.then_block, resolved, environment);
    };

    block_result_is_assignable(expected, &statement.then_block, resolved, environment)
        && block_result_is_assignable(expected, else_block, resolved, environment)
}

fn if_is_expression_is_assignable(
    expected: &Type,
    statement: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let then_environment = environment_for_if_is_binding(statement, resolved, environment);
    let Some(else_block) = &statement.else_block else {
        return expected == &Type::Void
            && block_result_is_assignable(
                expected,
                &statement.then_block,
                resolved,
                &then_environment,
            );
    };

    block_result_is_assignable(expected, &statement.then_block, resolved, &then_environment)
        && block_result_is_assignable(expected, else_block, resolved, environment)
}

fn match_expression_is_assignable(
    expected: &Type,
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let arms_fit = statement.arms.iter().all(|arm| {
        let arm_environment =
            environment_for_switch_arm(arm, &statement.expression, resolved, environment);
        block_result_is_assignable(expected, &arm.body, resolved, &arm_environment)
    });
    if !arms_fit {
        return false;
    }

    if let Some(wildcard_arm) = &statement.wildcard_arm {
        return block_result_is_assignable(expected, &wildcard_arm.body, resolved, environment);
    }

    switch_statement_covers_all_variants(statement, resolved, environment)
        || expected == &Type::Void
}

pub(super) fn block_result_is_assignable(
    expected: &Type,
    block: &crate::ast::Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let result_environment = block_result_environment(block, resolved, environment);
    if let Some(result) = &block.result {
        return is_expression_assignable(expected, result, resolved, &result_environment);
    }

    let actual = block_result_type(block, resolved, &result_environment);
    actual == Type::Never || is_assignable(expected, &actual)
}

fn generic_call_return_is_assignable(
    expected: &Type,
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let Some(signature) = resolved_call_signature(resolved, call, environment) else {
        return false;
    };
    if signature.signature.generic_parameters.is_empty() {
        return false;
    }

    let mut substitutions = infer_generic_substitutions(call, &signature, resolved, environment);
    let parameters = signature
        .signature
        .generic_parameters
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    infer_type_expr_substitutions(
        &signature.signature.return_type,
        expected,
        resolved,
        signature.self_type.as_ref(),
        &parameters,
        &mut substitutions,
    );
    if !signature
        .signature
        .generic_parameters
        .iter()
        .all(|parameter| substitutions.contains_key(parameter))
    {
        return false;
    }

    let actual = type_expr_to_type_with_substitutions(
        &signature.signature.return_type,
        resolved,
        signature.self_type.as_ref(),
        &substitutions,
    );
    is_assignable(expected, &actual)
}

pub(super) fn is_bool_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(name) if name == "bool")
}

pub(super) fn compound_assignment_operands_match(
    operator: AssignmentOperator,
    target_type: &Type,
    target: &Expr,
    value_type: &Type,
    value: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    match operator {
        AssignmentOperator::Assign => true,
        AssignmentOperator::AddAssign
        | AssignmentOperator::SubtractAssign
        | AssignmentOperator::MultiplyAssign
        | AssignmentOperator::DivideAssign
        | AssignmentOperator::RemainderAssign => arithmetic_operands_match(
            target_type,
            target,
            value_type,
            value,
            resolved,
            environment,
        ),
    }
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

fn shift_operands_match(
    left_type: &Type,
    right_type: &Type,
    right: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    is_integer_type(left_type)
        && ((is_integer_type(right_type) && same_known_type(left_type, right_type))
            || (is_integer_literal_expr(right)
                && is_expression_assignable(left_type, right, resolved, environment)))
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
