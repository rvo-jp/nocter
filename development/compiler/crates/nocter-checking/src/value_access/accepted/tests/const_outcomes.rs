//! Representable outcomes share ordinary source, constant publication and runtime consumers.
use crate::value_access::accepted::{Failure, SourceFailure, SourceUnit};
use nocter_constant_evaluation::state::Value as Datum;

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
