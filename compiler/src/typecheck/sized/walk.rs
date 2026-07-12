use crate::ast::{Block, Expr, InterpolatedStringPart, Stmt};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

use super::super::model::Type;
use super::value::check_value_type;

pub(in crate::typecheck::sized) fn check_block(
    sources: &SourceMap,
    block: &Block,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        check_statement(sources, statement, resolved, self_type, diagnostics);
    }
}

fn check_statement(
    sources: &SourceMap,
    statement: &Stmt,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Stmt::Binding(statement) => {
            if let Some(ty) = &statement.ty {
                check_value_type(
                    sources,
                    ty,
                    &format!("binding `{}` annotation", statement.name),
                    resolved,
                    self_type,
                    diagnostics,
                );
            }
            check_expression(
                sources,
                &statement.initializer,
                resolved,
                self_type,
                diagnostics,
            );
            if let Some(else_block) = &statement.else_block {
                check_block(sources, else_block, resolved, self_type, diagnostics);
            }
        }
        Stmt::Assignment(statement) => {
            check_expression(sources, &statement.target, resolved, self_type, diagnostics);
            check_expression(sources, &statement.value, resolved, self_type, diagnostics);
        }
        Stmt::If(statement) => {
            check_expression(
                sources,
                &statement.condition,
                resolved,
                self_type,
                diagnostics,
            );
            check_block(
                sources,
                &statement.then_block,
                resolved,
                self_type,
                diagnostics,
            );
            if let Some(else_block) = &statement.else_block {
                check_block(sources, else_block, resolved, self_type, diagnostics);
            }
        }
        Stmt::IfIs(statement) => {
            check_expression(
                sources,
                &statement.expression,
                resolved,
                self_type,
                diagnostics,
            );
            check_block(
                sources,
                &statement.then_block,
                resolved,
                self_type,
                diagnostics,
            );
            if let Some(else_block) = &statement.else_block {
                check_block(sources, else_block, resolved, self_type, diagnostics);
            }
        }
        Stmt::IfLet(statement) => {
            check_expression(
                sources,
                &statement.initializer,
                resolved,
                self_type,
                diagnostics,
            );
            check_block(
                sources,
                &statement.then_block,
                resolved,
                self_type,
                diagnostics,
            );
            if let Some(else_block) = &statement.else_block {
                check_block(sources, else_block, resolved, self_type, diagnostics);
            }
        }
        Stmt::Switch(statement) => {
            check_expression(
                sources,
                &statement.expression,
                resolved,
                self_type,
                diagnostics,
            );
            for arm in &statement.arms {
                check_block(sources, &arm.body, resolved, self_type, diagnostics);
            }
            if let Some(else_arm) = &statement.else_arm {
                check_block(sources, &else_arm.body, resolved, self_type, diagnostics);
            }
        }
        Stmt::ForRange(statement) => {
            check_expression(sources, &statement.start, resolved, self_type, diagnostics);
            check_expression(sources, &statement.end, resolved, self_type, diagnostics);
            check_block(sources, &statement.body, resolved, self_type, diagnostics);
        }
        Stmt::While(statement) => {
            check_expression(
                sources,
                &statement.condition,
                resolved,
                self_type,
                diagnostics,
            );
            check_block(sources, &statement.body, resolved, self_type, diagnostics);
        }
        Stmt::WhileLet(statement) => {
            check_expression(
                sources,
                &statement.initializer,
                resolved,
                self_type,
                diagnostics,
            );
            check_block(sources, &statement.body, resolved, self_type, diagnostics);
        }
        Stmt::Loop(statement) => {
            check_block(sources, &statement.body, resolved, self_type, diagnostics);
        }
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                check_expression(sources, expression, resolved, self_type, diagnostics);
            }
        }
        Stmt::Expression(statement) => {
            check_expression(
                sources,
                &statement.expression,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Drop(_) => {}
    }
}

fn check_expression(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    check_expression(sources, &part.expression, resolved, self_type, diagnostics);
                }
            }
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                check_expression(sources, element, resolved, self_type, diagnostics);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                check_expression(sources, &field.value, resolved, self_type, diagnostics);
            }
        }
        Expr::Propagate(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Force(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Catch(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                self_type,
                diagnostics,
            );
            check_block(
                sources,
                &expression.catch_block,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Borrow(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Unary(expression) => {
            check_expression(
                sources,
                &expression.operand,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Binary(expression) => {
            check_expression(sources, &expression.left, resolved, self_type, diagnostics);
            check_expression(sources, &expression.right, resolved, self_type, diagnostics);
        }
        Expr::TypeConversion(expression) => {
            check_value_type(
                sources,
                &expression.ty,
                "type conversion target",
                resolved,
                self_type,
                diagnostics,
            );
            check_expression(
                sources,
                &expression.expression,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Call(expression) => {
            check_expression(
                sources,
                &expression.callee,
                resolved,
                self_type,
                diagnostics,
            );
            for argument in &expression.arguments {
                check_expression(sources, argument, resolved, self_type, diagnostics);
            }
        }
        Expr::Member(expression) => {
            check_expression(
                sources,
                &expression.object,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Index(expression) => {
            check_expression(
                sources,
                &expression.object,
                resolved,
                self_type,
                diagnostics,
            );
            check_expression(sources, &expression.index, resolved, self_type, diagnostics);
        }
        Expr::Group(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::OptionalDefault(expression) => {
            check_expression(sources, &expression.value, resolved, self_type, diagnostics);
            check_expression(
                sources,
                &expression.default,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::PatternConditional(expression) => {
            check_expression(
                sources,
                &expression.target,
                resolved,
                self_type,
                diagnostics,
            );
            for arm in &expression.arms {
                check_expression(sources, &arm.expression, resolved, self_type, diagnostics);
            }
            check_expression(
                sources,
                &expression.fallback,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}
