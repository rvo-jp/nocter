use super::*;

pub(super) fn statement_suffix_exits_function(
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
    context: &LoweringContext,
) -> bool {
    statement_sequence_or_result_exits_function(
        statements.get(index + 1..).unwrap_or(&[]),
        result,
        context,
    )
}

fn statement_sequence_or_result_exits_function(
    statements: &[Stmt],
    result: Option<&Expr>,
    context: &LoweringContext,
) -> bool {
    for statement in statements {
        if statement_may_exit_current_loop(statement) {
            return false;
        }
        if statement_exits_function(statement, context) {
            return true;
        }
    }
    result.is_some_and(|expression| expression_exits_function(expression, context))
}

pub(in crate::ir::lower) fn statement_exits_function(
    statement: &Stmt,
    context: &LoweringContext,
) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Return(_) => true,
        Stmt::Expression(statement) => expression_exits_function(&statement.expression, context),
        Stmt::If(statement) => {
            let Some(else_block) = &statement.else_block else {
                return false;
            };
            block_exits_function(&statement.then_block, context)
                && block_exits_function(else_block, context)
        }
        Stmt::IfIs(statement) => {
            let Some(else_block) = &statement.else_block else {
                return false;
            };
            block_exits_function(&statement.then_block, context)
                && block_exits_function(else_block, context)
        }
        Stmt::Switch(statement) => {
            if statement.wildcard_arm.is_none()
                && !lowerable_switch_is_exhaustive(statement, context)
            {
                return false;
            }

            statement
                .arms
                .iter()
                .all(|arm| block_exits_function(&arm.body, context))
                && statement
                    .wildcard_arm
                    .as_ref()
                    .is_none_or(|wildcard_arm| block_exits_function(&wildcard_arm.body, context))
        }
        _ => false,
    }
}

fn block_exits_function(block: &Block, context: &LoweringContext) -> bool {
    statement_sequence_or_result_exits_function(&block.statements, block.result.as_deref(), context)
}

pub(super) fn expression_exits_function(expression: &Expr, context: &LoweringContext) -> bool {
    match unwrap_group(expression) {
        Expr::Call(call) => {
            if primitive_trap_call(call, context) {
                return true;
            }
            let Some((target, _call_name)) = context.direct_call_target_and_name(call) else {
                return false;
            };
            context.call_return_type(&target) == Some(&Type::Never)
        }
        Expr::If(statement) => {
            let Some(else_block) = &statement.else_block else {
                return false;
            };
            block_exits_function(&statement.then_block, context)
                && block_exits_function(else_block, context)
        }
        Expr::IfIs(statement) => {
            let Some(else_block) = &statement.else_block else {
                return false;
            };
            block_exits_function(&statement.then_block, context)
                && block_exits_function(else_block, context)
        }
        Expr::Match(statement) => {
            if statement.wildcard_arm.is_none()
                && !payloadless_switch_is_exhaustive(statement, context)
            {
                return false;
            }

            statement
                .arms
                .iter()
                .all(|arm| block_exits_function(&arm.body, context))
                && statement
                    .wildcard_arm
                    .as_ref()
                    .is_none_or(|wildcard_arm| block_exits_function(&wildcard_arm.body, context))
        }
        _ => false,
    }
}

pub(super) fn statement_may_exit_current_loop(statement: &Stmt) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::If(statement) => {
            block_may_exit_current_loop(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop)
        }
        Stmt::IfIs(statement) => {
            block_may_exit_current_loop(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop)
        }
        Stmt::Switch(statement) => {
            statement
                .arms
                .iter()
                .any(|arm| block_may_exit_current_loop(&arm.body))
                || statement
                    .wildcard_arm
                    .as_ref()
                    .is_some_and(|arm| block_may_exit_current_loop(&arm.body))
        }
        Stmt::While(_) | Stmt::Loop(_) => false,
        _ => false,
    }
}

fn block_may_exit_current_loop(block: &Block) -> bool {
    block.statements.iter().any(statement_may_exit_current_loop)
        || block
            .result
            .as_deref()
            .is_some_and(expression_may_exit_current_loop)
}

fn expression_may_exit_current_loop(expression: &Expr) -> bool {
    match unwrap_group(expression) {
        Expr::If(statement) => {
            block_may_exit_current_loop(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop)
        }
        Expr::IfIs(statement) => {
            block_may_exit_current_loop(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop)
        }
        Expr::Match(statement) => {
            statement
                .arms
                .iter()
                .any(|arm| block_may_exit_current_loop(&arm.body))
                || statement
                    .wildcard_arm
                    .as_ref()
                    .is_some_and(|arm| block_may_exit_current_loop(&arm.body))
        }
        _ => false,
    }
}
