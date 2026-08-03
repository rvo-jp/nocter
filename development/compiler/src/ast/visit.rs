//! Shared, exhaustive expression traversal for compiler analyses.

use super::{AstFile, Block, Expr, ImplMember, InterpolatedStringPart, Item, Stmt};

pub(crate) fn visit_file_expressions(ast: &AstFile, visitor: &mut impl FnMut(&Expr)) {
    for item in &ast.items {
        match item {
            Item::Function(function) => visit_block_expressions(&function.body, visitor),
            Item::Impl(impl_) => {
                for member in &impl_.members {
                    match member {
                        ImplMember::Method(method) => {
                            if let Some(body) = &method.body {
                                visit_block_expressions(body, visitor);
                            }
                        }
                        ImplMember::Drop(drop_) => visit_block_expressions(&drop_.body, visitor),
                    }
                }
            }
            Item::Literal(literal) => visit_block_expressions(&literal.body, visitor),
            Item::Import(_)
            | Item::FromImport(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_)
            | Item::Interface(_) => {}
        }
    }
}

pub(crate) fn visit_block_expressions(block: &Block, visitor: &mut impl FnMut(&Expr)) {
    for statement in &block.statements {
        visit_statement_expressions(statement, visitor);
    }
    if let Some(result) = &block.result {
        visit_expression(result, visitor);
    }
}

fn visit_statement_expressions(statement: &Stmt, visitor: &mut impl FnMut(&Expr)) {
    match statement {
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => {}
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
            visit_block_expressions(&statement.then_block, visitor);
            if let Some(block) = &statement.else_block {
                visit_block_expressions(block, visitor);
            }
        }
        Stmt::IfIs(statement) => {
            visit_expression(&statement.expression, visitor);
            visit_block_expressions(&statement.then_block, visitor);
            if let Some(block) = &statement.else_block {
                visit_block_expressions(block, visitor);
            }
        }
        Stmt::Switch(statement) => {
            visit_expression(&statement.expression, visitor);
            for arm in &statement.arms {
                visit_block_expressions(&arm.body, visitor);
            }
            if let Some(arm) = &statement.wildcard_arm {
                visit_block_expressions(&arm.body, visitor);
            }
        }
        Stmt::ForRange(statement) => {
            visit_expression(&statement.start, visitor);
            visit_expression(&statement.end, visitor);
            visit_block_expressions(&statement.body, visitor);
        }
        Stmt::CollectionFor(statement) => {
            visit_expression(&statement.source, visitor);
            visit_block_expressions(&statement.body, visitor);
        }
        Stmt::LiteralPackFor(statement) => visit_block_expressions(&statement.body, visitor),
        Stmt::While(statement) => {
            visit_expression(&statement.condition, visitor);
            visit_block_expressions(&statement.body, visitor);
        }
        Stmt::Loop(statement) => visit_block_expressions(&statement.body, visitor),
        Stmt::Region(statement) => {
            visit_expression(&statement.allocator, visitor);
            visit_block_expressions(&statement.body, visitor);
        }
        Stmt::Expression(statement) => visit_expression(&statement.expression, visitor),
    }
}

pub(crate) fn visit_expression(expression: &Expr, visitor: &mut impl FnMut(&Expr)) {
    visitor(expression);
    match expression {
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
            visit_block_expressions(&expression.catch_block, visitor);
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
            visit_block_expressions(&expression.fallback, visitor);
        }
        Expr::If(expression) => {
            visit_expression(&expression.condition, visitor);
            visit_block_expressions(&expression.then_block, visitor);
            if let Some(block) = &expression.else_block {
                visit_block_expressions(block, visitor);
            }
        }
        Expr::IfIs(expression) => {
            visit_expression(&expression.expression, visitor);
            visit_block_expressions(&expression.then_block, visitor);
            if let Some(block) = &expression.else_block {
                visit_block_expressions(block, visitor);
            }
        }
        Expr::Match(expression) => {
            visit_expression(&expression.expression, visitor);
            for arm in &expression.arms {
                visit_block_expressions(&arm.body, visitor);
            }
            if let Some(arm) = &expression.wildcard_arm {
                visit_block_expressions(&arm.body, visitor);
            }
        }
    }
}
