use super::*;

pub(super) fn check_statement_borrow_conflicts(
    sources: &SourceMap,
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    active_borrows: &[ActiveBorrow],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut new_borrows = Vec::new();
    collect_direct_borrow_expressions_in_statement(
        statement,
        resolved,
        environment,
        &mut new_borrows,
    );

    for borrow in active_borrows {
        if let Some(action) =
            statement_conflicting_action(statement, &borrow.source, resolved, environment)
        {
            let action_name = action.place.display();
            diagnostics.push(active_borrow_conflict_diagnostic(
                sources,
                &action_name,
                action.description,
                action.span,
                &borrow.borrow_name,
                borrow.borrow_span,
                borrow.is_readwrite,
            ));
            return;
        }

        if let Some(new_borrow) = new_borrows
            .iter()
            .find(|new_borrow| new_borrow.source.conflicts_with(&borrow.source))
            && (new_borrow.is_readwrite || borrow.is_readwrite)
        {
            let action = if new_borrow.is_readwrite {
                "create readwrite borrow of"
            } else {
                "create readonly borrow of"
            };
            let action_name = new_borrow.source.display();
            diagnostics.push(active_borrow_conflict_diagnostic(
                sources,
                &action_name,
                action,
                new_borrow.source_span,
                &borrow.borrow_name,
                borrow.borrow_span,
                borrow.is_readwrite,
            ));
            return;
        }

        if borrow.is_readwrite
            && let Some(action) =
                statement_read_action(statement, &borrow.source, resolved, environment)
        {
            let action_name = action.place.display();
            diagnostics.push(active_borrow_conflict_diagnostic(
                sources,
                &action_name,
                action.description,
                action.span,
                &borrow.borrow_name,
                borrow.borrow_span,
                borrow.is_readwrite,
            ));
            return;
        }
    }
}

pub(super) fn check_expression_borrow_conflicts(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    active_borrows: &[ActiveBorrow],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut new_borrows = Vec::new();
    collect_direct_borrow_expressions(expression, resolved, environment, &mut new_borrows);

    for borrow in active_borrows {
        if let Some(action) =
            expression_move_action(expression, &borrow.source, resolved, environment)
        {
            let action_name = action.place.display();
            diagnostics.push(active_borrow_conflict_diagnostic(
                sources,
                &action_name,
                action.description,
                action.span,
                &borrow.borrow_name,
                borrow.borrow_span,
                borrow.is_readwrite,
            ));
            return;
        }

        if let Some(new_borrow) = new_borrows
            .iter()
            .find(|new_borrow| new_borrow.source.conflicts_with(&borrow.source))
            && (new_borrow.is_readwrite || borrow.is_readwrite)
        {
            let action = if new_borrow.is_readwrite {
                "create readwrite borrow of"
            } else {
                "create readonly borrow of"
            };
            let action_name = new_borrow.source.display();
            diagnostics.push(active_borrow_conflict_diagnostic(
                sources,
                &action_name,
                action,
                new_borrow.source_span,
                &borrow.borrow_name,
                borrow.borrow_span,
                borrow.is_readwrite,
            ));
            return;
        }

        if borrow.is_readwrite
            && let Some(action) =
                expression_read_action(expression, &borrow.source, resolved, environment)
        {
            let action_name = action.place.display();
            diagnostics.push(active_borrow_conflict_diagnostic(
                sources,
                &action_name,
                action.description,
                action.span,
                &borrow.borrow_name,
                borrow.borrow_span,
                borrow.is_readwrite,
            ));
            return;
        }
    }
}

pub(super) fn record_statement_borrow(
    statement: &Stmt,
    later_statements: &[Stmt],
    later_result: Option<&Expr>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    active_borrows: &mut Vec<ActiveBorrow>,
) {
    let Stmt::Binding(binding) = statement else {
        return;
    };
    let Some(source) = direct_borrow_source(&binding.initializer) else {
        return;
    };
    if !statements_or_result_use_identifier_before_terminal(
        later_statements,
        later_result,
        &binding.name,
        resolved,
        environment,
    ) {
        return;
    }

    active_borrows.push(ActiveBorrow {
        source: source.source,
        borrow_name: binding.name.clone(),
        borrow_span: binding.name_span,
        is_readwrite: source.is_readwrite,
    });
}

fn statement_conflicting_action(
    statement: &Stmt,
    source: &BorrowPlace,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<BorrowAction> {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => None,
        Stmt::Drop(statement)
            if BorrowPlace::whole(statement.name.clone()).conflicts_with(source) =>
        {
            Some(BorrowAction {
                place: BorrowPlace::whole(statement.name.clone()),
                span: statement.name_span,
                description: "drop",
            })
        }
        Stmt::Assignment(statement)
            if assignment_target_place(&statement.target)
                .as_ref()
                .is_some_and(|target| target.conflicts_with(source)) =>
        {
            Some(BorrowAction {
                place: assignment_target_place(&statement.target)?,
                span: statement.target.span(),
                description: "assign to",
            })
        }
        Stmt::Return(statement) => statement.expression.as_ref().and_then(|expression| {
            expression_move_action(expression, source, resolved, environment)
        }),
        Stmt::Binding(statement) => {
            expression_move_action(&statement.initializer, source, resolved, environment)
        }
        Stmt::Assignment(statement) => {
            expression_move_action(&statement.value, source, resolved, environment)
        }
        Stmt::If(statement) => {
            expression_move_action(&statement.condition, source, resolved, environment)
                .or_else(|| block_move_action(&statement.then_block, source, resolved, environment))
                .or_else(|| {
                    statement
                        .else_block
                        .as_ref()
                        .and_then(|block| block_move_action(block, source, resolved, environment))
                })
        }
        Stmt::IfIs(statement) => {
            expression_move_action(&statement.expression, source, resolved, environment)
                .or_else(|| block_move_action(&statement.then_block, source, resolved, environment))
                .or_else(|| {
                    statement
                        .else_block
                        .as_ref()
                        .and_then(|block| block_move_action(block, source, resolved, environment))
                })
        }
        Stmt::Switch(statement) => {
            expression_move_action(&statement.expression, source, resolved, environment)
                .or_else(|| {
                    statement
                        .arms
                        .iter()
                        .find_map(|arm| block_move_action(&arm.body, source, resolved, environment))
                })
                .or_else(|| {
                    statement
                        .wildcard_arm
                        .as_ref()
                        .and_then(|arm| block_move_action(&arm.body, source, resolved, environment))
                })
        }
        Stmt::ForRange(statement) => {
            expression_move_action(&statement.start, source, resolved, environment)
                .or_else(|| expression_move_action(&statement.end, source, resolved, environment))
                .or_else(|| block_move_action(&statement.body, source, resolved, environment))
        }
        Stmt::While(statement) => {
            expression_move_action(&statement.condition, source, resolved, environment)
                .or_else(|| block_move_action(&statement.body, source, resolved, environment))
        }
        Stmt::Loop(statement) => block_move_action(&statement.body, source, resolved, environment),
        Stmt::Expression(statement) => {
            expression_move_action(&statement.expression, source, resolved, environment)
        }
        Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => None,
    }
}

fn block_move_action(
    block: &Block,
    source: &BorrowPlace,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<BorrowAction> {
    block
        .statements
        .iter()
        .find_map(|statement| {
            statement_conflicting_action(statement, source, resolved, environment)
        })
        .or_else(|| {
            block
                .result
                .as_ref()
                .and_then(|result| expression_move_action(result, source, resolved, environment))
        })
}

fn statement_read_action(
    statement: &Stmt,
    source: &BorrowPlace,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<BorrowAction> {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => None,
        Stmt::Return(statement) => statement.expression.as_ref().and_then(|expression| {
            expression_read_action(expression, source, resolved, environment)
        }),
        Stmt::Binding(statement) => {
            expression_read_action(&statement.initializer, source, resolved, environment)
        }
        Stmt::Assignment(statement) => {
            expression_read_action(&statement.target, source, resolved, environment)
                .or_else(|| expression_read_action(&statement.value, source, resolved, environment))
        }
        Stmt::If(statement) => {
            expression_read_action(&statement.condition, source, resolved, environment)
                .or_else(|| block_read_action(&statement.then_block, source, resolved, environment))
                .or_else(|| {
                    statement
                        .else_block
                        .as_ref()
                        .and_then(|block| block_read_action(block, source, resolved, environment))
                })
        }
        Stmt::IfIs(statement) => {
            expression_read_action(&statement.expression, source, resolved, environment)
                .or_else(|| block_read_action(&statement.then_block, source, resolved, environment))
                .or_else(|| {
                    statement
                        .else_block
                        .as_ref()
                        .and_then(|block| block_read_action(block, source, resolved, environment))
                })
        }
        Stmt::Switch(statement) => {
            expression_read_action(&statement.expression, source, resolved, environment)
                .or_else(|| {
                    statement
                        .arms
                        .iter()
                        .find_map(|arm| block_read_action(&arm.body, source, resolved, environment))
                })
                .or_else(|| {
                    statement
                        .wildcard_arm
                        .as_ref()
                        .and_then(|arm| block_read_action(&arm.body, source, resolved, environment))
                })
        }
        Stmt::ForRange(statement) => {
            expression_read_action(&statement.start, source, resolved, environment)
                .or_else(|| expression_read_action(&statement.end, source, resolved, environment))
                .or_else(|| block_read_action(&statement.body, source, resolved, environment))
        }
        Stmt::While(statement) => {
            expression_read_action(&statement.condition, source, resolved, environment)
                .or_else(|| block_read_action(&statement.body, source, resolved, environment))
        }
        Stmt::Loop(statement) => block_read_action(&statement.body, source, resolved, environment),
        Stmt::Expression(statement) => {
            expression_read_action(&statement.expression, source, resolved, environment)
        }
        Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => None,
    }
}

fn block_read_action(
    block: &Block,
    source: &BorrowPlace,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<BorrowAction> {
    block
        .statements
        .iter()
        .find_map(|statement| statement_read_action(statement, source, resolved, environment))
        .or_else(|| {
            block
                .result
                .as_ref()
                .and_then(|result| expression_read_action(result, source, resolved, environment))
        })
}

fn expression_move_action(
    expression: &Expr,
    source: &BorrowPlace,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<BorrowAction> {
    match expression {
        Expr::Unary(expression) if expression.operator == UnaryOperator::Move => {
            match expression.operand.as_ref() {
                Expr::Identifier(identifier)
                    if BorrowPlace::whole(identifier.name.clone()).conflicts_with(source) =>
                {
                    Some(BorrowAction {
                        place: BorrowPlace::whole(identifier.name.clone()),
                        span: identifier.span,
                        description: "move",
                    })
                }
                _ => None,
            }
        }
        Expr::Propagate(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment)
        }
        Expr::Force(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment)
        }
        Expr::Catch(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment).or_else(
                || block_move_action(&expression.catch_block, source, resolved, environment),
            )
        }
        Expr::Borrow(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment)
        }
        Expr::Binary(expression) => {
            expression_move_action(&expression.left, source, resolved, environment).or_else(|| {
                expression_move_action(&expression.right, source, resolved, environment)
            })
        }
        Expr::Unary(expression) => {
            expression_move_action(&expression.operand, source, resolved, environment)
        }
        Expr::TypeConversion(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment)
        }
        Expr::Call(expression) => {
            if let Some(identifier) =
                owned_method_receiver_identifier(expression, resolved, environment)
                && BorrowPlace::whole(identifier.name.clone()).conflicts_with(source)
            {
                return Some(BorrowAction {
                    place: BorrowPlace::whole(identifier.name.clone()),
                    span: identifier.span,
                    description: "move",
                });
            }
            expression_move_action(&expression.callee, source, resolved, environment).or_else(
                || {
                    expression.arguments.iter().find_map(|argument| {
                        expression_move_action(argument, source, resolved, environment)
                    })
                },
            )
        }
        Expr::Member(expression) => {
            expression_move_action(&expression.object, source, resolved, environment)
        }
        Expr::Index(expression) => {
            expression_move_action(&expression.object, source, resolved, environment).or_else(
                || expression_move_action(&expression.index, source, resolved, environment),
            )
        }
        Expr::ArrayLiteral(expression) => expression
            .elements
            .iter()
            .find_map(|element| expression_move_action(element, source, resolved, environment)),
        Expr::StructLiteral(expression) => expression
            .fields
            .iter()
            .find_map(|field| expression_move_action(&field.value, source, resolved, environment)),
        Expr::Group(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment)
        }
        Expr::InterpolatedString(expression) => {
            expression.parts.iter().find_map(|part| match part {
                crate::ast::InterpolatedStringPart::Expression(part) => {
                    expression_move_action(&part.expression, source, resolved, environment)
                }
                crate::ast::InterpolatedStringPart::Text(_) => None,
            })
        }
        Expr::Otherwise(expression) => {
            expression_move_action(&expression.value, source, resolved, environment)
                .or_else(|| block_move_action(&expression.fallback, source, resolved, environment))
        }
        Expr::If(expression) => {
            expression_move_action(&expression.condition, source, resolved, environment)
                .or_else(|| {
                    block_move_action(&expression.then_block, source, resolved, environment)
                })
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| block_move_action(block, source, resolved, environment))
                })
        }
        Expr::IfIs(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment)
                .or_else(|| {
                    block_move_action(&expression.then_block, source, resolved, environment)
                })
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| block_move_action(block, source, resolved, environment))
                })
        }
        Expr::Match(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment)
                .or_else(|| {
                    expression
                        .arms
                        .iter()
                        .find_map(|arm| block_move_action(&arm.body, source, resolved, environment))
                })
                .or_else(|| {
                    expression
                        .wildcard_arm
                        .as_ref()
                        .and_then(|arm| block_move_action(&arm.body, source, resolved, environment))
                })
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => None,
    }
}

fn expression_read_action(
    expression: &Expr,
    source: &BorrowPlace,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<BorrowAction> {
    match expression {
        Expr::Identifier(identifier) => {
            let place = BorrowPlace::whole(identifier.name.clone());
            place.conflicts_with(source).then_some(BorrowAction {
                place,
                span: identifier.span,
                description: "use",
            })
        }
        Expr::Unary(expression) if expression.operator == UnaryOperator::Move => None,
        Expr::Propagate(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment)
        }
        Expr::Force(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment)
        }
        Expr::Catch(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment).or_else(
                || block_read_action(&expression.catch_block, source, resolved, environment),
            )
        }
        Expr::Borrow(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment)
        }
        Expr::Binary(expression) => {
            expression_read_action(&expression.left, source, resolved, environment).or_else(|| {
                expression_read_action(&expression.right, source, resolved, environment)
            })
        }
        Expr::Unary(expression) => {
            expression_read_action(&expression.operand, source, resolved, environment)
        }
        Expr::TypeConversion(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment)
        }
        Expr::Call(expression) => {
            expression_read_action(&expression.callee, source, resolved, environment).or_else(
                || {
                    expression.arguments.iter().find_map(|argument| {
                        expression_read_action(argument, source, resolved, environment)
                    })
                },
            )
        }
        Expr::Member(expression) => {
            if let Some(place) = member_expression_place(expression)
                && place.conflicts_with(source)
            {
                return Some(BorrowAction {
                    place,
                    span: expression.span,
                    description: "use",
                });
            }
            if expression_place_has_only_named_fields(&expression.object) {
                None
            } else {
                expression_read_action(&expression.object, source, resolved, environment)
            }
        }
        Expr::Index(expression) => {
            if let Some(place) = index_expression_place(expression)
                && place.conflicts_with(source)
            {
                return Some(BorrowAction {
                    place,
                    span: expression.span,
                    description: "use",
                });
            }
            let object_action = if expression_place_has_only_named_fields(&expression.object) {
                None
            } else {
                expression_read_action(&expression.object, source, resolved, environment)
            };
            object_action.or_else(|| {
                expression_read_action(&expression.index, source, resolved, environment)
            })
        }
        Expr::ArrayLiteral(expression) => expression
            .elements
            .iter()
            .find_map(|element| expression_read_action(element, source, resolved, environment)),
        Expr::StructLiteral(expression) => expression
            .fields
            .iter()
            .find_map(|field| expression_read_action(&field.value, source, resolved, environment)),
        Expr::Group(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment)
        }
        Expr::InterpolatedString(expression) => {
            expression.parts.iter().find_map(|part| match part {
                crate::ast::InterpolatedStringPart::Expression(part) => {
                    expression_read_action(&part.expression, source, resolved, environment)
                }
                crate::ast::InterpolatedStringPart::Text(_) => None,
            })
        }
        Expr::Otherwise(expression) => {
            expression_read_action(&expression.value, source, resolved, environment)
                .or_else(|| block_read_action(&expression.fallback, source, resolved, environment))
        }
        Expr::If(expression) => {
            expression_read_action(&expression.condition, source, resolved, environment)
                .or_else(|| {
                    block_read_action(&expression.then_block, source, resolved, environment)
                })
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| block_read_action(block, source, resolved, environment))
                })
        }
        Expr::IfIs(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment)
                .or_else(|| {
                    block_read_action(&expression.then_block, source, resolved, environment)
                })
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| block_read_action(block, source, resolved, environment))
                })
        }
        Expr::Match(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment)
                .or_else(|| {
                    expression
                        .arms
                        .iter()
                        .find_map(|arm| block_read_action(&arm.body, source, resolved, environment))
                })
                .or_else(|| {
                    expression
                        .wildcard_arm
                        .as_ref()
                        .and_then(|arm| block_read_action(&arm.body, source, resolved, environment))
                })
        }
        Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => None,
    }
}
