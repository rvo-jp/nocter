use super::check;
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::resolve;
use crate::source::SourceMap;

fn check_text(text: &str) -> Vec<crate::diagnostics::Diagnostic> {
    let text = super::format_support::append_test_format_contract(text);
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, &text);
    let lexed = lex(&sources, source);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    let parsed = parse(&sources, source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ast = parsed.ast.unwrap();
    let mut resolved = resolve(&sources, &ast);
    super::format_support::attach_test_format_runtime(&ast, &mut resolved);
    let mut diagnostics = resolved.diagnostics.clone();
    diagnostics.extend(check(&sources, &ast, &resolved));
    diagnostics
}

fn check_text_without_runtime(text: &str) -> Vec<crate::diagnostics::Diagnostic> {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let lexed = lex(&sources, source);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    let parsed = parse(&sources, source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ast = parsed.ast.unwrap();
    let resolved = resolve(&sources, &ast);
    let mut diagnostics = resolved.diagnostics.clone();
    diagnostics.extend(check(&sources, &ast, &resolved));
    diagnostics
}

#[test]
fn diagnoses_missing_trusted_interpolation_runtime() {
    let diagnostics = check_text_without_runtime(
        r#"func main(): i32 {
    let message = "value ${1}"
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0440");
    assert!(diagnostics[0].message.contains("active Nocter home"));
}

#[test]
fn accepts_supported_string_interpolation_parts() {
    let diagnostics = check_text(
        r#"struct String {
    bytes: &[u8]
}

func main(): i32 {
    return 0
}

func message(name: &str, count: i32, byte: u8, size: usize, narrow: i16, wide: u64, ready: bool, owned: String): String {
    return "name ${name} count ${count} byte ${byte} size ${size} narrow ${narrow} wide ${wide} ready ${ready} owned ${owned}"
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_unsupported_string_interpolation_part_type() {
    let diagnostics = check_text(
        r#"struct String {
    bytes: &[u8]
}

func main(): i32 {
    return 0
}

func message(values: &[i32]): String {
    return "values ${values}"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0379");
    assert!(diagnostics[0].message.contains("&[i32]"));
    assert!(diagnostics[0].message.contains("std/fmt.Format"));
    assert!(
        diagnostics[0]
            .help
            .as_deref()
            .is_some_and(|help| help.contains("conform Format for Type"))
    );
}

#[test]
fn accepts_every_builtin_integer_interpolation_input() {
    let diagnostics = check_text(
        r#"struct String {
    bytes: &[u8]
}

func main(): i32 {
    return 0
}

func message(first: i8, second: u16, third: i64, fourth: u64): String {
    return "values ${first} ${second} ${third} ${fourth}"
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
