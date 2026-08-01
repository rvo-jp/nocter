use super::*;

fn statement_uses_identifier(
    statement: &Stmt,
    name: &str,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Return(statement) => statement.expression.as_ref().is_some_and(|expression| {
            expression_uses_identifier(expression, name, resolved, environment)
        }),
        Stmt::Binding(statement) => {
            expression_uses_identifier(&statement.initializer, name, resolved, environment)
        }
        Stmt::Assignment(statement) => {
            expression_uses_identifier(&statement.target, name, resolved, environment)
                || expression_uses_identifier(&statement.value, name, resolved, environment)
        }
        Stmt::If(statement) => {
            expression_uses_identifier(&statement.condition, name, resolved, environment)
                || block_uses_identifier(&statement.then_block, name, resolved, environment)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block_uses_identifier(block, name, resolved, environment))
        }
        Stmt::IfIs(statement) => {
            let then_environment = environment_for_if_is_binding(statement, resolved, environment);
            expression_uses_identifier(&statement.expression, name, resolved, environment)
                || block_uses_identifier(&statement.then_block, name, resolved, &then_environment)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block_uses_identifier(block, name, resolved, environment))
        }
        Stmt::Switch(statement) => {
            expression_uses_identifier(&statement.expression, name, resolved, environment)
                || statement.arms.iter().any(|arm| {
                    let arm_environment = environment_for_switch_arm(
                        arm,
                        &statement.expression,
                        resolved,
                        environment,
                    );
                    block_uses_identifier(&arm.body, name, resolved, &arm_environment)
                })
                || statement.wildcard_arm.as_ref().is_some_and(|arm| {
                    block_uses_identifier(&arm.body, name, resolved, environment)
                })
        }
        Stmt::ForRange(statement) => {
            let body_environment =
                environment_for_for_range_binding(statement, resolved, environment);
            expression_uses_identifier(&statement.start, name, resolved, environment)
                || expression_uses_identifier(&statement.end, name, resolved, environment)
                || block_uses_identifier(&statement.body, name, resolved, &body_environment)
        }
        Stmt::While(statement) => {
            expression_uses_identifier(&statement.condition, name, resolved, environment)
                || block_uses_identifier(&statement.body, name, resolved, environment)
        }
        Stmt::Loop(statement) => {
            block_uses_identifier(&statement.body, name, resolved, environment)
        }
        Stmt::Expression(statement) => {
            expression_uses_identifier(&statement.expression, name, resolved, environment)
        }
        Stmt::Drop(statement) => statement.name == name,
        Stmt::Break(_) | Stmt::Continue(_) => false,
    }
}

fn block_uses_identifier(
    block: &Block,
    name: &str,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    statements_or_result_use_identifier_before_terminal(
        &block.statements,
        block.result.as_deref(),
        name,
        resolved,
        environment,
    )
}

pub(super) fn statements_or_result_use_identifier_before_terminal(
    statements: &[Stmt],
    result: Option<&Expr>,
    name: &str,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let mut lookahead_environment = environment.clone();
    for statement in statements {
        if statement_uses_identifier(statement, name, resolved, &lookahead_environment) {
            return true;
        }
        if statement_stops_later_liveness(statement, resolved, &lookahead_environment) {
            return false;
        }
        extend_terminal_lookahead_environment(statement, resolved, &mut lookahead_environment);
    }
    result.is_some_and(|result| {
        expression_uses_identifier(result, name, resolved, &lookahead_environment)
    })
}

fn statement_stops_later_liveness(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    if statement_guarantees_control_exit_or_never(statement, resolved, environment) {
        return true;
    }
    statement_evaluates_never_before_fallthrough(statement, resolved, environment)
}

pub(super) fn expression_uses_identifier(
    expression: &Expr,
    name: &str,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    match expression {
        Expr::Identifier(identifier) => identifier.name == name,
        Expr::Propagate(expression) => {
            expression_uses_identifier(&expression.expression, name, resolved, environment)
        }
        Expr::Force(expression) => {
            expression_uses_identifier(&expression.expression, name, resolved, environment)
        }
        Expr::Catch(expression) => {
            let catch_environment = environment_for_catch(
                expression.error_name.clone(),
                &expression.expression,
                resolved,
                environment,
            );
            expression_uses_identifier(&expression.expression, name, resolved, environment)
                || block_uses_identifier(
                    &expression.catch_block,
                    name,
                    resolved,
                    &catch_environment,
                )
        }
        Expr::Borrow(expression) => {
            expression_uses_identifier(&expression.expression, name, resolved, environment)
        }
        Expr::Binary(expression) => {
            expression_uses_identifier(&expression.left, name, resolved, environment)
                || expression_uses_identifier(&expression.right, name, resolved, environment)
        }
        Expr::Unary(expression) => {
            expression_uses_identifier(&expression.operand, name, resolved, environment)
        }
        Expr::TypeConversion(expression) => {
            expression_uses_identifier(&expression.expression, name, resolved, environment)
        }
        Expr::Call(expression) => {
            expression_uses_identifier(&expression.callee, name, resolved, environment)
                || expression.arguments.iter().any(|argument| {
                    expression_uses_identifier(argument, name, resolved, environment)
                })
        }
        Expr::Member(expression) => {
            expression_uses_identifier(&expression.object, name, resolved, environment)
        }
        Expr::Index(expression) => {
            expression_uses_identifier(&expression.object, name, resolved, environment)
                || expression_uses_identifier(&expression.index, name, resolved, environment)
        }
        Expr::ArrayLiteral(expression) => expression
            .elements
            .iter()
            .any(|element| expression_uses_identifier(element, name, resolved, environment)),
        Expr::StructLiteral(expression) => expression
            .fields
            .iter()
            .any(|field| expression_uses_identifier(&field.value, name, resolved, environment)),
        Expr::Group(expression) => {
            expression_uses_identifier(&expression.expression, name, resolved, environment)
        }
        Expr::InterpolatedString(expression) => expression.parts.iter().any(|part| match part {
            crate::ast::InterpolatedStringPart::Expression(part) => {
                expression_uses_identifier(&part.expression, name, resolved, environment)
            }
            crate::ast::InterpolatedStringPart::Text(_) => false,
        }),
        Expr::Otherwise(expression) => {
            expression_uses_identifier(&expression.value, name, resolved, environment)
                || block_uses_identifier(&expression.fallback, name, resolved, environment)
        }
        Expr::If(expression) => {
            expression_uses_identifier(&expression.condition, name, resolved, environment)
                || block_uses_identifier(&expression.then_block, name, resolved, environment)
                || expression
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block_uses_identifier(block, name, resolved, environment))
        }
        Expr::IfIs(expression) => {
            let then_environment = environment_for_if_is_binding(expression, resolved, environment);
            expression_uses_identifier(&expression.expression, name, resolved, environment)
                || block_uses_identifier(&expression.then_block, name, resolved, &then_environment)
                || expression
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block_uses_identifier(block, name, resolved, environment))
        }
        Expr::Match(expression) => {
            expression_uses_identifier(&expression.expression, name, resolved, environment)
                || expression.arms.iter().any(|arm| {
                    let arm_environment = environment_for_switch_arm(
                        arm,
                        &expression.expression,
                        resolved,
                        environment,
                    );
                    block_uses_identifier(&arm.body, name, resolved, &arm_environment)
                })
                || expression.wildcard_arm.as_ref().is_some_and(|arm| {
                    block_uses_identifier(&arm.body, name, resolved, environment)
                })
        }
        Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => false,
    }
}
