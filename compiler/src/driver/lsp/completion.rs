use super::documents::OpenDocument;
use super::hover::resolve_single_file_for_hover;
use crate::analysis::FileAnalysis;
use crate::lexer::{KEYWORD_LEXEMES, lex};
use crate::parser::parse;
use crate::resolve::{ResolveOutput, Symbol, SymbolKind, TypeSymbolKind};
use crate::source::SourceMap;
use serde_json::{Value, json};
use std::collections::HashSet;

pub(super) const LSP_COMPLETION_ITEM_KIND_FUNCTION: u8 = 3;
const LSP_COMPLETION_ITEM_KIND_CLASS: u8 = 7;
const LSP_COMPLETION_ITEM_KIND_INTERFACE: u8 = 8;
const LSP_COMPLETION_ITEM_KIND_MODULE: u8 = 9;
const LSP_COMPLETION_ITEM_KIND_ENUM: u8 = 13;
const LSP_COMPLETION_ITEM_KIND_KEYWORD: u8 = 14;
pub(super) const LSP_COMPLETION_ITEM_KIND_STRUCT: u8 = 22;

pub(super) fn completion_items_for_file_analysis(file: &FileAnalysis) -> Vec<Value> {
    completion_items_for_resolved_symbols(&file.resolved)
}

pub(super) fn completion_items_for_document(document: &OpenDocument) -> Option<Vec<Value>> {
    let mut sources = SourceMap::new();
    let source = sources.add_source(
        document.display_path.clone(),
        document.absolute_path.clone(),
        document.text.clone(),
    );
    let lex_output = lex(&sources, source);
    if !lex_output.diagnostics.is_empty() {
        return None;
    }
    let ast = parse(&sources, source, &lex_output.tokens).ast?;
    let resolved = resolve_single_file_for_hover(&document.text, source, &ast);
    Some(completion_items_for_resolved_symbols(&resolved))
}

fn completion_items_for_resolved_symbols(resolved: &ResolveOutput) -> Vec<Value> {
    let mut items = keyword_completion_items();
    let mut seen = KEYWORD_LEXEMES
        .iter()
        .map(|keyword| (*keyword).to_string())
        .collect::<HashSet<_>>();

    let mut symbols = resolved.symbols.symbols().collect::<Vec<_>>();
    symbols.sort_by(|left, right| left.name.cmp(&right.name));

    for symbol in symbols {
        if !seen.insert(symbol.name.clone()) {
            continue;
        }
        items.push(completion_item(
            &symbol.name,
            completion_kind_for_symbol(symbol),
            Some(symbol_detail(symbol)),
        ));
    }

    items
}

pub(super) fn keyword_completion_items() -> Vec<Value> {
    KEYWORD_LEXEMES
        .iter()
        .map(|keyword| {
            completion_item(
                keyword,
                LSP_COMPLETION_ITEM_KIND_KEYWORD,
                Some("keyword".to_string()),
            )
        })
        .collect()
}

fn completion_kind_for_symbol(symbol: &Symbol) -> u8 {
    match &symbol.kind {
        SymbolKind::Function(_) | SymbolKind::Primitive(_) => LSP_COMPLETION_ITEM_KIND_FUNCTION,
        SymbolKind::Type(type_symbol) => match type_symbol.kind {
            TypeSymbolKind::Alias => LSP_COMPLETION_ITEM_KIND_CLASS,
            TypeSymbolKind::Struct => LSP_COMPLETION_ITEM_KIND_STRUCT,
            TypeSymbolKind::Enum => LSP_COMPLETION_ITEM_KIND_ENUM,
            TypeSymbolKind::Trait => LSP_COMPLETION_ITEM_KIND_INTERFACE,
        },
        SymbolKind::Imported(_) => LSP_COMPLETION_ITEM_KIND_MODULE,
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
            TypeSymbolKind::Trait => "trait".to_string(),
        },
        SymbolKind::Imported(imported) => format!("imported from {}", imported.path),
    }
}

fn completion_item(label: &str, kind: u8, detail: Option<String>) -> Value {
    let mut item = json!({
        "label": label,
        "kind": kind,
    });

    if let Some(detail) = detail
        && let Some(object) = item.as_object_mut()
    {
        object.insert("detail".to_string(), Value::String(detail));
    }

    item
}
