use super::*;

pub(super) fn collect_direct_borrow_expressions_in_statement(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrows: &mut Vec<DirectBorrowSource>,
) {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => {}
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_direct_borrow_expressions(expression, resolved, environment, borrows);
            }
        }
        Stmt::Binding(statement) => {
            collect_direct_borrow_expressions(
                &statement.initializer,
                resolved,
                environment,
                borrows,
            );
        }
        Stmt::Assignment(statement) => {
            collect_direct_borrow_expressions(&statement.target, resolved, environment, borrows);
            collect_direct_borrow_expressions(&statement.value, resolved, environment, borrows);
        }
        Stmt::If(statement) => {
            collect_direct_borrow_expressions(&statement.condition, resolved, environment, borrows);
            collect_direct_borrow_expressions_in_block(
                &statement.then_block,
                resolved,
                environment,
                borrows,
            );
            if let Some(else_block) = &statement.else_block {
                collect_direct_borrow_expressions_in_block(
                    else_block,
                    resolved,
                    environment,
                    borrows,
                );
            }
        }
        Stmt::IfIs(statement) => {
            collect_direct_borrow_expressions(
                &statement.expression,
                resolved,
                environment,
                borrows,
            );
            collect_direct_borrow_expressions_in_block(
                &statement.then_block,
                resolved,
                environment,
                borrows,
            );
            if let Some(else_block) = &statement.else_block {
                collect_direct_borrow_expressions_in_block(
                    else_block,
                    resolved,
                    environment,
                    borrows,
                );
            }
        }
        Stmt::Switch(statement) => {
            collect_direct_borrow_expressions(
                &statement.expression,
                resolved,
                environment,
                borrows,
            );
            for arm in &statement.arms {
                collect_direct_borrow_expressions_in_block(
                    &arm.body,
                    resolved,
                    environment,
                    borrows,
                );
            }
            if let Some(wildcard_arm) = &statement.wildcard_arm {
                collect_direct_borrow_expressions_in_block(
                    &wildcard_arm.body,
                    resolved,
                    environment,
                    borrows,
                );
            }
        }
        Stmt::ForRange(statement) => {
            collect_direct_borrow_expressions(&statement.start, resolved, environment, borrows);
            collect_direct_borrow_expressions(&statement.end, resolved, environment, borrows);
            collect_direct_borrow_expressions_in_block(
                &statement.body,
                resolved,
                environment,
                borrows,
            );
        }
        Stmt::While(statement) => {
            collect_direct_borrow_expressions(&statement.condition, resolved, environment, borrows);
            collect_direct_borrow_expressions_in_block(
                &statement.body,
                resolved,
                environment,
                borrows,
            );
        }
        Stmt::Loop(statement) => collect_direct_borrow_expressions_in_block(
            &statement.body,
            resolved,
            environment,
            borrows,
        ),
        Stmt::Expression(statement) => {
            collect_direct_borrow_expressions(&statement.expression, resolved, environment, borrows)
        }
        Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn collect_direct_borrow_expressions_in_block(
    block: &Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrows: &mut Vec<DirectBorrowSource>,
) {
    for statement in &block.statements {
        collect_direct_borrow_expressions_in_statement(statement, resolved, environment, borrows);
    }
    if let Some(result) = &block.result {
        collect_direct_borrow_expressions(result, resolved, environment, borrows);
    }
}

pub(super) fn collect_direct_borrow_expressions(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrows: &mut Vec<DirectBorrowSource>,
) {
    if let Some(source) = direct_borrow_source(expression) {
        borrows.push(source);
    }

    match expression {
        Expr::Propagate(expression) => collect_direct_borrow_expressions(
            &expression.expression,
            resolved,
            environment,
            borrows,
        ),
        Expr::Force(expression) => collect_direct_borrow_expressions(
            &expression.expression,
            resolved,
            environment,
            borrows,
        ),
        Expr::Catch(expression) => {
            collect_direct_borrow_expressions(
                &expression.expression,
                resolved,
                environment,
                borrows,
            );
            collect_direct_borrow_expressions_in_block(
                &expression.catch_block,
                resolved,
                environment,
                borrows,
            );
        }
        Expr::Borrow(expression) => collect_direct_borrow_expressions(
            &expression.expression,
            resolved,
            environment,
            borrows,
        ),
        Expr::Binary(expression) => {
            collect_direct_borrow_expressions(&expression.left, resolved, environment, borrows);
            collect_direct_borrow_expressions(&expression.right, resolved, environment, borrows);
        }
        Expr::Unary(expression) => {
            collect_direct_borrow_expressions(&expression.operand, resolved, environment, borrows)
        }
        Expr::TypeConversion(expression) => {
            collect_direct_borrow_expressions(
                &expression.expression,
                resolved,
                environment,
                borrows,
            );
        }
        Expr::Call(expression) => {
            if let Some(source) = method_borrow_receiver_source(expression, resolved, environment) {
                borrows.push(source);
            }
            collect_direct_borrow_expressions(&expression.callee, resolved, environment, borrows);
            for argument in &expression.arguments {
                collect_direct_borrow_expressions(argument, resolved, environment, borrows);
            }
        }
        Expr::Member(expression) => {
            collect_direct_borrow_expressions(&expression.object, resolved, environment, borrows)
        }
        Expr::Index(expression) => {
            collect_direct_borrow_expressions(&expression.object, resolved, environment, borrows);
            collect_direct_borrow_expressions(&expression.index, resolved, environment, borrows);
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_direct_borrow_expressions(element, resolved, environment, borrows);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_direct_borrow_expressions(&field.value, resolved, environment, borrows);
            }
        }
        Expr::Group(expression) => collect_direct_borrow_expressions(
            &expression.expression,
            resolved,
            environment,
            borrows,
        ),
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                    collect_direct_borrow_expressions(
                        &part.expression,
                        resolved,
                        environment,
                        borrows,
                    );
                }
            }
        }
        Expr::Otherwise(expression) => {
            collect_direct_borrow_expressions(&expression.value, resolved, environment, borrows);
            collect_direct_borrow_expressions_in_block(
                &expression.fallback,
                resolved,
                environment,
                borrows,
            );
        }
        Expr::If(expression) => {
            collect_direct_borrow_expressions(
                &expression.condition,
                resolved,
                environment,
                borrows,
            );
            collect_direct_borrow_expressions_in_block(
                &expression.then_block,
                resolved,
                environment,
                borrows,
            );
            if let Some(block) = &expression.else_block {
                collect_direct_borrow_expressions_in_block(block, resolved, environment, borrows);
            }
        }
        Expr::IfIs(expression) => {
            collect_direct_borrow_expressions(
                &expression.expression,
                resolved,
                environment,
                borrows,
            );
            collect_direct_borrow_expressions_in_block(
                &expression.then_block,
                resolved,
                environment,
                borrows,
            );
            if let Some(block) = &expression.else_block {
                collect_direct_borrow_expressions_in_block(block, resolved, environment, borrows);
            }
        }
        Expr::Match(expression) => {
            collect_direct_borrow_expressions(
                &expression.expression,
                resolved,
                environment,
                borrows,
            );
            for arm in &expression.arms {
                collect_direct_borrow_expressions_in_block(
                    &arm.body,
                    resolved,
                    environment,
                    borrows,
                );
            }
            if let Some(arm) = &expression.wildcard_arm {
                collect_direct_borrow_expressions_in_block(
                    &arm.body,
                    resolved,
                    environment,
                    borrows,
                );
            }
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

pub(super) fn direct_borrow_source(expression: &Expr) -> Option<DirectBorrowSource> {
    let Expr::Borrow(borrow) = unwrap_group(expression) else {
        return None;
    };
    let source = expression_place(&borrow.expression)?;
    Some(DirectBorrowSource {
        source,
        source_span: borrow.expression.span(),
        is_readwrite: borrow.is_readwrite,
    })
}

fn method_borrow_receiver_source(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<DirectBorrowSource> {
    let method = method_member_for_call(call)?;
    let (_, signature) = resolved_method_for_call(resolved, call, environment)?;
    let TypeExpr::Borrow(receiver) = &signature.receiver.ty else {
        return None;
    };
    let source = expression_place(&method.object)?;
    Some(DirectBorrowSource {
        source,
        source_span: method.object.span(),
        is_readwrite: receiver.is_readwrite,
    })
}
