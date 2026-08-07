use super::*;
use crate::analysis::test_support::{
    analyze_import_text, analyze_namespace_import_text, analyze_text,
    analyze_text_with_trusted_current_allocation_operation,
};

#[test]
fn native_test_hover_uses_the_fixed_contract_and_exact_name_range() {
    let text = "/// Keeps order.\ntest pushes { return }\n";
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("root file");
    let offset = text.find("pushes").unwrap();
    let hover = hover_for_file_analysis(&sources, &analysis, file, offset).expect("test hover");
    assert_eq!(hover.label, "test pushes: void!");
    assert_eq!(hover.documentation.as_deref(), Some("Keeps order."));
    assert_eq!(&text[hover.span.start..hover.span.end], "pushes");
}

#[test]
fn workspace_hover_normalizes_literal_declaration_signatures() {
    let text = r#"type TextInput = &str

struct Text { value: &str }

construct Text {
    pub default literal ""(text: TextInput): Self from text {
        return Text { value: text }
    }
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
fn workspace_hover_presents_coercion_entries_on_the_as_anchor() {
    let text = r#"struct Text { value: &str }
coerce Text {
    pub &self as &str from self { return self.value }
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("as &str").expect("expected as anchor");
    let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
        .expect("expected coercion hover");

    assert_eq!(hover.label, "pub &self as &str from self");
    assert_eq!(&text[hover.span.start..hover.span.end], "as");
}

#[test]
fn type_hover_lists_the_accessible_coercion_surface() {
    let text = r#"pub struct Text { value: &str }
coerce Text {
    pub &self as &str from self { return self.value }
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("struct Text").unwrap() + "struct ".len();
    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected type hover");
    let documentation = hover.documentation.expect("expected type documentation");

    assert!(documentation.contains("**Coercions**"));
    assert!(documentation.contains("`&Text as &str from self`"));
}

#[test]
fn coercion_hover_preserves_an_elided_result_origin() {
    let text = r#"pub struct Text { value: &str }
coerce Text {
    pub &self as &str { return self.value }
}
func project(value: &Text): &str { return value as &str }
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");

    let type_offset = text.find("struct Text").unwrap() + "struct ".len();
    let type_hover = hover_for_file_analysis(&sources, &analysis, file, type_offset)
        .expect("expected type hover");
    let type_documentation = type_hover
        .documentation
        .expect("expected type documentation");
    assert!(type_documentation.contains("`&Text as &str`"));
    assert!(!type_documentation.contains("from self"));

    let conversion_offset = text.rfind("as &str").expect("expected expression as");
    let conversion_hover = hover_for_file_analysis(&sources, &analysis, file, conversion_offset)
        .expect("expected conversion hover");
    let conversion_documentation = conversion_hover
        .documentation
        .expect("expected conversion documentation");
    assert!(conversion_documentation.contains("Selected `&Text as &str`"));
    assert!(!conversion_documentation.contains("from self"));
}

#[test]
fn explicit_coercion_hover_uses_the_exact_as_operator_and_selected_plan() {
    let text = r#"struct Text { value: &str }
coerce Text { pub &self as &str from self { return self.value } }
func project(value: &Text): &str from value { return value as &str }
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("as &str").expect("expected expression as");
    let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
        .expect("expected conversion hover");

    assert_eq!(&text[hover.span.start..hover.span.end], "as");
    assert_eq!(hover.label, "&Text as &str");
    let documentation = hover.documentation.expect("expected plan details");
    assert!(documentation.contains("type-owned borrow coercion"));
    assert!(documentation.contains("`&Text as &str from self`"));
}

#[test]
fn numeric_conversion_hover_uses_the_same_plan_presentation_boundary() {
    let text = "func widen(): i64 { return 1 as i64 }\n";
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("as i64").expect("expected expression as");
    let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
        .expect("expected conversion hover");

    assert_eq!(&text[hover.span.start..hover.span.end], "as");
    assert_eq!(hover.label, "i32 as i64");
    assert!(
        hover
            .documentation
            .unwrap()
            .contains("lossless integer conversion")
    );
}

#[test]
fn imported_explicit_coercion_hover_uses_the_selected_module_surface() {
    let root_text = r#"use lib/math.Text
func project(value: &Text): &str from value { return value as &str }
"#;
    let module_text = r#"pub struct Text { value: &str }
coerce Text { pub &self as &str from self { return self.value } }
"#;
    let (sources, analysis) = analyze_import_text(root_text, module_text);
    let file = analysis.root_file().expect("expected root file");
    let offset = root_text.rfind("as &str").expect("expected expression as");
    let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
        .expect("expected conversion hover");

    assert_eq!(&root_text[hover.span.start..hover.span.end], "as");
    assert_eq!(hover.label, "&Text as &str");
    assert!(
        hover
            .documentation
            .unwrap()
            .contains("`&Text as &str from self`")
    );
}

#[test]
fn construct_function_declaration_has_separate_owner_and_member_hover_targets() {
    let text = r#"struct File { fd: i32 }

construct File {
    pub default func open(): Self {
        return File { fd: 1 }
    }
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let owner_offset = text.find("construct File").unwrap() + "construct ".len();
    let member_offset = text.find("func open").unwrap() + "func ".len();

    let owner = hover_for_file_analysis(&sources, &analysis, file, owner_offset)
        .expect("expected owner hover");
    let member = hover_for_file_analysis(&sources, &analysis, file, member_offset)
        .expect("expected member hover");

    assert_eq!(owner.label, "struct File");
    assert_eq!(&text[owner.span.start..owner.span.end], "File");
    assert_eq!(member.label, "func File.open(): File");
    assert_eq!(&text[member.span.start..member.span.end], "open");
}

#[test]
fn construct_type_hover_explains_its_complete_public_surface() {
    let text = r#"pub struct Bucket<T> { pub value: T }

construct Bucket<T> {
    pub default func new(value: T): Self { return Bucket<T> { value: value } }
    pub literal [](...items: T): Self from items { return Bucket.new(move items[0]) }
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("struct Bucket").unwrap() + "struct ".len();

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected type hover");
    let documentation = hover
        .documentation
        .expect("expected construction documentation");

    assert_eq!(hover.label, "struct Bucket<T>");
    assert!(documentation.contains("**Construction**"));
    assert!(documentation.contains("default func Bucket<T>.new(value: T): Bucket<T>"));
    assert!(documentation.contains("literal Bucket<T> [](...items: T): Bucket<T> from items"));
    assert!(!documentation.contains("new<T>"));
}

#[test]
fn construct_member_hover_uses_normalized_owned_type_signature() {
    let text = r#"struct Bucket<T> { value: T }

construct Bucket<T> {
    pub default func new(value: T): Self { return Bucket<T> { value: value } }
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("func new").unwrap() + "func ".len();

    let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
        .expect("expected constructor member hover");

    assert_eq!(hover.label, "func Bucket<T>.new(value: T): Bucket<T>");
    assert_eq!(&text[hover.span.start..hover.span.end], "new");
}

#[test]
fn drop_declaration_has_separate_keyword_and_receiver_hover_targets() {
    let text = r#"struct Token { value: i32 }

impl Token {
    drop &+self {
        return
    }
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let declaration = text.find("drop &+self").expect("expected drop declaration");

    let drop_keyword = hover_for_file_analysis(&sources, &analysis, file, declaration)
        .expect("expected drop hover");
    let receiver =
        hover_for_file_analysis(&sources, &analysis, file, declaration + "drop &+".len())
            .expect("expected receiver hover");

    assert_eq!(drop_keyword.label, "drop &+self");
    assert_eq!(
        &text[drop_keyword.span.start..drop_keyword.span.end],
        "drop"
    );
    assert_eq!(&text[receiver.span.start..receiver.span.end], "self");
    assert!(receiver.label.starts_with("parameter self:"));
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
        Some(
            "A recoverable failure.\n\n**Construction**\n\nNo direct construction entry is available here."
        )
    );
    assert_eq!(&root_text[hover.span.start..hover.span.end], "Error");
}

#[test]
fn workspace_hover_presents_typed_literal_signature_and_documentation() {
    let text = r#"struct Text { value: &str }

construct Text {
    /// Copies text into owned storage.
    pub default literal ""(text: &str): Self from text {
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
    let offset = text.rfind("Text \"hello\"").unwrap();

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "literal Text \"\"(text: &str): Text from text");
    assert_eq!(
        hover.documentation.as_deref(),
        Some("Copies text into owned storage.")
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
    assert_eq!(hover.documentation.as_deref(), Some("Returns its input."));
}

#[test]
fn workspace_hover_summarizes_result_storage_without_private_layout() {
    let text = r#"struct Storage {
    pointer: *i32,
    allocator_state: usize,
}

struct Values {
    storage: Storage,
    end_index: usize,
}

func into_values(storage: Storage, len: usize): Values {
    return Values {
        storage: move storage,
        end_index: len,
    }
}

func inspect(storage: Storage): Values {
    return into_values(move storage, 1)
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("root file");
    let offset = text.rfind("into_values").expect("call");
    let hover = hover_for_file_analysis(&sources, &analysis, file, offset).expect("call hover");
    assert!(hover.documentation.is_none());
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
    assert_eq!(
        hover.documentation.as_deref(),
        Some("Request header.\n\n**Construction**\n\n- `default Header { code: i32 }`")
    );
}

#[test]
fn workspace_hover_presents_contextual_interface_type_applications() {
    let text = r#"interface Iterator<T> {
    pub method &self.next(): T?
}

interface ExactSizeIterator<T> {}

struct Indexed<T> { value: T }
struct EnumerateIter<T, I> { source: I }

pub func filter<T, I: Iterator<T>>(source: I): I from source {
    return source
}

impl<T, I: ExactSizeIterator<T>> ExactSizeIterator<Indexed<T>> for EnumerateIter<T, I> {}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");

    let declaration = hover_for_file_analysis(&sources, &analysis, file, "interface ".len())
        .expect("expected interface declaration hover");
    assert_eq!(declaration.label, "interface Iterator<T>");
    assert_eq!(
        &text[declaration.span.start..declaration.span.end],
        "Iterator"
    );

    let iterator_offset = text.find("I: Iterator").unwrap() + "I: ".len();
    let iterator = hover_for_file_analysis(&sources, &analysis, file, iterator_offset)
        .expect("expected function-bound hover");
    assert_eq!(iterator.label, "interface Iterator<T>");
    assert_eq!(&text[iterator.span.start..iterator.span.end], "Iterator");

    let impl_bound_offset = text.find("I: ExactSizeIterator").unwrap() + "I: ".len();
    let impl_bound = hover_for_file_analysis(&sources, &analysis, file, impl_bound_offset)
        .expect("expected impl-bound hover");
    assert_eq!(impl_bound.label, "interface ExactSizeIterator<T>");
    assert_eq!(
        &text[impl_bound.span.start..impl_bound.span.end],
        "ExactSizeIterator"
    );

    let implemented_offset = text.rfind(">> ExactSizeIterator").unwrap() + ">> ".len();
    let implemented = hover_for_file_analysis(&sources, &analysis, file, implemented_offset)
        .expect("expected implemented-interface hover");
    assert_eq!(implemented.label, "interface ExactSizeIterator<Indexed<T>>");
    assert_eq!(
        &text[implemented.span.start..implemented.span.end],
        "ExactSizeIterator"
    );
}

#[test]
fn workspace_hover_presents_method_generic_parameter_identity() {
    let text = r#"interface Iterator<T> {}

interface Transform<T> {
    pub method &self.map<U: Iterator<T>>(value: U): T
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let declaration_offset = text.find("U: Iterator").unwrap();
    let reference_offset = text.find("value: U").unwrap() + "value: ".len();

    for offset in [declaration_offset, reference_offset] {
        let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected generic parameter hover");
        assert_eq!(hover.label, "type parameter U: Iterator<T>");
        assert_eq!(&text[hover.span.start..hover.span.end], "U");
    }

    let target = crate::analysis::definition::definition_target_for_file_analysis(
        &sources,
        &analysis,
        file,
        reference_offset,
    )
    .expect("expected generic parameter definition");
    assert_eq!(target.declaration_span.start, declaration_offset);
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
    assert_eq!(hover.documentation.as_deref(), Some("Builds a value."));
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
    assert!(hover.documentation.is_none());
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

impl Value for Unit {}

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
fn workspace_hover_presents_conformance_member_with_concrete_receiver() {
    let text = r#"interface Lookup<V> {
    pub method &self.get(): &V from self
}

struct Box<T> { value: T }

impl<T> Lookup<T> for Box<T> {
    method &self.get(): &T from self {
        return &self.value
    }
}

func main(box: &Box<i32>): &i32 from box {
    return box.get()
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("get()").expect("expected concrete call");

    let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
        .expect("expected conformance method hover");

    assert_eq!(hover.label, "method &Box<i32>.get(): &i32 from self");
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

#[test]
fn workspace_hover_presents_closure_parameters_and_capture_modes() {
    let text = r#"func main(): i32 {
    let factor = 2
    let transform = (&factor; value: i32): i32 { value * factor }
    return 0
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");

    let parameter_offset = text.find("value *").unwrap();
    let parameter = hover_for_file_analysis(&sources, &analysis, file, parameter_offset)
        .expect("expected closure parameter hover");
    assert_eq!(parameter.label, "parameter value: i32");

    let capture_offset = text.rfind("factor }").unwrap();
    let capture = hover_for_file_analysis(&sources, &analysis, file, capture_offset)
        .expect("expected closure capture hover");
    assert_eq!(capture.label, "capture &factor: i32");

    let binding_offset = text.find("transform =").unwrap();
    let binding = hover_for_file_analysis(&sources, &analysis, file, binding_offset)
        .expect("expected closure binding hover");
    assert_eq!(binding.label, "let transform: closure (i32): i32");
}

#[test]
fn workspace_hover_presents_catch_binding_declaration_and_reference() {
    let text = r#"func attempt(): i32! {
    return 1
}

func main(): i32! {
    let value = attempt() catch problem {
        return problem
    }
    return value
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");

    let declaration_offset = text.find("problem {").expect("expected catch binding");
    let declaration = hover_for_file_analysis(&sources, &analysis, file, declaration_offset)
        .expect("expected catch binding declaration hover");
    assert_eq!(declaration.label, "catch problem: error");
    assert_eq!(
        &text[declaration.span.start..declaration.span.end],
        "problem"
    );

    let reference_offset = text
        .rfind("problem")
        .expect("expected catch binding reference");
    let reference = hover_for_file_analysis(&sources, &analysis, file, reference_offset)
        .expect("expected catch binding reference hover");
    assert_eq!(reference.label, "catch problem: error");
    assert_eq!(&text[reference.span.start..reference.span.end], "problem");
}
