use super::*;
use crate::value_access::Rule;
use nocter_model::{BuiltinType, ConstantValue, FrozenNode, ValueInputOperation};

#[test]
fn standalone_results_come_from_the_own_checked_body_in_required_execution() {
    let unit = accepted(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/inferred-closure-results.nct"
    )));
    let answer = unit
        .program()
        .resolve_code(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(answer).unwrap().node(),
        &FrozenNode::Scalar(ConstantValue::Integer(42))
    );
    let mut operations = std::collections::BTreeSet::new();
    let mut unit_results = 0;
    for (&id, body) in unit.closures() {
        let contract = unit.program().closure(id).unwrap().contract();
        assert_eq!(body.typed().result_data(), contract.result);
        if unit.program().data_kind(contract.result) == Some(BuiltinType::Void) {
            unit_results += 1;
        }
        for input in &contract.inputs {
            operations.insert(input.operation);
        }
    }
    assert_eq!(unit_results, 2);
    assert_eq!(
        operations,
        std::collections::BTreeSet::from([
            ValueInputOperation::Read,
            ValueInputOperation::Edit,
            ValueInputOperation::Take
        ])
    );
}

#[test]
fn inferred_callable_results_preserve_named_captured_and_application_witnesses() {
    use nocter_declarations::value_access::PayloadSchema;
    use nocter_model::ValueFunctionOrigin;
    let unit = accepted(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/inferred-callable-results.nct"
    )));
    let answer = unit
        .program()
        .resolve_code(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(answer).unwrap().node(),
        &FrozenNode::Scalar(ConstantValue::Integer(42))
    );
    let mut named_results = 0;
    let mut captured_results = 0;
    for (&id, body) in unit.closures() {
        let result = unit.program().closure(id).unwrap().contract().result;
        assert_eq!(result, body.typed().result_data());
        if let Some(PayloadSchema::Function(witness)) = unit.program().data_schema(result) {
            match witness.origin {
                ValueFunctionOrigin::Declaration(_) => named_results += 1,
                ValueFunctionOrigin::Closure(_) => captured_results += 1,
                _ => panic!("unclosed inferred result: {witness:?}"),
            }
        }
    }
    assert_eq!(named_results, 3);
    assert_eq!(captured_results, 2);
}

#[test]
fn explicit_and_expected_anonymous_callable_results_share_checked_value_supply_and_views() {
    let unit = accepted(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/anonymous-result-contracts.nct"
    )));
    let answer = unit
        .program()
        .resolve_code(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(answer).unwrap().node(),
        &FrozenNode::Scalar(ConstantValue::Integer(42))
    );
}

#[test]
fn anonymous_callable_result_constraints_keep_data_operations_and_actual_invocation_limits() {
    for body in [
        "let make = (): func(usize): i32 { identity }",
        "let make: func(): i32 = (): func(i32): i32 { identity }",
        "let make: func(): func(bool): i32 = () { (value: i32) { value } }",
        "let make = (): func(take Item): i32 { (edit value: Item) { value.value } }",
        "let make = (item^;): func(): Item { (item^;) { item^ } }",
    ] {
        let source = format!(
            "struct Item {{ value: i32 }}\nfunc identity(value: i32): i32 {{ value }}\nfunc run(take item: Item): void {{ {body} }}"
        );
        let (sources, tree) = input(&source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })) if diagnostic.rule() == Rule::TypeMismatch),
            "{source}"
        );
    }
    let source = "func run(): void { let make = (): func(): i32 { loop {} } }";
    let (sources, tree) = input(source);
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })) if diagnostic.rule() == Rule::UnknownType && diagnostic.source().message().contains("signature alone"))
    );
}

#[test]
fn anonymous_result_application_equalities_reject_distinct_code_at_the_real_return_value() {
    let source = "func left(value: i32): i32 { value }\nfunc right(value: i32): i32 { value + 1 }\nfunc first(): func(i32): i32 { left }\nfunc second(): func(i32): i32 { right }\nfunc run(): void { let make = (flag: bool): func(i32): i32 { if flag { return first() }\nsecond() } }";
    let (sources, tree) = input(source);
    let Err(SourceFailure::Checking(Failure::Body {
        failure: BodyFailure::Rejected(diagnostic),
        ..
    })) = SourceUnit::prepare(bound(&sources, &tree))
    else {
        panic!("distinct result witnesses");
    };
    assert_eq!(diagnostic.rule(), Rule::TypeMismatch);
    let range = diagnostic.source().primary().span().range();
    assert_eq!(
        &source[range.start().get() as usize..range.end().get() as usize],
        "second()"
    );
}

#[test]
fn a_known_interrupted_return_shape_remains_available_without_claiming_a_returned_value() {
    let unit = accepted(
        "func scalar(value: i32): i32 { value }\nfunc run(): void { let action = () { return scalar(if true { loop {} } else { loop {} }) } }",
    );
    assert_eq!(unit.closures().len(), 1);
    let (&id, body) = unit.closures().iter().next().unwrap();
    assert_eq!(
        unit.program()
            .data_kind(unit.program().closure(id).unwrap().contract().result),
        Some(BuiltinType::I32)
    );
    assert!(
        !body
            .typed()
            .body()
            .blocks()
            .get(body.typed().body().root())
            .unwrap()
            .ty
            .may_complete()
    );
}

#[test]
fn runtime_callable_transport_closes_without_a_required_initializer() {
    accepted(
        "func identity(value: i32): i32 { value }\nfunc selected(): func(i32): i32 { identity }\nfunc run(): i32 { let make = () { selected() }\nlet value = make()\nvalue(42) }",
    );
}

#[test]
fn transported_results_do_not_merge_distinct_code_witnesses() {
    let source = "func left(value: i32): i32 { value }\nfunc right(value: i32): i32 { value + 1 }\nfunc make_left(): func(i32): i32 { left }\nfunc make_right(): func(i32): i32 { right }\nfunc forward(flag: bool): func(i32): i32 { let first = () { make_left() }\nlet second = () { make_right() }\nif flag { first() } else { second() } }";
    let (sources, tree) = input(source);
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })) if diagnostic.rule() == Rule::TypeMismatch)
    );
    let unit = accepted(&source.replace("{ right }", "{ left }"));
    let forward = unit
        .program()
        .resolve(unit.program().symbols().get("forward").unwrap())
        .unwrap();
    let left = unit
        .program()
        .resolve(unit.program().symbols().get("left").unwrap())
        .unwrap();
    assert_eq!(
        unit.result_witness(forward.into()).unwrap().origin,
        nocter_model::ValueFunctionOrigin::Declaration(left)
    );
}

#[test]
fn returned_owned_environments_keep_transfer_and_retirement_obligations() {
    let prefix =
        "func run(take reason: error): void { let maker = (reason^;) { (reason^;) { reason^ } }\n";
    for body in [
        "let inner = maker()",
        "let inner = maker^()\nlet copy = inner",
    ] {
        let source = format!("{prefix}{body} }}");
        let (sources, tree) = input(&source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })) if diagnostic.rule() == Rule::MissingTransfer),
            "{source}"
        );
    }
    let source = format!(
        "{prefix}let inner = maker^()\nlet first = inner^()\ndrop first\nlet second = inner^()\ndrop second }}"
    );
    let (sources, tree) = input(&source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}

#[test]
fn returning_a_callable_does_not_manufacture_const_invocation_capability() {
    let source = "func runtime(value: i32): i32 { value }\nconst func bad(): i32 { let make = () { runtime }\nlet value = make()\nvalue(42) }";
    let (sources, tree) = input(source);
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) if matches!(diagnostic.rule(), crate::value_access::constant::Rule::RuntimeCall(_)))
    );
}

#[test]
fn inferred_returns_require_consistent_data_and_never_change_input_operations_or_repair_later() {
    for (body, rule, reason) in [
        (
            "let action = (flag: bool) { if flag { return scalar(if true { loop {} } else { loop {} }) }\ntrue }",
            Rule::TypeMismatch,
            "returns include",
        ),
        (
            "let action = (value: i32) { if value == 0 { return true }\n42 }",
            Rule::TypeMismatch,
            "returns include",
        ),
        (
            "let action = (value: i32) { if value == 0 { return 42 }\ntrue }",
            Rule::TypeMismatch,
            "returns include",
        ),
        (
            "let action = () { loop {} }",
            Rule::UnknownType,
            "no result data",
        ),
        (
            "let action = (value) { value }\nlet selected: func(i32): i32 = action",
            Rule::UnknownType,
            "parameter data",
        ),
        (
            "let action = (value: Item) { value^ }",
            Rule::MissingTransfer,
            "owning source",
        ),
        (
            "let action = (value: i32) { value += 1\nvalue }",
            Rule::NotWritable,
            "mutable",
        ),
    ] {
        let source = format!(
            "struct Item {{ value: i32 }}\nfunc scalar(value: i32): i32 {{ value }}\nfunc run(): void {{ {body} }}"
        );
        let (sources, tree) = input(&source);
        let Err(SourceFailure::Checking(Failure::Body {
            failure: BodyFailure::Rejected(diagnostic),
            ..
        })) = SourceUnit::prepare(bound(&sources, &tree))
        else {
            panic!("{source}");
        };
        assert_eq!(diagnostic.rule(), rule, "{}", diagnostic.source().message());
        assert!(
            diagnostic.source().message().contains(reason),
            "{}",
            diagnostic.source().message()
        );
        if reason == "returns include" {
            assert_eq!(diagnostic.source().notes().len(), 1);
            assert!(!diagnostic.source().primary().span().range().is_empty());
        }
    }
}

#[test]
fn result_dependent_operations_keep_the_declared_or_expected_context_requirement() {
    for body in [
        "let action = (value: i32?) { value? }",
        "let action = (take reason: error) { throw reason^ }",
        "let action = () { none }",
    ] {
        let source = format!("func run(): void {{ {body} }}");
        let (sources, tree) = input(&source);
        assert!(
            matches!(
                SourceUnit::prepare(bound(&sources, &tree)),
                Err(SourceFailure::Checking(Failure::Body {
                    failure: BodyFailure::Rejected(_),
                    ..
                }))
            ),
            "{source}"
        );
    }
    accepted("func run(): void { let action: func(i32?): i32? = (value) { value? } }");
}
