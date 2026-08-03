use super::*;
use crate::analysis::test_support::{
    analyze_import_text, analyze_namespace_import_text, analyze_text,
    analyze_text_with_trusted_current_allocation_operation,
};

#[test]
fn workspace_hover_normalizes_literal_declaration_signatures() {
    let text = r#"type TextInput = &str

struct Text { value: &str }

literal Text ""(text: TextInput): Self from text {
    return Text { value: text }
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("\"\"(text").expect("expected literal shape");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "literal Text \"\"(text: &str): Text from text");
    assert_eq!(&text[hover.span.start..hover.span.end], "\"\"");
}

#[test]
fn associated_function_declaration_has_separate_owner_and_member_hover_targets() {
    let text = r#"struct File { fd: i32 }

func File.open(): Self {
    return File { fd: 1 }
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let declaration = text.find("File.open").expect("expected declaration");

    let owner = hover_for_file_analysis(&sources, &analysis, file, declaration)
        .expect("expected owner hover");
    let member = hover_for_file_analysis(&sources, &analysis, file, declaration + "File.".len())
        .expect("expected member hover");

    assert_eq!(owner.label, "struct File");
    assert_eq!(&text[owner.span.start..owner.span.end], "File");
    assert_eq!(member.label, "func File.open(): File");
    assert_eq!(&text[member.span.start..member.span.end], "open");
}

#[test]
fn workspace_hover_resolves_an_imported_name_at_its_import_site() {
    let root_text = "use lib/math.Error\n";
    let module_text = "/// A recoverable failure.\npub struct Error {\n    code: i32\n}\n";
    let (sources, analysis) = analyze_import_text(root_text, module_text);
    let file = analysis.root_file().expect("expected root file");
    let offset = root_text.find("Error").expect("expected imported name");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "struct Error");
    assert_eq!(
        hover.documentation.as_deref(),
        Some("A recoverable failure.")
    );
    assert_eq!(&root_text[hover.span.start..hover.span.end], "Error");
}

#[test]
fn workspace_hover_presents_typed_literal_signature_and_documentation() {
    let text = r#"struct Text { value: &str }

/// Copies text into owned storage.
literal Text ""(text: &str): Self from text {
    return Text { value: text }
}

func main(): i32 {
    let text = Text "hello"
    return 0
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("Text \"hello\"").unwrap();

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "literal Text \"\"(text: &str): Text from text");
    assert_eq!(
        hover.documentation.as_deref(),
        Some("Copies text into owned storage.\n\n**Result provenance:** input `text`.")
    );
}

#[test]
fn workspace_hover_uses_typecheck_facts_and_documentation() {
    let text = "func main(): i32 {\n    /// Exit code.\n    var code = 0\n    return code\n}\n";
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("return code").expect("expected reference") + "return ".len();

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "var code: i32");
    assert_eq!(hover.documentation.as_deref(), Some("Exit code."));
}

#[test]
fn workspace_hover_preserves_stored_composed_outcome_type() {
    let text = r#"func main(): i32 {
    let saved = lookup()
    let forwarded = saved
    return 0
}

func lookup(): i32?! {
    return 42
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("forwarded = saved").unwrap() + "forwarded = ".len();
    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "let saved: i32?!");
}

#[test]
fn workspace_hover_uses_normalized_typecheck_facts_for_function_reference() {
    let text = "type Exit = i32\n\nfunc answer(value: Exit): Exit {\n    return value\n}\n\nfunc main(): i32 {\n    return answer(1)\n}\n";
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("answer(1)").expect("expected reference");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "func answer(value: i32): i32");
}

#[test]
fn workspace_hover_uses_typecheck_facts_for_namespace_imported_function_member_call() {
    let root_text = "use lib/math\n\nfunc main(): i32 {\n    return math.answer()\n}\n";
    let module_text = "/// Computes an answer.\npub func answer(): i32 {\n    return 7\n}\n";
    let (sources, analysis) = analyze_namespace_import_text(root_text, module_text);
    let file = analysis.root_file().expect("expected root file");
    let offset = root_text.find("answer()").expect("expected namespace call");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "func answer(): i32");
    assert_eq!(hover.documentation.as_deref(), Some("Computes an answer."));
}

#[test]
fn workspace_hover_presents_imported_generic_call_specialization() {
    let root_text = "use lib/math\n\nfunc main(): i32 {\n    return math.identity(42)\n}\n";
    let module_text =
        "/// Returns its input.\npub func identity<T>(value: T): T {\n    return value\n}\n";
    let (sources, analysis) = analyze_namespace_import_text(root_text, module_text);
    let file = analysis.root_file().expect("expected root file");
    let offset = root_text
        .find("identity(42)")
        .expect("expected generic call");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "func identity<i32>(value: i32): i32");
    assert_eq!(
        hover.documentation.as_deref(),
        Some("Returns its input.\n\n**Result provenance:** input `value`.")
    );
}

#[test]
fn workspace_hover_uses_normalized_typecheck_facts_for_method_call() {
    let text = "type Count = i32\n\nstruct File {\n    fd: Count\n}\n\nimpl File {\n    /// Reads a count.\n    method &self.read(amount: Count): Count {\n        return amount\n    }\n}\n\nfunc main(): i32 {\n    let file = File { fd: 1 }\n    return file.read(1)\n}\n";
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("read(1)").expect("expected method call");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "method &File.read(amount: i32): i32");
    assert_eq!(hover.documentation.as_deref(), Some("Reads a count."));
}

#[test]
fn method_receiver_hover_uses_binding_name_and_semantic_owner_type() {
    let text = r#"struct File { fd: i32 }

impl File {
    method &self.read(): i32 {
        return self.fd
    }
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("self.read").expect("expected receiver binding");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "parameter self: &File");
    assert_eq!(&text[hover.span.start..hover.span.end], "self");
}

#[test]
fn workspace_hover_uses_normalized_typecheck_facts_for_associated_function_call() {
    let text = "struct File {\n    fd: i32\n}\n\n/// Opens a file.\nfunc File.open(): Self {\n    return Self { fd: 1 }\n}\n\nfunc main(): i32 {\n    return File.open().fd\n}\n";
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("open()").expect("expected associated call");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "func File.open(): File");
    assert_eq!(hover.documentation.as_deref(), Some("Opens a file."));
}

#[test]
fn workspace_hover_uses_normalized_typecheck_facts_for_struct_field() {
    let text = "type Count = i32\n\nstruct File {\n    fd: Count\n}\n";
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("fd:").expect("expected field");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "field File.fd: i32");
}

#[test]
fn workspace_hover_uses_typecheck_facts_for_struct_field_reference() {
    let text = "type Count = i32\n\nstruct File {\n    fd: Count\n}\n\nfunc main(): i32 {\n    let file = File { fd: 1 }\n    return file.fd\n}\n";
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("fd").expect("expected field reference");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "field File.fd: i32");
}

#[test]
fn workspace_hover_uses_normalized_typecheck_facts_for_enum_variant() {
    let text = "type Count = i32\n\nenum Event {\n    count(value: Count)\n}\n";
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("count(value").expect("expected variant");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "variant Event.count(value: i32)");
}

#[test]
fn workspace_hover_qualifies_generic_member_declarations() {
    let text = r#"struct Box<T> {
    value: T
}

enum Option<T> {
    some(value: T)
    empty
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");

    let field = hover_for_file_analysis(
        &sources,
        &analysis,
        file,
        text.find("value: T").expect("expected field"),
    )
    .expect("expected field hover");
    let variant = hover_for_file_analysis(
        &sources,
        &analysis,
        file,
        text.find("some(value").expect("expected variant"),
    )
    .expect("expected variant hover");
    let payloadless = hover_for_file_analysis(
        &sources,
        &analysis,
        file,
        text.find("empty").expect("expected payloadless variant"),
    )
    .expect("expected payloadless variant hover");

    assert_eq!(field.label, "field Box<T>.value: T");
    assert_eq!(variant.label, "variant Option<T>.some(value: T)");
    assert_eq!(payloadless.label, "variant Option<T>.empty");
}

#[test]
fn workspace_hover_uses_typecheck_facts_for_enum_variant_reference() {
    let text = "type Count = i32\n\nenum Event {\n    ready\n    count(value: Count)\n}\n\nfunc main(): i32 {\n    let event = Event.count(1)\n    return 0\n}\n";
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("count(1)").expect("expected variant reference");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "variant Event.count(value: i32)");
}

#[test]
fn workspace_hover_uses_typecheck_facts_for_enum_pattern_variant_reference() {
    let text = r#"enum Choice {
/// Selected hit.
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
    let offset = text
        .find("hit(_)")
        .expect("expected pattern variant reference");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "variant Choice.hit(value: i32)");
    assert_eq!(hover.documentation.as_deref(), Some("Selected hit."));
}

#[test]
fn workspace_hover_uses_typecheck_facts_for_payloadless_enum_variant_reference() {
    let text = "enum Event {\n    /// Ready to run.\n    ready\n}\n\nfunc main(): i32 {\n    let event = Event.ready\n    return 0\n}\n";
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("ready").expect("expected variant reference");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "variant Event.ready");
    assert_eq!(hover.documentation.as_deref(), Some("Ready to run."));
}

#[test]
fn workspace_hover_uses_typecheck_facts_for_type_reference() {
    let text = "/// Request header.\nstruct Header {\n    code: i32\n}\n\nfunc inspect(value: Header): i32 {\n    return value.code\n}\n";
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("value: Header").expect("expected type reference") + "value: ".len();

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "struct Header");
    assert_eq!(hover.documentation.as_deref(), Some("Request header."));
}

#[test]
fn workspace_hover_reports_transitive_allocation_effects() {
    let text = r#"primitive allocate(): usize

/// Builds a value.
func build(): usize {
    return allocate()
}

func main(): i32 {
    let value = build()
    return 0
}
"#;
    let (sources, analysis) =
        analyze_text_with_trusted_current_allocation_operation(text, "allocate");
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("build()").expect("expected call");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "func build(): usize");
    assert_eq!(
        hover.documentation.as_deref(),
        Some("Builds a value.\n\n**Allocation effect:** uses the current allocation context.")
    );
}

#[test]
fn workspace_hover_reports_lexical_region_context_for_declarations_and_references() {
    let text = r#"copy struct Arena {
    id: usize
}

func run(arena: Arena): i32 {
    region outer using arena {
        region inner using outer {
            return 1
        }
    }
    return 0
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");

    let declaration_offset = text.find("outer using").expect("expected declaration");
    let declaration = hover_for_file_analysis(&sources, &analysis, file, declaration_offset)
        .expect("expected declaration hover");
    assert_eq!(declaration.label, "region outer: Arena");
    assert_eq!(
        declaration.documentation.as_deref(),
        Some(
            "**Allocation context:** lexical region `outer` using `arena` (Arena); parent is the root allocation context. Its owned allocations are released when the region exits."
        )
    );

    let reference_offset = text.rfind("outer").expect("expected reference");
    let reference = hover_for_file_analysis(&sources, &analysis, file, reference_offset)
        .expect("expected reference hover");
    assert_eq!(reference.label, "region outer: Arena");
    assert_eq!(reference.documentation, declaration.documentation);
}

#[test]
fn workspace_hover_presents_bound_method_provenance_contract() {
    let text = r#"interface Lookup<V> {
    pub method &self.get(): &V from self
}

func read<M: Lookup<i32>>(map: &M): &i32 from map {
    return map.get()
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("get()").expect("expected bound call");

    let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
        .expect("expected bound method hover");

    assert_eq!(hover.label, "method &M.get(): &i32 from self");
    assert!(
        hover.documentation.as_deref().is_some_and(|documentation| {
            documentation.contains("Result provenance") && documentation.contains("input `self`")
        }),
        "{:?}",
        hover.documentation
    );
}

#[test]
fn workspace_hover_presents_concrete_receiver_for_interface_default_call() {
    let text = r#"interface Value {
    pub method &self.value(): i32 {
        return 42
    }
}

copy struct Unit {
    marker: i32
}

impl Value for Unit

func main(): i32 {
    let unit = Unit { marker: 0 }
    return unit.value()
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("unit.value").unwrap() + "unit.".len();

    let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
        .expect("expected default method hover");

    assert_eq!(hover.label, "method &Unit.value(): i32");
}

#[test]
fn workspace_hover_preserves_complete_capability_set() {
    let text = r#"interface Readable {
    pub method &self.read(): i32
}

interface Measurable {
    pub method &self.measure(): usize
}

func inspect<T: Readable + Measurable>(value: &T): i32 {
    return value.read()
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text
        .find("inspect<")
        .expect("expected function declaration");
    let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
        .expect("expected function hover");

    assert!(
        hover.label.contains("T: Readable + Measurable"),
        "{}",
        hover.label
    );
}
