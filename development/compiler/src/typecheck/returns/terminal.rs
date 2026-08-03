use super::*;

pub(in crate::typecheck) fn block_guarantees_return(block: &Block) -> bool {
    for statement in &block.statements {
        if statement_guarantees_return(statement) {
            return true;
        }
    }

    block
        .result
        .as_deref()
        .is_some_and(expression_guarantees_return)
}

pub(in crate::typecheck) fn block_guarantees_return_or_never(
    block: &Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let mut environment = environment.clone();
    for statement in &block.statements {
        if statement_guarantees_return_or_never(statement, resolved, &environment)
            || statement_evaluates_never_before_fallthrough(statement, resolved, &environment)
        {
            return true;
        }
        extend_terminal_lookahead_environment(statement, resolved, &mut environment);
    }

    block
        .result
        .as_ref()
        .is_some_and(|result| expression_type(result, resolved, &environment) == Type::Never)
}

pub(in crate::typecheck) fn block_guarantees_control_exit_or_never(
    block: &Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let mut environment = environment.clone();
    for statement in &block.statements {
        if statement_guarantees_control_exit_or_never(statement, resolved, &environment)
            || statement_evaluates_never_before_fallthrough(statement, resolved, &environment)
        {
            return true;
        }
        extend_terminal_lookahead_environment(statement, resolved, &mut environment);
    }

    block
        .result
        .as_ref()
        .is_some_and(|result| expression_type(result, resolved, &environment) == Type::Never)
}

pub(in crate::typecheck) fn statement_evaluates_never_before_fallthrough(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    match statement {
        Stmt::Binding(statement) => {
            expression_type(&statement.initializer, resolved, environment) == Type::Never
        }
        Stmt::Assignment(statement) => {
            expression_type(&statement.value, resolved, environment) == Type::Never
        }
        Stmt::If(statement) => {
            expression_type(&statement.condition, resolved, environment) == Type::Never
        }
        Stmt::IfIs(statement) => {
            expression_type(&statement.expression, resolved, environment) == Type::Never
        }
        Stmt::Switch(statement) => {
            expression_type(&statement.expression, resolved, environment) == Type::Never
        }
        Stmt::ForRange(statement) => {
            expression_type(&statement.start, resolved, environment) == Type::Never
                || expression_type(&statement.end, resolved, environment) == Type::Never
        }
        Stmt::CollectionFor(statement) => {
            expression_type(&statement.source, resolved, environment) == Type::Never
        }
        Stmt::LiteralPackFor(_) => false,
        Stmt::While(statement) => {
            expression_type(&statement.condition, resolved, environment) == Type::Never
        }
        Stmt::Region(statement) => {
            expression_type(&statement.allocator, resolved, environment) == Type::Never
        }
        Stmt::Expression(statement) => {
            expression_type(&statement.expression, resolved, environment) == Type::Never
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Return(_)
        | Stmt::Loop(_)
        | Stmt::Drop(_)
        | Stmt::Break(_)
        | Stmt::Continue(_) => false,
    }
}

pub(in crate::typecheck) fn extend_terminal_lookahead_environment(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
) {
    let Stmt::Binding(statement) = statement else {
        return;
    };
    let initializer_type = expression_type(&statement.initializer, resolved, environment);
    if initializer_type == Type::Never {
        return;
    }
    let binding_type = continuing_binding_type(statement, initializer_type, resolved, environment);
    environment.define_binding(
        statement.name.clone(),
        binding_type,
        binding_kind_is_mutable(statement.kind),
    );
}

pub(in crate::typecheck) fn statement_guarantees_control_exit_or_never(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    match statement {
        Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::Expression(statement) => {
            expression_type(&statement.expression, resolved, environment) == Type::Never
        }
        Stmt::If(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_control_exit_or_never(&statement.then_block, resolved, environment)
                && block_guarantees_control_exit_or_never(else_block, resolved, environment)
        }),
        Stmt::IfIs(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_control_exit_or_never(&statement.then_block, resolved, environment)
                && block_guarantees_control_exit_or_never(else_block, resolved, environment)
        }),
        Stmt::Switch(statement) => {
            if !switch_arms_guarantee_control_exit_or_never(statement, resolved, environment) {
                return false;
            }

            statement.wildcard_arm.as_ref().map_or_else(
                || switch_statement_covers_all_variants(statement, resolved, environment),
                |wildcard_arm| {
                    block_guarantees_control_exit_or_never(
                        &wildcard_arm.body,
                        resolved,
                        environment,
                    )
                },
            )
        }
        Stmt::Loop(statement) => {
            block_guarantees_return_or_never(&statement.body, resolved, environment)
        }
        _ => statement_guarantees_return(statement),
    }
}

pub(super) fn switch_arms_guarantee_control_exit_or_never(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    statement.arms.iter().all(|arm| {
        let arm_environment =
            environment_for_switch_arm(arm, &statement.expression, resolved, environment);
        block_guarantees_control_exit_or_never(&arm.body, resolved, &arm_environment)
    })
}

pub(super) fn statement_guarantees_return_or_never(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    match statement {
        Stmt::Expression(statement) => {
            expression_type(&statement.expression, resolved, environment) == Type::Never
        }
        Stmt::If(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return_or_never(&statement.then_block, resolved, environment)
                && block_guarantees_return_or_never(else_block, resolved, environment)
        }),
        Stmt::IfIs(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return_or_never(&statement.then_block, resolved, environment)
                && block_guarantees_return_or_never(else_block, resolved, environment)
        }),
        Stmt::Switch(statement) => {
            if !switch_arms_guarantee_return_or_never(statement, resolved, environment) {
                return false;
            }

            statement.wildcard_arm.as_ref().map_or_else(
                || switch_statement_covers_all_variants(statement, resolved, environment),
                |wildcard_arm| {
                    block_guarantees_return_or_never(&wildcard_arm.body, resolved, environment)
                },
            )
        }
        Stmt::Loop(statement) => {
            block_guarantees_return_or_never(&statement.body, resolved, environment)
        }
        _ => statement_guarantees_return(statement),
    }
}

pub(super) fn switch_arms_guarantee_return_or_never(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    statement.arms.iter().all(|arm| {
        let arm_environment =
            environment_for_switch_arm(arm, &statement.expression, resolved, environment);
        block_guarantees_return_or_never(&arm.body, resolved, &arm_environment)
    })
}

pub(super) fn expression_guarantees_return(expression: &Expr) -> bool {
    match expression {
        Expr::If(expression) => expression.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return(&expression.then_block) && block_guarantees_return(else_block)
        }),
        Expr::IfIs(expression) => expression.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return(&expression.then_block) && block_guarantees_return(else_block)
        }),
        Expr::Match(expression) => expression
            .wildcard_arm
            .as_ref()
            .is_some_and(|wildcard_arm| {
                expression
                    .arms
                    .iter()
                    .all(|arm| block_guarantees_return(&arm.body))
                    && block_guarantees_return(&wildcard_arm.body)
            }),
        Expr::Group(group) => expression_guarantees_return(&group.expression),
        _ => false,
    }
}

pub(super) fn statement_guarantees_return(statement: &Stmt) -> bool {
    match statement {
        Stmt::Return(_) => true,
        Stmt::If(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return(&statement.then_block) && block_guarantees_return(else_block)
        }),
        Stmt::IfIs(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return(&statement.then_block) && block_guarantees_return(else_block)
        }),
        Stmt::Switch(statement) => statement.wildcard_arm.as_ref().is_some_and(|wildcard_arm| {
            statement
                .arms
                .iter()
                .all(|arm| block_guarantees_return(&arm.body))
                && block_guarantees_return(&wildcard_arm.body)
        }),
        Stmt::Loop(statement) => block_guarantees_return(&statement.body),
        Stmt::Region(statement) => block_guarantees_return(&statement.body),
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Binding(_)
        | Stmt::Assignment(_)
        | Stmt::ForRange(_)
        | Stmt::CollectionFor(_)
        | Stmt::LiteralPackFor(_)
        | Stmt::While(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_)
        | Stmt::Expression(_) => false,
    }
}
