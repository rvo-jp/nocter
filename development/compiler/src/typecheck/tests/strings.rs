use super::check;
use crate::ast::Item;
use crate::integer::IntegerType;
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
        InterpolationInputKind::Integer(IntegerType::I8),
        InterpolationInputKind::Integer(IntegerType::I16),
        InterpolationInputKind::Integer(IntegerType::I64),
        InterpolationInputKind::Integer(IntegerType::Isize),
        InterpolationInputKind::Integer(IntegerType::U16),
        InterpolationInputKind::Integer(IntegerType::U32),
        InterpolationInputKind::Integer(IntegerType::U64),
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
