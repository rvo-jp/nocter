//! Go-to-definition queries derived from compile-unit analysis.

use super::single_file::{parse_single_file_text, resolve_single_file_ast};
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::analysis::hover::{
    definition_span_for_ast as hover_definition_span_for_ast, module_path_at_offset,
};
use crate::ast::AstFile;
use crate::resolve::{ResolveOutput, SymbolKind};
use crate::source::{ByteSpan, SourceId, SourceMap};

pub(crate) fn definition_span_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<ByteSpan> {
    module_path_definition_span(analysis, file, offset)
        .or_else(|| method_call_definition_span_for_file_analysis(file, offset))
        .or_else(|| type_definition_span_for_file_analysis(analysis, file, offset))
        .or_else(|| {
            let text = sources.get(file.ast.span.source)?.text();
            definition_span_for_ast(text, &file.ast, &file.resolved, offset)
        })
}

pub(crate) fn definition_span_for_ast(
    text: &str,
    ast: &AstFile,
    resolved: &ResolveOutput,
    offset: usize,
) -> Option<ByteSpan> {
    hover_definition_span_for_ast(text, ast, resolved, offset)
}

pub(crate) fn definition_span_for_text(text: &str, offset: usize) -> Option<ByteSpan> {
    let parsed = parse_single_file_text("definition.nct", text)?;
    let resolved = resolve_single_file_for_definition(text, parsed.source, &parsed.ast);

    definition_span_for_ast(text, &parsed.ast, &resolved, offset)
}

pub(crate) fn resolve_single_file_for_definition(
    text: &str,
    source: SourceId,
    ast: &AstFile,
) -> ResolveOutput {
    resolve_single_file_ast("definition.nct", text, source, ast)
}

fn module_path_definition_span(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<ByteSpan> {
    let path = module_path_at_offset(&file.ast, offset)?;
    let import_source = analysis.import_sources.get(&path.span)?;
    let imported_file = analysis.file_by_source(import_source.source)?;

    Some(ByteSpan::new(imported_file.ast.span.source, 0, 0))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{CompileUnit, analyze_compile_unit_as_modules};
    use crate::lexer::lex;
    use crate::parser::parse;
    use std::collections::HashMap;

    #[test]
    fn definition_query_resolves_local_references() {
        let text = "func main(): i32 {\n    let code = 0\n    return code\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("return code").expect("expected reference") + "return ".len();

        let span = definition_span_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected definition span");

        assert_eq!(&text[span.start..span.end], "code");
        assert_eq!(span.start, text.find("code = 0").expect("expected binding"));
    }

    fn analyze_text(text: &str) -> (SourceMap, CompileUnitAnalysis) {
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
