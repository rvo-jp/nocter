//! Completion candidates derived from lexical keywords and resolver symbols.

use super::FileAnalysis;
use super::single_file::{parse_single_file_text, resolve_single_file_ast};
use crate::ast::{AstFile, Block, Expr, Item, Stmt};
use crate::lexer::KEYWORD_LEXEMES;
use crate::resolve::{ResolveOutput, Symbol, SymbolKind, TypeSymbolKind};
use crate::source::ByteSpan;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionItemKind {
    Function,
    Class,
    Interface,
    Module,
    Enum,
    Keyword,
    Struct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionItemInfo {
    pub(crate) label: String,
    pub(crate) kind: CompletionItemKind,
    pub(crate) detail: Option<String>,
}

#[cfg(test)]
pub(crate) fn completion_items_for_file_analysis(file: &FileAnalysis) -> Vec<CompletionItemInfo> {
    completion_items_for_resolved_symbols(&file.resolved, HashSet::new())
}

pub(crate) fn completion_items_for_file_analysis_at_offset(
    file: &FileAnalysis,
    offset: usize,
) -> Vec<CompletionItemInfo> {
    completion_items_for_resolved_symbols(
        &file.resolved,
        visible_scoped_import_spans_at_offset(&file.ast, offset),
    )
}

pub(crate) fn completion_items_for_text_at_offset(
    text: &str,
    offset: usize,
) -> Option<Vec<CompletionItemInfo>> {
    let parsed = parse_single_file_text("completion.nct", text)?;
    let resolved = resolve_single_file_ast("completion.nct", text, parsed.source, &parsed.ast);

    Some(completion_items_for_resolved_symbols(
        &resolved,
        visible_scoped_import_spans_at_offset(&parsed.ast, offset),
    ))
}

pub(crate) fn keyword_completion_items() -> Vec<CompletionItemInfo> {
    KEYWORD_LEXEMES
        .iter()
        .map(|keyword| CompletionItemInfo {
            label: (*keyword).to_string(),
            kind: CompletionItemKind::Keyword,
            detail: Some("keyword".to_string()),
        })
        .collect()
}

fn completion_items_for_resolved_symbols(
    resolved: &ResolveOutput,
    visible_hidden_symbol_spans: HashSet<ByteSpan>,
) -> Vec<CompletionItemInfo> {
    let mut items = keyword_completion_items();
    let mut seen = KEYWORD_LEXEMES
        .iter()
        .map(|keyword| (*keyword).to_string())
        .collect::<HashSet<_>>();

    let mut symbols = resolved
        .symbols
        .symbols()
        .filter(|symbol| {
            !symbol.is_hidden || visible_hidden_symbol_spans.contains(&symbol.name_span)
        })
        .collect::<Vec<_>>();
    symbols.sort_by(|left, right| left.name.cmp(&right.name));

    for symbol in symbols {
        if !seen.insert(symbol.name.clone()) {
            continue;
        }
        items.push(CompletionItemInfo {
            label: symbol.name.clone(),
            kind: completion_kind_for_symbol(symbol),
            detail: Some(symbol_detail(symbol)),
        });
    }

    items
}

fn visible_scoped_import_spans_at_offset(ast: &AstFile, offset: usize) -> HashSet<ByteSpan> {
    ast.items
        .iter()
        .find_map(|item| scoped_import_spans_in_item_at_offset(item, offset))
        .unwrap_or_default()
}

fn scoped_import_spans_in_item_at_offset(item: &Item, offset: usize) -> Option<HashSet<ByteSpan>> {
    if !span_contains(item.span(), offset) {
        return None;
    }

    match item {
        Item::Function(function) => {
            scoped_import_spans_in_block_at_offset(&function.body, offset, &HashSet::new())
        }
        Item::Impl(impl_) => impl_.members.iter().find_map(|member| match member {
            crate::ast::ImplMember::Method(method) => method.body.as_ref().and_then(|body| {
                scoped_import_spans_in_block_at_offset(body, offset, &HashSet::new())
            }),
            crate::ast::ImplMember::Drop(drop_) => {
                scoped_import_spans_in_block_at_offset(&drop_.body, offset, &HashSet::new())
            }
        }),
        _ => None,
    }
}

fn scoped_import_spans_in_block_at_offset(
    block: &Block,
    offset: usize,
    inherited: &HashSet<ByteSpan>,
) -> Option<HashSet<ByteSpan>> {
    if !span_contains(block.span, offset) {
        return None;
    }

    let mut visible = inherited.clone();
    for statement in &block.statements {
        let statement_span = statement.span();
        if statement_span.start > offset {
            break;
        }

        match statement {
            Stmt::Import(import) => {
                if import.span.end <= offset {
                    visible.insert(import.alias.span);
                    continue;
                }
            }
            Stmt::FromImport(import) => {
                if import.span.end <= offset {
                    visible.extend(import.names.iter().map(|name| name.local_span()));
                    continue;
                }
            }
            _ => {}
        }

        if let Some(scoped) =
            scoped_import_spans_in_statement_at_offset(statement, offset, &visible)
        {
            return Some(scoped);
        }

        if span_contains(statement_span, offset) {
            return Some(visible);
        }
    }

    if let Some(result) = &block.result
        && let Some(scoped) = scoped_import_spans_in_expression_at_offset(result, offset, &visible)
    {
        return Some(scoped);
    }

    Some(visible)
}

fn scoped_import_spans_in_statement_at_offset(
    statement: &Stmt,
    offset: usize,
    visible: &HashSet<ByteSpan>,
) -> Option<HashSet<ByteSpan>> {
    match statement {
        Stmt::Return(statement) => statement.expression.as_ref().and_then(|expression| {
            scoped_import_spans_in_expression_at_offset(expression, offset, visible)
        }),
        Stmt::Binding(statement) => {
            scoped_import_spans_in_expression_at_offset(&statement.initializer, offset, visible)
        }
        Stmt::Assignment(statement) => {
            scoped_import_spans_in_expression_at_offset(&statement.target, offset, visible).or_else(
                || scoped_import_spans_in_expression_at_offset(&statement.value, offset, visible),
            )
        }
        Stmt::If(statement) => scoped_import_spans_in_if_at_offset(statement, offset, visible),
        Stmt::IfIs(statement) => scoped_import_spans_in_if_is_at_offset(statement, offset, visible),
        Stmt::Switch(statement) => {
            scoped_import_spans_in_switch_at_offset(statement, offset, visible)
        }
        Stmt::ForRange(statement) => {
            scoped_import_spans_in_expression_at_offset(&statement.start, offset, visible)
                .or_else(|| {
                    scoped_import_spans_in_expression_at_offset(&statement.end, offset, visible)
                })
                .or_else(|| {
                    scoped_import_spans_in_block_at_offset(&statement.body, offset, visible)
                })
        }
        Stmt::While(statement) => {
            scoped_import_spans_in_expression_at_offset(&statement.condition, offset, visible)
                .or_else(|| {
                    scoped_import_spans_in_block_at_offset(&statement.body, offset, visible)
                })
        }
        Stmt::Loop(statement) => {
            scoped_import_spans_in_block_at_offset(&statement.body, offset, visible)
        }
        Stmt::Expression(statement) => {
            scoped_import_spans_in_expression_at_offset(&statement.expression, offset, visible)
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => None,
    }
}

fn scoped_import_spans_in_expression_at_offset(
    expression: &Expr,
    offset: usize,
    visible: &HashSet<ByteSpan>,
) -> Option<HashSet<ByteSpan>> {
    if !span_contains(expression.span(), offset) {
        return None;
    }

    match expression {
        Expr::InterpolatedString(expression) => expression.parts.iter().find_map(|part| {
            let crate::ast::InterpolatedStringPart::Expression(part) = part else {
                return None;
            };
            scoped_import_spans_in_expression_at_offset(&part.expression, offset, visible)
        }),
        Expr::ArrayLiteral(expression) => expression.elements.iter().find_map(|element| {
            scoped_import_spans_in_expression_at_offset(element, offset, visible)
        }),
        Expr::StructLiteral(expression) => expression.fields.iter().find_map(|field| {
            scoped_import_spans_in_expression_at_offset(&field.value, offset, visible)
        }),
        Expr::Propagate(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.expression, offset, visible)
        }
        Expr::Force(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.expression, offset, visible)
        }
        Expr::Catch(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.expression, offset, visible)
                .or_else(|| {
                    scoped_import_spans_in_block_at_offset(&expression.catch_block, offset, visible)
                })
        }
        Expr::Borrow(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.expression, offset, visible)
        }
        Expr::Unary(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.operand, offset, visible)
        }
        Expr::Binary(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.left, offset, visible).or_else(
                || scoped_import_spans_in_expression_at_offset(&expression.right, offset, visible),
            )
        }
        Expr::TypeConversion(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.expression, offset, visible)
        }
        Expr::Call(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.callee, offset, visible)
                .or_else(|| {
                    expression.arguments.iter().find_map(|argument| {
                        scoped_import_spans_in_expression_at_offset(argument, offset, visible)
                    })
                })
        }
        Expr::Member(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.object, offset, visible)
        }
        Expr::Index(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.object, offset, visible)
                .or_else(|| {
                    scoped_import_spans_in_expression_at_offset(&expression.index, offset, visible)
                })
        }
        Expr::Group(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.expression, offset, visible)
        }
        Expr::Otherwise(expression) => {
            scoped_import_spans_in_expression_at_offset(&expression.value, offset, visible).or_else(
                || scoped_import_spans_in_block_at_offset(&expression.fallback, offset, visible),
            )
        }
        Expr::If(statement) => scoped_import_spans_in_if_at_offset(statement, offset, visible),
        Expr::IfIs(statement) => scoped_import_spans_in_if_is_at_offset(statement, offset, visible),
        Expr::Match(statement) => {
            scoped_import_spans_in_switch_at_offset(statement, offset, visible)
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => Some(visible.clone()),
    }
}

fn scoped_import_spans_in_if_at_offset(
    statement: &crate::ast::IfStmt,
    offset: usize,
    visible: &HashSet<ByteSpan>,
) -> Option<HashSet<ByteSpan>> {
    scoped_import_spans_in_expression_at_offset(&statement.condition, offset, visible)
        .or_else(|| scoped_import_spans_in_block_at_offset(&statement.then_block, offset, visible))
        .or_else(|| {
            statement
                .else_block
                .as_ref()
                .and_then(|block| scoped_import_spans_in_block_at_offset(block, offset, visible))
        })
}

fn scoped_import_spans_in_if_is_at_offset(
    statement: &crate::ast::IfIsStmt,
    offset: usize,
    visible: &HashSet<ByteSpan>,
) -> Option<HashSet<ByteSpan>> {
    scoped_import_spans_in_expression_at_offset(&statement.expression, offset, visible)
        .or_else(|| scoped_import_spans_in_block_at_offset(&statement.then_block, offset, visible))
        .or_else(|| {
            statement
                .else_block
                .as_ref()
                .and_then(|block| scoped_import_spans_in_block_at_offset(block, offset, visible))
        })
}

fn scoped_import_spans_in_switch_at_offset(
    statement: &crate::ast::SwitchStmt,
    offset: usize,
    visible: &HashSet<ByteSpan>,
) -> Option<HashSet<ByteSpan>> {
    scoped_import_spans_in_expression_at_offset(&statement.expression, offset, visible)
        .or_else(|| {
            statement
                .arms
                .iter()
                .find_map(|arm| scoped_import_spans_in_block_at_offset(&arm.body, offset, visible))
        })
        .or_else(|| {
            statement
                .else_arm
                .as_ref()
                .and_then(|arm| scoped_import_spans_in_block_at_offset(&arm.body, offset, visible))
        })
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

fn completion_kind_for_symbol(symbol: &Symbol) -> CompletionItemKind {
    match &symbol.kind {
        SymbolKind::Function(_) | SymbolKind::Primitive(_) => CompletionItemKind::Function,
        SymbolKind::Type(type_symbol) => match type_symbol.kind {
            TypeSymbolKind::Alias => CompletionItemKind::Class,
            TypeSymbolKind::Struct => CompletionItemKind::Struct,
            TypeSymbolKind::Enum => CompletionItemKind::Enum,
            TypeSymbolKind::Interface => CompletionItemKind::Interface,
        },
        SymbolKind::Imported(_) => CompletionItemKind::Module,
    }
}

fn symbol_detail(symbol: &Symbol) -> String {
    match &symbol.kind {
        SymbolKind::Function(_) => "function".to_string(),
        SymbolKind::Primitive(_) => "primitive".to_string(),
        SymbolKind::Type(type_symbol) => match type_symbol.kind {
            TypeSymbolKind::Alias => "type".to_string(),
            TypeSymbolKind::Struct => "struct".to_string(),
            TypeSymbolKind::Enum => "enum".to_string(),
            TypeSymbolKind::Interface => "interface".to_string(),
        },
        SymbolKind::Imported(imported) => format!("imported from {}", imported.path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::{analyze_namespace_import_text, analyze_text};

    #[test]
    fn completion_candidates_include_keywords_and_symbols() {
        let text = "struct File {\n    fd: i32\n}\n\nfunc main(): i32 {\n    return 0\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let source = sources.get(file.ast.span.source).expect("expected source");

        let items = completion_items_for_file_analysis(file);

        assert!(items.iter().any(|item| {
            item.label == "func"
                && item.kind == CompletionItemKind::Keyword
                && item.detail.as_deref() == Some("keyword")
        }));
        assert!(items.iter().any(|item| {
            item.label == "File"
                && item.kind == CompletionItemKind::Struct
                && item.detail.as_deref() == Some("struct")
        }));
        assert!(items.iter().any(|item| {
            item.label == "main"
                && item.kind == CompletionItemKind::Function
                && item.detail.as_deref() == Some("function")
        }));
        assert_eq!(source.text(), text);
    }

    #[test]
    fn completion_candidates_hide_namespace_import_members() {
        let root_text = "use lib/math\n\nfunc main(): i32 {\n    return math.answer()\n}\n";
        let module_text = "pub func answer(): i32 {\n    return 7\n}\n";
        let (_, analysis) = analyze_namespace_import_text(root_text, module_text);
        let file = analysis.root_file().expect("expected root file");

        let items = completion_items_for_file_analysis(file);

        assert!(items.iter().any(|item| {
            item.label == "math"
                && item.kind == CompletionItemKind::Module
                && item.detail.as_deref() == Some("imported from lib/math")
        }));
        assert!(!items.iter().any(|item| item.label == "answer"));
    }

    #[test]
    fn completion_candidates_include_block_imports_only_inside_scope() {
        let text = r#"func main(): i32 {
    use lib/math.answer

    return answer()
}

func other(): i32 {
    return 0
}
"#;
        let (_, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let inside_offset = text.rfind("answer()").expect("expected answer call");
        let outside_offset = text.rfind("return 0").expect("expected other function");

        let inside_items = completion_items_for_file_analysis_at_offset(file, inside_offset);
        let outside_items = completion_items_for_file_analysis_at_offset(file, outside_offset);

        assert!(inside_items.iter().any(|item| item.label == "answer"));
        assert!(!outside_items.iter().any(|item| item.label == "answer"));
    }
}
