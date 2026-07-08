use super::*;
use crate::comments::{AttachedDocumentation, DocumentationTarget, attach_documentation};
use crate::source::{ByteSpan, SourceMap};

pub(super) fn collect_ast_documentation(
    ast: &AstFile,
    sources: &SourceMap,
) -> AttachedDocumentation {
    let Some(file) = sources.get(ast.span.source) else {
        return AttachedDocumentation::default();
    };

    let text = file.text();
    let mut targets = Vec::new();
    for item in &ast.items {
        collect_item_targets(text, item, &mut targets);
    }

    attach_documentation(ast.span.source, text, &targets)
}

fn collect_item_targets(text: &str, item: &Item, targets: &mut Vec<DocumentationTarget>) {
    match item {
        Item::Use(_) | Item::Import(_) | Item::FromImport(_) => {}
        Item::Function(function) => {
            push_target(text, function.span, targets);
            collect_block_targets(text, &function.body, targets);
        }
        Item::Primitive(primitive) => push_target(text, primitive.span, targets),
        Item::TypeAlias(alias) => push_target(text, alias.span, targets),
        Item::Struct(struct_) => {
            push_target(text, struct_.span, targets);
            for field in &struct_.fields {
                push_target(text, field.span, targets);
            }
        }
        Item::Enum(enum_) => {
            push_target(text, enum_.span, targets);
            for variant in &enum_.variants {
                push_target(text, variant.span, targets);
            }
        }
        Item::Trait(trait_) => {
            push_target(text, trait_.span, targets);
            for method in &trait_.methods {
                push_target(text, method.span, targets);
            }
        }
        Item::Impl(impl_) => {
            push_target(text, impl_.span, targets);
            for member in &impl_.members {
                collect_impl_member_targets(text, member, targets);
            }
        }
    }
}

fn collect_impl_member_targets(
    text: &str,
    member: &ImplMember,
    targets: &mut Vec<DocumentationTarget>,
) {
    match member {
        ImplMember::Function(function) => {
            push_target(text, function.span, targets);
            collect_block_targets(text, &function.body, targets);
        }
        ImplMember::Method(method) => {
            push_target(text, method.span, targets);
            if let Some(body) = &method.body {
                collect_block_targets(text, body, targets);
            }
        }
    }
}

fn collect_block_targets(text: &str, block: &Block, targets: &mut Vec<DocumentationTarget>) {
    for statement in &block.statements {
        collect_statement_targets(text, statement, targets);
    }
}

fn collect_statement_targets(text: &str, statement: &Stmt, targets: &mut Vec<DocumentationTarget>) {
    match statement {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression_targets(text, expression, targets);
            }
        }
        Stmt::Binding(statement) => {
            push_target(text, statement.span, targets);
            collect_expression_targets(text, &statement.initializer, targets);
            if let Some(block) = &statement.else_block {
                collect_block_targets(text, block, targets);
            }
        }
        Stmt::If(statement) => {
            collect_expression_targets(text, &statement.condition, targets);
            collect_block_targets(text, &statement.then_block, targets);
            if let Some(block) = &statement.else_block {
                collect_block_targets(text, block, targets);
            }
        }
        Stmt::IfIs(statement) => {
            collect_expression_targets(text, &statement.expression, targets);
            collect_block_targets(text, &statement.then_block, targets);
            if let Some(block) = &statement.else_block {
                collect_block_targets(text, block, targets);
            }
        }
        Stmt::IfLet(statement) => {
            collect_expression_targets(text, &statement.initializer, targets);
            collect_block_targets(text, &statement.then_block, targets);
            if let Some(block) = &statement.else_block {
                collect_block_targets(text, block, targets);
            }
        }
        Stmt::Switch(statement) => {
            collect_expression_targets(text, &statement.expression, targets);
            for arm in &statement.arms {
                collect_block_targets(text, &arm.body, targets);
            }
            if let Some(arm) = &statement.else_arm {
                collect_block_targets(text, &arm.body, targets);
            }
        }
        Stmt::ForRange(statement) => {
            collect_expression_targets(text, &statement.start, targets);
            collect_expression_targets(text, &statement.end, targets);
            collect_block_targets(text, &statement.body, targets);
        }
        Stmt::While(statement) => {
            collect_expression_targets(text, &statement.condition, targets);
            collect_block_targets(text, &statement.body, targets);
        }
        Stmt::WhileLet(statement) => {
            collect_expression_targets(text, &statement.initializer, targets);
            collect_block_targets(text, &statement.body, targets);
        }
        Stmt::Loop(statement) => collect_block_targets(text, &statement.body, targets),
        Stmt::Break(_) | Stmt::Continue(_) => {}
        Stmt::Expression(statement) => {
            collect_expression_targets(text, &statement.expression, targets);
        }
    }
}

fn collect_expression_targets(
    text: &str,
    expression: &Expr,
    targets: &mut Vec<DocumentationTarget>,
) {
    match expression {
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_targets(text, element, targets);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_expression_targets(text, &field.value, targets);
            }
        }
        Expr::Propagate(expression) => {
            collect_expression_targets(text, &expression.expression, targets);
        }
        Expr::Force(expression) => {
            collect_expression_targets(text, &expression.expression, targets)
        }
        Expr::Catch(expression) => {
            collect_expression_targets(text, &expression.expression, targets);
            collect_block_targets(text, &expression.catch_block, targets);
        }
        Expr::Unary(expression) => collect_expression_targets(text, &expression.operand, targets),
        Expr::Binary(expression) => {
            collect_expression_targets(text, &expression.left, targets);
            collect_expression_targets(text, &expression.right, targets);
        }
        Expr::TypeConversion(expression) => {
            collect_expression_targets(text, &expression.expression, targets);
        }
        Expr::Call(expression) => {
            collect_expression_targets(text, &expression.callee, targets);
            for argument in &expression.arguments {
                collect_expression_targets(text, argument, targets);
            }
        }
        Expr::Member(expression) => collect_expression_targets(text, &expression.object, targets),
        Expr::Index(expression) => {
            collect_expression_targets(text, &expression.object, targets);
            collect_expression_targets(text, &expression.index, targets);
        }
        Expr::Group(expression) => {
            collect_expression_targets(text, &expression.expression, targets)
        }
        Expr::OptionalDefault(expression) => {
            collect_expression_targets(text, &expression.value, targets);
            collect_expression_targets(text, &expression.default, targets);
        }
        Expr::PatternConditional(expression) => {
            collect_expression_targets(text, &expression.target, targets);
            for arm in &expression.arms {
                collect_expression_targets(text, &arm.expression, targets);
            }
            collect_expression_targets(text, &expression.fallback, targets);
        }
    }
}

fn push_target(text: &str, span: ByteSpan, targets: &mut Vec<DocumentationTarget>) {
    targets.push(DocumentationTarget::new(
        declaration_line_start(text, span.start),
        span.start,
    ));
}

fn declaration_line_start(text: &str, node_start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut line_start = node_start.min(bytes.len());
    while line_start > 0 && bytes[line_start - 1] != b'\n' {
        line_start -= 1;
    }

    let mut start = line_start;
    while start < node_start && matches!(bytes[start], b' ' | b'\t') {
        start += 1;
    }

    start
}
