use nocter_declaration_lowering::lower_compile_unit_declarations;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    CheckedControl, CheckedOperation, CleanupCondition, CleanupTarget, CleanupTiming, PlaceAccess,
    prepare_program_checking,
};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

fn assignment_node(body: &crate::CheckedBody) -> nocter_model::BodyNodeId {
    body.nodes()
        .iter()
        .find_map(|(node, checked)| {
            matches!(
                checked.operation(),
                CheckedOperation::Control(CheckedControl::Assign { .. })
            )
            .then_some(node)
        })
        .unwrap()
}

const OWNED: &str = "struct Owned {\n    value: i32\n}\n";

#[test]
fn initialized_var_assignment_drops_the_old_value_before_replacement() {
    let output = check(&format!(
        "{OWNED}func replace(first: Owned, second: Owned): void {{\n    var value = move first\n    value = move second\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let assign = assignment_node(body);
    let [action] = body.cleanups().actions(assign).unwrap() else {
        panic!("replacement must clean exactly one initialized value");
    };

    assert_eq!(
        body.cleanups().schedule(assign).unwrap().timing(),
        CleanupTiming::BeforeStore
    );
    assert_eq!(action.condition(), CleanupCondition::Always);
    assert!(matches!(action.target(), CleanupTarget::Path(path) if path.fields().is_empty()));
}

#[test]
fn moved_or_dropped_var_can_be_reinitialized_without_old_value_cleanup() {
    for consume in ["let _ = move value", "drop value"] {
        let output = check(&format!(
            "{OWNED}func restore(first: Owned, second: Owned): void {{\n    var value = move first\n    {consume}\n    value = move second\n    let _ = move value\n    return\n}}\n"
        ))
        .unwrap();
        let (_, body) = output.program().bodies().iter().next().unwrap();

        assert!(
            body.cleanups()
                .actions(assignment_node(body))
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn assignment_restores_a_moved_named_field_and_complete_parent() {
    let output = check(&format!(
        "{OWNED}struct Pair {{\n    first: Owned\n    second: Owned\n}}\nfunc restore(pair_input: Pair, replacement: Owned): void {{\n    var pair = move pair_input\n    let _ = move pair.first\n    pair.first = move replacement\n    let _ = move pair\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();

    assert!(
        body.cleanups()
            .actions(assignment_node(body))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn maybe_initialized_var_uses_conditional_replacement_cleanup() {
    let output = check(&format!(
        "{OWNED}func restore(condition: bool, first: Owned, second: Owned): void {{\n    var value = move first\n    if condition {{\n        let _ = move value\n    }}\n    value = move second\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let [action] = body.cleanups().actions(assignment_node(body)).unwrap() else {
        panic!("maybe initialized replacement needs one conditional cleanup");
    };

    assert_eq!(action.condition(), CleanupCondition::IfInitialized);
}

#[test]
fn rhs_move_is_observed_before_replacement_cleanup_is_planned() {
    let output = check(&format!(
        "{OWNED}func preserve(input: Owned): void {{\n    var value = move input\n    value = move value\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();

    assert!(
        body.cleanups()
            .actions(assignment_node(body))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn whole_assignment_over_a_partial_parent_cleans_only_remaining_fields() {
    let output = check(&format!(
        "{OWNED}struct Pair {{\n    first: Owned\n    second: Owned\n}}\nfunc replace(input: Pair, replacement: Pair): void {{\n    var pair = move input\n    let _ = move pair.first\n    pair = move replacement\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let [action] = body.cleanups().actions(assignment_node(body)).unwrap() else {
        panic!("only the remaining initialized field should be cleaned");
    };
    let CleanupTarget::Path(path) = action.target() else {
        panic!("partial owned replacement must clean an owned field path");
    };

    assert_eq!(path.fields().len(), 1);
    assert_eq!(action.condition(), CleanupCondition::Always);
}

#[test]
fn maybe_initialized_field_uses_conditional_replacement_cleanup() {
    let output = check(&format!(
        "{OWNED}struct Pair {{\n    first: Owned\n}}\nfunc restore(condition: bool, input: Pair, replacement: Owned): void {{\n    var pair = move input\n    if condition {{\n        let _ = move pair.first\n    }}\n    pair.first = move replacement\n    let _ = move pair\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let [action] = body.cleanups().actions(assignment_node(body)).unwrap() else {
        panic!("maybe initialized field needs one conditional cleanup");
    };

    assert_eq!(action.condition(), CleanupCondition::IfInitialized);
}

#[test]
fn readwrite_borrowed_field_uses_an_exact_place_cleanup() {
    let output = check(&format!(
        "{OWNED}struct Pair {{\n    field: Owned\n}}\nfunc replace(pair: &+Pair, replacement: Owned): void {{\n    pair.field = move replacement\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let assign = assignment_node(body);
    let CheckedOperation::Control(CheckedControl::Assign { target, .. }) =
        body.nodes().get(assign).unwrap().operation()
    else {
        unreachable!();
    };
    let [action] = body.cleanups().actions(assign).unwrap() else {
        panic!("borrowed replacement needs one place cleanup");
    };

    assert_eq!(
        body.places().get(*target).unwrap().access(),
        PlaceAccess::Borrowed(nocter_model::BorrowCapability::ReadWrite)
    );
    assert!(matches!(action.target(), CleanupTarget::Place { place, .. } if place == target));
}

#[test]
fn immutable_and_non_place_targets_are_rejected_structurally() {
    for source in [
        "func invalid(): void {\n    let value = 1\n    value = 2\n    return\n}\n",
        "func invalid(value: i32): void {\n    value = 2\n    return\n}\n",
        "struct Pair { field: i32 }\nfunc invalid(value: &Pair): void {\n    value.field = 2\n    return\n}\n",
        "func make(): i32 { 1 }\nfunc invalid(): void {\n    make() = 2\n    return\n}\n",
        "func invalid(): void {\n    return\n    let value = 1\n    value = 2\n}\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0384");
    }
}

#[test]
fn field_assignment_cannot_recreate_a_whole_moved_parent() {
    let error = check(&format!(
        "{OWNED}struct Pair {{\n    first: Owned\n}}\nfunc invalid(input: Pair, replacement: Owned): void {{\n    var pair = move input\n    let _ = move pair\n    pair.first = move replacement\n    return\n}}\n"
    ))
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0385");
}

#[test]
fn assignment_provides_its_target_type_as_the_rhs_expectation() {
    let error =
        check("func invalid(): void {\n    var value = 1\n    value = true\n    return\n}\n")
            .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0370");
}

#[test]
fn continuation_newline_does_not_obscure_the_assignment_operator() {
    let output =
        check("func update(): i32 {\n    var value = 1\n    value\n        = 2\n    value\n}\n")
            .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();

    assert!(matches!(
        body.nodes().get(assignment_node(body)).unwrap().operation(),
        CheckedOperation::Control(CheckedControl::Assign { .. })
    ));
}
