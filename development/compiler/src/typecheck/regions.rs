use super::diagnostics::region_allocator_not_place_diagnostic;
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use super::places::expression_is_established_place;
use super::provenance::{LexicalRegionTree, RegionId};
use crate::ast::{
    AstFile, Block, ConformanceMember, Expr, InterpolatedStringPart, Item, RegionStmt, Stmt,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

pub(super) fn region_id(statement: &RegionStmt) -> RegionId {
    RegionId::declared_at(statement.name_span)
}

pub(super) fn region_binding_type(
    statement: &RegionStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    expression_type(&statement.allocator, resolved, environment)
}

pub(super) fn check_region_statements(
    sources: &SourceMap,
    ast: &AstFile,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut tree = LexicalRegionTree::default();
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                if let Some(body) = &function.body {
                    check_block(sources, body, None, &mut tree, diagnostics);
                }
            }
            Item::Test(test) => check_block(sources, &test.body, None, &mut tree, diagnostics),
            Item::Instance(instance) => {
                for method in instance.callable_methods() {
                    if let Some(body) = &method.body {
                        check_block(sources, body, None, &mut tree, diagnostics);
                    }
                }
            }
            Item::Destruct(destruct) => {
                check_block(sources, &destruct.body, None, &mut tree, diagnostics)
            }
            Item::Conformance(conformance) => {
                for member in &conformance.members {
                    if let ConformanceMember::Method(method) = member
                        && let Some(body) = &method.body
                    {
                        check_block(sources, body, None, &mut tree, diagnostics);
                    }
                }
            }
            Item::Interface(interface) => {
                for method in &interface.methods {
                    if let Some(body) = &method.body {
                        check_block(sources, body, None, &mut tree, diagnostics);
                    }
                }
            }
            Item::Import(_)
            | Item::FromImport(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_) => {}
            Item::Construct(construct) => {
                for (_, function) in construct.functions() {
                    if let Some(body) = &function.body {
                        check_block(sources, body, None, &mut tree, diagnostics);
                    }
                }
                for (_, literal) in construct.literals() {
                    if let Some(body) = &literal.body {
                        check_block(sources, body, None, &mut tree, diagnostics);
                    }
                }
            }
        }
    }
}

fn check_block(
    sources: &SourceMap,
    block: &Block,
    parent: Option<RegionId>,
    tree: &mut LexicalRegionTree,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        check_statement(sources, statement, parent, tree, diagnostics);
    }
    if let Some(result) = &block.result {
        check_expression_blocks(sources, result, parent, tree, diagnostics);
    }
}

fn check_statement(
    sources: &SourceMap,
    statement: &Stmt,
    parent: Option<RegionId>,
    tree: &mut LexicalRegionTree,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Stmt::Region(statement) => {
            check_expression_blocks(sources, &statement.allocator, parent, tree, diagnostics);
            if !expression_is_established_place(&statement.allocator) {
                diagnostics.push(region_allocator_not_place_diagnostic(
                    sources,
                    statement.allocator.span(),
                ));
            }
            let id = region_id(statement);
            tree.define(id, parent);
            if let Some(parent) = parent {
                debug_assert!(tree.is_same_or_nested_within(id, parent));
            }
            check_block(sources, &statement.body, Some(id), tree, diagnostics);
        }
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                check_expression_blocks(sources, expression, parent, tree, diagnostics);
            }
        }
        Stmt::Binding(statement) => {
            check_expression_blocks(sources, &statement.initializer, parent, tree, diagnostics)
        }
        Stmt::Assignment(statement) => {
            check_expression_blocks(sources, &statement.target, parent, tree, diagnostics);
            check_expression_blocks(sources, &statement.value, parent, tree, diagnostics);
        }
        Stmt::If(statement) => {
            check_expression_blocks(sources, &statement.condition, parent, tree, diagnostics);
            check_block(sources, &statement.then_block, parent, tree, diagnostics);
            if let Some(block) = &statement.else_block {
                check_block(sources, block, parent, tree, diagnostics);
            }
        }
        Stmt::IfIs(statement) => {
            check_expression_blocks(sources, &statement.expression, parent, tree, diagnostics);
            check_block(sources, &statement.then_block, parent, tree, diagnostics);
            if let Some(block) = &statement.else_block {
                check_block(sources, block, parent, tree, diagnostics);
            }
        }
        Stmt::Switch(statement) => {
            check_expression_blocks(sources, &statement.expression, parent, tree, diagnostics);
            for arm in &statement.arms {
                check_block(sources, &arm.body, parent, tree, diagnostics);
            }
            if let Some(arm) = &statement.wildcard_arm {
                check_block(sources, &arm.body, parent, tree, diagnostics);
            }
        }
        Stmt::ForRange(statement) => {
            check_expression_blocks(sources, &statement.start, parent, tree, diagnostics);
            check_expression_blocks(sources, &statement.end, parent, tree, diagnostics);
            check_block(sources, &statement.body, parent, tree, diagnostics);
        }
        Stmt::CollectionFor(statement) => {
            check_expression_blocks(sources, &statement.source, parent, tree, diagnostics);
            check_block(sources, &statement.body, parent, tree, diagnostics);
        }
        Stmt::LiteralPackFor(statement) => {
            check_block(sources, &statement.body, parent, tree, diagnostics);
        }
        Stmt::While(statement) => {
            check_expression_blocks(sources, &statement.condition, parent, tree, diagnostics);
            check_block(sources, &statement.body, parent, tree, diagnostics);
        }
        Stmt::Loop(statement) => check_block(sources, &statement.body, parent, tree, diagnostics),
        Stmt::Expression(statement) => {
            check_expression_blocks(sources, &statement.expression, parent, tree, diagnostics)
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => {}
    }
}

fn check_expression_blocks(
    sources: &SourceMap,
    expression: &Expr,
    parent: Option<RegionId>,
    tree: &mut LexicalRegionTree,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expr::Closure(closure) => {
            check_block(sources, &closure.body, parent, tree, diagnostics);
        }
        Expr::TypedSequenceLiteral(expression) => {
            for element in &expression.elements {
                check_expression_blocks(sources, element, parent, tree, diagnostics);
            }
            if let Some(using) = &expression.using {
                check_expression_blocks(sources, &using.allocator, parent, tree, diagnostics);
            }
        }
        Expr::TypedStringLiteral(expression) => {
            if let Some(using) = &expression.using {
                check_expression_blocks(sources, &using.allocator, parent, tree, diagnostics);
            }
        }
        Expr::Catch(expression) => {
            check_expression_blocks(sources, &expression.expression, parent, tree, diagnostics);
            check_block(sources, &expression.catch_block, parent, tree, diagnostics);
        }
        Expr::Otherwise(expression) => {
            check_expression_blocks(sources, &expression.value, parent, tree, diagnostics);
            check_block(sources, &expression.fallback, parent, tree, diagnostics);
        }
        Expr::If(expression) => {
            check_expression_blocks(sources, &expression.condition, parent, tree, diagnostics);
            check_block(sources, &expression.then_block, parent, tree, diagnostics);
            if let Some(block) = &expression.else_block {
                check_block(sources, block, parent, tree, diagnostics);
            }
        }
        Expr::IfIs(expression) => {
            check_expression_blocks(sources, &expression.expression, parent, tree, diagnostics);
            check_block(sources, &expression.then_block, parent, tree, diagnostics);
            if let Some(block) = &expression.else_block {
                check_block(sources, block, parent, tree, diagnostics);
            }
        }
        Expr::Match(expression) => {
            check_expression_blocks(sources, &expression.expression, parent, tree, diagnostics);
            for arm in &expression.arms {
                check_block(sources, &arm.body, parent, tree, diagnostics);
            }
            if let Some(arm) = &expression.wildcard_arm {
                check_block(sources, &arm.body, parent, tree, diagnostics);
            }
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    check_expression_blocks(sources, &part.expression, parent, tree, diagnostics);
                }
            }
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                check_expression_blocks(sources, element, parent, tree, diagnostics);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                check_expression_blocks(sources, &field.value, parent, tree, diagnostics);
            }
        }
        Expr::Call(expression) => {
            check_expression_blocks(sources, &expression.callee, parent, tree, diagnostics);
            for argument in &expression.arguments {
                check_expression_blocks(sources, argument, parent, tree, diagnostics);
            }
        }
        Expr::Index(expression) => {
            check_expression_blocks(sources, &expression.object, parent, tree, diagnostics);
            check_expression_blocks(sources, &expression.index, parent, tree, diagnostics);
        }
        Expr::Binary(expression) => {
            check_expression_blocks(sources, &expression.left, parent, tree, diagnostics);
            check_expression_blocks(sources, &expression.right, parent, tree, diagnostics);
        }
        Expr::Propagate(expression) => {
            check_expression_blocks(sources, &expression.expression, parent, tree, diagnostics)
        }
        Expr::Force(expression) => {
            check_expression_blocks(sources, &expression.expression, parent, tree, diagnostics)
        }
        Expr::Borrow(expression) => {
            check_expression_blocks(sources, &expression.expression, parent, tree, diagnostics)
        }
        Expr::Unary(expression) => {
            check_expression_blocks(sources, &expression.operand, parent, tree, diagnostics)
        }
        Expr::TypeConversion(expression) => {
            check_expression_blocks(sources, &expression.expression, parent, tree, diagnostics)
        }
        Expr::Member(expression) => {
            check_expression_blocks(sources, &expression.object, parent, tree, diagnostics)
        }
        Expr::Group(expression) => {
            check_expression_blocks(sources, &expression.expression, parent, tree, diagnostics)
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}
