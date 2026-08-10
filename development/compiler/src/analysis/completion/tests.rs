use super::*;
use crate::analysis::test_support::{
    analyze_namespace_import_text, analyze_text, analyze_text_with_trusted_allocator_capabilities,
};

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
            && detail_starts_with(item, "struct File")
    }));
    assert!(items.iter().any(|item| {
        item.label == "main"
            && item.kind == CompletionItemKind::Function
            && detail_starts_with(item, "func main")
    }));
    assert_eq!(source.text(), text);
}

#[test]
fn copy_completion_is_scoped_to_generic_requirements() {
    let text = r#"interface Copyable {}

func duplicate<T>(value: T): T where T: Copyable {
    return value
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let generic_offset = text.find("<T").expect("generic list") + 1;
    let where_offset = text.find("where ").expect("where clause") + "where ".len();
    let body_offset = text.find("return").expect("body");

    let generic_items = completion_items_for_file_analysis_at_offset(file, generic_offset);
    let where_items = completion_items_for_file_analysis_at_offset(file, where_offset);
    let body_items = completion_items_for_file_analysis_at_offset(file, body_offset);
    assert!(!generic_items.iter().any(|item| item.label == "copy"));
    assert!(
        where_items
            .iter()
            .any(|item| { item.label == "copy" && item.kind == CompletionItemKind::Keyword })
    );
    assert!(!body_items.iter().any(|item| item.label == "copy"));
    assert!(sources.get(file.ast.span.source).is_some());
}

#[test]
fn completion_recovers_an_empty_where_predicate_and_offers_copy() {
    let text = r#"func duplicate<T>(value: T): T where {
    return value
}
"#;
    let offset = text.find("where ").expect("where clause") + "where ".len();
    let items = completion_items_for_text_at_offset(text, offset)
        .expect("expected recovered where-predicate completion");

    assert!(
        items
            .iter()
            .any(|item| { item.label == "copy" && item.kind == CompletionItemKind::Keyword })
    );
}

#[test]
fn copy_completion_is_not_offered_as_an_interface_bound() {
    let text = r#"interface Copyable {}

func duplicate<T>(value: T): T where T: {
    return value
}
"#;
    let offset = text.find("T: ").expect("bound") + "T: ".len();
    let items = completion_items_for_text_at_offset(text, offset)
        .expect("expected recovered interface-bound completion");

    assert!(!items.iter().any(|item| item.label == "copy"));
    assert!(items.iter().any(|item| item.label == "Copyable"));
}

#[test]
fn completion_candidates_offer_declared_literal_shapes_after_target() {
    let text = r#"struct Bucket<T> { length: usize }

construct Bucket<T> {
    pub default literal [](...items: T): Self {
        return Bucket<T> { length: items.len() }
    }
}

func main(): i32 {
    let values: Bucket<i32> = Bucket []
    return 0
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("Bucket []").unwrap() + "Bucket ".len();

    let items = literal_shape_completion_items_for_file_analysis_at_offset(file, offset)
        .expect("expected literal shape completion");
    let sequence = items
        .iter()
        .find(|item| item.label == "[]")
        .expect("expected sequence shape");

    assert_eq!(sequence.kind, CompletionItemKind::Constructor);
    assert_eq!(
        sequence.detail.as_deref(),
        Some("literal Bucket<T> [](...items: T): Bucket<T>")
    );
    assert_eq!(sequence.insert_text.as_deref(), Some("[]"));
}

#[test]
fn completion_candidates_offer_string_literal_shape() {
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
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("Text \"hello\"").unwrap() + "Text ".len();

    let items = literal_shape_completion_items_for_file_analysis_at_offset(file, offset)
        .expect("expected literal shape completion");
    let string = items
        .iter()
        .find(|item| item.label == "\"\"")
        .expect("expected string shape");

    assert_eq!(string.kind, CompletionItemKind::Constructor);
    assert_eq!(
        string.detail.as_deref(),
        Some("literal Text \"\"(text: &str): Text")
    );
}

#[test]
fn completion_candidates_hide_namespace_import_members() {
    let root_text = "use lib/math\n\nfunc main(): i32 {\n    return math.answer()\n}\n";
    let module_text = "pub func answer(): i32 {\n    return 7\n}\n";
    let (_, analysis) = analyze_namespace_import_text(root_text, module_text);
    let file = analysis.root_file().expect("expected root file");

    let items = completion_items_for_file_analysis(file);

    assert!(items.iter().any(|item| {
        item.label == "math"
            && item.kind == CompletionItemKind::Module
            && item.detail.as_deref() == Some("imported from lib/math")
    }));
    assert!(!items.iter().any(|item| item.label == "answer"));
}

#[test]
fn completion_candidates_hide_imported_signature_dependencies() {
    let root_text = "use lib/math.make\n\nfunc main(): i32 {\n    return 0\n}\n";
    let module_text = r#"pub struct Produced {
    value: i32
}

pub func make(): Produced {
    return Produced { value: 7 }
}
"#;
    let (_, analysis) = analyze_namespace_import_text(root_text, module_text);
    let file = analysis.root_file().expect("expected root file");

    let items = completion_items_for_file_analysis(file);

    assert!(items.iter().any(|item| item.label == "make"));
    assert!(
        items.iter().all(|item| item.label != "lib/math.Produced"),
        "signature-only dependencies must not become source-visible completion items: {items:#?}"
    );
}

#[test]
fn completion_candidates_include_block_imports_only_inside_scope() {
    let text = r#"func main(): i32 {
    use lib/math.answer

    return answer()
}
func other(): i32 {
    return 0
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let inside_offset = text.rfind("answer()").expect("expected answer call");
    let outside_offset = text.rfind("return 0").expect("expected other function");

    let inside_items = completion_items_for_file_analysis_at_offset(file, inside_offset);
    let outside_items = completion_items_for_file_analysis_at_offset(file, outside_offset);

    assert!(inside_items.iter().any(|item| item.label == "answer"));
    assert!(!outside_items.iter().any(|item| item.label == "answer"));
}

#[test]
fn completion_candidates_follow_lexical_local_scope() {
    let text = r#"func main(input: i32): i32 {
    let outer = input
    if true {
        let inner = 2
        return inner
    }
    let later = 3
    return outer
}

func other(hidden: i32): i32 {
    return hidden
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let inner_offset = text.find("return inner").expect("expected inner return");
    let outer_offset = text.rfind("return outer").expect("expected outer return");

    let inner_items = completion_items_for_file_analysis_at_offset(file, inner_offset);
    for expected in ["input", "outer", "inner"] {
        assert!(
            inner_items.iter().any(|item| item.label == expected),
            "expected `{expected}` inside branch: {inner_items:#?}"
        );
    }
    for excluded in ["later", "hidden"] {
        assert!(!inner_items.iter().any(|item| item.label == excluded));
    }

    let outer_items = completion_items_for_file_analysis_at_offset(file, outer_offset);
    for expected in ["input", "outer", "later"] {
        assert!(outer_items.iter().any(|item| item.label == expected));
    }
    assert!(!outer_items.iter().any(|item| item.label == "inner"));
    assert!(!outer_items.iter().any(|item| item.label == "hidden"));
}

#[test]
fn completion_candidates_include_closure_parameters_and_captures_only_inside_body() {
    let text = r#"func main(): i32 {
    let factor = 2
    let transform = (&factor; value: i32): i32 {
        return value + factor
    }
    return transform(3)
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let inside_offset = text.find("return value").unwrap();
    let outside_offset = text.rfind("return transform").unwrap();

    let inside = completion_items_for_file_analysis_at_offset(file, inside_offset);
    for expected in ["value", "factor"] {
        assert!(
            inside.iter().any(|item| item.label == expected),
            "expected `{expected}` in closure body: {inside:#?}"
        );
    }

    let outside = completion_items_for_file_analysis_at_offset(file, outside_offset);
    assert!(!outside.iter().any(|item| item.label == "value"));
    assert!(outside.iter().any(|item| item.label == "factor"));
}

#[test]
fn completion_recovers_member_facts_inside_unclosed_closure_body() {
    let text = r#"copy struct Box {
    value: i32
}

func main(): i32 {
    let box = Box { value: 4 }
    let transform = (&box; input: i32): i32 {
        return box."#;
    let offset = text.len();

    let items = completion_items_for_text_at_offset(text, offset)
        .expect("expected completion from recovered closure body");

    assert!(
        items.iter().any(|item| {
            item.label == "value"
                && item.kind == CompletionItemKind::Field
                && item.detail.as_deref() == Some("field Box.value: i32")
        }),
        "items: {items:#?}"
    );
}

#[test]
fn completion_preserves_stored_outcome_details() {
    let text = r#"func main(): i32 {
    let saved = lookup()
    return 0
}

func lookup(): i32!? {
    return 42
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("return 0").unwrap();
    let items = completion_items_for_file_analysis_at_offset(file, offset);
    let saved = items
        .iter()
        .find(|item| item.label == "saved")
        .expect("expected stored outcome local");

    assert_eq!(saved.detail.as_deref(), Some("let saved: i32!?"));
}

#[test]
fn region_allocator_completion_keeps_only_aborting_capabilities() {
    let text = r#"struct Allocator {
    state: usize
}

struct TryAllocator {
    state: usize
}

func run(parent: Allocator, recoverable: TryAllocator, count: usize): void {
    region temp using parent {
        return
    }
}
"#;
    let (_, analysis) = analyze_text_with_trusted_allocator_capabilities(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("using parent").expect("expected allocator") + "using ".len();

    let items = completion_items_for_file_analysis_at_offset(file, offset);

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["parent"]
    );
    assert_eq!(
        items[0].detail.as_deref(),
        Some("parameter parent: Allocator")
    );
}

#[test]
fn completion_candidates_include_enum_variants_after_pattern_dot() {
    let text = r#"enum Choice {
    hit(value: i32)
    miss
}

func main(choice: Choice): i32 {
    if choice is Choice.hit(_) {
    }
    return match choice {
        Choice.hit(_) { 1 }
        Choice.miss { 2 }
    }
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let if_is_offset = text.find("Choice.hit").expect("expected if-is pattern") + "Choice.".len();
    let match_offset = text.rfind("Choice.hit").expect("expected match pattern") + "Choice.".len();

    for offset in [if_is_offset, match_offset] {
        let items = completion_items_for_file_analysis_at_offset(file, offset);
        assert!(items.iter().any(|item| {
            item.label == "hit"
                && item.kind == CompletionItemKind::EnumMember
                && detail_starts_with(item, "variant ")
        }));
        assert!(items.iter().any(|item| {
            item.label == "miss"
                && item.kind == CompletionItemKind::EnumMember
                && detail_starts_with(item, "variant ")
        }));
        assert!(!items.iter().any(|item| item.label == "Choice"));
    }
}

#[test]
fn completion_candidates_include_enum_variants_after_type_member_dot() {
    let text = r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    return 0
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("Choice.yes").expect("expected enum member") + "Choice.".len();

    let items = completion_items_for_file_analysis_at_offset(file, offset);

    assert!(items.iter().any(|item| {
        item.label == "yes"
            && item.kind == CompletionItemKind::EnumMember
            && item.detail.as_deref() == Some("variant Choice.yes")
    }));
    assert!(items.iter().any(|item| {
        item.label == "no"
            && item.kind == CompletionItemKind::EnumMember
            && item.detail.as_deref() == Some("variant Choice.no")
    }));
    assert!(!items.iter().any(|item| item.label == "Choice"));
}

#[test]
fn completion_candidates_include_associated_functions_after_type_member_dot() {
    let text = r#"struct File {
    fd: i32
}

construct File {
    pub default func open(): Self {
        return File { fd: 1 }
    }
}

func main(): i32 {
    let file = File.open()
    return file.fd
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text
        .rfind("File.open")
        .expect("expected associated function call")
        + "File.".len();

    let items = completion_items_for_file_analysis_at_offset(file, offset);

    assert!(items.iter().any(|item| {
        item.label == "open"
            && item.kind == CompletionItemKind::Constructor
            && item.detail.as_deref() == Some("func File.open(): File")
    }));
    assert!(!items.iter().any(|item| item.label == "File"));
}

#[test]
fn completion_candidates_present_construct_members_as_owned_constructors() {
    let text = r#"struct Bucket<T> { value: T }

construct Bucket<T> {
    pub default func new(value: T): Self { return Bucket<T> { value: value } }
}

func main(): i32 {
    let value = Bucket.new(1)
    return 0
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("Bucket.new").unwrap() + "Bucket.".len();

    let items = completion_items_for_file_analysis_at_offset(file, offset);
    let constructor = items
        .iter()
        .find(|item| item.label == "new")
        .expect("expected constructor completion");

    assert_eq!(constructor.kind, CompletionItemKind::Constructor);
    assert_eq!(
        constructor.detail.as_deref(),
        Some("func Bucket<T>.new(value: T): Bucket<T>")
    );
    assert_eq!(constructor.sort_text.as_deref(), Some("0-new"));
}

#[test]
fn completion_candidates_include_type_members_after_incomplete_member_dot() {
    let text = r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.
    return 0
}
"#;
    let offset = text
        .find("Choice.")
        .expect("expected incomplete enum member")
        + "Choice.".len();

    let items =
        completion_items_for_text_at_offset(text, offset).expect("expected completion items");

    assert!(items.iter().any(|item| {
        item.label == "yes"
            && item.kind == CompletionItemKind::EnumMember
            && detail_starts_with(item, "variant ")
    }));
    assert!(items.iter().any(|item| {
        item.label == "no"
            && item.kind == CompletionItemKind::EnumMember
            && detail_starts_with(item, "variant ")
    }));
    assert!(!items.iter().any(|item| item.label == "Choice"));
}

#[test]
fn completion_candidates_do_not_fall_back_to_globals_after_unknown_type_member_dot() {
    let text = r#"enum Choice {
    yes
}

func main(): i32 {
    let choice = Missing.yes
    return 0
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("Missing.yes").expect("expected unknown member") + "Missing.".len();

    let items = completion_items_for_file_analysis_at_offset(file, offset);

    assert!(
        items.is_empty(),
        "expected no global fallback, got {items:#?}"
    );
}

#[test]
fn completion_candidates_include_fields_and_methods_after_value_member_dot() {
    let text = r#"struct File {
    fd: i32
    size: i32
}

instance File {
    method &self.describe(): i32 {
        return self.size
    }
}

func main(): i32 {
    let file = File { fd: 1, size: 2 }
    return file.fd
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("file.fd").expect("expected field access") + "file.".len();

    let items = completion_items_for_file_analysis_at_offset(file, offset);

    assert!(items.iter().any(|item| {
        item.label == "fd"
            && item.kind == CompletionItemKind::Field
            && item.detail.as_deref() == Some("field File.fd: i32")
    }));
    assert!(items.iter().any(|item| {
        item.label == "size"
            && item.kind == CompletionItemKind::Field
            && item.detail.as_deref() == Some("field File.size: i32")
    }));
    assert!(items.iter().any(|item| {
        item.label == "describe"
            && item.kind == CompletionItemKind::Method
            && detail_starts_with(item, "method ")
    }));
    assert!(!items.iter().any(|item| item.label == "File"));
}

#[test]
fn member_completion_includes_unambiguous_interface_default_method() {
    let text = r#"interface Value {
    pub method &self.value(): i32 {
        return 42
    }
}

copy struct Unit {
    marker: i32
}

conform Value for Unit {}

func main(): i32 {
    let unit = Unit { marker: 0 }
    return unit.value()
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("unit.value").unwrap() + "unit.".len();

    let items = completion_items_for_file_analysis_at_offset(file, offset);
    let value = items
        .iter()
        .find(|item| item.label == "value")
        .expect("expected interface default method");

    assert_eq!(value.kind, CompletionItemKind::Method);
    assert_eq!(value.detail.as_deref(), Some("method &Unit.value(): i32"));
}

#[test]
fn member_completion_omits_inherent_interface_name_conflict() {
    let text = r#"interface Value {
    pub method &self.value(): i32 {
        return 1
    }
}

copy struct Unit {
    marker: i32
}

instance Unit {
    pub method &self.value(): i32 {
        return 42
    }
}

conform Value for Unit {}

func main(): i32 {
    let unit = Unit { marker: 0 }
    return unit.value()
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("unit.value").unwrap() + "unit.".len();

    let values = completion_items_for_file_analysis_at_offset(file, offset)
        .into_iter()
        .filter(|item| item.label == "value")
        .collect::<Vec<_>>();

    assert!(values.is_empty(), "{values:?}");
}

#[test]
fn member_completion_omits_competing_interface_default_methods() {
    let text = r#"interface Left {
    pub method &self.inspect(): i32 {
        return 1
    }
}

interface Right {
    pub method &self.inspect(): i32 {
        return 2
    }
}

copy struct Unit {
    marker: i32
}

conform Left for Unit {}
conform Right for Unit {}

func main(): i32 {
    let unit = Unit { marker: 0 }
    return unit.marker
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("unit.marker").unwrap() + "unit.".len();

    let items = completion_items_for_file_analysis_at_offset(file, offset);

    assert!(!items.iter().any(|item| item.label == "inspect"));
}

#[test]
fn completion_candidates_include_fields_and_methods_after_incomplete_value_member_dot() {
    let text = r#"struct File {
    fd: i32
    size: i32
}

instance File {
    method &self.describe(): i32 {
        return self.size
    }
}

func main(): i32 {
    let file = File { fd: 1, size: 2 }
    return file.
}
"#;
    let offset = text
        .rfind("file.")
        .expect("expected incomplete field access")
        + "file.".len();

    let items =
        completion_items_for_text_at_offset(text, offset).expect("expected completion items");

    assert!(items.iter().any(|item| {
        item.label == "fd"
            && item.kind == CompletionItemKind::Field
            && detail_starts_with(item, "field ")
    }));
    assert!(items.iter().any(|item| {
        item.label == "size"
            && item.kind == CompletionItemKind::Field
            && detail_starts_with(item, "field ")
    }));
    assert!(items.iter().any(|item| {
        item.label == "describe"
            && item.kind == CompletionItemKind::Method
            && detail_starts_with(item, "method ")
    }));
    assert!(!items.iter().any(|item| item.label == "File"));
}

#[test]
fn completion_candidates_include_struct_fields_inside_struct_literal_field_name() {
    let text = r#"struct File {
    fd: i32
    size: i32
}

func main(): i32 {
    let file = File { fd: 1 }
    return 0
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("fd: 1").expect("expected struct literal field");

    let items = completion_items_for_file_analysis_at_offset(file, offset);

    assert!(items.iter().any(|item| {
        item.label == "fd"
            && item.kind == CompletionItemKind::Field
            && detail_starts_with(item, "field ")
    }));
    assert!(items.iter().any(|item| {
        item.label == "size"
            && item.kind == CompletionItemKind::Field
            && detail_starts_with(item, "field ")
    }));
    assert!(!items.iter().any(|item| item.label == "File"));
}

#[test]
fn completion_candidates_skip_used_struct_fields_inside_struct_literal() {
    let text = r#"struct File {
    fd: i32
    size: i32
}

func main(): i32 {
    let file = File { fd: 1,  }
    return 0
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text
        .find("File { fd: 1,  }")
        .expect("expected struct literal")
        + "File { fd: 1, ".len();

    let items = completion_items_for_file_analysis_at_offset(file, offset);

    assert!(items.iter().any(|item| {
        item.label == "size"
            && item.kind == CompletionItemKind::Field
            && detail_starts_with(item, "field ")
    }));
    assert!(!items.iter().any(|item| item.label == "fd"));
    assert!(!items.iter().any(|item| item.label == "File"));
}

#[test]
fn completion_candidates_include_struct_fields_after_empty_struct_literal_braces() {
    let text = r#"struct File {
    fd: i32
    size: i32
}

func main(): i32 {
    let file = File {  }
    return 0
}
"#;
    let offset = text.find("File {  }").expect("expected struct literal") + "File { ".len();

    let items =
        completion_items_for_text_at_offset(text, offset).expect("expected completion items");

    assert!(items.iter().any(|item| {
        item.label == "fd"
            && item.kind == CompletionItemKind::Field
            && detail_starts_with(item, "field ")
    }));
    assert!(items.iter().any(|item| {
        item.label == "size"
            && item.kind == CompletionItemKind::Field
            && detail_starts_with(item, "field ")
    }));
    assert!(!items.iter().any(|item| item.label == "File"));
}

#[test]
fn completion_candidates_include_struct_fields_after_unclosed_struct_literal_brace() {
    let text = r#"struct File {
    fd: i32
    size: i32
}

func main(): i32 {
    let file = File {
    return 0
}
"#;
    let offset = text
        .find("let file = File {")
        .expect("expected struct literal")
        + "let file = File {".len();

    let items =
        completion_items_for_text_at_offset(text, offset).expect("expected completion items");

    assert!(items.iter().any(|item| {
        item.label == "fd"
            && item.kind == CompletionItemKind::Field
            && detail_starts_with(item, "field ")
    }));
    assert!(items.iter().any(|item| {
        item.label == "size"
            && item.kind == CompletionItemKind::Field
            && detail_starts_with(item, "field ")
    }));
    assert!(!items.iter().any(|item| item.label == "File"));
}

#[test]
fn completion_candidates_include_fields_and_methods_after_borrowed_value_member_dot() {
    let text = r#"struct File {
    fd: i32
}

instance File {
    method &self.describe(): i32 {
        return self.fd
    }
}

func inspect(file: &File): i32 {
    return file.fd
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("file.fd").expect("expected field access") + "file.".len();

    let items = completion_items_for_file_analysis_at_offset(file, offset);

    assert!(items.iter().any(|item| {
        item.label == "fd"
            && item.kind == CompletionItemKind::Field
            && detail_starts_with(item, "field ")
    }));
    assert!(items.iter().any(|item| {
        item.label == "describe"
            && item.kind == CompletionItemKind::Method
            && detail_starts_with(item, "method ")
    }));
    assert!(!items.iter().any(|item| item.label == "File"));
}

#[test]
fn completion_candidates_include_pattern_members_after_incomplete_pattern_dot() {
    let text = r#"enum Choice {
    hit(value: i32)
    miss
}

func main(choice: Choice): i32 {
    if choice is Choice. {
    }
    return 0
}
"#;
    let offset = text.find("Choice.").expect("expected incomplete pattern") + "Choice.".len();

    let items =
        completion_items_for_text_at_offset(text, offset).expect("expected completion items");

    assert!(items.iter().any(|item| {
        item.label == "hit"
            && item.kind == CompletionItemKind::EnumMember
            && detail_starts_with(item, "variant ")
    }));
    assert!(items.iter().any(|item| {
        item.label == "miss"
            && item.kind == CompletionItemKind::EnumMember
            && detail_starts_with(item, "variant ")
    }));
    assert!(!items.iter().any(|item| item.label == "Choice"));
}

#[test]
fn completion_candidates_do_not_fall_back_to_globals_after_unknown_pattern_dot() {
    let text = r#"enum Choice {
    hit
}

func main(choice: Choice): i32 {
    if choice is Missing.hit {
    }
    return 0
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("Missing.hit").expect("expected unknown pattern") + "Missing.".len();

    let items = completion_items_for_file_analysis_at_offset(file, offset);

    assert!(
        items.is_empty(),
        "expected no global fallback, got {items:#?}"
    );
}

fn detail_starts_with(item: &CompletionItemInfo, prefix: &str) -> bool {
    item.detail
        .as_deref()
        .is_some_and(|detail| detail.starts_with(prefix))
}

#[test]
fn member_completion_specializes_generics_and_filters_receiver_capability() {
    let text = r#"struct Box<T> {
    value: T
}

instance<T> Box<T> {
    method &self.inspect(): void {
        return
    }

    method &+self.mutate(value: T): void {
        return
    }
}

func use_boxes(readonly: &Box<i32>, readwrite: &+Box<i32>): void {
    readonly.inspect()
    readwrite.inspect()
    return
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let readonly_offset = text.find("readonly.inspect").unwrap() + "readonly.".len();
    let readwrite_offset = text.find("readwrite.inspect").unwrap() + "readwrite.".len();

    let readonly_items = completion_items_for_file_analysis_at_offset(file, readonly_offset);
    assert!(readonly_items.iter().any(|item| {
        item.label == "value" && item.detail.as_deref() == Some("field Box<i32>.value: i32")
    }));
    assert!(readonly_items.iter().any(|item| item.label == "inspect"));
    assert!(!readonly_items.iter().any(|item| item.label == "mutate"));

    let readwrite_items = completion_items_for_file_analysis_at_offset(file, readwrite_offset);
    let mutate = readwrite_items
        .iter()
        .find(|item| item.label == "mutate")
        .expect("readwrite receiver should include mutate");
    assert_eq!(
        mutate.detail.as_deref(),
        Some("method &+Box<i32>.mutate(value: i32): void")
    );
}

#[test]
fn member_completion_uses_generic_interface_bound() {
    let text = r#"interface Lookup<V> {
    pub method &self.get(): &V from self
}

func read<M>(map: &M): &i32 from map where M: Lookup<i32> {
    return map.get()
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("map.get").expect("expected bound method") + "map.".len();

    let items = completion_items_for_file_analysis_at_offset(file, offset);
    let get = items
        .iter()
        .find(|item| item.label == "get")
        .expect("bound method should be suggested");
    assert_eq!(
        get.detail.as_deref(),
        Some("method &M.get(): &i32 from self")
    );
    let declaration_start = text.find("get(): &V").expect("expected declaration");
    assert_eq!(
        get.declaration_span.map(|span| (span.start, span.end)),
        Some((declaration_start, declaration_start + "get".len()))
    );
}

#[test]
fn member_completion_targets_conformance_member_implementation() {
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
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("count.measure").unwrap() + "count.".len();

    let item = completion_items_for_file_analysis_at_offset(file, offset)
        .into_iter()
        .find(|item| item.label == "measure")
        .expect("expected conformance method completion");

    assert_eq!(item.detail.as_deref(), Some("method &Count.measure(): i32"));
    let declaration_start = text.find("method &self.measure(): i32 {").unwrap() + 13;
    assert_eq!(
        item.declaration_span.map(|span| span.start),
        Some(declaration_start)
    );
}

#[test]
fn member_completion_combines_unambiguous_capability_set_members() {
    let text = r#"interface Readable {
    pub method &self.read(): i32
}

interface Measurable {
    pub method &self.measure(): usize
}

func inspect<T>(value: &T): i32 where T: Readable + Measurable {
    return value.read()
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("value.read").unwrap() + "value.".len();
    let items = completion_items_for_file_analysis_at_offset(file, offset);

    assert!(items.iter().any(|item| item.label == "read"));
    assert!(items.iter().any(|item| item.label == "measure"));
}

#[test]
fn generic_method_completion_keeps_constraints_in_a_specialized_where_clause() {
    let text = r#"interface Reader<T> {}

struct Box<T> { value: T }

instance<T> Box<T> {
    method &self.map<U>(value: U): T where U: Reader<T> {
        return self.value
    }
}

func inspect(box: &Box<i32>): i32 {
    return box.map
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("box.map").unwrap() + "box.".len();
    let item = completion_items_for_file_analysis_at_offset(file, offset)
        .into_iter()
        .find(|item| item.label == "map")
        .expect("generic method completion");

    assert_eq!(
        item.detail.as_deref(),
        Some("method &Box<i32>.map<U>(value: U): i32 where U: Reader<i32>")
    );
}

#[test]
fn member_completion_omits_ambiguous_capability_set_member() {
    let text = r#"interface Left {
    pub method &self.inspect(): i32
}

interface Right {
    pub method &self.inspect(): i32
}

func inspect<T>(value: &T): i32 where T: Left + Right {
    value.
    return 0
}
"#;
    let completion_offset = text.find("value.").unwrap() + "value.".len();
    let items = completion_items_for_text_at_offset(text, completion_offset)
        .expect("expected member completion response");

    assert!(!items.iter().any(|item| item.label == "inspect"));
}

#[test]
fn member_completion_omits_methods_ambiguous_across_receiver_coercions() {
    let text = r#"struct Source { left: Left, right: Right }
struct Left { value: usize }
struct Right { value: usize }

coerce Source {
    pub &self as &Left { return &self.left }
    pub &self as &Right { return &self.right }
}

instance Left {
    pub method &self.inspect(): usize { return self.value }
}

instance Right {
    pub method &self.inspect(): usize { return self.value }
}

func read(source: &Source): usize {
    return source.inspect()
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("source.inspect").unwrap() + "source.".len();
    let items = completion_items_for_file_analysis_at_offset(file, offset);

    assert!(!items.iter().any(|item| item.label == "inspect"));
}

#[test]
fn unavailable_original_method_still_shadows_receiver_coercion_completion() {
    let text = r#"struct Text { value: &str }

coerce Text {
    pub &self as &str { return self.value }
}

instance Text {
    pub method &+self.inspect(): usize { return 1 }
}

instance str {
    pub method &self.inspect(): usize { return 2 }
}

func read(text: &Text): usize {
    return text.inspect()
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("text.inspect").unwrap() + "text.".len();
    let items = completion_items_for_file_analysis_at_offset(file, offset);

    assert!(!items.iter().any(|item| item.label == "inspect"));
}

#[test]
fn readwrite_receiver_coercion_completion_keeps_target_capability() {
    let text = r#"struct Buffer { read: &[u8], write: &+[u8] }

coerce Buffer {
    pub &self as &[u8] { return self.read }
    pub &+self as &+[u8] { return self.write }
}

instance<T> [T] {
    pub method &self.len(): usize { return 1 }
    pub method &+self.clear(): void { return }
}

func clear(buffer: &+Buffer): void {
    buffer.clear()
    return
}
"#;
    let (_, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.rfind("buffer.clear").unwrap() + "buffer.".len();
    let items = completion_items_for_file_analysis_at_offset(file, offset);

    assert!(items.iter().any(|item| {
        item.label == "clear" && item.detail.as_deref() == Some("method &+[u8].clear(): void")
    }));
    assert!(items.iter().any(|item| {
        item.label == "len" && item.detail.as_deref() == Some("method &[u8].len(): usize")
    }));
}

#[test]
fn completion_recovers_incomplete_result_provenance_clause() {
    let text = r#"func choose(left: &i32, right: &i32): &i32 from {
    return left
}
"#;
    let offset = text.find("from ").unwrap() + "from ".len();

    let items = completion_items_for_text_at_offset(text, offset)
        .expect("expected recovered provenance completion");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"left"), "{labels:?}");
    assert!(labels.contains(&"right"), "{labels:?}");
    assert!(labels.contains(&"static"), "{labels:?}");
    assert!(labels.contains(&"current"), "{labels:?}");
}

#[test]
fn completion_recovers_incomplete_literal_result_provenance_clause() {
    let text = r#"struct Text { value: &str }

construct Text {
    pub default literal ""(text: &str): Self from {
        return Text { value: text }
    }
}
"#;
    let offset = text.find("from ").unwrap() + "from ".len();

    let items = completion_items_for_text_at_offset(text, offset)
        .expect("expected recovered literal provenance completion");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"text"), "{labels:?}");
    assert!(labels.contains(&"static"), "{labels:?}");
    assert!(labels.contains(&"current"), "{labels:?}");
    assert!(!labels.contains(&"self"), "{labels:?}");
}

#[test]
fn completion_recovers_incomplete_generic_bound() {
    let text = r#"interface Measure {
    pub method &self.measure(): i32
}

func read<T>(value: &T): i32 where T: {
    return 0
}
"#;
    let offset = text.find("T: ").unwrap() + "T: ".len();

    let items = completion_items_for_text_at_offset(text, offset)
        .expect("expected recovered bound completion");

    assert!(
        items
            .iter()
            .any(|item| { item.label == "Measure" && item.kind == CompletionItemKind::Interface })
    );
}

#[test]
fn completion_recovers_incomplete_additional_generic_bound() {
    let text = r#"interface Readable {
    pub method &self.read(): i32
}

interface Measurable {
    pub method &self.measure(): usize
}

func inspect<T>(value: &T): i32 where T: Readable + {
    return 0
}
"#;
    let offset = text.find("Readable + ").unwrap() + "Readable + ".len();
    let items = completion_items_for_text_at_offset(text, offset)
        .expect("expected recovered additional-bound completion");

    assert!(
        items.iter().any(|item| {
            item.label == "Measurable" && item.kind == CompletionItemKind::Interface
        })
    );
}

#[test]
fn completion_recovers_associated_types_after_projection_dot() {
    let text = r#"interface Source {
    pub type Item
    pub type Error
}

func project<S>(source: S): S. where S: Source {
    return source
}
"#;
    let offset = text.find("S. where").unwrap() + "S.".len();

    let items = completion_items_for_text_at_offset(text, offset)
        .expect("expected associated type completion");

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Item", "Error"]
    );
    assert!(items.iter().all(|item| {
        item.detail
            .as_deref()
            .is_some_and(|detail| detail.starts_with("associated type Source."))
    }));
}

#[test]
fn completion_uses_where_clause_for_associated_types() {
    let text = r#"interface Source {
    pub type Item
}

func project<S>(source: S): S. where S: Source {
    return source
}
"#;
    let offset = text.find("S. where").unwrap() + "S.".len();

    let items = completion_items_for_text_at_offset(text, offset)
        .expect("expected associated type completion from where clause");

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Item"]
    );
}

#[test]
fn method_presentation_matches_completion_hover_and_signature_help() {
    let text = r#"struct Box<T> { value: T }

instance<T> Box<T> {
    method &self.replace(value: T): T {
        return value
    }
}

func main(box: &Box<i32>): i32 {
    return box.replace(42)
}
"#;
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("root file");
    let member = text.find("box.replace").expect("method call") + "box.".len();
    let completion = completion_items_for_file_analysis_at_offset(file, member)
        .into_iter()
        .find(|item| item.label == "replace")
        .expect("method completion");
    let hover = crate::analysis::hover::hover_for_file_analysis(&sources, &analysis, file, member)
        .expect("method hover");
    let signature = crate::analysis::signature_help::signature_help_for_file_analysis(
        &sources,
        &analysis,
        file,
        text.find("42").expect("argument"),
    )
    .expect("signature help");

    assert_eq!(completion.detail.as_deref(), Some(hover.label.as_str()));
    assert_eq!(hover.label, signature.label);
    assert_eq!(signature.label, "method &Box<i32>.replace(value: i32): i32");
}
