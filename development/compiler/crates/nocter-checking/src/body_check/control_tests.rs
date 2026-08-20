use nocter_declaration_lowering::lower_compile_unit_declarations;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{CheckedControl, CheckedOperation, prepare_program_checking};

fn check(source: &str) -> Result<(), crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    check_prepared_program(&input, prepared).map(|_| ())
}

#[test]
fn if_expression_constructs_one_typed_control_node() {
    let fixture = Fixture::new(
        "func choose(condition: bool): i32 {\n    if condition {\n        1\n    } else {\n        2\n    }\n}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();

    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.nodes().iter().any(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Control(CheckedControl::If { .. })
            )
        })
    }));
}

#[test]
fn inferred_if_result_and_else_if_share_the_same_branch_rule() {
    check(
        "func choose(first: bool, second: bool): i32 {\n    let value = if first {\n        1\n    } else if second {\n        2\n    } else {\n        3\n    }\n    value\n}\n",
    )
    .unwrap();
}

#[test]
fn if_checks_condition_and_branch_result_types() {
    for source in [
        "func invalid(): void {\n    if 1 {}\n    return\n}\n",
        "func invalid(condition: bool): i32 {\n    if condition {\n        1\n    } else {\n        true\n    }\n}\n",
        "func invalid(condition: bool): void {\n    if condition {\n        1\n    }\n    return\n}\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0370");
    }
}

#[test]
fn if_join_marks_a_value_moved_on_only_one_path_as_maybe_initialized() {
    let error = check(
        "struct Owned {\n    value: i32\n}\n\
         func invalid(condition: bool, value: Owned): Owned {\n    if condition {\n        let _ = move value\n    }\n    move value\n}\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn if_join_retains_a_move_shared_by_every_reachable_branch() {
    let error = check(
        "struct Owned {\n    value: i32\n}\n\
         func invalid(condition: bool, value: Owned): Owned {\n    let _ = if condition {\n        move value\n    } else {\n        move value\n    }\n    move value\n}\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn terminal_if_branches_do_not_enter_the_continuation_join() {
    check(
        "struct Owned {\n    value: i32\n}\n\
         func select(condition: bool, value: Owned): Owned {\n    if condition {\n        return move value\n    }\n    move value\n}\n",
    )
    .unwrap();

    check(
        "struct Owned {\n    value: i32\n}\n\
         func finish(condition: bool, value: Owned): Owned {\n    if condition {\n        return move value\n    } else {\n        return move value\n    }\n    move value\n}\n",
    )
    .unwrap();
}

#[test]
fn a_maybe_moved_field_does_not_invalidate_its_disjoint_sibling() {
    check(
        "struct Owned {\n    value: i32\n}\n\
         struct Pair {\n    first: Owned\n    second: Owned\n}\n\
         func take_second(condition: bool, pair: Pair): Owned {\n    if condition {\n        let _ = move pair.first\n    }\n    move pair.second\n}\n",
    )
    .unwrap();
}
