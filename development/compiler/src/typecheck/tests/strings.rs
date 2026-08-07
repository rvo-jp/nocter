use super::check;
use crate::ast::Item;
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::resolve;
use crate::semantics::{
    InterpolationInputKind, InterpolationRuntime, RuntimeCallable, TrustedDeclarationFacts,
};
use crate::source::SourceMap;
use std::collections::HashMap;

fn check_text(text: &str) -> Vec<crate::diagnostics::Diagnostic> {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let lexed = lex(&sources, source);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    let parsed = parse(&sources, source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ast = parsed.ast.unwrap();
    let string_span = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(struct_) if struct_.name == "String" => Some(struct_.span),
            _ => None,
        })
        .expect("expected test String");
    let callable = RuntimeCallable {
        declaration: string_span,
        target_name: "test".to_string(),
    };
    let formatters = [
        InterpolationInputKind::Str,
        InterpolationInputKind::String,
        InterpolationInputKind::I32,
        InterpolationInputKind::U8,
        InterpolationInputKind::Usize,
        InterpolationInputKind::Bool,
    ]
    .into_iter()
    .map(|kind| (kind, callable.clone()))
    .collect::<HashMap<_, _>>();
    let mut trusted = TrustedDeclarationFacts::default();
    trusted.set_interpolation_runtime(InterpolationRuntime::new(string_span, callable, formatters));
    let mut resolved = resolve(&sources, &ast);
    resolved.trusted_declarations = trusted;
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

func message(name: &str, count: i32, byte: u8, size: usize, ready: bool, owned: String): String {
    return "name ${name} count ${count} byte ${byte} size ${size} ready ${ready} owned ${owned}"
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
}

#[test]
fn rejects_storage_only_integer_before_runtime_abi_promotion() {
    let diagnostics = check_text(
        r#"struct String {
    bytes: &[u8]
}

func main(): i32 {
    return 0
}

func message(value: u16): String {
    return "value ${value}"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0379");
    assert!(diagnostics[0].message.contains("u16"));
}
