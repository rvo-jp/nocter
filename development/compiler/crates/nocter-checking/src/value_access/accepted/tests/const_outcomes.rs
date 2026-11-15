//! Representable outcomes share ordinary source, constant publication and runtime consumers.
use crate::value_access::accepted::{Failure, SourceFailure, SourceUnit};
use nocter_constant_evaluation::state::Value as Datum;

#[test]
fn compound_received_assertions_report_the_original_component_check() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-compound-outcome-control.nct"
    ));
    for (name, site) in [
        ("asserted", "value.0^!"),
        ("asserted_read", "value.0!"),
        ("asserted_edit", "value.0!"),
    ] {
        let absent = source.replace(
            &format!("{name}((present(29), addition(100), 7))"),
            &format!("{name}((absent(), addition(100), 7))"),
        );
        assert_ne!(absent, source);
        let fixture = nocter_test_support::ValueSourceFixture::new(&absent);
        let failure = SourceUnit::prepare(fixture.syntax()).unwrap_err();
        let SourceFailure::Checking(Failure::Evaluation { diagnostic, .. }) = failure else {
            panic!("expected the received component assertion to trap: {failure:?}");
        };
        assert_eq!(
            diagnostic.rule(),
            crate::value_access::evaluation::Rule::Execution(
                nocter_constant_evaluation::state::Rule::InactiveVariant
            )
        );
        assert_eq!(
            fixture
                .syntax()
                .text_at(diagnostic.source().primary().span().range()),
            Some(site)
        );
    }
}

#[test]
fn received_optional_propagation_preserves_absence_and_positive_calls() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-outcome-propagation.nct"
    ));
    for (parameter, binding) in [
        ("take value", "let action = value^?"),
        ("value", "read action = value?"),
        ("edit value", "edit action = value?"),
    ] {
        let source = source
            .replace("propagate(take value", &format!("propagate({parameter}"))
            .replace("let action = value^?", binding);
        assert_eq!(
            super::initializers::answer(&source, "ANSWER"),
            Datum::integer(42)
        );
    }
}

#[test]
fn received_fallible_projection_cannot_extract_the_callers_error() {
    for (parameter, binding) in [("value", "read action"), ("edit value", "edit action")] {
        let source = format!(
            "func inspect({parameter}: (func(): i32)!): i32! {{ {binding} = value?\naction() }}"
        );
        let fixture = nocter_test_support::ValueSourceFixture::new(&source);
        let Err(SourceFailure::Checking(Failure::Body {
            failure: crate::value_access::Failure::Rejected(diagnostic),
            ..
        })) = SourceUnit::prepare(fixture.syntax())
        else {
            panic!("a projected failure cannot transfer another owner's error");
        };
        assert_eq!(
            diagnostic.rule(),
            crate::value_access::Rule::MissingTransfer
        );
        assert_eq!(
            fixture
                .syntax()
                .text_at(diagnostic.source().primary().span().range()),
            Some("value?")
        );
    }
}

#[test]
fn received_callable_assertions_preserve_the_original_trap() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-outcome-assertions.nct"
    ));
    for (parameter, binding, site) in [
        ("take value", "let action = value^!", "value^!"),
        ("value", "read action = value!", "value!"),
        ("edit value", "edit action = value!", "value!"),
    ] {
        let present = source
            .replace("take value", parameter)
            .replace("let action = value^!", binding);
        assert_eq!(
            super::initializers::answer(&present, "ANSWER"),
            Datum::integer(42)
        );
        let absent = present.replace("inspect(present())", "inspect(absent())");
        let fixture = nocter_test_support::ValueSourceFixture::new(&absent);
        let failure = SourceUnit::prepare(fixture.syntax()).unwrap_err();
        let SourceFailure::Checking(Failure::Evaluation { diagnostic, .. }) = failure else {
            panic!("expected the received assertion to trap: {failure:?}");
        };
        assert_eq!(
            diagnostic.rule(),
            crate::value_access::evaluation::Rule::Execution(
                nocter_constant_evaluation::state::Rule::InactiveVariant
            )
        );
        assert_eq!(
            fixture
                .syntax()
                .text_at(diagnostic.source().primary().span().range()),
            Some(site)
        );
    }
}

#[test]
fn outcome_representation_closes_each_ordinary_source_seam() {
    for (source, expected) in [
        (
            "const VALUE: i32! = 42",
            Datum::variant(true, Some(Datum::integer(42))),
        ),
        (
            "const func produce(): i32! { 42 }\nconst VALUE: i32! = produce()",
            Datum::variant(true, Some(Datum::integer(42))),
        ),
        ("const VALUE: error? = none", Datum::variant(false, None)),
        (
            "const func absent(): error? { none }\nconst VALUE: error? = absent()",
            Datum::variant(false, None),
        ),
        (
            "const func forward(take value: i32!): i32! { value^ }\nconst VALUE: i32! = forward(42)",
            Datum::variant(true, Some(Datum::integer(42))),
        ),
        (
            "const func asserted(): i32 { let value: i32! = 42\nvalue^! }\nconst VALUE: i32 = asserted()",
            Datum::integer(42),
        ),
    ] {
        assert_eq!(
            super::initializers::answer(source, "VALUE"),
            expected,
            "{source}"
        );
    }
}

#[test]
fn inactive_payload_cleanup_uses_only_representable_cases() {
    for source in [
        "const func discard(take value: i32!): i32 { 42 }\nconst VALUE: i32 = discard(1)",
        "const func discard(take value: error?): i32 { 42 }\nconst VALUE: i32 = discard(none)",
        "const func discard(): i32 { let value: error? = none\ndrop value\n42 }\nconst VALUE: i32 = discard()",
        "const func discard(): i32 { let values: [error?; 3] = [none, none, none]\nlet selected = values[1]^\ndrop selected\n42 }\nconst VALUE: i32 = discard()",
    ] {
        assert_eq!(
            super::initializers::answer(source, "VALUE"),
            Datum::integer(42)
        );
    }
}

#[test]
fn success_propagation_does_not_materialize_its_impossible_error_case() {
    assert_eq!(
        super::initializers::answer(
            "const func forward(take value: i32!): i32! { value^? }\nconst VALUE: i32! = forward(42)",
            "VALUE"
        ),
        Datum::variant(true, Some(Datum::integer(42)))
    );
}

#[test]
fn excluded_callable_assertions_preserve_the_original_trap() {
    for (binding, site) in [
        ("let action = value^!", "value^!"),
        ("read action = value!", "value!"),
        ("edit action = value!", "value!"),
    ] {
        let source = format!(
            "const func absent(): (func(): i32)? {{ none }}\nconst func calculate(): i32 {{ var value = absent()\n{binding}\naction() }}\nconst VALUE: i32 = calculate()"
        );
        let fixture = nocter_test_support::ValueSourceFixture::new(&source);
        let failure = SourceUnit::prepare(fixture.syntax()).unwrap_err();
        let SourceFailure::Checking(Failure::Evaluation { diagnostic, .. }) = failure else {
            panic!("expected original assertion failure: {failure:?}");
        };
        assert_eq!(
            diagnostic.rule(),
            crate::value_access::evaluation::Rule::Execution(
                nocter_constant_evaluation::state::Rule::InactiveVariant
            )
        );
        assert_eq!(
            fixture
                .syntax()
                .text_at(diagnostic.source().primary().span().range()),
            Some(site)
        );
        let constructed = source.replace(
            "const func absent(): (func(): i32)? { none }",
            "const func forward(take value: (func(): i32)?): (func(): i32)? { value^ }\nconst func absent(): (func(): i32)? { forward(none) }",
        );
        let fixture = nocter_test_support::ValueSourceFixture::new(&constructed);
        let failure = SourceUnit::prepare(fixture.syntax()).unwrap_err();
        let SourceFailure::Checking(Failure::Evaluation { diagnostic, .. }) = failure else {
            panic!("expected constructed assertion failure: {failure:?}")
        };
        assert_eq!(
            diagnostic.rule(),
            crate::value_access::evaluation::Rule::Execution(
                nocter_constant_evaluation::state::Rule::InactiveVariant
            )
        );
        assert_eq!(
            fixture
                .syntax()
                .text_at(diagnostic.source().primary().span().range()),
            Some(site)
        );
        let present = source.replace(
            "const func absent(): (func(): i32)? { none }",
            "const func live(): i32 { 7 }\nconst func absent(): (func(): i32)? { live }",
        );
        assert_ne!(source, present);
        assert_eq!(
            super::initializers::answer(&present, "VALUE"),
            Datum::integer(7)
        );
    }
}

#[test]
fn an_uninhabited_projection_still_performs_its_variant_assertion() {
    let fixture = nocter_test_support::ValueSourceFixture::new(
        "const func fail(): i32! { let value: error? = none\nthrow value^! }\nconst VALUE: i32! = fail()",
    );
    let Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) =
        SourceUnit::prepare(fixture.syntax())
    else {
        panic!("expected the assertion on an absent payload to fail during evaluation");
    };
    assert_eq!(
        diagnostic.rule(),
        crate::value_access::evaluation::Rule::Execution(
            nocter_constant_evaluation::state::Rule::InactiveVariant
        )
    );
}

#[test]
fn nested_unavailable_payloads_preserve_the_original_failure_site() {
    use std::fmt::Write as _;
    let mut source = String::from("struct Layer0 { child: error }\n");
    for index in 1..64 {
        writeln!(
            source,
            "struct Layer{index} {{ child: Layer{} }}",
            index - 1
        )
        .unwrap();
    }
    source.push_str("const func fail(): i32! { let value: Layer63? = none\nthrow ");
    source.push_str(&"(".repeat(64));
    source.push_str("value^!");
    for _ in 0..64 {
        source.push_str(").child");
    }
    source.push_str(" }\nconst VALUE: i32! = fail()");
    let fixture = nocter_test_support::ValueSourceFixture::new(&source);
    let diagnostic = match SourceUnit::prepare(fixture.syntax()) {
        Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) => diagnostic,
        Err(failure) => panic!("expected an absent-payload assertion: {failure:?}"),
        Ok(_) => panic!("expected the original absent-payload assertion to fail"),
    };
    assert_eq!(
        diagnostic.rule(),
        crate::value_access::evaluation::Rule::Execution(
            nocter_constant_evaluation::state::Rule::InactiveVariant
        )
    );
    let range = diagnostic.source().primary().span().range();
    assert_eq!(fixture.syntax().text_at(range), Some("value^!"));
}

#[test]
fn uninhabited_projection_does_not_authorize_error_operations_or_dead_runtime_calls() {
    use crate::value_access::constant::Rule;
    let rejected = |source: &str| {
        let fixture = nocter_test_support::ValueSourceFixture::new(source);
        let Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) =
            SourceUnit::prepare(fixture.syntax())
        else {
            panic!("expected a constant-domain failure: {source}");
        };
        diagnostic.rule()
    };
    assert!(matches!(
        rejected("const func forward(take value: error): i32! { throw value^ }"),
        Rule::UnsupportedData(_)
    ));
    assert!(matches!(
        rejected(
            "const func recover(take value: i32!): i32 { value^ catch reason { drop reason\n0 } }"
        ),
        Rule::UnsupportedCleanup(_)
    ));
    assert!(matches!(
        rejected(
            "func runtime(): i32 { 0 }\nconst func recover(take value: i32!): i32 { value^ catch _ { runtime() } }"
        ),
        Rule::RuntimeCall(_)
    ));
}

#[test]
fn success_and_inactive_error_data_execute_and_freeze_without_error_values() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/const-outcome-values.nct"
    ));
    assert_eq!(
        super::initializers::answer(source, "VALUE"),
        Datum::variant(true, Some(Datum::integer(42)))
    );
    assert_eq!(
        super::initializers::answer(source, "GUARDED"),
        Datum::integer(42)
    );
    assert_eq!(
        super::initializers::answer(source, "EMPTY"),
        Datum::variant(false, None)
    );
    assert_eq!(
        super::initializers::answer(source, "NESTED"),
        Datum::variant(false, None)
    );
    assert_eq!(
        super::initializers::answer(source, "ANSWER"),
        Datum::integer(42)
    );
}

#[test]
fn an_unavailable_variant_does_not_authorize_live_error_cleanup_or_callable_publication() {
    let fixture = nocter_test_support::ValueSourceFixture::new(
        "const func discard(take value: error): void {}",
    );
    assert!(
        matches!(SourceUnit::prepare(fixture.syntax()), Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) if matches!(diagnostic.rule(), crate::value_access::constant::Rule::UnsupportedCleanup(_)))
    );
    let fixture = nocter_test_support::ValueSourceFixture::new("func named(): i32 { 42 }");
    let source = SourceUnit::prepare(fixture.syntax()).unwrap();
    let context = source.declarations().bind(fixture.syntax()).unwrap();
    let program = context.program();
    let code = program
        .resolve_code(program.symbols().get("named").unwrap())
        .unwrap();
    let callable = program
        .function_data(nocter_declarations::value_access::FunctionWitness {
            origin: nocter_model::ValueFunctionOrigin::Declaration(code),
            invocation: nocter_model::ValueInvocation::Read,
        })
        .unwrap();
    let mut types = program.body_types(code).unwrap();
    let wrapped = types.fallible(callable).unwrap();
    let program = types.finish();
    let mut domain = crate::value_access::evaluation::Types::default();
    assert!(domain.supports(&program, wrapped).unwrap());
    assert!(!domain.supports_final(&program, wrapped).unwrap());
}
