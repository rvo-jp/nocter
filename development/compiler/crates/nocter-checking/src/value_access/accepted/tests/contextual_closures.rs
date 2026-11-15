//! Only an already selected contract can complete an anonymous declaration's operations/data.
use super::*;
use nocter_model::ValueInputOperation;

#[test]
fn empty_compound_constraints_do_not_require_unstored_nominal_headers() {
    for element in ["(func(): i32, Node)", "func(Node): i32"] {
        for result in ["0", "apply([])"] {
            let source = format!(
                "struct Node {{ payload: [i32; COUNT] }}\nconst COUNT: usize = compute()\nconst func compute(): usize {{ let apply: func(take [{element}; 0]): usize = (items) {{ drop items\n0 }}\n{result} }}"
            );
            accepted(&source);
            let nonempty = source.replace("; 0]", "; 1]");
            let fixture = nocter_test_support::ValueSourceFixture::new(&nonempty);
            let Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) =
                SourceUnit::prepare(fixture.syntax())
            else {
                panic!("a stored nominal input still requires its header")
            };
            assert_eq!(diagnostic.source().code(), "E0324");
        }
        let element = element.replace("Node", "Missing");
        let source = format!(
            "func run(): void {{ let apply: func(take [{element}; 0]): usize = (items) {{ drop items\n0 }} }}"
        );
        let fixture = nocter_test_support::ValueSourceFixture::new(&source);
        let Err(SourceFailure::Checking(Failure::Body {
            failure: BodyFailure::Rejected(diagnostic),
            ..
        })) = SourceUnit::prepare(fixture.syntax())
        else {
            panic!("metadata still needs a known name")
        };
        assert_eq!(diagnostic.rule(), crate::value_access::Rule::UnknownType);
    }
}

#[test]
fn compound_contracts_infer_lexical_inputs_without_foreign_formal_identities() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-constraints.nct"
    ));
    assert_eq!(
        super::initializers::answer(source, "ANSWER"),
        nocter_constant_evaluation::state::Value::integer(42)
    );
    let unit = accepted(source);
    for &owner in unit.closures().keys() {
        for input in &unit.program().closure(owner).unwrap().contract().inputs {
            for witness in unit
                .program()
                .payload_dependencies(input.data)
                .unwrap()
                .parameters()
            {
                let nocter_model::ValueFunctionOrigin::Parameter(parameter) = witness.origin else {
                    panic!("formal dependency")
                };
                assert_eq!(
                    unit.program().parameter_owner(parameter),
                    Some(nocter_model::ValueCode::Closure(owner))
                );
            }
        }
    }
}

#[test]
fn compound_constraint_mismatches_keep_nested_authority_and_local_component_causes() {
    for (annotation, reason) in [
        (
            "func(take [(func(bool): i32, i32); 2]): i32",
            "array element -> tuple field 1 -> callable input 1",
        ),
        (
            "func(take [(func(i32): i32, bool); 2]): i32",
            "tuple field 2 -> component requires bool",
        ),
        (
            "func(take [(func(i32): i32, i32); 1]): i32",
            "an array with 1 elements",
        ),
        (
            "func(take [(mut(i32): i32, i32); 2]): i32",
            "nested callable contract requires mut",
        ),
        (
            "func(take [(once(i32): i32, i32); 2]): i32",
            "nested callable contract requires once",
        ),
        (
            "func([(func(i32): i32, i32); 2]): i32",
            "callable input 1 requires Read",
        ),
        (
            "func(take [(func(i32): i32, i32); 2]?): i32",
            "an optional value",
        ),
    ] {
        let source = format!(
            "func receive(take value: [(func(i32): i32, i32); 2]): i32 {{ 42 }}\nfunc run(): void {{ let selected: {annotation} = receive }}"
        );
        let fixture = nocter_test_support::ValueSourceFixture::new(&source);
        let Err(SourceFailure::Checking(Failure::Body {
            failure: BodyFailure::Rejected(diagnostic),
            ..
        })) = SourceUnit::prepare(fixture.syntax())
        else {
            panic!("must reject {source}")
        };
        assert_eq!(diagnostic.rule(), crate::value_access::Rule::TypeMismatch);
        assert!(
            diagnostic.source().message().contains(reason),
            "{}",
            diagnostic.source().message()
        );
        let range = diagnostic.source().primary().span().range();
        assert_eq!(
            &source[range.start().get() as usize..range.end().get() as usize],
            "receive"
        );
    }
}

#[test]
fn compound_pattern_annotations_reject_repeated_failure_at_the_original_marker() {
    for ty in ["((func(i32): i32)!)!", "(((func(i32): i32)!)?)!"] {
        let source =
            format!("func run(): void {{ let selected: func(take {ty}): i32 = (value) {{ 42 }} }}");
        let fixture = nocter_test_support::ValueSourceFixture::new(&source);
        let Err(SourceFailure::Checking(Failure::Body {
            failure: BodyFailure::Rejected(diagnostic),
            ..
        })) = SourceUnit::prepare(fixture.syntax())
        else {
            panic!("must reject {source}")
        };
        assert_eq!(diagnostic.rule(), crate::value_access::Rule::TypeMismatch);
        assert!(
            diagnostic
                .source()
                .message()
                .contains("uninterrupted outcome chain")
        );
        let range = diagnostic.source().primary().span().range();
        assert_eq!(
            &source[range.start().get() as usize..range.end().get() as usize],
            "!"
        );
    }
}

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
