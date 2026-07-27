use super::{CompileUnit, CompileUnitAnalysis, analyze_module_compile_unit};
use crate::ast::{AstFile, Item};
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::{ImportAccess, ImportSource};
use crate::source::{ByteSpan, SourceId, SourceMap};
use std::collections::HashMap;

pub(crate) fn analyze_text(text: &str) -> (SourceMap, CompileUnitAnalysis) {
    let mut sources = SourceMap::new();
    let source = sources.add_source("test.nct", None, text.to_string());
    let ast = parse_source(&sources, source);
    let unit = CompileUnit::new(ast.clone(), vec![ast], HashMap::new(), HashMap::new(), None);
    let analysis = analyze_module_compile_unit(&sources, &unit);

    (sources, analysis)
}

pub(crate) fn analyze_namespace_import_text(
    root_text: &str,
    module_text: &str,
) -> (SourceMap, CompileUnitAnalysis) {
    let mut sources = SourceMap::new();
    let root_source = sources.add_source("app.nct", None, root_text.to_string());
    let module_source = sources.add_source("lib/math.nct", None, module_text.to_string());
    let root_ast = parse_source(&sources, root_source);
    let module_ast = parse_source(&sources, module_source);
    let path_span = match &root_ast.items[0] {
        Item::Import(item) => item.path.span,
        item => panic!("expected namespace import, got {item:?}"),
    };
    let mut import_sources = HashMap::new();
    import_sources.insert(
        path_span,
        ImportSource {
            source: module_source,
            access: ImportAccess::Public,
        },
    );
    let unit = CompileUnit::new(
        root_ast.clone(),
        vec![root_ast, module_ast],
        import_sources,
        HashMap::new(),
        None,
    );
    let analysis = analyze_module_compile_unit(&sources, &unit);

    (sources, analysis)
}

fn parse_source(sources: &SourceMap, source: SourceId) -> AstFile {
    let lex_output = lex(sources, source);
    assert!(
        lex_output.diagnostics.is_empty(),
        "unexpected lex diagnostics: {:?}",
        lex_output.diagnostics
    );
    parse(sources, source, &lex_output.tokens)
        .ast
        .expect("expected ast")
}

pub(crate) fn span_fragments_from_sources<'a>(
    sources: &'a SourceMap,
    spans: &[ByteSpan],
) -> Vec<&'a str> {
    spans
        .iter()
        .map(|span| {
            let text = sources.get(span.source).expect("expected source").text();
            &text[span.start..span.end]
        })
        .collect()
}
