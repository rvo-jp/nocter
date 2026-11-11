//! Only an already selected contract can complete an anonymous declaration's operations/data.
use super::*;
use nocter_model::ValueInputOperation;

#[test]
fn selected_contracts_complete_closure_data_and_preserve_input_operations() {
    let unit = accepted(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-closures.nct"
    )));
    let mut modes = std::collections::BTreeSet::new();
    for (&id, query) in unit.closures() {
        let contract = unit.program().closure(id).unwrap().contract();
        assert_eq!(contract.inputs.len(), 1);
        let input = &contract.inputs[0];
        assert_eq!(
            query.typed().body().inputs().explicit[0].operation,
            input.operation
        );
        modes.insert(input.operation);
    }
    assert_eq!(
        modes,
        std::collections::BTreeSet::from([
            ValueInputOperation::Read,
            ValueInputOperation::Edit,
            ValueInputOperation::Take
        ])
    );
}

#[test]
fn explicit_closure_contract_conflicts_reject_before_body_use_can_repair_them() {
    for (ty, closure, reason) in [
        (
            "func(i32): i32",
            "(edit value: i32): i32 { 42 }",
            "operation",
        ),
        (
            "func(edit i32): i32",
            "(take value: i32): i32 { 42 }",
            "operation",
        ),
        (
            "func(take i32): i32",
            "(edit value: i32): i32 { 42 }",
            "operation",
        ),
        (
            "func(i32): i32",
            "(value: usize): i32 { 42 }",
            "parameter data",
        ),
        ("func(i32): i32", "(value): usize { 42 }", "result data"),
        (
            "func(i32): i32",
            "(first, second) { first + second }",
            "number",
        ),
    ] {
        let source = format!("func work(): void {{ let action: {ty} = {closure} }}");
        let (sources, tree) = input(&source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })) if diagnostic.rule() == crate::value_access::Rule::TypeMismatch && diagnostic.source().message().contains(reason)),
            "{source}"
        );
    }
}

#[test]
fn missing_context_and_later_uses_do_not_infer_anonymous_input_authority() {
    for source in [
        "func work(): void { let action = (value) { value } }",
        "func work(): void { let action = (value): i32 { 42 } }",
        "func work(): void { let action = (value) { value }\nlet selected: func(i32): i32 = action }",
    ] {
        let (sources, tree) = input(source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })) if diagnostic.rule() == crate::value_access::Rule::UnknownType),
            "{source}"
        );
    }
    let source = "struct Item { value: i32 }\nfunc work(): void { let action = (value: Item): Item { value^ } }";
    let (sources, tree) = input(source);
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })) if diagnostic.rule() == crate::value_access::Rule::MissingTransfer)
    );
}

#[test]
fn selected_taking_input_is_not_weakened_when_its_body_only_reads() {
    let unit = accepted(
        "func work(): i32 { let action: func(take i32): i32 = (value) { value }\naction(42) }",
    );
    let (&id, _) = unit.closures().iter().next().unwrap();
    assert_eq!(
        unit.program().closure(id).unwrap().contract().inputs[0].operation,
        ValueInputOperation::Take
    );
}

#[test]
fn contextual_completion_does_not_rewrite_a_named_declarations_operations() {
    let source = "func observe(value: i32): void {}\nfunc apply(action: func(edit i32): void): void {}\nfunc work(): void { apply(observe) }";
    let (sources, tree) = input(source);
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })) if diagnostic.rule() == crate::value_access::Rule::TypeMismatch)
    );
    accepted("func work(): void { let action: func(): void = () {}\naction() }");
}
