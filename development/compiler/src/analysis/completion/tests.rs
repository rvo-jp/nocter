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
fn completion_candidates_offer_declared_literal_shapes_after_target() {
    let text = r#"struct Bucket<T> { length: usize }

literal Bucket<T> [](...items: T): Self {
    return Bucket<T> { length: items.len() }
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

literal Text ""(text: &str): Self {
    return Text { value: text }
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
fn completion_candidates_include_associated_functions_after_type_member_dot() {
    let text = r#"struct File {
    fd: i32
}

func File.open(): File {
    return File { fd: 1 }
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
            && item.kind == CompletionItemKind::Function
            && detail_starts_with(item, "func open")
    }));
    assert!(!items.iter().any(|item| item.label == "File"));
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

impl File {
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
fn completion_candidates_include_fields_and_methods_after_incomplete_value_member_dot() {
    let text = r#"struct File {
    fd: i32
    size: i32
}

impl File {
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

impl File {
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

impl<T> Box<T> {
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

func read<M: Lookup<i32>>(map: &M): &i32 from map {
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

literal Text ""(text: &str): Self from {
    return Text { value: text }
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

func read<T: >(value: &T): i32 {
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
