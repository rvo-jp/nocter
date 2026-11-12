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
fn inferred_returns_require_consistent_data_and_never_change_input_operations_or_repair_later() {
    for (body, rule, reason) in [
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
        let source = format!("struct Item {{ value: i32 }}\nfunc run(): void {{ {body} }}");
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
