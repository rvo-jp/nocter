use super::*;
use crate::ast::ConformanceMember;

pub(crate) fn module_path_at_offset(ast: &AstFile, offset: usize) -> Option<&ModulePath> {
    ast.items
        .iter()
        .find_map(|item| module_path_in_item_at_offset(item, offset))
}

pub(in crate::analysis::hover) fn module_path_in_item_at_offset(
    item: &Item,
    offset: usize,
) -> Option<&ModulePath> {
    match item {
        Item::Import(item) => path_if_at_offset(&item.path, offset),
        Item::FromImport(item) => path_if_at_offset(&item.path, offset),
        Item::Function(function) => function
            .body
            .as_ref()
            .and_then(|body| module_path_in_block_at_offset(body, offset)),
        Item::Test(test) => module_path_in_block_at_offset(&test.body, offset),
        Item::Instance(instance) => instance.methods.iter().find_map(|method| {
            method
                .body
                .as_ref()
                .and_then(|body| module_path_in_block_at_offset(body, offset))
        }),
        Item::Destruct(destruct) => module_path_in_block_at_offset(&destruct.body, offset),
        Item::Conformance(conformance) => {
            conformance.members.iter().find_map(|member| match member {
                ConformanceMember::AssociatedType(_) => None,
                ConformanceMember::Method(method) => method
                    .body
                    .as_ref()
                    .and_then(|body| module_path_in_block_at_offset(body, offset)),
            })
        }
        Item::Interface(interface) => interface.methods.iter().find_map(|method| {
            method
                .body
                .as_ref()
                .and_then(|body| module_path_in_block_at_offset(body, offset))
        }),
        Item::Construct(construct) => construct
            .functions()
            .find_map(|(_, function)| {
                function
                    .body
                    .as_ref()
                    .and_then(|body| module_path_in_block_at_offset(body, offset))
            })
            .or_else(|| {
                construct.literals().find_map(|(_, literal)| {
                    literal
                        .body
                        .as_ref()
                        .and_then(|body| module_path_in_block_at_offset(body, offset))
                })
            }),
        Item::Coerce(coerce) => coerce.entries.iter().find_map(|entry| {
            entry
                .body
                .as_ref()
                .and_then(|body| module_path_in_block_at_offset(body, offset))
        }),
        Item::Primitive(_) | Item::TypeAlias(_) | Item::Struct(_) | Item::Enum(_) => None,
    }
}

pub(in crate::analysis::hover) fn module_path_in_block_at_offset(
    block: &Block,
    offset: usize,
) -> Option<&ModulePath> {
    block
        .statements
        .iter()
        .find_map(|statement| module_path_in_statement_at_offset(statement, offset))
        .or_else(|| {
            block
                .result
                .as_deref()
                .and_then(|result| module_path_in_expression_at_offset(result, offset))
        })
}

pub(in crate::analysis::hover) fn module_path_in_statement_at_offset(
    statement: &Stmt,
    offset: usize,
) -> Option<&ModulePath> {
    match statement {
        Stmt::Import(statement) => path_if_at_offset(&statement.path, offset),
        Stmt::FromImport(statement) => path_if_at_offset(&statement.path, offset),
        Stmt::Return(statement) => statement
            .expression
            .as_ref()
            .and_then(|expression| module_path_in_expression_at_offset(expression, offset)),
        Stmt::Binding(statement) => {
            module_path_in_expression_at_offset(&statement.initializer, offset)
        }
        Stmt::Assignment(statement) => {
            module_path_in_expression_at_offset(&statement.target, offset)
                .or_else(|| module_path_in_expression_at_offset(&statement.value, offset))
        }
        Stmt::If(statement) => module_path_in_expression_at_offset(&statement.condition, offset)
            .or_else(|| module_path_in_block_at_offset(&statement.then_block, offset))
            .or_else(|| {
                statement
                    .else_block
                    .as_ref()
                    .and_then(|block| module_path_in_block_at_offset(block, offset))
            }),
        Stmt::IfIs(statement) => module_path_in_expression_at_offset(&statement.expression, offset)
            .or_else(|| module_path_in_block_at_offset(&statement.then_block, offset))
            .or_else(|| {
                statement
                    .else_block
                    .as_ref()
                    .and_then(|block| module_path_in_block_at_offset(block, offset))
            }),
        Stmt::Switch(statement) => {
            module_path_in_expression_at_offset(&statement.expression, offset)
                .or_else(|| {
                    statement
                        .arms
                        .iter()
                        .find_map(|arm| module_path_in_block_at_offset(&arm.body, offset))
                })
                .or_else(|| {
                    statement
                        .wildcard_arm
                        .as_ref()
                        .and_then(|arm| module_path_in_block_at_offset(&arm.body, offset))
                })
        }
        Stmt::ForRange(statement) => module_path_in_expression_at_offset(&statement.start, offset)
            .or_else(|| module_path_in_expression_at_offset(&statement.end, offset))
            .or_else(|| module_path_in_block_at_offset(&statement.body, offset)),
        Stmt::CollectionFor(statement) => {
            module_path_in_expression_at_offset(&statement.source, offset)
                .or_else(|| module_path_in_block_at_offset(&statement.body, offset))
        }
        Stmt::LiteralPackFor(statement) => module_path_in_block_at_offset(&statement.body, offset),
        Stmt::While(statement) => module_path_in_expression_at_offset(&statement.condition, offset)
            .or_else(|| module_path_in_block_at_offset(&statement.body, offset)),
        Stmt::Loop(statement) => module_path_in_block_at_offset(&statement.body, offset),
        Stmt::Region(statement) => {
            module_path_in_expression_at_offset(&statement.allocator, offset)
                .or_else(|| module_path_in_block_at_offset(&statement.body, offset))
        }
        Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => None,
        Stmt::Expression(statement) => {
            module_path_in_expression_at_offset(&statement.expression, offset)
        }
    }
}

pub(in crate::analysis::hover) fn module_path_in_expression_at_offset(
    expression: &Expr,
    offset: usize,
) -> Option<&ModulePath> {
    match expression {
        Expr::Closure(expression) => module_path_in_block_at_offset(&expression.body, offset),
        Expr::InterpolatedString(expression) => {
            expression.parts.iter().find_map(|part| match part {
                InterpolatedStringPart::Expression(part) => {
                    module_path_in_expression_at_offset(&part.expression, offset)
                }
                InterpolatedStringPart::Text(_) => None,
            })
        }
        Expr::ArrayLiteral(expression) => expression
            .elements
            .iter()
            .find_map(|element| module_path_in_expression_at_offset(element, offset)),
        Expr::TypedSequenceLiteral(expression) => expression
            .elements
            .iter()
            .find_map(|element| module_path_in_expression_at_offset(element, offset))
            .or_else(|| {
                expression
                    .using
                    .as_ref()
                    .and_then(|using| module_path_in_expression_at_offset(&using.allocator, offset))
            }),
        Expr::TypedStringLiteral(expression) => expression
            .using
            .as_ref()
            .and_then(|using| module_path_in_expression_at_offset(&using.allocator, offset)),
        Expr::StructLiteral(expression) => expression
            .fields
            .iter()
            .find_map(|field| module_path_in_expression_at_offset(&field.value, offset)),
        Expr::Propagate(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Force(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Catch(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
                .or_else(|| module_path_in_block_at_offset(&expression.catch_block, offset))
        }
        Expr::Borrow(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Unary(expression) => module_path_in_expression_at_offset(&expression.operand, offset),
        Expr::Binary(expression) => module_path_in_expression_at_offset(&expression.left, offset)
            .or_else(|| module_path_in_expression_at_offset(&expression.right, offset)),
        Expr::TypeConversion(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Call(expression) => module_path_in_expression_at_offset(&expression.callee, offset)
            .or_else(|| {
                expression
                    .arguments
                    .iter()
                    .find_map(|argument| module_path_in_expression_at_offset(argument, offset))
            }),
        Expr::Member(expression) => module_path_in_expression_at_offset(&expression.object, offset),
        Expr::Index(expression) => module_path_in_expression_at_offset(&expression.object, offset)
            .or_else(|| module_path_in_expression_at_offset(&expression.index, offset)),
        Expr::Group(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Otherwise(expression) => {
            module_path_in_expression_at_offset(&expression.value, offset)
                .or_else(|| module_path_in_block_at_offset(&expression.fallback, offset))
        }
        Expr::If(expression) => module_path_in_expression_at_offset(&expression.condition, offset)
            .or_else(|| module_path_in_block_at_offset(&expression.then_block, offset))
            .or_else(|| {
                expression
                    .else_block
                    .as_ref()
                    .and_then(|block| module_path_in_block_at_offset(block, offset))
            }),
        Expr::IfIs(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
                .or_else(|| module_path_in_block_at_offset(&expression.then_block, offset))
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| module_path_in_block_at_offset(block, offset))
                })
        }
        Expr::Match(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
                .or_else(|| {
                    expression
                        .arms
                        .iter()
                        .find_map(|arm| module_path_in_block_at_offset(&arm.body, offset))
                })
                .or_else(|| {
                    expression
                        .wildcard_arm
                        .as_ref()
                        .and_then(|arm| module_path_in_block_at_offset(&arm.body, offset))
                })
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => None,
    }
}

pub(in crate::analysis::hover) fn path_if_at_offset(
    path: &ModulePath,
    offset: usize,
) -> Option<&ModulePath> {
    span_contains(path.span, offset).then_some(path)
}

pub(in crate::analysis::hover) fn module_path_hover_for_ast(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let path = module_path_at_offset(&file.ast, offset)?;
    let import_source = analysis.import_sources.get(&path.span)?;
    let imported_file = analysis.file_by_source(import_source.source)?;
    let imported_source = sources.get(imported_file.ast.span.source)?;
    let docs = attach_documentation(imported_file.ast.span.source, imported_source.text(), &[]);

    Some(HoverInfo {
        span: path.span,
        label: format!("module {}", path.value),
        documentation: docs.file().map(str::to_string),
    })
}
