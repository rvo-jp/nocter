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
        Item::Import(_) | Item::FromImport(_) => {}
        Item::Function(function) => {
            push_target(text, function.span, targets);
            if let Some(body) = &function.body {
                collect_block_targets(text, body, targets);
            }
        }
        Item::Test(test) => {
            push_target(text, test.span, targets);
            collect_block_targets(text, &test.body, targets);
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
        Item::Interface(interface) => {
            push_target(text, interface.span, targets);
            for method in &interface.methods {
                push_target(text, method.span, targets);
                if let Some(body) = &method.body {
                    collect_block_targets(text, body, targets);
                }
            }
        }
        Item::Instance(instance) => {
            push_target(text, instance.span, targets);
            for method in instance.callable_methods() {
                push_target(text, method.span, targets);
                if let Some(body) = &method.body {
                    collect_block_targets(text, body, targets);
                }
            }
        }
        Item::Destruct(destruct) => {
            push_target(text, destruct.span, targets);
            collect_block_targets(text, &destruct.body, targets);
        }
        Item::Conformance(conformance) => {
            push_target(text, conformance.span, targets);
            for member in &conformance.members {
                match member {
                    ConformanceMember::AssociatedType(binding) => {
                        push_target(text, binding.span, targets);
                    }
                    ConformanceMember::Method(method) => {
                        push_target(text, method.span, targets);
                        if let Some(body) = &method.body {
                            collect_block_targets(text, body, targets);
                        }
                    }
                }
            }
        }
        Item::Construct(construct) => {
            push_target(text, construct.span, targets);
            for member in &construct.members {
                match &member.declaration {
                    ConstructMemberDecl::Function(function) => {
                        push_target(text, member.span, targets);
                        if let Some(body) = &function.body {
                            collect_block_targets(text, body, targets);
                        }
                    }
                    ConstructMemberDecl::Literal(literal) => {
                        push_target(text, member.span, targets);
                        if let Some(body) = &literal.body {
                            collect_block_targets(text, body, targets);
                        }
                    }
                }
            }
        }
        Item::Coerce(coerce) => {
            push_target(text, coerce.span, targets);
            for entry in &coerce.entries {
                push_target(text, entry.span, targets);
                if let Some(body) = &entry.body {
                    collect_block_targets(text, body, targets);
                }
            }
        }
    }
}

fn collect_block_targets(text: &str, block: &Block, targets: &mut Vec<DocumentationTarget>) {
    for statement in &block.statements {
        collect_statement_targets(text, statement, targets);
    }
    if let Some(result) = &block.result {
        collect_expression_targets(text, result, targets);
    }
}

fn collect_statement_targets(text: &str, statement: &Stmt, targets: &mut Vec<DocumentationTarget>) {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => {}
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression_targets(text, expression, targets);
            }
        }
        Stmt::Binding(statement) => {
            push_target(text, statement.span, targets);
            collect_expression_targets(text, &statement.initializer, targets);
        }
        Stmt::Assignment(statement) => {
            collect_expression_targets(text, &statement.target, targets);
            collect_expression_targets(text, &statement.value, targets);
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
        Stmt::Switch(statement) => {
            collect_expression_targets(text, &statement.expression, targets);
            for arm in &statement.arms {
                collect_block_targets(text, &arm.body, targets);
            }
            if let Some(arm) = &statement.wildcard_arm {
                collect_block_targets(text, &arm.body, targets);
            }
        }
        Stmt::ForRange(statement) => {
            collect_expression_targets(text, &statement.start, targets);
            collect_expression_targets(text, &statement.end, targets);
            collect_block_targets(text, &statement.body, targets);
        }
        Stmt::CollectionFor(statement) => {
            collect_expression_targets(text, &statement.source, targets);
            collect_block_targets(text, &statement.body, targets);
        }
        Stmt::LiteralPackFor(statement) => {
            collect_block_targets(text, &statement.body, targets);
        }
        Stmt::While(statement) => {
            collect_expression_targets(text, &statement.condition, targets);
            collect_block_targets(text, &statement.body, targets);
        }
        Stmt::Loop(statement) => collect_block_targets(text, &statement.body, targets),
        Stmt::Region(statement) => {
            push_target(text, statement.span, targets);
            collect_expression_targets(text, &statement.allocator, targets);
            collect_block_targets(text, &statement.body, targets);
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Drop(_) => {}
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
        Expr::Closure(expression) => collect_block_targets(text, &expression.body, targets),
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    collect_expression_targets(text, &part.expression, targets);
                }
            }
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_targets(text, element, targets);
            }
        }
        Expr::TypedSequenceLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_targets(text, element, targets);
            }
            if let Some(using) = &expression.using {
                collect_expression_targets(text, &using.allocator, targets);
            }
        }
        Expr::TypedStringLiteral(expression) => {
            if let Some(using) = &expression.using {
                collect_expression_targets(text, &using.allocator, targets);
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
        Expr::Borrow(expression) => {
            collect_expression_targets(text, &expression.expression, targets);
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
        Expr::Otherwise(expression) => {
            collect_expression_targets(text, &expression.value, targets);
            collect_block_targets(text, &expression.fallback, targets);
        }
        Expr::If(expression) => {
            collect_expression_targets(text, &expression.condition, targets);
            collect_block_targets(text, &expression.then_block, targets);
            if let Some(block) = &expression.else_block {
                collect_block_targets(text, block, targets);
            }
        }
        Expr::IfIs(expression) => {
            collect_expression_targets(text, &expression.expression, targets);
            collect_block_targets(text, &expression.then_block, targets);
            if let Some(block) = &expression.else_block {
                collect_block_targets(text, block, targets);
            }
        }
        Expr::Match(expression) => {
            collect_expression_targets(text, &expression.expression, targets);
            for arm in &expression.arms {
                collect_block_targets(text, &arm.body, targets);
            }
            if let Some(arm) = &expression.wildcard_arm {
                collect_block_targets(text, &arm.body, targets);
            }
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
