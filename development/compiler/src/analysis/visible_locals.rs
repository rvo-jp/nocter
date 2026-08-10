//! Lexically visible local declarations at an editor cursor.

use crate::ast::{AstFile, Block, ConformanceMember, Expr, Item, MethodDecl, Stmt};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VisibleLocalBinding {
    pub(super) name: String,
    pub(super) name_span: ByteSpan,
    pub(super) kind: &'static str,
}

pub(super) fn visible_local_bindings_at_offset(
    ast: &AstFile,
    offset: usize,
) -> Vec<VisibleLocalBinding> {
    let mut locals = Vec::new();
    for item in &ast.items {
        match item {
            Item::Function(function)
                if function
                    .body
                    .as_ref()
                    .is_some_and(|body| contains(body.span, offset)) =>
            {
                for parameter in &function.parameters.parameters {
                    define(
                        &mut locals,
                        &parameter.name,
                        parameter.name_span,
                        "parameter",
                    );
                }
                collect_block(
                    function.body.as_ref().expect("guarded function body"),
                    offset,
                    &mut locals,
                );
                return locals;
            }
            Item::Instance(instance) => {
                for method in &instance.methods {
                    if collect_method(method, offset, &mut locals) {
                        return locals;
                    }
                }
            }
            Item::Destruct(destruct) if contains(destruct.body.span, offset) => {
                define(
                    &mut locals,
                    &destruct.binding.name,
                    destruct.binding.name_span,
                    "parameter",
                );
                collect_block(&destruct.body, offset, &mut locals);
                return locals;
            }
            Item::Conformance(conformance) => {
                for member in &conformance.members {
                    if let ConformanceMember::Method(method) = member
                        && collect_method(method, offset, &mut locals)
                    {
                        return locals;
                    }
                }
            }
            _ => {}
        }
    }
    locals
}

fn collect_method(
    method: &MethodDecl,
    offset: usize,
    locals: &mut Vec<VisibleLocalBinding>,
) -> bool {
    let Some(body) = &method.body else {
        return false;
    };
    if !contains(body.span, offset) {
        return false;
    }
    define(
        locals,
        &method.receiver.name,
        method.receiver.name_span,
        "parameter",
    );
    for parameter in &method.parameters.parameters {
        define(locals, &parameter.name, parameter.name_span, "parameter");
    }
    collect_block(body, offset, locals);
    true
}

fn collect_block(block: &Block, offset: usize, locals: &mut Vec<VisibleLocalBinding>) {
    for statement in &block.statements {
        let span = statement.span();
        if span.start >= offset {
            return;
        }
        if contains(span, offset) {
            collect_statement_scope(statement, offset, locals);
            return;
        }
        if let Stmt::Binding(binding) = statement {
            define(
                locals,
                &binding.name,
                binding.name_span,
                match binding.kind {
                    crate::ast::BindingKind::Let => "let",
                    crate::ast::BindingKind::Var => "var",
                },
            );
        }
    }

    if let Some(result) = &block.result
        && contains(result.span(), offset)
    {
        collect_expression_scope(result, offset, locals);
    }
}

fn collect_statement_scope(statement: &Stmt, offset: usize, locals: &mut Vec<VisibleLocalBinding>) {
    match statement {
        Stmt::If(statement) => {
            collect_matching_block(&statement.then_block, offset, locals);
            if let Some(block) = &statement.else_block {
                collect_matching_block(block, offset, locals);
            }
        }
        Stmt::IfIs(statement) => {
            if contains(statement.then_block.span, offset) {
                define_payload(locals, statement.payload.as_ref());
                collect_block(&statement.then_block, offset, locals);
            } else if let Some(block) = &statement.else_block {
                collect_matching_block(block, offset, locals);
            }
        }
        Stmt::Switch(statement) => {
            for arm in &statement.arms {
                if contains(arm.body.span, offset) {
                    define_payload(locals, arm.payload.as_ref());
                    collect_block(&arm.body, offset, locals);
                    return;
                }
            }
            if let Some(arm) = &statement.wildcard_arm {
                collect_matching_block(&arm.body, offset, locals);
            }
        }
        Stmt::ForRange(statement) if contains(statement.body.span, offset) => {
            define(locals, &statement.name, statement.name_span, "range");
            collect_block(&statement.body, offset, locals);
        }
        Stmt::CollectionFor(statement) if contains(statement.body.span, offset) => {
            define(
                locals,
                &statement.name,
                statement.name_span,
                "collection element",
            );
            collect_block(&statement.body, offset, locals);
        }
        Stmt::LiteralPackFor(statement) if contains(statement.body.span, offset) => {
            define(
                locals,
                &statement.name,
                statement.name_span,
                "literal pack element",
            );
            collect_block(&statement.body, offset, locals);
        }
        Stmt::While(statement) => collect_matching_block(&statement.body, offset, locals),
        Stmt::Loop(statement) => collect_matching_block(&statement.body, offset, locals),
        Stmt::Region(statement) if contains(statement.body.span, offset) => {
            define(locals, &statement.name, statement.name_span, "region");
            collect_block(&statement.body, offset, locals);
        }
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression_scope(expression, offset, locals);
            }
        }
        Stmt::Binding(statement) => {
            collect_expression_scope(&statement.initializer, offset, locals)
        }
        Stmt::Assignment(statement) => {
            collect_expression_scope(&statement.target, offset, locals);
            collect_expression_scope(&statement.value, offset, locals);
        }
        Stmt::Expression(statement) => {
            collect_expression_scope(&statement.expression, offset, locals)
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_)
        | Stmt::Region(_)
        | Stmt::ForRange(_)
        | Stmt::CollectionFor(_)
        | Stmt::LiteralPackFor(_) => {}
    }
}

fn collect_expression_scope(
    expression: &Expr,
    offset: usize,
    locals: &mut Vec<VisibleLocalBinding>,
) {
    if !contains(expression.span(), offset) {
        return;
    }
    match expression {
        Expr::Closure(expression) => {
            if contains(expression.body.span, offset) {
                locals.clear();
                for capture in &expression.captures {
                    define(locals, &capture.name, capture.name_span, "capture");
                }
                for parameter in &expression.parameters {
                    define(locals, &parameter.name, parameter.name_span, "parameter");
                }
                collect_block(&expression.body, offset, locals);
            }
        }
        Expr::Catch(expression) => {
            if contains(expression.catch_block.span, offset) {
                define(
                    locals,
                    &expression.error_name,
                    expression.error_span,
                    "error",
                );
                collect_block(&expression.catch_block, offset, locals);
            } else {
                collect_expression_scope(&expression.expression, offset, locals);
            }
        }
        Expr::Otherwise(expression) => {
            if contains(expression.fallback.span, offset) {
                collect_block(&expression.fallback, offset, locals);
            } else {
                collect_expression_scope(&expression.value, offset, locals);
            }
        }
        Expr::If(expression) => {
            if contains(expression.then_block.span, offset) {
                collect_block(&expression.then_block, offset, locals);
            } else if let Some(block) = &expression.else_block
                && contains(block.span, offset)
            {
                collect_block(block, offset, locals);
            } else {
                collect_expression_scope(&expression.condition, offset, locals);
            }
        }
        Expr::IfIs(expression) => {
            if contains(expression.then_block.span, offset) {
                define_payload(locals, expression.payload.as_ref());
                collect_block(&expression.then_block, offset, locals);
            } else if let Some(block) = &expression.else_block
                && contains(block.span, offset)
            {
                collect_block(block, offset, locals);
            } else {
                collect_expression_scope(&expression.expression, offset, locals);
            }
        }
        Expr::Match(expression) => {
            for arm in &expression.arms {
                if contains(arm.body.span, offset) {
                    define_payload(locals, arm.payload.as_ref());
                    collect_block(&arm.body, offset, locals);
                    return;
                }
            }
            if let Some(arm) = &expression.wildcard_arm
                && contains(arm.body.span, offset)
            {
                collect_block(&arm.body, offset, locals);
            } else {
                collect_expression_scope(&expression.expression, offset, locals);
            }
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                    collect_expression_scope(&part.expression, offset, locals);
                }
            }
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_scope(element, offset, locals);
            }
        }
        Expr::TypedSequenceLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_scope(element, offset, locals);
            }
            if let Some(using) = &expression.using {
                collect_expression_scope(&using.allocator, offset, locals);
            }
        }
        Expr::TypedStringLiteral(expression) => {
            if let Some(using) = &expression.using {
                collect_expression_scope(&using.allocator, offset, locals);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_expression_scope(&field.value, offset, locals);
            }
        }
        Expr::Propagate(expression) => {
            collect_expression_scope(&expression.expression, offset, locals)
        }
        Expr::Force(expression) => collect_expression_scope(&expression.expression, offset, locals),
        Expr::Borrow(expression) => {
            collect_expression_scope(&expression.expression, offset, locals)
        }
        Expr::Unary(expression) => collect_expression_scope(&expression.operand, offset, locals),
        Expr::Binary(expression) => {
            collect_expression_scope(&expression.left, offset, locals);
            collect_expression_scope(&expression.right, offset, locals);
        }
        Expr::TypeConversion(expression) => {
            collect_expression_scope(&expression.expression, offset, locals)
        }
        Expr::Call(expression) => {
            collect_expression_scope(&expression.callee, offset, locals);
            for argument in &expression.arguments {
                collect_expression_scope(argument, offset, locals);
            }
        }
        Expr::Member(expression) => collect_expression_scope(&expression.object, offset, locals),
        Expr::Index(expression) => {
            collect_expression_scope(&expression.object, offset, locals);
            collect_expression_scope(&expression.index, offset, locals);
        }
        Expr::Group(expression) => collect_expression_scope(&expression.expression, offset, locals),
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn define_payload(
    locals: &mut Vec<VisibleLocalBinding>,
    payload: Option<&crate::ast::SwitchPayloadPattern>,
) {
    if let Some(binding) = payload.and_then(|payload| payload.binding()) {
        define(locals, &binding.name, binding.span, "payload");
    }
}

fn collect_matching_block(block: &Block, offset: usize, locals: &mut Vec<VisibleLocalBinding>) {
    if contains(block.span, offset) {
        collect_block(block, offset, locals);
    }
}

fn define(
    locals: &mut Vec<VisibleLocalBinding>,
    name: &str,
    name_span: ByteSpan,
    kind: &'static str,
) {
    locals.retain(|local| local.name != name);
    locals.push(VisibleLocalBinding {
        name: name.to_string(),
        name_span,
        kind,
    });
}

fn contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset <= span.end
}
