//! Go-to-definition queries derived from compile-unit analysis.

use super::single_file::{parse_single_file_text, resolve_single_file_ast};
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::analysis::hover::{
    definition_span_for_ast as hover_definition_span_for_ast, module_path_at_offset,
};
use crate::ast::AstFile;
use crate::resolve::{ResolveOutput, SymbolKind};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::collect_typecheck_facts;

pub(crate) fn definition_span_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<ByteSpan> {
    module_path_definition_span(analysis, file, offset)
        .or_else(|| function_call_definition_span_for_file_analysis(file, offset))
        .or_else(|| method_call_definition_span_for_file_analysis(file, offset))
        .or_else(|| associated_function_definition_span_for_file_analysis(file, offset))
        .or_else(|| field_definition_span_for_file_analysis(file, offset))
        .or_else(|| enum_variant_definition_span_for_file_analysis(file, offset))
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
    let facts = collect_typecheck_facts(ast, resolved);
    if let Some((_, target)) = facts.field_target_at_offset(offset) {
        return Some(target);
    }

    if let Some((_, target)) = facts.function_call_target_at_offset(offset) {
        return Some(target);
    }

    if let Some((_, target)) = facts.associated_function_target_at_offset(offset) {
        return Some(target);
    }

    if let Some((_, target)) = facts.enum_variant_target_at_offset(offset) {
        return Some(target);
    }

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

fn function_call_definition_span_for_file_analysis(
    file: &FileAnalysis,
    offset: usize,
) -> Option<ByteSpan> {
    file.typecheck_facts
        .function_call_target_at_offset(offset)
        .map(|(_, target)| target)
}

fn field_definition_span_for_file_analysis(file: &FileAnalysis, offset: usize) -> Option<ByteSpan> {
    file.typecheck_facts
        .field_target_at_offset(offset)
        .map(|(_, target)| target)
}

fn associated_function_definition_span_for_file_analysis(
    file: &FileAnalysis,
    offset: usize,
) -> Option<ByteSpan> {
    file.typecheck_facts
        .associated_function_target_at_offset(offset)
        .map(|(_, target)| target)
}

fn enum_variant_definition_span_for_file_analysis(
    file: &FileAnalysis,
    offset: usize,
) -> Option<ByteSpan> {
    file.typecheck_facts
        .enum_variant_target_at_offset(offset)
        .map(|(_, target)| target)
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::{analyze_namespace_import_text, analyze_text};

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

    #[test]
    fn definition_query_resolves_struct_field_references() {
        let text = "struct File {\n    fd: i32\n}\n\nfunc main(): i32 {\n    let file = File { fd: 1 }\n    return file.fd\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.rfind("fd").expect("expected field reference");

        let span = definition_span_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected definition span");

        assert_eq!(&text[span.start..span.end], "fd");
        assert_eq!(span.start, text.find("fd: i32").expect("expected field"));
    }

    #[test]
    fn definition_query_resolves_struct_literal_fields() {
        let text = "struct File {\n    fd: i32\n}\n\nfunc main(): i32 {\n    let file = File { fd: 1 }\n    return file.fd\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("fd: 1").expect("expected literal field");

        let span = definition_span_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected definition span");

        assert_eq!(&text[span.start..span.end], "fd");
        assert_eq!(span.start, text.find("fd: i32").expect("expected field"));
    }

    #[test]
    fn definition_query_resolves_associated_function_calls() {
        let text = "struct File {\n    fd: i32\n}\n\nfunc File.open(): Self {\n    return Self { fd: 1 }\n}\n\nfunc main(): i32 {\n    return File.open().fd\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.rfind("open()").expect("expected associated call");

        let span = definition_span_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected definition span");

        assert_eq!(&text[span.start..span.end], "open");
        assert_eq!(
            span.start,
            text.find("open(): Self")
                .expect("expected associated function")
        );
    }

    #[test]
    fn definition_query_resolves_namespace_imported_function_member_call() {
        let root_text = "use lib/math\n\nfunc main(): i32 {\n    return math.answer()\n}\n";
        let module_text = "pub func answer(): i32 {\n    return 7\n}\n";
        let (sources, analysis) = analyze_namespace_import_text(root_text, module_text);
        let file = analysis.root_file().expect("expected root file");
        let offset = root_text.find("answer()").expect("expected namespace call");

        let span = definition_span_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected definition span");
        let target_text = sources
            .get(span.source)
            .expect("expected target source")
            .text();

        assert_eq!(&target_text[span.start..span.end], "answer");
        assert_eq!(
            span.start,
            module_text.find("answer():").expect("expected function")
        );
    }

    #[test]
    fn definition_query_resolves_enum_variant_references() {
        let text = "enum Event {\n    ready\n    count(value: i32)\n}\n\nfunc main(): i32 {\n    let ready = Event.ready\n    let count = Event.count(1)\n    return 0\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let ready_offset = text.rfind("ready").expect("expected payloadless variant");
        let count_offset = text.rfind("count(1)").expect("expected payload variant");

        let ready_span = definition_span_for_file_analysis(&sources, &analysis, file, ready_offset)
            .expect("expected ready definition span");
        let count_span = definition_span_for_file_analysis(&sources, &analysis, file, count_offset)
            .expect("expected count definition span");

        assert_eq!(&text[ready_span.start..ready_span.end], "ready");
        assert_eq!(
            ready_span.start,
            text.find("ready").expect("expected ready declaration")
        );
        assert_eq!(&text[count_span.start..count_span.end], "count");
        assert_eq!(
            count_span.start,
            text.find("count(value")
                .expect("expected count declaration")
        );
    }

    #[test]
    fn definition_query_resolves_enum_pattern_variant_references() {
        let text = r#"enum Choice {
    hit(value: i32)
    miss(value: i32)
}

func main(choice: Choice): i32 {
    if choice is Choice.hit(_) {
    }
    let code = match choice {
        Choice.hit(_) { 1 }
        Choice.miss(_) { 2 }
    }
    return code
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");

        for offset in [
            text.find("hit(_)").expect("expected if-is hit pattern"),
            text.rfind("hit(_)").expect("expected match hit pattern"),
        ] {
            let span = definition_span_for_file_analysis(&sources, &analysis, file, offset)
                .expect("expected hit definition span");
            assert_eq!(&text[span.start..span.end], "hit");
            assert_eq!(
                span.start,
                text.find("hit(value").expect("expected hit declaration")
            );
        }

        let miss_offset = text.rfind("miss(_)").expect("expected match miss pattern");
        let miss_span = definition_span_for_file_analysis(&sources, &analysis, file, miss_offset)
            .expect("expected miss definition span");
        assert_eq!(&text[miss_span.start..miss_span.end], "miss");
        assert_eq!(
            miss_span.start,
            text.find("miss(value").expect("expected miss declaration")
        );
    }
}
