use super::check;
use crate::ast::Item;
use crate::diagnostics::Diagnostic;
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::resolve;
use crate::semantics::{
    AllocationFailurePolicy, AllocationSource, AllocatorCapabilityKind, TrustedDeclarationRole,
};
use crate::source::SourceMap;

fn check_text(text: &str) -> Vec<Diagnostic> {
    check_text_with_trusted_allocator(text, true)
}

fn check_text_with_trusted_allocator(text: &str, trust_allocator: bool) -> Vec<Diagnostic> {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let lexed = lex(&sources, source);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    let parsed = parse(&sources, source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ast = parsed.ast.unwrap();
    let mut resolved = resolve(&sources, &ast);
    for item in &ast.items {
        if trust_allocator {
            match item {
                Item::Struct(struct_) if struct_.name == "Arena" => {
                    resolved.trusted_declarations.insert(
                        struct_.span,
                        TrustedDeclarationRole::AllocatorCapability(
                            AllocatorCapabilityKind::Aborting,
                        ),
                    );
                }
                Item::Primitive(primitive) if primitive.name == "current_allocator_state" => {
                    resolved.trusted_declarations.insert(
                        primitive.name_span,
                        TrustedDeclarationRole::CurrentAllocationContext,
                    );
                }
                Item::Function(function) if function.name == "allocate" => {
                    resolved.trusted_declarations.insert(
                        function.name_span,
                        TrustedDeclarationRole::AllocationOperation {
                            source: AllocationSource::Input(0),
                            failure_policy: AllocationFailurePolicy::Abort,
                        },
                    );
                }
                _ => {}
            }
        }
    }
    let mut diagnostics = resolved.diagnostics.clone();
    diagnostics.extend(check(&sources, &ast, &resolved));
    diagnostics
}

#[test]
fn rejects_untrusted_region_allocator_type() {
    let diagnostics = check_text_with_trusted_allocator(
        r#"copy struct Arena {
    id: usize
}

func use_region(parent: Arena): void {
    region temp using parent {
        let value = 1
    }
    return
}

func main(): i32 {
    return 0
}
"#,
        false,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0439");
}

#[test]
fn diagnoses_direct_region_handle_return() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

func leak(parent: Arena): Arena {
    region temp using parent {
        return temp
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0436");
    assert!(diagnostics[0].message.contains("region `temp`"));
    assert_eq!(diagnostics[0].notes.len(), 1);
}

#[test]
fn diagnoses_region_handle_nested_in_owned_aggregate() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

copy struct Holder {
    arena: Arena
}

func leak(parent: Arena): Holder {
    region temp using parent {
        return Holder { arena: temp }
    }
}


func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0436");
}

#[test]
fn permits_region_independent_copy_result() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

func value(parent: Arena): usize {
    region temp using parent {
        return 42
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_assignment_of_region_value_to_outer_binding() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

func leak(parent: Arena): void {
    var escaped = parent
    region temp using parent {
        escaped = temp
    }
    return
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0437");
    assert_eq!(diagnostics[0].notes.len(), 2);
}

#[test]
fn diagnoses_effectful_region_parent_expression() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

func make_arena(): Arena {
    return Arena { id: 0 }
}

func use_region(): void {
    region temp using make_arena() {
        return
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0438");
}

#[test]
fn accepts_nested_regions_with_established_parents() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

func use_regions(parent: Arena): void {
    region outer using parent {
        region inner using outer {
            let value = 1
        }
    }
    return
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_region_value_returned_through_helper() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

func identity(value: Arena): Arena {
    return value
}

func leak(parent: Arena): Arena {
    region temp using parent {
        return identity(temp)
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0436");
}

#[test]
fn diagnoses_region_value_wrapped_by_helper() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

copy struct Holder {
    arena: Arena
}

func wrap(value: Arena): Holder {
    return Holder { arena: value }
}

func leak(parent: Arena): Holder {
    region temp using parent {
        return wrap(temp)
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0436");
}

#[test]
fn diagnoses_owned_storage_from_transitive_current_allocation_context() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

struct Buffer {
    id: usize
}

struct Text {
    buffer: Buffer
}

primitive current_allocator_state(): usize

func current_allocator(): Arena {
    return Arena { id: current_allocator_state() }
}

func allocate(allocator: &+Arena): Buffer {
    return Buffer { id: allocator.id }
}

func make_text(): Text {
    var allocator = current_allocator()
    var text = Text { buffer: allocate(&+allocator) }
    return move text
}

func leak(parent: Arena): Text {
    region temp using parent {
        return make_text()
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0436");
    assert!(diagnostics[0].message.contains("region `temp`"));
}

#[test]
fn diagnoses_region_storage_in_bare_fallible_error_return() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

primitive current_allocator_state(): usize
primitive error_from_state(state: usize): error

func current_error(): error {
    return error_from_state(current_allocator_state())
}

func leak(parent: Arena): i32! {
    region temp using parent {
        return current_error()
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0436");
    assert!(diagnostics[0].message.contains("region `temp`"));
}
