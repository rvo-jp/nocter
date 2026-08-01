use super::*;

pub(super) fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

pub(super) fn whole_identifier(expression: &Expr) -> Option<&crate::ast::IdentifierExpr> {
    match expression {
        Expr::Identifier(identifier) => Some(identifier),
        Expr::Group(group) => whole_identifier(&group.expression),
        _ => None,
    }
}

pub(super) fn expression_root_identifier(expression: &Expr) -> Option<&crate::ast::IdentifierExpr> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Some(identifier),
        Expr::Member(member) => expression_root_identifier(&member.object),
        Expr::Index(index) => expression_root_identifier(&index.object),
        _ => None,
    }
}

pub(super) fn return_expression_is_fallible_failure(
    expression: &Expr,
    actual: &Type,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    expression_is_fallible_failure_for_return_type(
        expression,
        actual,
        &context.declared_type,
        resolved,
        environment,
    )
}

pub(super) fn expression_is_fallible_failure_for_return_type(
    expression: &Expr,
    actual: &Type,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let Type::Fallible { error, .. } = return_type else {
        return false;
    };

    !error.is_unknown_or_unresolved()
        && (is_expression_assignable(error, expression, resolved, environment)
            || crate::typecheck::operations::is_assignable(error, actual))
}

pub(super) fn propagated_fallible_error_can_escape(
    expression: &Expr,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let Type::Fallible {
        error: current_error,
        ..
    } = return_type
    else {
        return false;
    };
    let Type::Fallible {
        error: attempted_error,
        ..
    } = expression_type(expression, resolved, environment)
    else {
        return false;
    };

    same_known_type(current_error, &attempted_error)
}
