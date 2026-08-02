use super::{CompileUnit, CompileUnitAnalysis, analyze_module_compile_unit};
use crate::ast::{AstFile, Item};
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::{ImportAccess, ImportSource};
use crate::semantics::{
    AllocationFailurePolicy, AllocationSource, AllocatorCapabilityKind, TrustedDeclarationFacts,
    TrustedDeclarationRole,
};
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

pub(crate) fn analyze_text_with_trusted_allocator_capabilities(
    text: &str,
) -> (SourceMap, CompileUnitAnalysis) {
    let mut sources = SourceMap::new();
    let source = sources.add_source("test.nct", None, text.to_string());
    let ast = parse_source(&sources, source);
    let mut trusted = TrustedDeclarationFacts::default();
    for item in &ast.items {
        let Item::Struct(struct_) = item else {
            continue;
        };
        let kind = match struct_.name.as_str() {
            "Allocator" => AllocatorCapabilityKind::Aborting,
            "TryAllocator" => AllocatorCapabilityKind::Recoverable,
            _ => continue,
        };
        trusted.insert(
            struct_.span,
            TrustedDeclarationRole::AllocatorCapability(kind),
        );
    }
    let unit = CompileUnit::new(ast.clone(), vec![ast], HashMap::new(), HashMap::new(), None)
        .with_trusted_declarations(trusted);
    let analysis = analyze_module_compile_unit(&sources, &unit);

    (sources, analysis)
}

pub(crate) fn analyze_namespace_import_text(
    root_text: &str,
    module_text: &str,
) -> (SourceMap, CompileUnitAnalysis) {
    analyze_import_text(root_text, module_text)
}

pub(crate) fn analyze_import_text(
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
        Item::FromImport(item) => item.path.span,
        item => panic!("expected import, got {item:?}"),
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

pub(crate) fn analyze_text_with_trusted_current_allocation_operation(
    text: &str,
    primitive_name: &str,
) -> (SourceMap, CompileUnitAnalysis) {
    let mut sources = SourceMap::new();
    let source = sources.add_source("test.nct", None, text.to_string());
    let ast = parse_source(&sources, source);
    let declaration = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Primitive(primitive) if primitive.name == primitive_name => {
                Some(primitive.name_span)
            }
            _ => None,
        })
        .expect("expected trusted allocation primitive");
    let mut trusted = TrustedDeclarationFacts::default();
    trusted.insert(
        declaration,
        TrustedDeclarationRole::AllocationOperation {
            source: AllocationSource::CurrentContext,
            failure_policy: AllocationFailurePolicy::Abort,
        },
    );
    let unit = CompileUnit::new(ast.clone(), vec![ast], HashMap::new(), HashMap::new(), None)
        .with_trusted_declarations(trusted);
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
