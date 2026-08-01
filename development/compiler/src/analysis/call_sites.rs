//! Cursor-to-call lookup shared by call-oriented editor features.

use crate::ast::{AstFile, Block, CallExpr, Expr, ImplMember, Item, Stmt};
use crate::source::ByteSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CallCursorRegion {
    FullCall,
    Arguments,
}

pub(super) fn call_at_offset(
    ast: &AstFile,
    offset: usize,
    region: CallCursorRegion,
) -> Option<&CallExpr> {
    ast.items
        .iter()
        .find_map(|item| call_in_item_at_offset(item, offset, region))
}

fn call_in_item_at_offset(
    item: &Item,
    offset: usize,
    region: CallCursorRegion,
) -> Option<&CallExpr> {
    match item {
        Item::Function(function) => call_in_block_at_offset(&function.body, offset, region),
        Item::Impl(impl_) => impl_.members.iter().find_map(|member| match member {
            ImplMember::Method(method) => method
                .body
                .as_ref()
                .and_then(|body| call_in_block_at_offset(body, offset, region)),
            ImplMember::Drop(drop_) => call_in_block_at_offset(&drop_.body, offset, region),
        }),
        Item::Import(_)
        | Item::FromImport(_)
        | Item::Primitive(_)
        | Item::TypeAlias(_)
        | Item::Struct(_)
        | Item::Enum(_)
        | Item::Interface(_) => None,
    }
}

fn call_in_block_at_offset(
    block: &Block,
    offset: usize,
    region: CallCursorRegion,
) -> Option<&CallExpr> {
    block
        .statements
        .iter()
        .find_map(|statement| call_in_statement_at_offset(statement, offset, region))
        .or_else(|| {
            block
                .result
                .as_ref()
                .and_then(|result| call_in_expression_at_offset(result, offset, region))
        })
}

fn call_in_statement_at_offset(
    statement: &Stmt,
    offset: usize,
    region: CallCursorRegion,
) -> Option<&CallExpr> {
    match statement {
        Stmt::Return(statement) => statement
            .expression
            .as_ref()
            .and_then(|expression| call_in_expression_at_offset(expression, offset, region)),
        Stmt::Binding(statement) => {
            call_in_expression_at_offset(&statement.initializer, offset, region)
        }
        Stmt::Assignment(statement) => {
            call_in_expression_at_offset(&statement.target, offset, region)
                .or_else(|| call_in_expression_at_offset(&statement.value, offset, region))
        }
        Stmt::If(statement) => call_in_expression_at_offset(&statement.condition, offset, region)
            .or_else(|| call_in_block_at_offset(&statement.then_block, offset, region))
            .or_else(|| {
                statement
                    .else_block
                    .as_ref()
                    .and_then(|block| call_in_block_at_offset(block, offset, region))
            }),
        Stmt::IfIs(statement) => {
            call_in_expression_at_offset(&statement.expression, offset, region)
                .or_else(|| call_in_block_at_offset(&statement.then_block, offset, region))
                .or_else(|| {
                    statement
                        .else_block
                        .as_ref()
                        .and_then(|block| call_in_block_at_offset(block, offset, region))
                })
        }
        Stmt::Switch(statement) => {
            call_in_expression_at_offset(&statement.expression, offset, region)
                .or_else(|| {
                    statement
                        .arms
                        .iter()
                        .find_map(|arm| call_in_block_at_offset(&arm.body, offset, region))
                })
                .or_else(|| {
                    statement
                        .wildcard_arm
                        .as_ref()
                        .and_then(|arm| call_in_block_at_offset(&arm.body, offset, region))
                })
        }
        Stmt::ForRange(statement) => call_in_expression_at_offset(&statement.start, offset, region)
            .or_else(|| call_in_expression_at_offset(&statement.end, offset, region))
            .or_else(|| call_in_block_at_offset(&statement.body, offset, region)),
        Stmt::While(statement) => {
            call_in_expression_at_offset(&statement.condition, offset, region)
                .or_else(|| call_in_block_at_offset(&statement.body, offset, region))
        }
        Stmt::Loop(statement) => call_in_block_at_offset(&statement.body, offset, region),
        Stmt::Expression(statement) => {
            call_in_expression_at_offset(&statement.expression, offset, region)
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => None,
    }
}

fn call_in_expression_at_offset(
    expression: &Expr,
    offset: usize,
    region: CallCursorRegion,
) -> Option<&CallExpr> {
    if !span_contains_cursor(expression.span(), offset) {
        return None;
    }

    let nested = match expression {
        Expr::InterpolatedString(expression) => {
            expression.parts.iter().find_map(|part| match part {
                crate::ast::InterpolatedStringPart::Expression(part) => {
                    call_in_expression_at_offset(&part.expression, offset, region)
                }
                crate::ast::InterpolatedStringPart::Text(_) => None,
            })
        }
        Expr::ArrayLiteral(expression) => expression
            .elements
            .iter()
            .find_map(|element| call_in_expression_at_offset(element, offset, region)),
        Expr::StructLiteral(expression) => expression
            .fields
            .iter()
            .find_map(|field| call_in_expression_at_offset(&field.value, offset, region)),
        Expr::Propagate(expression) => {
            call_in_expression_at_offset(&expression.expression, offset, region)
        }
        Expr::Force(expression) => {
            call_in_expression_at_offset(&expression.expression, offset, region)
        }
        Expr::Catch(expression) => {
            call_in_expression_at_offset(&expression.expression, offset, region)
                .or_else(|| call_in_block_at_offset(&expression.catch_block, offset, region))
        }
        Expr::Borrow(expression) => {
            call_in_expression_at_offset(&expression.expression, offset, region)
        }
        Expr::Unary(expression) => {
            call_in_expression_at_offset(&expression.operand, offset, region)
        }
        Expr::Binary(expression) => call_in_expression_at_offset(&expression.left, offset, region)
            .or_else(|| call_in_expression_at_offset(&expression.right, offset, region)),
        Expr::TypeConversion(expression) => {
            call_in_expression_at_offset(&expression.expression, offset, region)
        }
        Expr::Call(call) => {
            call_in_expression_at_offset(&call.callee, offset, region).or_else(|| {
                call.arguments
                    .iter()
                    .find_map(|argument| call_in_expression_at_offset(argument, offset, region))
            })
        }
        Expr::Member(expression) => {
            call_in_expression_at_offset(&expression.object, offset, region)
        }
        Expr::Index(expression) => call_in_expression_at_offset(&expression.object, offset, region)
            .or_else(|| call_in_expression_at_offset(&expression.index, offset, region)),
        Expr::Group(expression) => {
            call_in_expression_at_offset(&expression.expression, offset, region)
        }
        Expr::Otherwise(expression) => {
            call_in_expression_at_offset(&expression.value, offset, region)
                .or_else(|| call_in_block_at_offset(&expression.fallback, offset, region))
        }
        Expr::If(expression) => call_in_expression_at_offset(&expression.condition, offset, region)
            .or_else(|| call_in_block_at_offset(&expression.then_block, offset, region))
            .or_else(|| {
                expression
                    .else_block
                    .as_ref()
                    .and_then(|block| call_in_block_at_offset(block, offset, region))
            }),
        Expr::IfIs(expression) => {
            call_in_expression_at_offset(&expression.expression, offset, region)
                .or_else(|| call_in_block_at_offset(&expression.then_block, offset, region))
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| call_in_block_at_offset(block, offset, region))
                })
        }
        Expr::Match(expression) => {
            call_in_expression_at_offset(&expression.expression, offset, region)
                .or_else(|| {
                    expression
                        .arms
                        .iter()
                        .find_map(|arm| call_in_block_at_offset(&arm.body, offset, region))
                })
                .or_else(|| {
                    expression
                        .wildcard_arm
                        .as_ref()
                        .and_then(|arm| call_in_block_at_offset(&arm.body, offset, region))
                })
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => None,
    };
    if nested.is_some() {
        return nested;
    }

    let Expr::Call(call) = expression else {
        return None;
    };
    let span = match region {
        CallCursorRegion::FullCall => call.span,
        CallCursorRegion::Arguments => call.arguments_span,
    };
    span_contains_cursor(span, offset).then_some(call)
}

fn span_contains_cursor(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset <= span.end
}
