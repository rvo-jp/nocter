//! Optional tags and payload supply use the same checked and stateful value path.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value as Datum;
#[test]
fn optional_construction_forwarding_inputs_and_payload_selection_execute_required_source() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/optional-values.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn optional_absence_is_not_a_present_wrapper_and_assertion_failure_has_its_source_cause() {
    for source in [
        "const func absent(): i32? { none }\nconst ANSWER: i32 = absent()!",
        "const func same(value: i32?): i32? { value }\nconst ANSWER: i32 = same(none)!",
        "const func lift(value: i32?): i32?? { value }\nconst ANSWER: i32 = (lift(none)!)!",
        "const VALUE: void? = none\nconst ANSWER: void = VALUE!",
        "const func receive(value: i32, ignored: i32): i32 { value }\nconst func fail(): i32 { let value: i32? = none\nreceive(value!, if true { return 42 } else { 0 }) }\nconst ANSWER: i32 = fail()",
    ] {
        let fixture = nocter_test_support::ValueSourceFixture::new(source);
        assert!(
            matches!(SourceUnit::prepare(fixture.syntax()), Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) if diagnostic.rule() == crate::value_access::evaluation::Rule::Execution(nocter_constant_evaluation::state::Rule::AbsentValue)),
            "{source}"
        );
    }
}
#[test]
fn optional_context_cannot_invent_ownership_or_mutate_an_unwrapped_direct_place() {
    for source in [
        "func run(): void { let value = none }",
        "func run(): void { let value: i32 = none }",
        "func run(): void { let value = 42! }",
        "func run(): void { let first: i32? = 1\nlet next: i64? = first }",
        "func run(): void { read alias: i32? = 42 }",
        "func update(edit value: i32?): void {}\nfunc run(): void { var value = 1\nupdate(value) }",
        "struct Item { value: i32 }\nfunc receive(value: Item?): void {}\nfunc run(): void { let item = Item { value: 1 }\nreceive(item) }",
        "struct Item { value: i32 }\nfunc run(): void { let item: Item? = Item { value: 1 }\nlet extracted = item! }",
        "func run(): void { let item: i32? = 42\nlet extracted = item^!\nlet reused = item }",
        "func run(): void { let item: i32? = 42\nread alias: i32? = item\nlet extracted = alias^! }",
    ] {
        let fixture = nocter_test_support::ValueSourceFixture::new(source);
        assert!(
            matches!(
                SourceUnit::prepare(fixture.syntax()),
                Err(SourceFailure::Checking(
                    Failure::Body {
                        failure: BodyFailure::Rejected(_),
                        ..
                    } | Failure::Flow { .. }
                ))
            ),
            "{source}"
        );
    }
}

#[test]
fn deep_flat_optional_metadata_and_absence_close_without_recursive_source_or_type_walks() {
    let source = nocter_test_support::deep_optional_source(10_000);
    let unit = accepted(&source);
    let constant = unit
        .program()
        .resolve_constant(unit.program().symbols().get("VALUE").unwrap())
        .unwrap();
    let value = unit.constant_value(constant).unwrap();
    assert_eq!(
        value.node(),
        &nocter_model::FrozenNode::Aggregate(vec![0].into())
    );
    assert_eq!(
        value.nodes()[0],
        nocter_model::FrozenNode::Variant {
            primary: false,
            payload: None
        }
    );
}

#[test]
fn implicit_wrapper_reservations_reject_mutation_double_transfer_and_post_commit_reuse() {
    for body in [
        "receive(item^, change(item))",
        "receive(item^, read_twice(item^))",
        "receive(item^, inspect(item))\nlet reused = inspect(item)",
    ] {
        let source = format!(
            "struct Item {{ value: i32 }}\nfunc receive(take value: Item?, other: i32): void {{}}\nfunc change(edit value: Item): i32 {{ value.value += 1\nvalue.value }}\nfunc inspect(value: Item): i32 {{ value.value }}\nfunc read_twice(take value: Item?): i32 {{ 0 }}\nfunc run(): void {{ var item = Item {{ value: 42 }}\n{body} }}"
        );
        let fixture = nocter_test_support::ValueSourceFixture::new(&source);
        assert!(
            matches!(
                SourceUnit::prepare(fixture.syntax()),
                Err(SourceFailure::Checking(Failure::Flow { .. }))
            ),
            "{source}"
        );
    }
}

#[test]
fn missing_payloads_do_not_turn_into_absence_or_invent_unit_conversion() {
    for source in [
        "func run(): i32? {}",
        "func run(): i32?? { return }",
        "func run(): (void, void)? {}",
        "func run(): i32? { if true { return } else { 42 } }",
    ] {
        let fixture = nocter_test_support::ValueSourceFixture::new(source);
        assert!(
            matches!(SourceUnit::prepare(fixture.syntax()),
            Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. }))
                if diagnostic.rule() == crate::value_access::Rule::MissingResult),
            "{source}"
        );
    }
}

#[test]
fn a_conditional_without_else_still_requires_a_unit_branch_before_contextual_wrapping() {
    let fixture =
        nocter_test_support::ValueSourceFixture::new("func run(): void? { if true { none } }");
    assert!(matches!(SourceUnit::prepare(fixture.syntax()),
        Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. }))
            if diagnostic.rule() == crate::value_access::Rule::TypeMismatch));
}
