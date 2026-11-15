//! Anonymous formals retain lexical identities and share named-call argument preparation.
use super::*;
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{ValueCode, ValueFunctionOrigin, ValueParameter};

#[test]
fn captured_nullable_inputs_retain_explicit_input_contexts_and_the_environment_prefix() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/captured-compound-inputs.nct"
    ));
    for (parameter, binding) in [
        ("take value", "let action = value^"),
        ("value", "read action = value"),
        ("edit value", "edit action = value"),
    ] {
        let source = source
            .replace("take value", parameter)
            .replace("let action = value^", binding);
        assert_eq!(
            super::initializers::answer(&source, "ANSWER"),
            nocter_constant_evaluation::state::Value::integer(42)
        );
        let unit = accepted(&source);
        let (&owner, body) = unit.closures().iter().next().unwrap();
        let inputs = body.typed().body().inputs();
        assert!(inputs.environment().is_some());
        assert_eq!(inputs.ordered().count(), 2);
        let data = unit.program().closure(owner).unwrap().contract().inputs[0].data;
        let Some(PayloadSchema::Optional(payload)) = unit.program().data_schema(data) else {
            panic!("nullable input")
        };
        let Some(PayloadSchema::Function(witness)) = unit.program().data_schema(payload) else {
            panic!("formal leaf")
        };
        assert_eq!(
            witness.origin,
            ValueFunctionOrigin::Parameter(ValueParameter::Component {
                input: nocter_model::ValueInput::Closure { owner, position: 0 },
                ordinal: 0,
            })
        );
        assert_eq!(
            unit.program().parameter_owner(match witness.origin {
                ValueFunctionOrigin::Parameter(parameter) => parameter,
                _ => unreachable!(),
            }),
            Some(ValueCode::Closure(owner))
        );
    }
}

#[test]
fn captured_compound_inputs_distinguish_same_signature_leaves_and_actual_suppliers() {
    let source = "const func first(value: i32): i32 { value + 1 }\nconst func second(value: i32): i32 { value + 7 }\nconst func calculate(): i32 { let amount = 30\nlet apply = (amount; take pair: (func(i32): i32, func(i32): i32)): i32 { pair.0(1) + pair.1(3) + amount }\napply((first, second)) }\nconst ANSWER: i32 = calculate()";
    assert_eq!(
        super::initializers::answer(source, "ANSWER"),
        nocter_constant_evaluation::state::Value::integer(42)
    );
    let unit = accepted(source);
    let (&owner, _) = unit.closures().iter().next().unwrap();
    let input = nocter_model::ValueInput::Closure { owner, position: 0 };
    let first = ValueParameter::Component { input, ordinal: 0 };
    let second = ValueParameter::Component { input, ordinal: 1 };
    assert_ne!(first, second);
    assert_eq!(
        unit.program().parameter_contract(first),
        unit.program().parameter_contract(second)
    );
    assert!(
        unit.program()
            .parameter_contract(ValueParameter::Component { input, ordinal: 2 })
            .is_none()
    );
}

#[test]
fn captured_array_input_schemas_preserve_one_element_formal_without_inventing_empty_values() {
    for (length, argument, body) in [
        (2, "[addition, addition]", "items[1](4) + amount"),
        (0, "[addition; 0]", "drop items\namount + 11"),
    ] {
        let source = format!(
            "const func addition(value: i32): i32 {{ value + 7 }}\nconst func calculate(): i32 {{ let amount = 31\nlet apply = (amount; take items: [func(i32): i32; {length}]): i32 {{ {body} }}\napply({argument}) }}\nconst ANSWER: i32 = calculate()"
        );
        assert_eq!(
            super::initializers::answer(&source, "ANSWER"),
            nocter_constant_evaluation::state::Value::integer(42)
        );
    }
}

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
