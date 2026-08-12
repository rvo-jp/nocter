//! Source-ordered declaration traversal inside executable bodies.

use crate::ast::{Block, Expr, FromImportItem, ImportItem, InterpolatedStringPart, Stmt};

pub(super) enum BodyDeclaration<'a> {
    Import(&'a ImportItem),
    FromImport(&'a FromImportItem),
}

pub(super) fn visit_body_declarations<'a>(
    block: &'a Block,
    visitor: &mut impl FnMut(BodyDeclaration<'a>),
) {
    for statement in &block.statements {
        visit_statement(statement, visitor);
    }
    if let Some(result) = &block.result {
        visit_expression(result, visitor);
    }
}

fn visit_statement<'a>(statement: &'a Stmt, visitor: &mut impl FnMut(BodyDeclaration<'a>)) {
    match statement {
        Stmt::Import(import) => visitor(BodyDeclaration::Import(import)),
        Stmt::FromImport(import) => visitor(BodyDeclaration::FromImport(import)),
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                visit_expression(expression, visitor);
            }
        }
        Stmt::Binding(statement) => visit_expression(&statement.initializer, visitor),
        Stmt::Assignment(statement) => {
            visit_expression(&statement.target, visitor);
            visit_expression(&statement.value, visitor);
        }
        Stmt::If(statement) => {
            visit_expression(&statement.condition, visitor);
            visit_body_declarations(&statement.then_block, visitor);
            if let Some(block) = &statement.else_block {
                visit_body_declarations(block, visitor);
            }
        }
        Stmt::IfIs(statement) => {
            visit_expression(&statement.expression, visitor);
            visit_body_declarations(&statement.then_block, visitor);
            if let Some(block) = &statement.else_block {
                visit_body_declarations(block, visitor);
            }
        }
        Stmt::Switch(statement) => {
            visit_expression(&statement.expression, visitor);
            for arm in &statement.arms {
                visit_body_declarations(&arm.body, visitor);
            }
            if let Some(arm) = &statement.wildcard_arm {
                visit_body_declarations(&arm.body, visitor);
            }
        }
        Stmt::ForRange(statement) => {
            visit_expression(&statement.start, visitor);
            visit_expression(&statement.end, visitor);
            visit_body_declarations(&statement.body, visitor);
        }
        Stmt::CollectionFor(statement) => {
            visit_expression(&statement.source, visitor);
            visit_body_declarations(&statement.body, visitor);
        }
        Stmt::LiteralPackFor(statement) => visit_body_declarations(&statement.body, visitor),
        Stmt::While(statement) => {
            visit_expression(&statement.condition, visitor);
            visit_body_declarations(&statement.body, visitor);
        }
        Stmt::Loop(statement) => visit_body_declarations(&statement.body, visitor),
        Stmt::Region(statement) => {
            visit_expression(&statement.allocator, visitor);
            visit_body_declarations(&statement.body, visitor);
        }
        Stmt::Expression(statement) => visit_expression(&statement.expression, visitor),
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Drop(_) => {}
    }
}

fn visit_expression<'a>(expression: &'a Expr, visitor: &mut impl FnMut(BodyDeclaration<'a>)) {
    match expression {
        Expr::Closure(expression) => visit_body_declarations(&expression.body, visitor),
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
        Expr::Unary(expression) => visit_expression(&expression.operand, visitor),
        Expr::Binary(expression) => {
            visit_expression(&expression.left, visitor);
            visit_expression(&expression.right, visitor);
        }
        Expr::TypeConversion(expression) => visit_expression(&expression.expression, visitor),
        Expr::Propagate(expression) => visit_expression(&expression.expression, visitor),
        Expr::Force(expression) => visit_expression(&expression.expression, visitor),
        Expr::Catch(expression) => {
            visit_expression(&expression.expression, visitor);
            visit_body_declarations(&expression.catch_block, visitor);
        }
        Expr::Borrow(expression) => visit_expression(&expression.expression, visitor),
        Expr::Call(expression) => {
            visit_expression(&expression.callee, visitor);
            for argument in &expression.arguments {
                visit_expression(argument, visitor);
            }
        }
        Expr::Member(expression) => visit_expression(&expression.object, visitor),
        Expr::Index(expression) => {
            visit_expression(&expression.object, visitor);
            visit_expression(&expression.index, visitor);
        }
        Expr::Group(expression) => visit_expression(&expression.expression, visitor),
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                visit_expression(element, visitor);
            }
        }
        Expr::TypedSequenceLiteral(expression) => {
            for element in &expression.elements {
                visit_expression(element, visitor);
            }
            if let Some(using) = &expression.using {
                visit_expression(&using.allocator, visitor);
            }
        }
        Expr::TypedStringLiteral(expression) => {
            if let Some(using) = &expression.using {
                visit_expression(&using.allocator, visitor);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                visit_expression(&field.value, visitor);
            }
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    visit_expression(&part.expression, visitor);
                }
            }
        }
        Expr::Otherwise(expression) => {
            visit_expression(&expression.value, visitor);
            visit_body_declarations(&expression.fallback, visitor);
        }
        Expr::If(expression) => {
            visit_expression(&expression.condition, visitor);
            visit_body_declarations(&expression.then_block, visitor);
            if let Some(block) = &expression.else_block {
                visit_body_declarations(block, visitor);
            }
        }
        Expr::IfIs(expression) => {
            visit_expression(&expression.expression, visitor);
            visit_body_declarations(&expression.then_block, visitor);
            if let Some(block) = &expression.else_block {
                visit_body_declarations(block, visitor);
            }
        }
        Expr::Match(expression) => {
            visit_expression(&expression.expression, visitor);
            for arm in &expression.arms {
                visit_body_declarations(&arm.body, visitor);
            }
            if let Some(arm) = &expression.wildcard_arm {
                visit_body_declarations(&arm.body, visitor);
            }
        }
    }
}
