use super::check_text;
use crate::ast::Item;
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::resolve;
use crate::semantics::{AllocationFailurePolicy, AllocationSource, TrustedDeclarationRole};
use crate::source::SourceMap;

fn check_text_with_trusted_allocation(text: &str) -> Vec<crate::diagnostics::Diagnostic> {
    check_text_with_trusted_primitive(
        text,
        "allocate",
        TrustedDeclarationRole::AllocationOperation {
            source: AllocationSource::CurrentContext,
            failure_policy: AllocationFailurePolicy::Abort,
        },
    )
}

fn check_text_with_trusted_primitive(
    text: &str,
    name: &str,
    role: TrustedDeclarationRole,
) -> Vec<crate::diagnostics::Diagnostic> {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let lexed = lex(&sources, source);
    let parsed = parse(&sources, source, &lexed.tokens);
    let ast = parsed.ast.unwrap();
    let mut resolved = resolve(&sources, &ast);
    let allocation = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Primitive(primitive) if primitive.name == name => Some(primitive.name_span),
            _ => None,
        })
        .unwrap();
    resolved.trusted_declarations.insert(allocation, role);
    let mut diagnostics = resolved.diagnostics.clone();
    diagnostics.extend(super::super::check(&sources, &ast, &resolved));
    diagnostics
}

fn check_text_with_trusted_function(
    text: &str,
    name: &str,
    role: TrustedDeclarationRole,
) -> Vec<crate::diagnostics::Diagnostic> {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let lexed = lex(&sources, source);
    let parsed = parse(&sources, source, &lexed.tokens);
    let ast = parsed.ast.unwrap();
    let declaration = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == name => Some(function.name_span),
            _ => None,
        })
        .unwrap();
    let mut resolved = resolve(&sources, &ast);
    resolved.trusted_declarations.insert(declaration, role);
    let mut diagnostics = resolved.diagnostics.clone();
    diagnostics.extend(super::super::check(&sources, &ast, &resolved));
    diagnostics
}

#[test]
fn trusted_allocation_primitives_must_declare_alloc() {
    let diagnostics = check_text_with_trusted_allocation("pub(nocter) primitive allocate(): *u8\n");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0462"),
        "{diagnostics:?}"
    );

    let diagnostics =
        check_text_with_trusted_allocation("pub(nocter) alloc primitive allocate(): *u8\n");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "E0462"),
        "{diagnostics:?}"
    );
}

#[test]
fn other_trusted_primitives_cannot_claim_result_allocation() {
    let diagnostics = check_text_with_trusted_primitive(
        "pub(nocter) alloc primitive current(): usize\n",
        "current",
        TrustedDeclarationRole::CurrentAllocationContext,
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0463"),
        "{diagnostics:?}"
    );
}

#[test]
fn requires_alloc_when_a_function_returns_allocated_storage() {
    let diagnostics = check_text_with_trusted_allocation(
        r#"struct Buffer { pointer: *u8 }
pub(nocter) alloc primitive allocate(): Buffer
func make(): Buffer {
    return allocate()
}
"#,
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E0462")
        .expect("missing allocation contract diagnostic");
    let witness = diagnostic
        .notes
        .iter()
        .find(|note| note.message.contains("returned expression"))
        .and_then(|note| note.span.as_ref())
        .expect("returned allocation witness");
    assert_eq!(witness.start_line, 4, "{diagnostics:?}");
}

#[test]
fn missing_contract_witness_ignores_scratch_allocation() {
    let diagnostics = check_text_with_trusted_allocation(
        r#"struct Buffer { pointer: *u8 }
pub(nocter) alloc primitive allocate(): Buffer
func make(): Buffer {
    let scratch = allocate()
    return allocate()
}
"#,
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E0462")
        .expect("missing allocation contract diagnostic");
    let witness = diagnostic
        .notes
        .iter()
        .find(|note| note.message.contains("returned expression"))
        .and_then(|note| note.span.as_ref())
        .expect("returned allocation witness");
    assert_eq!(witness.start_line, 5, "{diagnostics:?}");
}

#[test]
fn accepts_an_exact_result_allocation_contract() {
    let diagnostics = check_text_with_trusted_allocation(
        r#"struct Buffer { pointer: *u8 }
pub(nocter) alloc primitive allocate(): Buffer
alloc func make(): Buffer {
    return allocate()
}
"#,
    );

    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0462" || diagnostic.code == "E0463"),
        "{diagnostics:?}"
    );
}

#[test]
fn rejects_alloc_when_no_returned_storage_is_allocated() {
    let diagnostics = check_text("alloc func value(): usize { return 1 }\n");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0463")
    );
}

#[test]
fn accepts_bodyless_alloc_contracts_without_guessing_an_implementation() {
    let diagnostics = check_text(
        r#"alloc primitive make(): Value
interface Factory {
    pub alloc method &self.make(): Value
}
"#,
    );

    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0462" || diagnostic.code == "E0463"),
        "{diagnostics:?}"
    );
}

#[test]
fn does_not_require_alloc_for_scratch_allocation() {
    let diagnostics = check_text_with_trusted_allocation(
        r#"struct Buffer { pointer: *u8 }
pub(nocter) alloc primitive allocate(): Buffer
func size(): usize {
    let temporary = allocate()
    return 1
}
"#,
    );

    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0462"),
        "{diagnostics:?}"
    );
}

#[test]
fn propagates_alloc_through_callable_type_calls() {
    let diagnostics = check_text(
        r#"struct Buffer { pointer: *u8 }
func invoke<F: alloc &func(): Buffer>(callback: F): Buffer {
    return callback()
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0462"),
        "{diagnostics:?}"
    );
}

#[test]
fn propagates_alloc_retained_by_readwrite_input() {
    let diagnostics = check_text_with_trusted_allocation(
        r#"struct Buffer { pointer: *u8 }
pub(nocter) alloc primitive allocate(): Buffer
primitive empty_pointer(): *u8
func grow(buffer: &+Buffer): void {
    let replacement = allocate()
    buffer.pointer = replacement.pointer
    return
}
func make(): Buffer {
    var result = Buffer { pointer: empty_pointer() }
    grow(&+result)
    return move result
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0462"),
        "{diagnostics:?}"
    );
}

#[test]
fn trusted_growth_of_neutral_storage_uses_the_current_allocation_origin() {
    let diagnostics = check_text_with_trusted_function(
        r#"struct Buffer { pointer: *u8 }
primitive empty_pointer(): *u8
func grow(buffer: &+Buffer): void { return }
func make(): Buffer {
    var result = Buffer { pointer: empty_pointer() }
    grow(&+result)
    return move result
}
"#,
        "grow",
        TrustedDeclarationRole::AllocationMutation {
            target: 0,
            source: AllocationSource::Input(0),
            fallback_to_current: true,
        },
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0462"),
        "{diagnostics:?}"
    );
}

#[test]
fn ignores_allocated_local_storage_outside_storage_bearing_result_projections() {
    let diagnostics = check_text_with_trusted_function(
        r#"struct Buffer { pointer: *u8 }
primitive empty_pointer(): *u8
func grow(buffer: &+Buffer): void { return }
func use_buffer(): usize! {
    var buffer = Buffer { pointer: empty_pointer() }
    grow(&+buffer)
    return 1
}
"#,
        "grow",
        TrustedDeclarationRole::AllocationMutation {
            target: 0,
            source: AllocationSource::Input(0),
            fallback_to_current: true,
        },
    );

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "E0462"),
        "{diagnostics:?}"
    );
}

#[test]
fn propagates_retained_mutations_through_methods_wrappers_and_loops() {
    let diagnostics = check_text_with_trusted_allocation(
        r#"struct Buffer<T> { pointer: *T }
pub(nocter) alloc primitive allocate<T>(): Buffer<T>
primitive empty_pointer(): *u8
func grow<T>(buffer: &+Buffer<T>): void {
    let replacement = allocate()
    buffer.pointer = replacement.pointer
    return
}
impl<T> Buffer<T> {
    method &+self.grow(): void {
        grow(self)
        return
    }
}
func make(): Buffer<u8> {
    var result = Buffer<u8> { pointer: empty_pointer() }
    for index in 0..<1 {
        result.grow()
    }
    return move result
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0462"),
        "{diagnostics:?}"
    );
}

#[test]
fn interface_alloc_is_an_upper_bound_for_implementations() {
    let accepted = check_text(
        r#"struct Value {}
struct FactoryImpl {}
primitive empty(): Value
interface Factory {
    pub alloc method &self.make(): Value
}
impl Factory for FactoryImpl {
    method &self.make(): Value { return empty() }
}
"#,
    );
    assert!(
        !accepted
            .iter()
            .any(|diagnostic| diagnostic.message.contains("does not match")),
        "{accepted:?}"
    );

    let rejected = check_text(
        r#"struct Value {}
struct FactoryImpl {}
primitive empty(): Value
interface Factory {
    pub method &self.make(): Value
}
impl Factory for FactoryImpl {
    alloc method &self.make(): Value { return empty() }
}
"#,
    );
    assert!(
        rejected
            .iter()
            .any(|diagnostic| diagnostic.message.contains("does not match")),
        "{rejected:?}"
    );
}

#[test]
fn written_alloc_does_not_justify_a_recursive_cycle_without_an_allocation() {
    let diagnostics = check_text(
        r#"struct Buffer { pointer: *u8 }
func main(): i32 { return 0 }
alloc func cycle(): Buffer {
    return cycle()
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0463"),
        "{diagnostics:?}"
    );
}

#[test]
fn written_alloc_does_not_justify_a_mutually_recursive_cycle() {
    let diagnostics = check_text(
        r#"struct Buffer { pointer: *u8 }
func main(): i32 { return 0 }
alloc func first(): Buffer {
    return second()
}
alloc func second(): Buffer {
    return first()
}
"#,
    );

    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "E0463")
            .count(),
        2,
        "{diagnostics:?}"
    );
}

#[test]
fn mutually_recursive_summaries_converge_from_a_concrete_allocation() {
    let diagnostics = check_text_with_trusted_allocation(
        r#"struct Buffer { pointer: *u8 }
pub(nocter) alloc primitive allocate(): Buffer
func first(flag: bool): Buffer {
    if flag {
        return allocate()
    }
    return second(true)
}
func second(flag: bool): Buffer {
    return first(flag)
}
"#,
    );

    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "E0462")
            .count(),
        2,
        "{diagnostics:?}"
    );
}
