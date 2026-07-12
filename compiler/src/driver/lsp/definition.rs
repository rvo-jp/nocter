use super::documents::OpenDocument;
use super::hover::{definition_span_for_ast, module_path_at_offset, resolve_single_file_for_hover};
use super::protocol::{lsp_position_to_byte_offset, position_from_params, range_for_byte_span};
use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::SymbolKind;
use crate::source::{ByteSpan, SourceMap};
use serde_json::{Value, json};

pub(super) fn definition_for_document(
    document: &OpenDocument,
    params: Option<&Value>,
) -> Option<Value> {
    let position = position_from_params(params)?;
    let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
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
    definition_span_for_ast(&document.text, &ast, &resolved, offset)
        .and_then(|span| location_for_byte_span(&sources, span))
}

pub(super) fn definition_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<Value> {
    let text = sources.get(file.ast.span.source)?.text();
    module_path_definition_location(sources, analysis, file, offset)
        .or_else(|| {
            method_call_definition_span_for_file_analysis(file, offset)
                .and_then(|span| location_for_byte_span(sources, span))
        })
        .or_else(|| {
            type_definition_span_for_file_analysis(analysis, file, offset)
                .and_then(|span| location_for_byte_span(sources, span))
        })
        .or_else(|| {
            definition_span_for_ast(text, &file.ast, &file.resolved, offset)
                .and_then(|span| location_for_byte_span(sources, span))
        })
}

fn module_path_definition_location(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<Value> {
    let path = module_path_at_offset(&file.ast, offset)?;
    let import_source = analysis.import_sources.get(&path.span)?;
    let imported_file = analysis.file_by_source(import_source.source)?;
    let span = ByteSpan::new(imported_file.ast.span.source, 0, 0);

    location_for_byte_span(sources, span)
}

fn type_definition_span_for_file_analysis(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<ByteSpan> {
    let reference = file.typecheck_facts.type_reference_at_offset(offset)?;
    let declaration_span = reference.symbol_declaration_span?;

    if declaration_span.source != file.ast.span.source
        && let Some(declaration_file) = analysis.file_by_source(declaration_span.source)
        && let Some(name_span) = declaration_file
            .resolved
            .symbols
            .symbols()
            .find_map(|candidate| match &candidate.kind {
                SymbolKind::Type(_) if candidate.declaration_span == declaration_span => {
                    Some(candidate.name_span)
                }
                SymbolKind::Function(_)
                | SymbolKind::Primitive(_)
                | SymbolKind::Type(_)
                | SymbolKind::Imported(_) => None,
            })
    {
        return Some(name_span);
    }

    reference.symbol_name_span
}

fn method_call_definition_span_for_file_analysis(
    file: &FileAnalysis,
    offset: usize,
) -> Option<ByteSpan> {
    file.typecheck_facts
        .method_call_spans()
        .filter(|span| span_contains(*span, offset))
        .min_by_key(|span| (span.len(), span.start))
        .and_then(|span| file.typecheck_facts.method_call_target(span))
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

fn location_for_byte_span(sources: &SourceMap, span: ByteSpan) -> Option<Value> {
    let source = sources.get(span.source)?;
    Some(json!({
        "uri": uri_for_source_file(source),
        "range": range_for_byte_span(source.text(), span)
    }))
}

fn uri_for_source_file(source: &crate::source::SourceFile) -> String {
    source
        .absolute_path()
        .map(|path| format!("file://{}", percent_encode_path(&path.to_string_lossy())))
        .unwrap_or_else(|| source.display_path().to_string())
}

fn percent_encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
