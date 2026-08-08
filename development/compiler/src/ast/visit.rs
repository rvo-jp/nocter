//! Shared, exhaustive expression traversal for compiler analyses.

use super::{
    AstFile, Block, ConstructMemberDecl, Expr, ImplMember, InterpolatedStringPart, Item, Stmt,
};
use crate::source::ByteSpan;

pub(crate) fn closure_expression_by_span(
    ast: &AstFile,
    span: ByteSpan,
) -> Option<&super::ClosureExpr> {
    let mut found = None;
    visit_file_expressions(ast, &mut |expression| {
        if let Expr::Closure(closure) = expression
            && closure.span == span
        {
            found = Some(closure);
        }
    });
    found
}

pub(crate) fn visit_file_expressions<'a>(ast: &'a AstFile, visitor: &mut impl FnMut(&'a Expr)) {
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                if let Some(body) = &function.body {
                    visit_block_expressions(body, visitor);
                }
            }
            Item::Test(test) => visit_block_expressions(&test.body, visitor),
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
            Item::Interface(interface) => {
                for method in &interface.methods {
                    if let Some(body) = &method.body {
                        visit_block_expressions(body, visitor);
                    }
                }
            }
            Item::Construct(construct) => {
                for member in &construct.members {
                    match &member.declaration {
                        ConstructMemberDecl::Function(function) => {
                            if let Some(body) = &function.body {
                                visit_block_expressions(body, visitor);
                            }
                        }
                        ConstructMemberDecl::Literal(literal) => {
                            if let Some(body) = &literal.body {
                                visit_block_expressions(body, visitor);
                            }
                        }
                    }
                }
            }
            Item::Coerce(coerce) => {
                for entry in &coerce.entries {
                    if let Some(body) = &entry.body {
                        visit_block_expressions(body, visitor);
                    }
                }
            }
            Item::Import(_)
            | Item::FromImport(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_) => {}
        }
    }
}

fn visit_block_expressions<'a>(block: &'a Block, visitor: &mut impl FnMut(&'a Expr)) {
    visit_block_expressions_with_closure_policy(block, visitor, true);
}

/// Visits expressions owned by one callable body while treating nested closure
/// bodies as separate callable boundaries.
pub(crate) fn visit_block_expressions_without_nested_closures<'a>(
    block: &'a Block,
    visitor: &mut impl FnMut(&'a Expr),
) {
    visit_block_expressions_with_closure_policy(block, visitor, false);
}

fn visit_block_expressions_with_closure_policy<'a>(
    block: &'a Block,
    visitor: &mut impl FnMut(&'a Expr),
    enter_closures: bool,
) {
    for statement in &block.statements {
        visit_statement_expressions_with_closure_policy(statement, visitor, enter_closures);
    }
    if let Some(result) = &block.result {
        visit_expression_with_closure_policy(result, visitor, enter_closures);
    }
}

fn visit_statement_expressions_with_closure_policy<'a>(
    statement: &'a Stmt,
    visitor: &mut impl FnMut(&'a Expr),
    enter_closures: bool,
) {
    match statement {
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => {}
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                visit_expression_with_closure_policy(expression, visitor, enter_closures);
            }
        }
        Stmt::Binding(statement) => {
            visit_expression_with_closure_policy(&statement.initializer, visitor, enter_closures)
        }
        Stmt::Assignment(statement) => {
            visit_expression_with_closure_policy(&statement.target, visitor, enter_closures);
            visit_expression_with_closure_policy(&statement.value, visitor, enter_closures);
        }
        Stmt::If(statement) => {
            visit_expression_with_closure_policy(&statement.condition, visitor, enter_closures);
            visit_block_expressions_with_closure_policy(
                &statement.then_block,
                visitor,
                enter_closures,
            );
            if let Some(block) = &statement.else_block {
                visit_block_expressions_with_closure_policy(block, visitor, enter_closures);
            }
        }
        Stmt::IfIs(statement) => {
            visit_expression_with_closure_policy(&statement.expression, visitor, enter_closures);
            visit_block_expressions_with_closure_policy(
                &statement.then_block,
                visitor,
                enter_closures,
            );
            if let Some(block) = &statement.else_block {
                visit_block_expressions_with_closure_policy(block, visitor, enter_closures);
            }
        }
        Stmt::Switch(statement) => {
            visit_expression_with_closure_policy(&statement.expression, visitor, enter_closures);
            for arm in &statement.arms {
                visit_block_expressions_with_closure_policy(&arm.body, visitor, enter_closures);
            }
            if let Some(arm) = &statement.wildcard_arm {
                visit_block_expressions_with_closure_policy(&arm.body, visitor, enter_closures);
            }
        }
        Stmt::ForRange(statement) => {
            visit_expression_with_closure_policy(&statement.start, visitor, enter_closures);
            visit_expression_with_closure_policy(&statement.end, visitor, enter_closures);
            visit_block_expressions_with_closure_policy(&statement.body, visitor, enter_closures);
        }
        Stmt::CollectionFor(statement) => {
            visit_expression_with_closure_policy(&statement.source, visitor, enter_closures);
            visit_block_expressions_with_closure_policy(&statement.body, visitor, enter_closures);
        }
        Stmt::LiteralPackFor(statement) => {
            visit_block_expressions_with_closure_policy(&statement.body, visitor, enter_closures)
        }
        Stmt::While(statement) => {
            visit_expression_with_closure_policy(&statement.condition, visitor, enter_closures);
            visit_block_expressions_with_closure_policy(&statement.body, visitor, enter_closures);
        }
        Stmt::Loop(statement) => {
            visit_block_expressions_with_closure_policy(&statement.body, visitor, enter_closures)
        }
        Stmt::Region(statement) => {
            visit_expression_with_closure_policy(&statement.allocator, visitor, enter_closures);
            visit_block_expressions_with_closure_policy(&statement.body, visitor, enter_closures);
        }
        Stmt::Expression(statement) => {
            visit_expression_with_closure_policy(&statement.expression, visitor, enter_closures)
        }
    }
}

pub(crate) fn visit_expression<'a>(expression: &'a Expr, visitor: &mut impl FnMut(&'a Expr)) {
    visit_expression_with_closure_policy(expression, visitor, true);
}

fn visit_expression_with_closure_policy<'a>(
    expression: &'a Expr,
    visitor: &mut impl FnMut(&'a Expr),
    enter_closures: bool,
) {
    visitor(expression);
    match expression {
        Expr::Closure(expression) if enter_closures => {
            visit_block_expressions_with_closure_policy(&expression.body, visitor, enter_closures)
        }
        Expr::Closure(_) => {}
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
        Expr::Unary(expression) => {
            visit_expression_with_closure_policy(&expression.operand, visitor, enter_closures)
        }
        Expr::Binary(expression) => {
            visit_expression_with_closure_policy(&expression.left, visitor, enter_closures);
            visit_expression_with_closure_policy(&expression.right, visitor, enter_closures);
        }
        Expr::TypeConversion(expression) => {
            visit_expression_with_closure_policy(&expression.expression, visitor, enter_closures)
        }
        Expr::Propagate(expression) => {
            visit_expression_with_closure_policy(&expression.expression, visitor, enter_closures)
        }
        Expr::Force(expression) => {
            visit_expression_with_closure_policy(&expression.expression, visitor, enter_closures)
        }
        Expr::Catch(expression) => {
            visit_expression_with_closure_policy(&expression.expression, visitor, enter_closures);
            visit_block_expressions_with_closure_policy(
                &expression.catch_block,
                visitor,
                enter_closures,
            );
        }
        Expr::Borrow(expression) => {
            visit_expression_with_closure_policy(&expression.expression, visitor, enter_closures)
        }
        Expr::Call(expression) => {
            visit_expression_with_closure_policy(&expression.callee, visitor, enter_closures);
            for argument in &expression.arguments {
                visit_expression_with_closure_policy(argument, visitor, enter_closures);
            }
        }
        Expr::Member(expression) => {
            visit_expression_with_closure_policy(&expression.object, visitor, enter_closures)
        }
        Expr::Index(expression) => {
            visit_expression_with_closure_policy(&expression.object, visitor, enter_closures);
            visit_expression_with_closure_policy(&expression.index, visitor, enter_closures);
        }
        Expr::Group(expression) => {
            visit_expression_with_closure_policy(&expression.expression, visitor, enter_closures)
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                visit_expression_with_closure_policy(element, visitor, enter_closures);
            }
        }
        Expr::TypedSequenceLiteral(expression) => {
            for element in &expression.elements {
                visit_expression_with_closure_policy(element, visitor, enter_closures);
            }
            if let Some(using) = &expression.using {
                visit_expression_with_closure_policy(&using.allocator, visitor, enter_closures);
            }
        }
        Expr::TypedStringLiteral(expression) => {
            if let Some(using) = &expression.using {
                visit_expression_with_closure_policy(&using.allocator, visitor, enter_closures);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                visit_expression_with_closure_policy(&field.value, visitor, enter_closures);
            }
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    visit_expression_with_closure_policy(&part.expression, visitor, enter_closures);
                }
            }
        }
        Expr::Otherwise(expression) => {
            visit_expression_with_closure_policy(&expression.value, visitor, enter_closures);
            visit_block_expressions_with_closure_policy(
                &expression.fallback,
                visitor,
                enter_closures,
            );
        }
        Expr::If(expression) => {
            visit_expression_with_closure_policy(&expression.condition, visitor, enter_closures);
            visit_block_expressions_with_closure_policy(
                &expression.then_block,
                visitor,
                enter_closures,
            );
            if let Some(block) = &expression.else_block {
                visit_block_expressions_with_closure_policy(block, visitor, enter_closures);
            }
        }
        Expr::IfIs(expression) => {
            visit_expression_with_closure_policy(&expression.expression, visitor, enter_closures);
            visit_block_expressions_with_closure_policy(
                &expression.then_block,
                visitor,
                enter_closures,
            );
            if let Some(block) = &expression.else_block {
                visit_block_expressions_with_closure_policy(block, visitor, enter_closures);
            }
        }
        Expr::Match(expression) => {
            visit_expression_with_closure_policy(&expression.expression, visitor, enter_closures);
            for arm in &expression.arms {
                visit_block_expressions_with_closure_policy(&arm.body, visitor, enter_closures);
            }
            if let Some(arm) = &expression.wildcard_arm {
                visit_block_expressions_with_closure_policy(&arm.body, visitor, enter_closures);
            }
        }
    }
}
