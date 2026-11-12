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

#[test]
fn optional_value_propagation_uses_ordinary_scopes_and_cancels_prepared_calls() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/optional-propagation.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn optional_propagation_preserves_absence_and_chooses_the_outermost_result_layer() {
    for source in [
        "const func forward(value: i32?): i32? { value? }\nconst ANSWER: i32 = forward(none)!",
        "const func forward(value: i32??): i32?? { (value?)? }\nconst func some(value: i32?): i32?? { value }\nconst ANSWER: i32? = forward(some(none))!",
    ] {
        let fixture = nocter_test_support::ValueSourceFixture::new(source);
        assert!(
            matches!(SourceUnit::prepare(fixture.syntax()),
            Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. }))
                if diagnostic.rule() == crate::value_access::evaluation::Rule::Execution(nocter_constant_evaluation::state::Rule::AbsentValue)),
            "{source}"
        );
    }
}
#[test]
fn propagation_cannot_invent_a_return_layer_or_reuse_a_transferred_carrier() {
    for source in [
        "func run(value: i32?): i32 { value? }",
        "func run(value: i32): i32? { value? }",
        "const VALUE: i32? = none\nconst ANSWER: i32? = VALUE?",
        "struct Item { value: i32 }\nfunc run(value: Item?): i32? { let item = value?\nitem.value }",
        "func run(): i32? { let value: i32? = 42\nlet first = value^?\nfirst + value? }",
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
fn scoped_optional_edit_targets_the_existing_payload_instead_of_a_copy() {
    assert_eq!(
        answer(
            "const func set(edit value: i32): void { value = 42 }\nconst func run(): i32 { var value: i32? = 1\nset(value!)\nvalue! }\nconst ANSWER: i32 = run()",
            "ANSWER"
        ),
        Datum::integer(42)
    );
}

#[test]
fn scoped_optional_places_preserve_payload_identity_authority_and_saved_indices() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/optional-places.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn live_optional_payload_places_prevent_stale_tag_access_and_owner_retirement() {
    for body in [
        "read payload = value!\nvalue = none\npayload",
        "observe(value!, clear(value))",
        "observe(value!, change(value!))",
        "read payload = value!\nlet moved = value^\npayload",
    ] {
        let source = format!(
            "func clear(edit value: i32?): i32 {{ value = none\n0 }}\nfunc change(edit value: i32): i32 {{ value = 42\n0 }}\nfunc observe(value: i32, ignored: i32): i32 {{ value }}\nfunc run(): i32 {{ var value: i32? = 42\n{body} }}"
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
fn scoped_optional_places_cannot_manufacture_update_authority_or_surviving_temporary_owners() {
    for source in [
        "func set(edit value: i32): void { value = 42 }\nfunc run(value: i32?): void { set(value!) }",
        "func run(): void { let value: i32? = 42\nedit payload = value! }",
        "func make(): i32? { 42 }\nfunc run(): void { read payload = make()! }",
    ] {
        let fixture = nocter_test_support::ValueSourceFixture::new(source);
        assert!(
            matches!(
                SourceUnit::prepare(fixture.syntax()),
                Err(SourceFailure::Checking(Failure::Body {
                    failure: BodyFailure::Rejected(_),
                    ..
                }))
            ),
            "{source}"
        );
    }
}

#[test]
fn scoped_optional_propagation_preserves_noncopy_sources_callers_and_saved_indices() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/optional-place-propagation.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    for (name, ty) in [("absent_nested", "i32??"), ("absent_unit", "void?")] {
        let source = format!("{source}\nconst EMPTY: {ty} = {name}()");
        assert_eq!(answer(&source, "EMPTY"), Datum::variant(false, None));
    }
}
#[test]
fn scoped_optional_propagation_keeps_tag_stability_and_readonly_authority() {
    for source in [
        "func run(value: i32?): i32 { read payload = value?\npayload }",
        "func set(edit value: i32): void { value = 42 }\nfunc run(value: i32?): void? { set(value?) }",
        "func make(): i32? { 42 }\nfunc run(): i32? { read payload = make()?\npayload }",
        "func run(): i32? { var value: i32? = 42\nread payload = value?\nvalue = none\npayload }",
        "func run(): i32? { let value: i32? = 42\nread payload = value?\ndrop value\npayload }",
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
fn optional_recovery_is_lazy_preserves_layers_and_reuses_scope_exit_protocols() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/optional-recovery.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
    let source = format!("{source}\nconst INNER: i32? = some(none) otherwise {{ 42 }}");
    assert_eq!(answer(&source, "INNER"), Datum::variant(false, None));
}
#[test]
fn optional_recovery_cannot_invent_ownership_change_payload_data_or_restore_a_retired_source() {
    for source in [
        "func run(): i32 { 42 otherwise { 0 } }",
        "func run(value: i32?): i32 { value otherwise { true } }",
        "struct Item { value: i32 }\nfunc run(value: Item?): Item { value otherwise { Item { value: 42 } } }",
        "func run(): i32 { let value: i32? = none\nloop { let made = value^ otherwise { break }\nreturn made }\nvalue! }",
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
fn scoped_recovery_selects_a_surviving_place_without_copying_it() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/optional-recovery-places.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}
#[test]
fn scoped_recovery_cannot_keep_handler_locals_or_upgrade_readonly_alternatives() {
    for source in [
        "func run(value: i32?): i32 { read payload = value otherwise { let local = 42\nlocal }\npayload }",
        "func run(edit value: i32?, fallback: i32): i32 { edit payload = value otherwise { fallback }\npayload }",
        "func run(value: i32?, edit fallback: i32): i32 { edit payload = value otherwise { fallback }\npayload }",
        "func run(value: i32?): i32 { read payload = value otherwise { 42 }\npayload }",
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
fn alternative_recovery_places_keep_tag_owner_and_call_reservation_protection() {
    for body in [
        "read selected = source otherwise { backup }\nsource = none\nselected",
        "read selected = source otherwise { backup }\nbackup = 0\nselected",
        "read selected = source otherwise { backup }\ndrop backup\nselected",
        "observe(source otherwise { backup }, clear(source))",
        "observe(source otherwise { backup }, change(backup))",
        "set_two(source otherwise { backup }, backup)\n0",
        "set_two(source otherwise { backup }, source!)\n0",
    ] {
        let source = format!(
            "func observe(value: i32, later: i32): i32 {{ value }}\nfunc clear(edit value: i32?): i32 {{ value = none\n0 }}\nfunc change(edit value: i32): i32 {{ value = 0\n0 }}\nfunc set_two(edit first: i32, edit second: i32): void {{}}\nfunc run(flag: bool): i32 {{ var source: i32? = if flag {{ 42 }} else {{ none }}\nvar backup = 42\n{body} }}"
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
fn every_selected_fallback_place_is_initialized_before_forwarding() {
    let source = "struct Pair { first: i32\nsecond: i32 }\nfunc run(source: i32?): i32 { var backup = Pair { first: 42, second: 0 }\nlet removed = backup.first^\nread selected = source otherwise { backup.first }\nselected }";
    let fixture = nocter_test_support::ValueSourceFixture::new(source);
    assert!(
        matches!(
            SourceUnit::prepare(fixture.syntax()),
            Err(SourceFailure::Checking(Failure::Flow { .. }))
        ),
        "{source}"
    );
}
