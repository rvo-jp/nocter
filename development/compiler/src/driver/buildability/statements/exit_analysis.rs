use super::*;

pub(in crate::driver::buildability) fn statement_sequence_or_result_exits_function_for_buildability(
    statements: &[Stmt],
    result: Option<&Expr>,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    for statement in statements {
        if statement_may_exit_current_loop_for_buildability(statement) {
            return false;
        }
        if statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ) {
            return true;
        }
    }
    result.is_some_and(|expression| {
        expression_exits_function_for_buildability(
            expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )
    })
}

pub(in crate::driver::buildability) fn statement_exits_function_for_buildability(
    statement: &Stmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Return(_) => true,
        Stmt::Expression(statement) => expression_exits_function_for_buildability(
            &statement.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::If(statement) => if_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::IfIs(statement) => if_is_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::Switch(statement) => switch_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => false,
    }
}

pub(in crate::driver::buildability) fn if_statement_exits_function_for_buildability(
    statement: &crate::ast::IfStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(else_block) = &statement.else_block else {
        return false;
    };
    block_exits_function_for_buildability(
        &statement.then_block,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) && block_exits_function_for_buildability(
        else_block,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
}

pub(in crate::driver::buildability) fn block_exits_function_for_buildability(
    block: &Block,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    statement_sequence_or_result_exits_function_for_buildability(
        &block.statements,
        block.result.as_deref(),
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
}

pub(in crate::driver::buildability) fn expression_exits_function_for_buildability(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => matches!(
            call_return_shape(call, resolved, typecheck_facts, generic_substitutions),
            Some(ReturnShape::Never)
        ),
        Expr::If(statement) => if_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::IfIs(statement) => if_is_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Match(statement) => switch_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => false,
    }
}

pub(in crate::driver::buildability) fn statement_may_exit_current_loop_for_buildability(
    statement: &Stmt,
) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::If(statement) => {
            block_may_exit_current_loop_for_buildability(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop_for_buildability)
        }
        Stmt::IfIs(statement) => {
            block_may_exit_current_loop_for_buildability(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop_for_buildability)
        }
        Stmt::Switch(statement) => {
            statement
                .arms
                .iter()
                .any(|arm| block_may_exit_current_loop_for_buildability(&arm.body))
                || statement
                    .wildcard_arm
                    .as_ref()
                    .is_some_and(|arm| block_may_exit_current_loop_for_buildability(&arm.body))
        }
        Stmt::While(_) | Stmt::Loop(_) => false,
        _ => false,
    }
}

pub(in crate::driver::buildability) fn block_may_exit_current_loop_for_buildability(
    block: &Block,
) -> bool {
    block
        .statements
        .iter()
        .any(statement_may_exit_current_loop_for_buildability)
        || block
            .result
            .as_deref()
            .is_some_and(expression_may_exit_current_loop_for_buildability)
}

pub(in crate::driver::buildability) fn expression_may_exit_current_loop_for_buildability(
    expression: &Expr,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::If(statement) => {
            block_may_exit_current_loop_for_buildability(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop_for_buildability)
        }
        Expr::IfIs(statement) => {
            block_may_exit_current_loop_for_buildability(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop_for_buildability)
        }
        Expr::Match(statement) => {
            statement
                .arms
                .iter()
                .any(|arm| block_may_exit_current_loop_for_buildability(&arm.body))
                || statement
                    .wildcard_arm
                    .as_ref()
                    .is_some_and(|arm| block_may_exit_current_loop_for_buildability(&arm.body))
        }
        _ => false,
    }
}
