use super::check_text;
use crate::ast::Item;
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::resolve;
use crate::semantics::{AllocationFailurePolicy, AllocationSource, TrustedDeclarationRole};
use crate::source::SourceMap;

fn check_text_with_trusted_allocation(text: &str) -> Vec<crate::diagnostics::Diagnostic> {
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
            Item::Primitive(primitive) if primitive.name == "allocate" => Some(primitive.name_span),
            _ => None,
        })
        .unwrap();
    resolved.trusted_declarations.insert(
        allocation,
        TrustedDeclarationRole::AllocationOperation {
            source: AllocationSource::CurrentContext,
            failure_policy: AllocationFailurePolicy::Abort,
        },
    );
    let mut diagnostics = resolved.diagnostics.clone();
    diagnostics.extend(super::super::check(&sources, &ast, &resolved));
    diagnostics
}

#[test]
fn requires_alloc_when_a_function_returns_allocated_storage() {
    let diagnostics = check_text_with_trusted_allocation(
        r#"struct Buffer { pointer: *u8 }
pub(nocter) primitive allocate(): Buffer
func make(): Buffer {
    return allocate()
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0462")
    );
}

#[test]
fn accepts_an_exact_result_allocation_contract() {
    let diagnostics = check_text_with_trusted_allocation(
        r#"struct Buffer { pointer: *u8 }
pub(nocter) primitive allocate(): Buffer
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
pub(nocter) primitive allocate(): Buffer
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
pub(nocter) primitive allocate(): Buffer
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
fn propagates_retained_mutations_through_methods_wrappers_and_loops() {
    let diagnostics = check_text_with_trusted_allocation(
        r#"struct Buffer<T> { pointer: *T }
pub(nocter) primitive allocate<T>(): Buffer<T>
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
