//! Lexical-region facts derived from resolved AST and typecheck facts.

use super::FileAnalysis;
use crate::ast::{Block, Expr, InterpolatedStringPart, Item, Stmt};
use crate::source::{ByteSpan, SourceMap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegionAnalysisFact {
    pub(crate) declaration: ByteSpan,
    pub(crate) name: String,
    pub(crate) parent: Option<ByteSpan>,
    pub(crate) allocator: ByteSpan,
    pub(crate) allocator_type: Option<String>,
}

pub(crate) fn region_fact_for_declaration(
    file: &FileAnalysis,
    declaration: ByteSpan,
) -> Option<RegionAnalysisFact> {
    region_facts(file)
        .into_iter()
        .find(|fact| fact.declaration == declaration)
}

pub(crate) fn region_facts(file: &FileAnalysis) -> Vec<RegionAnalysisFact> {
    let mut facts = Vec::new();
    for item in &file.ast.items {
        match item {
            Item::Function(function) => collect_block(file, &function.body, None, &mut facts),
            Item::Test(test) => collect_block(file, &test.body, None, &mut facts),
            Item::Impl(impl_) => {
                for member in &impl_.members {
                    match member {
                        crate::ast::ImplMember::Method(method) => {
                            if let Some(body) = &method.body {
                                collect_block(file, body, None, &mut facts);
                            }
                        }
                        crate::ast::ImplMember::Drop(drop_) => {
                            collect_block(file, &drop_.body, None, &mut facts)
                        }
                    }
                }
            }
            Item::Interface(interface) => {
                for method in &interface.methods {
                    if let Some(body) = &method.body {
                        collect_block(file, body, None, &mut facts);
                    }
                }
            }
            Item::Construct(construct) => {
                for (_, function) in construct.functions() {
                    collect_block(file, &function.body, None, &mut facts);
                }
                for (_, literal) in construct.literals() {
                    collect_block(file, &literal.body, None, &mut facts);
                }
            }
            Item::Coerce(coerce) => {
                for entry in &coerce.entries {
                    collect_block(file, &entry.body, None, &mut facts);
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
    facts
}

pub(crate) fn region_markdown(
    sources: &SourceMap,
    file: &FileAnalysis,
    declaration: ByteSpan,
) -> Option<String> {
    let fact = region_fact_for_declaration(file, declaration)?;
    let allocator = sources
        .get(fact.allocator.source)
        .and_then(|source| source.text().get(fact.allocator.start..fact.allocator.end))
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .unwrap_or("allocator");
    let parent = fact
        .parent
        .and_then(|parent| region_fact_for_declaration(file, parent))
        .map(|parent| format!("region `{}`", parent.name))
        .unwrap_or_else(|| "the root allocation context".to_string());
    let allocator_type = fact
        .allocator_type
        .as_deref()
        .map(|ty| format!(" ({ty})"))
        .unwrap_or_default();

    Some(format!(
        "**Allocation context:** lexical region `{}` using `{allocator}`{allocator_type}; parent is {parent}. Its owned allocations are released when the region exits.",
        fact.name
    ))
}

fn collect_block(
    file: &FileAnalysis,
    block: &Block,
    parent: Option<ByteSpan>,
    facts: &mut Vec<RegionAnalysisFact>,
) {
    for statement in &block.statements {
        collect_statement(file, statement, parent, facts);
    }
    if let Some(result) = &block.result {
        collect_expression(file, result, parent, facts);
    }
}

fn collect_statement(
    file: &FileAnalysis,
    statement: &Stmt,
    parent: Option<ByteSpan>,
    facts: &mut Vec<RegionAnalysisFact>,
) {
    match statement {
        Stmt::Region(statement) => {
            collect_expression(file, &statement.allocator, parent, facts);
            let declaration = statement.name_span;
            facts.push(RegionAnalysisFact {
                declaration,
                name: statement.name.clone(),
                parent,
                allocator: statement.allocator.span(),
                allocator_type: file
                    .typecheck_facts
                    .binding_type_label(declaration)
                    .map(str::to_string),
            });
            collect_block(file, &statement.body, Some(declaration), facts);
        }
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression(file, expression, parent, facts);
            }
        }
        Stmt::Binding(statement) => collect_expression(file, &statement.initializer, parent, facts),
        Stmt::Assignment(statement) => {
            collect_expression(file, &statement.target, parent, facts);
            collect_expression(file, &statement.value, parent, facts);
        }
        Stmt::If(statement) => {
            collect_expression(file, &statement.condition, parent, facts);
            collect_block(file, &statement.then_block, parent, facts);
            if let Some(block) = &statement.else_block {
                collect_block(file, block, parent, facts);
            }
        }
        Stmt::IfIs(statement) => {
            collect_expression(file, &statement.expression, parent, facts);
            collect_block(file, &statement.then_block, parent, facts);
            if let Some(block) = &statement.else_block {
                collect_block(file, block, parent, facts);
            }
        }
        Stmt::Switch(statement) => {
            collect_expression(file, &statement.expression, parent, facts);
            for arm in &statement.arms {
                collect_block(file, &arm.body, parent, facts);
            }
            if let Some(arm) = &statement.wildcard_arm {
                collect_block(file, &arm.body, parent, facts);
            }
        }
        Stmt::ForRange(statement) => {
            collect_expression(file, &statement.start, parent, facts);
            collect_expression(file, &statement.end, parent, facts);
            collect_block(file, &statement.body, parent, facts);
        }
        Stmt::CollectionFor(statement) => {
            collect_expression(file, &statement.source, parent, facts);
            collect_block(file, &statement.body, parent, facts);
        }
        Stmt::LiteralPackFor(statement) => {
            collect_block(file, &statement.body, parent, facts);
        }
        Stmt::While(statement) => {
            collect_expression(file, &statement.condition, parent, facts);
            collect_block(file, &statement.body, parent, facts);
        }
        Stmt::Loop(statement) => collect_block(file, &statement.body, parent, facts),
        Stmt::Expression(statement) => {
            collect_expression(file, &statement.expression, parent, facts)
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => {}
    }
}

fn collect_expression(
    file: &FileAnalysis,
    expression: &Expr,
    parent: Option<ByteSpan>,
    facts: &mut Vec<RegionAnalysisFact>,
) {
    match expression {
        Expr::Closure(expression) => collect_block(file, &expression.body, parent, facts),
        Expr::Catch(expression) => {
            collect_expression(file, &expression.expression, parent, facts);
            collect_block(file, &expression.catch_block, parent, facts);
        }
        Expr::Otherwise(expression) => {
            collect_expression(file, &expression.value, parent, facts);
            collect_block(file, &expression.fallback, parent, facts);
        }
        Expr::If(expression) => {
            collect_expression(file, &expression.condition, parent, facts);
            collect_block(file, &expression.then_block, parent, facts);
            if let Some(block) = &expression.else_block {
                collect_block(file, block, parent, facts);
            }
        }
        Expr::IfIs(expression) => {
            collect_expression(file, &expression.expression, parent, facts);
            collect_block(file, &expression.then_block, parent, facts);
            if let Some(block) = &expression.else_block {
                collect_block(file, block, parent, facts);
            }
        }
        Expr::Match(expression) => {
            collect_expression(file, &expression.expression, parent, facts);
            for arm in &expression.arms {
                collect_block(file, &arm.body, parent, facts);
            }
            if let Some(arm) = &expression.wildcard_arm {
                collect_block(file, &arm.body, parent, facts);
            }
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    collect_expression(file, &part.expression, parent, facts);
                }
            }
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_expression(file, element, parent, facts);
            }
        }
        Expr::TypedSequenceLiteral(expression) => {
            for element in &expression.elements {
                collect_expression(file, element, parent, facts);
            }
            if let Some(using) = &expression.using {
                collect_expression(file, &using.allocator, parent, facts);
            }
        }
        Expr::TypedStringLiteral(expression) => {
            if let Some(using) = &expression.using {
                collect_expression(file, &using.allocator, parent, facts);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_expression(file, &field.value, parent, facts);
            }
        }
        Expr::Propagate(expression) => {
            collect_expression(file, &expression.expression, parent, facts)
        }
        Expr::Force(expression) => collect_expression(file, &expression.expression, parent, facts),
        Expr::Borrow(expression) => collect_expression(file, &expression.expression, parent, facts),
        Expr::Unary(expression) => collect_expression(file, &expression.operand, parent, facts),
        Expr::Binary(expression) => {
            collect_expression(file, &expression.left, parent, facts);
            collect_expression(file, &expression.right, parent, facts);
        }
        Expr::TypeConversion(expression) => {
            collect_expression(file, &expression.expression, parent, facts)
        }
        Expr::Call(expression) => {
            collect_expression(file, &expression.callee, parent, facts);
            for argument in &expression.arguments {
                collect_expression(file, argument, parent, facts);
            }
        }
        Expr::Member(expression) => collect_expression(file, &expression.object, parent, facts),
        Expr::Index(expression) => {
            collect_expression(file, &expression.object, parent, facts);
            collect_expression(file, &expression.index, parent, facts);
        }
        Expr::Group(expression) => collect_expression(file, &expression.expression, parent, facts),
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}
