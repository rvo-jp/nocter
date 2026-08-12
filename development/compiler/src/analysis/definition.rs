//! Go-to-definition queries derived from compile-unit analysis.

use super::single_file::{parse_single_file_text, resolve_single_file_ast};
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::analysis::editor_targets::SourceTarget;
use crate::analysis::hover::definition_target_for_ast as hover_definition_target_for_ast;
use crate::ast::AstFile;
use crate::resolve::ResolveOutput;
#[cfg(test)]
use crate::source::ByteSpan;
use crate::source::{SourceId, SourceMap};
use crate::typecheck::collect_typecheck_facts;

#[cfg(test)]
pub(crate) fn definition_span_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<ByteSpan> {
    definition_target_for_file_analysis(sources, analysis, file, offset)
        .map(|target| target.declaration_span)
}

pub(crate) fn definition_target_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<SourceTarget> {
    crate::analysis::editor_targets::editor_target_at_offset(file, offset)
        .and_then(|target| target.source_target(analysis))
        .or_else(|| {
            crate::analysis::conversions::conversion_definition_target_at_offset(
                &file.typecheck_facts,
                offset,
            )
        })
        .or_else(|| {
            crate::analysis::literals::literal_definition_target_at_offset(analysis, file, offset)
        })
        .or_else(|| {
            file.occurrences
                .at_offset(offset)
                .and_then(|occurrence| occurrence.source_target(analysis))
        })
        .or_else(|| {
            let text = sources.get(file.ast.span.source)?.text();
            hover_definition_target_for_ast(text, &file.ast, &file.resolved, offset)
        })
}

pub(crate) fn definition_target_for_ast(
    text: &str,
    ast: &AstFile,
    resolved: &ResolveOutput,
    offset: usize,
) -> Option<SourceTarget> {
    let facts = collect_typecheck_facts(ast, resolved);
    if let Some(target) =
        crate::analysis::conversions::conversion_definition_target_at_offset(&facts, offset)
    {
        return Some(target);
    }
    if let Some((origin, target)) = facts.field_target_at_offset(offset) {
        return Some(SourceTarget::new(origin, target));
    }

    if let Some((origin, target)) = facts.function_call_target_at_offset(offset) {
        return Some(SourceTarget::new(origin, target));
    }

    if let Some((origin, target)) = facts.associated_function_target_at_offset(offset) {
        return Some(SourceTarget::new(origin, target));
    }

    if let Some((origin, target)) = facts.enum_variant_target_at_offset(offset) {
        return Some(SourceTarget::new(origin, target));
    }

    hover_definition_target_for_ast(text, ast, resolved, offset)
}

pub(crate) fn definition_target_for_text(text: &str, offset: usize) -> Option<SourceTarget> {
    definition_target_for_complete_text(text, offset).or_else(|| {
        let recovered = super::delimiter_recovery::block_recovery_text(text, text.len())?;
        definition_target_for_complete_text(&recovered, offset)
    })
}

fn definition_target_for_complete_text(text: &str, offset: usize) -> Option<SourceTarget> {
    let parsed = parse_single_file_text("definition.nct", text)?;
    let resolved = resolve_single_file_for_definition(text, parsed.source, &parsed.ast);

    definition_target_for_ast(text, &parsed.ast, &resolved, offset)
}

pub(crate) fn resolve_single_file_for_definition(
    text: &str,
    source: SourceId,
    ast: &AstFile,
) -> ResolveOutput {
    resolve_single_file_ast("definition.nct", text, source, ast)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::{
        analyze_import_text, analyze_namespace_import_text, analyze_text,
    };

    #[test]
    fn definition_query_survives_an_unclosed_function_body() {
        let text = "func main(): i32 {\n    let code = 0\n    return code\n";
        let reference = text.rfind("code").expect("expected reference");

        let target = definition_target_for_text(text, reference)
            .expect("expected recovered definition target");

        assert_eq!(
            &text[target.focus_span.start..target.focus_span.end],
            "code"
        );
        assert_eq!(
            &text[target.declaration_span.start..target.declaration_span.end],
            "code"
        );
        assert_eq!(target.declaration_span.start, text.find("code =").unwrap());
    }

    #[test]
    fn native_test_declaration_defines_only_its_name_span() {
        let text = "test pushes { return }\n";
        let offset = text.find("pushes").unwrap();
        let target = definition_target_for_text(text, offset).expect("test definition");
        assert_eq!(
            &text[target.focus_span.start..target.focus_span.end],
            "pushes"
        );
        assert_eq!(target.focus_span, target.declaration_span);
        assert!(definition_target_for_text(text, 0).is_none());
    }

    #[test]
    fn definition_query_keeps_the_whole_module_path_as_its_origin() {
        let root_text = "use lib/math\n";
        let module_text = "pub func answer(): i32 { return 7 }\n";
        let (sources, analysis) = analyze_namespace_import_text(root_text, module_text);
        let file = analysis.root_file().expect("expected root file");

        let target = definition_target_for_file_analysis(&sources, &analysis, file, 5)
            .expect("expected module target");

        assert_eq!(
            &root_text[target.focus_span.start..target.focus_span.end],
            "lib/math"
        );
        assert_ne!(target.declaration_span.source, file.ast.span.source);
    }

    #[test]
    fn definition_query_resolves_an_imported_name_at_its_import_site() {
        let root_text = "use lib/math.Error\n";
        let module_text = "pub struct Error { code: i32 }\n";
        let (sources, analysis) = analyze_import_text(root_text, module_text);
        let file = analysis.root_file().expect("expected root file");
        let offset = root_text.find("Error").expect("expected imported name");

        let target = definition_target_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected imported definition target");
        let target_text = sources
            .get(target.declaration_span.source)
            .expect("expected target source")
            .text();

        assert_eq!(
            &root_text[target.focus_span.start..target.focus_span.end],
            "Error"
        );
        assert_eq!(
            &target_text[target.declaration_span.start..target.declaration_span.end],
            "Error"
        );
    }

    #[test]
    fn definition_query_resolves_typed_literal_delimiter_to_shape_declaration() {
        let text = r#"struct Text { value: &str }

construct Text {
    pub default literal ""(text: &str): Self {
        return Text { value: text }
    }

}

func main(): i32 {
    let text = Text "hello"
    return 0
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.rfind("\"hello\"").unwrap();

        let span = definition_span_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected literal definition span");

        assert_eq!(&text[span.start..span.end], "\"\"");
        assert_eq!(span.start, text.find("\"\"(text").unwrap());
    }

    #[test]
    fn opaque_associated_binding_navigates_to_interface_declaration() {
        let text = r#"interface Source {
    pub type Item
}
struct Number { value: i32 }
conform Source for Number { type Item = i32 }
func make(): some Source<Item = i32> { return Number { value: 7 } }
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.rfind("Item =").expect("expected opaque binding");
        let target = definition_target_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected associated type target");

        assert_eq!(
            &text[target.focus_span.start..target.focus_span.end],
            "Item"
        );
        assert_eq!(
            &text[target.declaration_span.start..target.declaration_span.end],
            "Item"
        );
        assert_eq!(target.declaration_span.start, text.find("Item").unwrap());
    }

    #[test]
    fn definition_query_resolves_explicit_as_to_the_selected_coercion_entry() {
        let text = r#"struct Text { value: &str }
coerce Text { pub &self as &str from self { return self.value } }
func project(value: &Text): &str from value { return value as &str }
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let expression_as = text.rfind("as &str").expect("expected expression as");
        let target = definition_target_for_file_analysis(&sources, &analysis, file, expression_as)
            .expect("expected coercion definition");

        assert_eq!(&text[target.focus_span.start..target.focus_span.end], "as");
        assert_eq!(
            &text[target.declaration_span.start..target.declaration_span.end],
            "as"
        );
        assert_eq!(target.declaration_span.start, text.find("as &str").unwrap());
    }

    #[test]
    fn numeric_as_has_no_conversion_declaration_target() {
        let text = "func widen(): i64 { return 1 as i64 }\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let expression_as = text.find("as i64").expect("expected expression as");

        assert!(
            definition_target_for_file_analysis(&sources, &analysis, file, expression_as).is_none()
        );
    }

    #[test]
    fn imported_explicit_coercion_navigates_to_its_defining_module() {
        let root_text = r#"use lib/math.Text
func project(value: &Text): &str from value { return value as &str }
"#;
        let module_text = r#"pub struct Text { value: &str }
coerce Text { pub &self as &str from self { return self.value } }
"#;
        let (sources, analysis) = analyze_import_text(root_text, module_text);
        let file = analysis.root_file().expect("expected root file");
        let expression_as = root_text.rfind("as &str").expect("expected expression as");
        let target = definition_target_for_file_analysis(&sources, &analysis, file, expression_as)
            .expect("expected imported coercion definition");
        let declaration_source = sources
            .get(target.declaration_span.source)
            .expect("expected declaration source");

        assert_eq!(
            &root_text[target.focus_span.start..target.focus_span.end],
            "as"
        );
        assert_eq!(
            &declaration_source.text()[target.declaration_span.start..target.declaration_span.end],
            "as"
        );
        assert_ne!(target.declaration_span.source, file.ast.span.source);
    }

    #[test]
    fn imported_private_coercion_is_diagnosed_at_explicit_as() {
        let root_text = r#"use lib/math.Text
func project(value: &Text): &str from value { return value as &str }
"#;
        let module_text = r#"pub struct Text { value: &str }
coerce Text { &self as &str from self { return self.value } }
"#;
        let (_sources, analysis) = analyze_import_text(root_text, module_text);
        let diagnostics = analysis.diagnostics();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("selects a coercion that is not accessible here")
        }));
    }

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
    fn definition_query_resolves_closure_parameters_and_captures() {
        let text = r#"func main(): i32 {
    let factor = 2
    let transform = (&factor; value: i32): i32 { value * factor }
    return transform(3)
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");

        let parameter_offset = text.find("value *").expect("expected parameter use");
        let parameter =
            definition_span_for_file_analysis(&sources, &analysis, file, parameter_offset)
                .expect("expected closure parameter definition");
        assert_eq!(parameter.start, text.find("value: i32").unwrap());

        let capture_offset = text.rfind("factor }").expect("expected capture use");
        let capture = definition_span_for_file_analysis(&sources, &analysis, file, capture_offset)
            .expect("expected closure capture definition");
        assert_eq!(capture.start, text.find("&factor").unwrap() + 1);
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
    fn definition_query_resolves_associated_binding_and_projection() {
        let text = r#"interface Source {
    pub type Item
}
struct NumberSource { value: i32 }
conform Source for NumberSource {
    type Item = i32
}
func project<S>(source: S): S.Item where S: Source { return source }
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let expected = text.find("type Item").unwrap() + "type ".len();
        for offset in [
            text.rfind("type Item").unwrap() + "type ".len(),
            text.rfind("S.Item").unwrap() + "S.".len(),
        ] {
            let span = definition_span_for_file_analysis(&sources, &analysis, file, offset)
                .expect("associated type definition");
            assert_eq!(span.start, expected);
            assert_eq!(&text[span.start..span.end], "Item");
        }
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
    fn definition_query_resolves_construct_function_calls() {
        let text = r#"struct Bucket<T> { value: T }

construct Bucket<T> {
    pub default func new(value: T): Self {
        return Bucket<T> { value: value }
    }
}

func main(): i32 {
    let bucket = Bucket.new(42)
    return 0
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.rfind("new(42)").expect("expected construct call");

        let span = definition_span_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected definition span");

        assert_eq!(&text[span.start..span.end], "new");
        assert_eq!(span.start, text.find("new(value").unwrap());
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

    #[test]
    fn definition_query_resolves_bound_call_to_interface_method() {
        let text = r#"interface Measure {
    pub method &self.measure(): i32
}

func read<T>(value: &T): i32 where T: Measure {
    return value.measure()
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.rfind("measure()").expect("expected bound call");

        let span = definition_span_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected interface method definition");

        assert_eq!(&text[span.start..span.end], "measure");
        assert_eq!(span.start, text.find("measure():").unwrap());
    }

    #[test]
    fn definition_query_resolves_equality_use_to_operator_token() {
        let text = r#"struct Text { value: i32 }
instance Text {
    operator (&self == other: &Self): bool { return self.value == other.value }
}
func equal(left: &Text, right: &Text): bool { return left == right }
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("root file");
        let use_offset = text.rfind("== right").expect("operator use");

        let span = definition_span_for_file_analysis(&sources, &analysis, file, use_offset)
            .expect("operator definition");

        assert_eq!(&text[span.start..span.end], "==");
        assert_eq!(span.start, text.find("== other").unwrap());
    }

    #[test]
    fn definition_query_resolves_derived_ordering_to_strict_order_token() {
        let text = r#"struct Rank { value: i32 }
instance Rank {
    operator (&self < other: &Self): bool { return self.value < other.value }
}
func ordered(left: &Rank, right: &Rank): bool { return left >= right }
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("root file");
        let use_offset = text.rfind(">= right").expect("derived operator use");

        let span = definition_span_for_file_analysis(&sources, &analysis, file, use_offset)
            .expect("strict-order definition");

        assert_eq!(&text[span.start..span.end], "<");
        assert_eq!(span.start, text.find("< other").unwrap());
    }

    #[test]
    fn definition_query_resolves_index_use_to_operator_bracket() {
        let text = r#"struct Buffer { values: [i32; 1] }
instance Buffer {
    operator (&self[index: usize]): &i32 { return &self.values[index] }
}
func read(buffer: &Buffer, index: usize): i32 { return buffer[index] }
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("root file");
        let use_offset = text.rfind("[index]").expect("operator use");

        let span = definition_span_for_file_analysis(&sources, &analysis, file, use_offset)
            .expect("operator definition");

        assert_eq!(&text[span.start..span.end], "[");
        assert_eq!(span.start, text.find("[index: usize]").unwrap());
    }

    #[test]
    fn definition_query_resolves_concrete_call_to_conformance_member() {
        let text = r#"interface Measure {
    pub method &self.measure(): i32
}

struct Count { value: i32 }

conform Measure for Count {
    method &self.measure(): i32 {
        return self.value
    }
}

func main(): i32 {
    let count = Count { value: 7 }
    return count.measure()
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.rfind("measure()").expect("expected concrete call");

        let span = definition_span_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected conformance member definition");

        assert_eq!(&text[span.start..span.end], "measure");
        assert_eq!(
            span.start,
            text.find("method &self.measure(): i32 {").unwrap() + 13
        );
    }

    #[test]
    fn definition_query_resolves_imported_associated_type_identity() {
        let root_text = r#"use lib/math.Source

struct Number { value: i32 }

conform Source for Number {
    type Item = i32
}

func project<S>(source: S): S.Item where S: Source {
    return source
}
"#;
        let module_text = r#"pub interface Source {
    pub type Item
}
"#;
        let (sources, analysis) = analyze_import_text(root_text, module_text);
        let file = analysis.root_file().expect("expected root file");
        let offset = root_text.rfind("S.Item").unwrap() + "S.".len();

        let target = definition_target_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected imported associated type definition");
        let target_text = sources
            .get(target.declaration_span.source)
            .expect("expected imported source")
            .text();

        assert_eq!(
            &root_text[target.focus_span.start..target.focus_span.end],
            "Item"
        );
        assert_eq!(
            &target_text[target.declaration_span.start..target.declaration_span.end],
            "Item"
        );
        assert_ne!(target.focus_span.source, target.declaration_span.source);
    }
}
