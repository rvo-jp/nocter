//! Completion candidates derived from lexical keywords and resolver symbols.

use super::FileAnalysis;
use super::scoped_imports::visible_scoped_import_spans_at_offset;
use super::single_file::{parse_single_file_text, resolve_single_file_ast};
use crate::ast::{AstFile, Block, Expr, IfIsStmt, ImplMember, Item, Stmt, SwitchArm, SwitchStmt};
use crate::lexer::KEYWORD_LEXEMES;
use crate::resolve::{
    EnumVariantSignature, ResolveOutput, Symbol, SymbolKind, TypeSymbol, TypeSymbolKind,
};
use crate::source::ByteSpan;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionItemKind {
    Function,
    Class,
    Interface,
    Module,
    Enum,
    EnumMember,
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
    if let Some(items) = contextual_completion_items(&file.ast, &file.resolved, offset) {
        return items;
    }

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

    if let Some(items) = contextual_completion_items(&parsed.ast, &resolved, offset) {
        return Some(items);
    }

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

fn contextual_completion_items(
    ast: &AstFile,
    resolved: &ResolveOutput,
    offset: usize,
) -> Option<Vec<CompletionItemInfo>> {
    let enum_name = enum_pattern_member_owner_at_offset(ast, offset)?;
    Some(enum_variant_completion_items(
        resolved.type_symbol_by_name(enum_name)?,
    ))
}

fn enum_variant_completion_items(symbol: &TypeSymbol) -> Vec<CompletionItemInfo> {
    symbol
        .variants
        .iter()
        .map(enum_variant_completion_item)
        .collect()
}

fn enum_variant_completion_item(variant: &EnumVariantSignature) -> CompletionItemInfo {
    CompletionItemInfo {
        label: variant.name.clone(),
        kind: CompletionItemKind::EnumMember,
        detail: Some("enum variant".to_string()),
    }
}

fn enum_pattern_member_owner_at_offset(ast: &AstFile, offset: usize) -> Option<&str> {
    ast.items
        .iter()
        .find_map(|item| enum_pattern_member_owner_in_item_at_offset(item, offset))
}

fn enum_pattern_member_owner_in_item_at_offset(item: &Item, offset: usize) -> Option<&str> {
    match item {
        Item::Function(function) => {
            enum_pattern_member_owner_in_block_at_offset(&function.body, offset)
        }
        Item::Impl(impl_) => impl_.members.iter().find_map(|member| match member {
            ImplMember::Method(method) => method
                .body
                .as_ref()
                .and_then(|body| enum_pattern_member_owner_in_block_at_offset(body, offset)),
            ImplMember::Drop(drop_) => {
                enum_pattern_member_owner_in_block_at_offset(&drop_.body, offset)
            }
        }),
        Item::Import(_)
        | Item::FromImport(_)
        | Item::Primitive(_)
        | Item::TypeAlias(_)
        | Item::Struct(_)
        | Item::Enum(_)
        | Item::Interface(_) => None,
    }
}

fn enum_pattern_member_owner_in_block_at_offset(block: &Block, offset: usize) -> Option<&str> {
    block
        .statements
        .iter()
        .find_map(|statement| enum_pattern_member_owner_in_statement_at_offset(statement, offset))
        .or_else(|| {
            block.result.as_ref().and_then(|result| {
                enum_pattern_member_owner_in_expression_at_offset(result, offset)
            })
        })
}

fn enum_pattern_member_owner_in_statement_at_offset(
    statement: &Stmt,
    offset: usize,
) -> Option<&str> {
    match statement {
        Stmt::Return(statement) => statement.expression.as_ref().and_then(|expression| {
            enum_pattern_member_owner_in_expression_at_offset(expression, offset)
        }),
        Stmt::Binding(statement) => {
            enum_pattern_member_owner_in_expression_at_offset(&statement.initializer, offset)
        }
        Stmt::Assignment(statement) => {
            enum_pattern_member_owner_in_expression_at_offset(&statement.target, offset).or_else(
                || enum_pattern_member_owner_in_expression_at_offset(&statement.value, offset),
            )
        }
        Stmt::If(statement) => {
            enum_pattern_member_owner_in_expression_at_offset(&statement.condition, offset)
                .or_else(|| {
                    enum_pattern_member_owner_in_block_at_offset(&statement.then_block, offset)
                })
                .or_else(|| {
                    statement.else_block.as_ref().and_then(|block| {
                        enum_pattern_member_owner_in_block_at_offset(block, offset)
                    })
                })
        }
        Stmt::IfIs(statement) => enum_pattern_member_owner_in_if_is_at_offset(statement, offset)
            .or_else(|| {
                enum_pattern_member_owner_in_expression_at_offset(&statement.expression, offset)
            })
            .or_else(|| enum_pattern_member_owner_in_block_at_offset(&statement.then_block, offset))
            .or_else(|| {
                statement
                    .else_block
                    .as_ref()
                    .and_then(|block| enum_pattern_member_owner_in_block_at_offset(block, offset))
            }),
        Stmt::Switch(statement) => enum_pattern_member_owner_in_switch_at_offset(statement, offset)
            .or_else(|| {
                enum_pattern_member_owner_in_expression_at_offset(&statement.expression, offset)
            }),
        Stmt::ForRange(statement) => {
            enum_pattern_member_owner_in_expression_at_offset(&statement.start, offset)
                .or_else(|| {
                    enum_pattern_member_owner_in_expression_at_offset(&statement.end, offset)
                })
                .or_else(|| enum_pattern_member_owner_in_block_at_offset(&statement.body, offset))
        }
        Stmt::While(statement) => {
            enum_pattern_member_owner_in_expression_at_offset(&statement.condition, offset)
                .or_else(|| enum_pattern_member_owner_in_block_at_offset(&statement.body, offset))
        }
        Stmt::Loop(statement) => {
            enum_pattern_member_owner_in_block_at_offset(&statement.body, offset)
        }
        Stmt::Expression(statement) => {
            enum_pattern_member_owner_in_expression_at_offset(&statement.expression, offset)
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => None,
    }
}

fn enum_pattern_member_owner_in_expression_at_offset(
    expression: &Expr,
    offset: usize,
) -> Option<&str> {
    match expression {
        Expr::InterpolatedString(expression) => {
            expression.parts.iter().find_map(|part| match part {
                crate::ast::InterpolatedStringPart::Expression(part) => {
                    enum_pattern_member_owner_in_expression_at_offset(&part.expression, offset)
                }
                crate::ast::InterpolatedStringPart::Text(_) => None,
            })
        }
        Expr::ArrayLiteral(expression) => expression
            .elements
            .iter()
            .find_map(|element| enum_pattern_member_owner_in_expression_at_offset(element, offset)),
        Expr::StructLiteral(expression) => expression.fields.iter().find_map(|field| {
            enum_pattern_member_owner_in_expression_at_offset(&field.value, offset)
        }),
        Expr::Propagate(expression) => {
            enum_pattern_member_owner_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Force(expression) => {
            enum_pattern_member_owner_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Catch(expression) => {
            enum_pattern_member_owner_in_expression_at_offset(&expression.expression, offset)
                .or_else(|| {
                    enum_pattern_member_owner_in_block_at_offset(&expression.catch_block, offset)
                })
        }
        Expr::Borrow(expression) => {
            enum_pattern_member_owner_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Unary(expression) => {
            enum_pattern_member_owner_in_expression_at_offset(&expression.operand, offset)
        }
        Expr::Binary(expression) => {
            enum_pattern_member_owner_in_expression_at_offset(&expression.left, offset).or_else(
                || enum_pattern_member_owner_in_expression_at_offset(&expression.right, offset),
            )
        }
        Expr::TypeConversion(expression) => {
            enum_pattern_member_owner_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Call(expression) => {
            enum_pattern_member_owner_in_expression_at_offset(&expression.callee, offset).or_else(
                || {
                    expression.arguments.iter().find_map(|argument| {
                        enum_pattern_member_owner_in_expression_at_offset(argument, offset)
                    })
                },
            )
        }
        Expr::Member(expression) => {
            enum_pattern_member_owner_in_expression_at_offset(&expression.object, offset)
        }
        Expr::Index(expression) => {
            enum_pattern_member_owner_in_expression_at_offset(&expression.object, offset).or_else(
                || enum_pattern_member_owner_in_expression_at_offset(&expression.index, offset),
            )
        }
        Expr::Group(expression) => {
            enum_pattern_member_owner_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Otherwise(expression) => {
            enum_pattern_member_owner_in_expression_at_offset(&expression.value, offset).or_else(
                || enum_pattern_member_owner_in_block_at_offset(&expression.fallback, offset),
            )
        }
        Expr::If(expression) => {
            enum_pattern_member_owner_in_expression_at_offset(&expression.condition, offset)
                .or_else(|| {
                    enum_pattern_member_owner_in_block_at_offset(&expression.then_block, offset)
                })
                .or_else(|| {
                    expression.else_block.as_ref().and_then(|block| {
                        enum_pattern_member_owner_in_block_at_offset(block, offset)
                    })
                })
        }
        Expr::IfIs(expression) => enum_pattern_member_owner_in_if_is_at_offset(expression, offset)
            .or_else(|| {
                enum_pattern_member_owner_in_expression_at_offset(&expression.expression, offset)
            })
            .or_else(|| {
                enum_pattern_member_owner_in_block_at_offset(&expression.then_block, offset)
            })
            .or_else(|| {
                expression
                    .else_block
                    .as_ref()
                    .and_then(|block| enum_pattern_member_owner_in_block_at_offset(block, offset))
            }),
        Expr::Match(expression) => {
            enum_pattern_member_owner_in_switch_at_offset(expression, offset).or_else(|| {
                enum_pattern_member_owner_in_expression_at_offset(&expression.expression, offset)
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

fn enum_pattern_member_owner_in_if_is_at_offset(
    statement: &IfIsStmt,
    offset: usize,
) -> Option<&str> {
    offset_in_enum_pattern_member(
        statement.enum_name_span,
        statement.variant_name_span,
        offset,
    )
    .then_some(statement.enum_name.as_str())
}

fn enum_pattern_member_owner_in_switch_at_offset(
    statement: &SwitchStmt,
    offset: usize,
) -> Option<&str> {
    statement
        .arms
        .iter()
        .find_map(|arm| enum_pattern_member_owner_in_switch_arm_at_offset(arm, offset))
        .or_else(|| {
            statement
                .arms
                .iter()
                .find_map(|arm| enum_pattern_member_owner_in_block_at_offset(&arm.body, offset))
        })
        .or_else(|| {
            statement
                .wildcard_arm
                .as_ref()
                .and_then(|arm| enum_pattern_member_owner_in_block_at_offset(&arm.body, offset))
        })
}

fn enum_pattern_member_owner_in_switch_arm_at_offset(
    arm: &SwitchArm,
    offset: usize,
) -> Option<&str> {
    offset_in_enum_pattern_member(arm.enum_name_span, arm.variant_name_span, offset)
        .then_some(arm.enum_name.as_str())
}

fn offset_in_enum_pattern_member(
    enum_name_span: ByteSpan,
    variant_name_span: ByteSpan,
    offset: usize,
) -> bool {
    enum_name_span.source == variant_name_span.source
        && enum_name_span.end < offset
        && offset <= variant_name_span.end
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

    #[test]
    fn completion_candidates_include_enum_variants_after_pattern_dot() {
        let text = r#"enum Choice {
    hit(value: i32)
    miss
}

func main(choice: Choice): i32 {
    if choice is Choice.hit(_) {
    }
    return match choice {
        Choice.hit(_) { 1 }
        Choice.miss { 2 }
    }
}
"#;
        let (_, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let if_is_offset =
            text.find("Choice.hit").expect("expected if-is pattern") + "Choice.".len();
        let match_offset =
            text.rfind("Choice.hit").expect("expected match pattern") + "Choice.".len();

        for offset in [if_is_offset, match_offset] {
            let items = completion_items_for_file_analysis_at_offset(file, offset);
            assert!(items.iter().any(|item| {
                item.label == "hit"
                    && item.kind == CompletionItemKind::EnumMember
                    && item.detail.as_deref() == Some("enum variant")
            }));
            assert!(items.iter().any(|item| {
                item.label == "miss"
                    && item.kind == CompletionItemKind::EnumMember
                    && item.detail.as_deref() == Some("enum variant")
            }));
            assert!(!items.iter().any(|item| item.label == "Choice"));
        }
    }
}
