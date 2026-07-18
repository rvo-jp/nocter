//! Completion candidates derived from lexical keywords and resolver symbols.

use super::FileAnalysis;
use super::single_file::{parse_single_file_text, resolve_single_file_ast};
use crate::lexer::KEYWORD_LEXEMES;
use crate::resolve::{ResolveOutput, Symbol, SymbolKind, TypeSymbolKind};
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

pub(crate) fn completion_items_for_file_analysis(file: &FileAnalysis) -> Vec<CompletionItemInfo> {
    completion_items_for_resolved_symbols(&file.resolved)
}

pub(crate) fn completion_items_for_text(text: &str) -> Option<Vec<CompletionItemInfo>> {
    let parsed = parse_single_file_text("completion.nct", text)?;
    let resolved = resolve_single_file_ast("completion.nct", text, parsed.source, &parsed.ast);

    Some(completion_items_for_resolved_symbols(&resolved))
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

fn completion_items_for_resolved_symbols(resolved: &ResolveOutput) -> Vec<CompletionItemInfo> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{CompileUnit, analyze_compile_unit_as_modules};
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::source::SourceMap;
    use std::collections::HashMap;

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

    fn analyze_text(text: &str) -> (SourceMap, crate::analysis::CompileUnitAnalysis) {
        let mut sources = SourceMap::new();
        let source = sources.add_source("test.nct", None, text.to_string());
        let lex_output = lex(&sources, source);
        assert!(
            lex_output.diagnostics.is_empty(),
            "unexpected lex diagnostics: {:?}",
            lex_output.diagnostics
        );
        let ast = parse(&sources, source, &lex_output.tokens)
            .ast
            .expect("expected ast");
        let unit = CompileUnit::new(ast.clone(), vec![ast], HashMap::new());
        let analysis = analyze_compile_unit_as_modules(&sources, &unit);

        (sources, analysis)
    }
}
