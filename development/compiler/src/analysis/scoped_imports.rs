//! Incomplete-source fallback for block-scope import visibility.
//!
//! Complete files use `LexicalScopeIndex`; this syntax walk is retained only
//! for completion recovery before a normal `FileAnalysis` can be built.

use crate::ast::{
    AstFile, Block, ConformanceMember, Expr, IfIsStmt, IfStmt, InterpolatedStringPart, Item, Stmt,
    SwitchStmt,
};
use crate::source::ByteSpan;
use std::collections::HashSet;

pub(crate) fn recovery_visible_scoped_import_spans_at_offset(
    ast: &AstFile,
    offset: usize,
) -> HashSet<ByteSpan> {
    ast.items
        .iter()
        .find_map(|item| scoped_import_spans_in_item_at_offset(item, offset))
        .unwrap_or_default()
}

fn scoped_import_spans_in_item_at_offset(item: &Item, offset: usize) -> Option<HashSet<ByteSpan>> {
    if !span_contains(item.span(), offset) {
        return None;
    }

    match item {
        Item::Function(function) => function
            .body
            .as_ref()
            .and_then(|body| scoped_import_spans_in_block_at_offset(body, offset, &HashSet::new())),
        Item::Instance(instance) => instance.callables().find_map(|method| {
            method.body.as_ref().and_then(|body| {
                scoped_import_spans_in_block_at_offset(body, offset, &HashSet::new())
            })
        }),
        Item::Destruct(destruct) => {
            scoped_import_spans_in_block_at_offset(&destruct.body, offset, &HashSet::new())
        }
        Item::Conformance(conformance) => {
            conformance.members.iter().find_map(|member| match member {
                ConformanceMember::AssociatedType(_) => None,
                ConformanceMember::Method(method) => method.body.as_ref().and_then(|body| {
                    scoped_import_spans_in_block_at_offset(body, offset, &HashSet::new())
                }),
            })
        }
        _ => None,
    }
}

fn scoped_import_spans_in_block_at_offset(
    block: &Block,
    offset: usize,
    inherited: &HashSet<ByteSpan>,
) -> Option<HashSet<ByteSpan>> {
    if !span_contains(block.span, offset) {
        return None;
    }

    let mut visible = inherited.clone();
    for statement in &block.statements {
        let statement_span = statement.span();
        if statement_span.start > offset {
            break;
        }

        match statement {
            Stmt::Import(import) if import.span.end <= offset => {
                visible.insert(import.alias.span);
                continue;
            }
            Stmt::FromImport(import) if import.span.end <= offset => {
                visible.extend(import.names.iter().map(|name| name.local_span()));
                continue;
            }
            _ => {}
        }

        if let Some(scoped) =
            scoped_import_spans_in_statement_at_offset(statement, offset, &visible)
        {
            return Some(scoped);
        }

        if span_contains(statement_span, offset) {
            return Some(visible);
        }
    }

    if let Some(result) = &block.result
        && let Some(scoped) = scoped_import_spans_in_expression_at_offset(result, offset, &visible)
    {
        return Some(scoped);
    }

    Some(visible)
}

fn scoped_import_spans_in_statement_at_offset(
    statement: &Stmt,
    offset: usize,
    visible: &HashSet<ByteSpan>,
) -> Option<HashSet<ByteSpan>> {
    match statement {
        Stmt::Return(statement) => statement.expression.as_ref().and_then(|expression| {
            scoped_import_spans_in_expression_at_offset(expression, offset, visible)
        }),
        Stmt::Binding(statement) => {
            scoped_import_spans_in_expression_at_offset(&statement.initializer, offset, visible)
        }
        Stmt::Assignment(statement) => {
            scoped_import_spans_in_expression_at_offset(&statement.target, offset, visible).or_else(
                || scoped_import_spans_in_expression_at_offset(&statement.value, offset, visible),
            )
        }
        Stmt::If(statement) => scoped_import_spans_in_if_at_offset(statement, offset, visible),
        Stmt::IfIs(statement) => scoped_import_spans_in_if_is_at_offset(statement, offset, visible),
        Stmt::Switch(statement) => {
            scoped_import_spans_in_switch_at_offset(statement, offset, visible)
        }
        Stmt::ForRange(statement) => {
            scoped_import_spans_in_expression_at_offset(&statement.start, offset, visible)
                .or_else(|| {
                    scoped_import_spans_in_expression_at_offset(&statement.end, offset, visible)
                })
                .or_else(|| {
                    scoped_import_spans_in_block_at_offset(&statement.body, offset, visible)
                })
        }
        Stmt::CollectionFor(statement) => {
            scoped_import_spans_in_expression_at_offset(&statement.source, offset, visible).or_else(
                || scoped_import_spans_in_block_at_offset(&statement.body, offset, visible),
            )
        }
        Stmt::LiteralPackFor(statement) => {
            scoped_import_spans_in_block_at_offset(&statement.body, offset, visible)
        }
        Stmt::While(statement) => {
            scoped_import_spans_in_expression_at_offset(&statement.condition, offset, visible)
                .or_else(|| {
                    scoped_import_spans_in_block_at_offset(&statement.body, offset, visible)
                })
        }
        Stmt::Loop(statement) => {
            scoped_import_spans_in_block_at_offset(&statement.body, offset, visible)
        }
        Stmt::Region(statement) => {
            scoped_import_spans_in_expression_at_offset(&statement.allocator, offset, visible)
                .or_else(|| {
                    scoped_import_spans_in_block_at_offset(&statement.body, offset, visible)
                })
        }
        Stmt::Expression(statement) => {
            scoped_import_spans_in_expression_at_offset(&statement.expression, offset, visible)
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => None,
    }
}

fn scoped_import_spans_in_expression_at_offset(
    expression: &Expr,
    offset: usize,
    visible: &HashSet<ByteSpan>,
) -> Option<HashSet<ByteSpan>> {
    if !span_contains(expression.span(), offset) {
        return None;
    }

    match expression {
        Expr::Closure(expression) => {
            scoped_import_spans_in_block_at_offset(&expression.body, offset, visible)
                .or_else(|| Some(visible.clone()))
        }
        Expr::InterpolatedString(expression) => expression.parts.iter().find_map(|part| {
            let InterpolatedStringPart::Expression(part) = part else {
                return None;
            };
            scoped_import_spans_in_expression_at_offset(&part.expression, offset, visible)
        }),
        Expr::ArrayLiteral(expression) => expression.elements.iter().find_map(|element| {
            scoped_import_spans_in_expression_at_offset(element, offset, visible)
        }),
        Expr::TypedSequenceLiteral(expression) => expression
            .elements
            .iter()
            .find_map(|element| {
                scoped_import_spans_in_expression_at_offset(element, offset, visible)
            })
            .or_else(|| {
                expression.using.as_ref().and_then(|using| {
                    scoped_import_spans_in_expression_at_offset(&using.allocator, offset, visible)
                })
            }),
        Expr::TypedStringLiteral(expression) => expression
            .using
            .as_ref()
            .and_then(|using| {
                scoped_import_spans_in_expression_at_offset(&using.allocator, offset, visible)
            })
            .or_else(|| Some(visible.clone())),
        Expr::StructLiteral(expression) => expression.fields.iter().find_map(|field| {
            scoped_import_spans_in_expression_at_offset(&field.value, offset, visible)
        }),
        Expr::Propagate(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.expression, offset, visible)
        }
        Expr::Force(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.expression, offset, visible)
        }
        Expr::Catch(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.expression, offset, visible)
                .or_else(|| {
                    scoped_import_spans_in_block_at_offset(&expression.catch_block, offset, visible)
                })
        }
        Expr::Borrow(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.expression, offset, visible)
        }
        Expr::Unary(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.operand, offset, visible)
        }
        Expr::Binary(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.left, offset, visible).or_else(
                || scoped_import_spans_in_expression_at_offset(&expression.right, offset, visible),
            )
        }
        Expr::TypeConversion(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.expression, offset, visible)
        }
        Expr::Call(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.callee, offset, visible)
                .or_else(|| {
                    expression.arguments.iter().find_map(|argument| {
                        scoped_import_spans_in_expression_at_offset(argument, offset, visible)
                    })
                })
        }
        Expr::Member(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.object, offset, visible)
        }
        Expr::Index(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.object, offset, visible)
                .or_else(|| {
                    scoped_import_spans_in_expression_at_offset(&expression.index, offset, visible)
                })
        }
        Expr::Group(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.expression, offset, visible)
        }
        Expr::Otherwise(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.value, offset, visible).or_else(
                || scoped_import_spans_in_block_at_offset(&expression.fallback, offset, visible),
            )
        }
        Expr::If(statement) => scoped_import_spans_in_if_at_offset(statement, offset, visible),
        Expr::IfIs(statement) => scoped_import_spans_in_if_is_at_offset(statement, offset, visible),
        Expr::Match(statement) => {
            scoped_import_spans_in_switch_at_offset(statement, offset, visible)
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => Some(visible.clone()),
    }
}

fn scoped_import_spans_in_if_at_offset(
    statement: &IfStmt,
    offset: usize,
    visible: &HashSet<ByteSpan>,
) -> Option<HashSet<ByteSpan>> {
    scoped_import_spans_in_expression_at_offset(&statement.condition, offset, visible)
        .or_else(|| scoped_import_spans_in_block_at_offset(&statement.then_block, offset, visible))
        .or_else(|| {
            statement
                .else_block
                .as_ref()
                .and_then(|block| scoped_import_spans_in_block_at_offset(block, offset, visible))
        })
}

fn scoped_import_spans_in_if_is_at_offset(
    statement: &IfIsStmt,
    offset: usize,
    visible: &HashSet<ByteSpan>,
) -> Option<HashSet<ByteSpan>> {
    scoped_import_spans_in_expression_at_offset(&statement.expression, offset, visible)
        .or_else(|| scoped_import_spans_in_block_at_offset(&statement.then_block, offset, visible))
        .or_else(|| {
            statement
                .else_block
                .as_ref()
                .and_then(|block| scoped_import_spans_in_block_at_offset(block, offset, visible))
        })
}

fn scoped_import_spans_in_switch_at_offset(
    statement: &SwitchStmt,
    offset: usize,
    visible: &HashSet<ByteSpan>,
) -> Option<HashSet<ByteSpan>> {
    scoped_import_spans_in_expression_at_offset(&statement.expression, offset, visible)
        .or_else(|| {
            statement
                .arms
                .iter()
                .find_map(|arm| scoped_import_spans_in_block_at_offset(&arm.body, offset, visible))
        })
        .or_else(|| {
            statement
                .wildcard_arm
                .as_ref()
                .and_then(|arm| scoped_import_spans_in_block_at_offset(&arm.body, offset, visible))
        })
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}
