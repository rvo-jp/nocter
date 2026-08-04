//! Find-references queries derived from compile-unit analysis.

use super::single_file::{parse_single_file_text, resolve_single_file_ast};
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::analysis::occurrences::{
    SemanticIdentity, SemanticOccurrenceIndex, SemanticOccurrenceRole,
};
use crate::source::ByteSpan;
use crate::typecheck::collect_typecheck_facts;

pub(crate) fn reference_spans_for_file_analysis(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
    include_declaration: bool,
) -> Vec<ByteSpan> {
    let Some(target) = file
        .occurrences
        .at_offset(offset)
        .and_then(|occurrence| occurrence.identity)
    else {
        return Vec::new();
    };

    reference_spans_for_semantic_identity(
        analysis.files.iter().map(|file| &file.occurrences),
        target,
        include_declaration,
    )
}

pub(crate) fn reference_spans_for_text(
    text: &str,
    offset: usize,
    include_declaration: bool,
) -> Option<Vec<ByteSpan>> {
    reference_spans_for_complete_text(text, offset, include_declaration).or_else(|| {
        let recovered = super::delimiter_recovery::block_recovery_text(text, text.len())?;
        reference_spans_for_complete_text(&recovered, offset, include_declaration)
    })
}

fn reference_spans_for_complete_text(
    text: &str,
    offset: usize,
    include_declaration: bool,
) -> Option<Vec<ByteSpan>> {
    let parsed = parse_single_file_text("references.nct", text)?;
    let resolved = resolve_single_file_ast("references.nct", text, parsed.source, &parsed.ast);
    let facts = collect_typecheck_facts(&parsed.ast, &resolved);
    let occurrences = SemanticOccurrenceIndex::new(&parsed.ast, &resolved, &facts);
    let target = occurrences.at_offset(offset)?.identity?;
    let spans = reference_spans_for_semantic_identity(
        std::iter::once(&occurrences),
        target,
        include_declaration,
    );

    Some(spans)
}

fn reference_spans_for_semantic_identity<'a>(
    indexes: impl Iterator<Item = &'a SemanticOccurrenceIndex>,
    target: SemanticIdentity,
    include_declaration: bool,
) -> Vec<ByteSpan> {
    let spans = indexes
        .flat_map(SemanticOccurrenceIndex::iter)
        .filter(|occurrence| occurrence.identity == Some(target))
        .filter(|occurrence| {
            include_declaration || occurrence.role != SemanticOccurrenceRole::Declaration
        })
        .map(|occurrence| occurrence.focus_span)
        .collect();
    sort_and_dedup_spans(spans)
}

fn sort_and_dedup_spans(mut spans: Vec<ByteSpan>) -> Vec<ByteSpan> {
    spans.sort_by_key(|span| (span.source.raw(), span.start, span.end));
    spans.dedup_by_key(|span| (span.source.raw(), span.start, span.end));
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::{
        analyze_namespace_import_text, analyze_text, span_fragments_from_sources,
    };

    #[test]
    fn reference_query_survives_an_unclosed_function_body() {
        let text = "func main(): i32 {\n    let code = 0\n    return code + code\n";
        let offset = text.find("code =").expect("expected declaration");

        let spans =
            reference_spans_for_text(text, offset, true).expect("expected recovered references");

        assert_eq!(span_fragments(text, &spans), vec!["code", "code", "code"]);
    }

    #[test]
    fn reference_query_finds_local_binding_references() {
        let text = "func main(): i32 {\n    let code = 0\n    return code + code\n}\n";
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("code = 0").expect("expected declaration");

        let spans = reference_spans_for_file_analysis(&analysis, file, offset, true);
        let fragments = span_fragments(text, &spans);

        assert_eq!(fragments, vec!["code", "code", "code"]);
    }

    #[test]
    fn reference_query_distinguishes_capture_from_outer_binding() {
        let text = r#"func main(): i32 {
    let factor = 2
    let transform = (&factor; value: i32): i32 { value * factor }
    return transform(3)
}
"#;
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let capture_offset = text.find("&factor").unwrap() + 1;

        let spans = reference_spans_for_file_analysis(&analysis, file, capture_offset, true);
        let fragments = span_fragments(text, &spans);

        assert_eq!(fragments, vec!["factor", "factor"]);
        assert_eq!(spans[0].start, text.find("factor =").unwrap());
        assert_eq!(spans[1].start, capture_offset);

        let body_offset = text.rfind("factor }").unwrap();
        let body_spans = reference_spans_for_file_analysis(&analysis, file, body_offset, true);
        assert_eq!(span_fragments(text, &body_spans), vec!["factor", "factor"]);
        assert_eq!(body_spans[0].start, capture_offset);
        assert_eq!(body_spans[1].start, body_offset);
    }

    #[test]
    fn reference_query_finds_top_level_function_references() {
        let text = "func answer(): i32 {\n    return 1\n}\n\nfunc main(): i32 {\n    return answer() + answer()\n}\n";
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("answer():").expect("expected declaration");

        let spans = reference_spans_for_file_analysis(&analysis, file, offset, true);
        let fragments = span_fragments(text, &spans);

        assert_eq!(fragments, vec!["answer", "answer", "answer"]);
    }

    #[test]
    fn reference_query_finds_namespace_imported_function_member_calls() {
        let root_text =
            "use lib/math\n\nfunc main(): i32 {\n    return math.answer() + math.answer()\n}\n";
        let module_text = "pub func answer(): i32 {\n    return 7\n}\n";
        let (sources, analysis) = analyze_namespace_import_text(root_text, module_text);
        let file = analysis.root_file().expect("expected root file");
        let offset = root_text.find("answer()").expect("expected namespace call");

        let spans = reference_spans_for_file_analysis(&analysis, file, offset, true);
        let fragments = span_fragments_from_sources(&sources, &spans);

        assert_eq!(fragments, vec!["answer", "answer", "answer"]);
        assert_eq!(spans.len(), 3);
    }

    #[test]
    fn reference_query_finds_type_references() {
        let text =
            "struct File {\n    fd: i32\n}\n\nfunc open(file: File): File {\n    return file\n}\n";
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("File {").expect("expected declaration");

        let spans = reference_spans_for_file_analysis(&analysis, file, offset, true);
        let fragments = span_fragments(text, &spans);

        assert_eq!(fragments, vec!["File", "File", "File"]);
    }

    #[test]
    fn reference_query_finds_member_references() {
        let text = "struct File {\n    fd: i32\n}\n\nimpl File {\n    method &self.read(): i32 {\n        return self.fd\n    }\n}\n\nfunc main(): i32 {\n    let file = File { fd: 1 }\n    return file.fd + file.read()\n}\n";
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let field_offset = text.find("fd: i32").expect("expected field");
        let method_offset = text.find("read():").expect("expected method");

        let field_spans = reference_spans_for_file_analysis(&analysis, file, field_offset, true);
        let method_spans = reference_spans_for_file_analysis(&analysis, file, method_offset, true);

        assert_eq!(
            span_fragments(text, &field_spans),
            vec!["fd", "fd", "fd", "fd"]
        );
        assert_eq!(span_fragments(text, &method_spans), vec!["read", "read"]);
    }

    #[test]
    fn reference_query_finds_enum_pattern_variant_references() {
        let text = r#"enum Choice {
    hit(value: i32)
    miss(value: i32)
}

func main(choice: Choice): i32 {
    let event = Choice.hit(1)
    if choice is Choice.hit(_) {
    }
    let code = match choice {
        Choice.hit(_) { 1 }
        Choice.miss(_) { 2 }
    }
    return code
}
"#;
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text
            .find("hit(value")
            .expect("expected variant declaration");

        let spans = reference_spans_for_file_analysis(&analysis, file, offset, true);
        let fragments = span_fragments(text, &spans);

        assert_eq!(fragments, vec!["hit", "hit", "hit", "hit"]);
    }

    #[test]
    fn reference_query_groups_bound_calls_with_interface_method() {
        let text = r#"interface Measure {
    pub method &self.measure(): i32
}

func first<T: Measure>(value: &T): i32 {
    return value.measure()
}

func second<U: Measure>(value: &U): i32 {
    return value.measure()
}
"#;
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("measure():").expect("expected declaration");

        let spans = reference_spans_for_file_analysis(&analysis, file, offset, true);

        assert_eq!(
            span_fragments(text, &spans),
            vec!["measure", "measure", "measure"]
        );
    }

    fn span_fragments<'a>(text: &'a str, spans: &[ByteSpan]) -> Vec<&'a str> {
        spans
            .iter()
            .map(|span| &text[span.start..span.end])
            .collect()
    }
}
