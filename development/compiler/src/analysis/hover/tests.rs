use super::*;
use crate::analysis::test_support::{analyze_namespace_import_text, analyze_text};

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
fn workspace_hover_uses_normalized_typecheck_facts_for_method_call() {
    let text = "type Count = i32\n\nstruct File {\n    fd: Count\n}\n\nimpl File {\n    /// Reads a count.\n    method self.read(amount: Count): Count {\n        return amount\n    }\n}\n\nfunc main(): i32 {\n    let file = File { fd: 1 }\n    return file.read(1)\n}\n";
    let (sources, analysis) = analyze_text(text);
    let file = analysis.root_file().expect("expected root file");
    let offset = text.find("read(1)").expect("expected method call");

    let hover =
        hover_for_file_analysis(&sources, &analysis, file, offset).expect("expected hover info");

    assert_eq!(hover.label, "method self.read(amount: i32): i32");
    assert_eq!(hover.documentation.as_deref(), Some("Reads a count."));
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

    assert_eq!(hover.label, "field fd: i32");
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

    assert_eq!(hover.label, "variant count(value: i32)");
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
