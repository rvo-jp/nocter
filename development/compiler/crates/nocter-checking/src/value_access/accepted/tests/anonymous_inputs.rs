//! Anonymous formals retain lexical identities and share named-call argument preparation.
use super::*;
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{ValueCode, ValueFunctionOrigin, ValueParameter};

#[test]
fn anonymous_inputs_use_their_own_witnesses_and_frozen_call_site_bindings() {
    let unit = accepted(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/anonymous-callable-inputs.nct"
    )));
    let mut formals = std::collections::BTreeSet::new();
    for (&owner, query) in unit.closures() {
        for (position, input) in unit
            .program()
            .closure(owner)
            .unwrap()
            .contract()
            .inputs
            .iter()
            .enumerate()
        {
            if let Some(PayloadSchema::Function(witness)) = unit.program().data_schema(input.data) {
                let formal = ValueParameter::Closure { owner, position };
                assert_eq!(witness.origin, ValueFunctionOrigin::Parameter(formal));
                assert_eq!(
                    unit.program().parameter_owner(formal),
                    Some(ValueCode::Closure(owner))
                );
                assert_eq!(unit.program().data_copyable(input.data), Some(false));
                assert!(formals.insert(formal));
            }
        }
        assert_eq!(query.typed().code(), ValueCode::Closure(owner));
    }
    assert_eq!(formals.len(), 7);
}

#[test]
fn anonymous_input_signatures_do_not_grant_ownership_or_repair_conflicts_from_body_use() {
    use crate::value_access::Rule;
    for (body, rule) in [
        (
            "let apply = (action: func(i32): i32) { action }",
            Rule::MissingTransfer,
        ),
        (
            "let apply = (action: once(i32): i32) { action^(42) }",
            Rule::MissingTransfer,
        ),
        (
            "let apply: func(func(i32): i32): i32 = (action: mut(i32): i32) { 42 }",
            Rule::TypeMismatch,
        ),
        (
            "let apply: func(edit mut(i32): i32): i32 = (take action: mut(i32): i32) { 42 }",
            Rule::TypeMismatch,
        ),
        (
            "let apply = (action: func(func(i32): i32): i32) { 42 }",
            Rule::UnknownType,
        ),
        (
            "let apply = (action: func(): func(i32): i32) { 42 }",
            Rule::UnknownType,
        ),
    ] {
        let source = format!("func run(): void {{ {body} }}");
        let (sources, tree) = input(&source);
        let result = SourceUnit::prepare(bound(&sources, &tree));
        assert!(
            matches!(&result, Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })) if diagnostic.rule() == rule),
            "{source}: {result:?}"
        );
    }
}

#[test]
fn dependent_anonymous_result_carriers_close_against_their_actual_call_inputs() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-tuple-results.nct"
    ));
    assert_eq!(
        super::initializers::answer(source, "ANSWER"),
        nocter_constant_evaluation::state::Value::integer(42)
    );
}

#[test]
fn pending_anonymous_result_bindings_close_through_owned_call_applications() {
    let unit = accepted(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/anonymous-callable-applications.nct"
    )));
    let answer = unit
        .program()
        .resolve_code(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(answer).unwrap().node(),
        &nocter_model::FrozenNode::Scalar(nocter_model::ConstantValue::Integer(42))
    );
}

#[test]
fn an_anonymous_result_shape_with_no_return_producer_does_not_create_a_witness() {
    let source = "func identity(value: i32): i32 { value }\nfunc run(): i32 { let make = () { if true { loop {} }\nreturn identity }\nlet action = make()\naction(42) }";
    let (sources, tree) = input(source);
    assert!(matches!(SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. }))
        if diagnostic.rule() == crate::value_access::Rule::TypeMismatch));
}
