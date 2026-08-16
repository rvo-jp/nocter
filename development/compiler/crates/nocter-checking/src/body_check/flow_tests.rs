use nocter_declaration_lowering::lower_compile_unit_declarations;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{CheckedControl, CheckedOperation, prepare_program_checking};

#[test]
fn unreachable_source_is_checked_but_has_no_ownership_continuation() {
    let fixture = Fixture::new(
        "struct Owned {\n    value: i32\n}\n\
         func consume(value: Owned): Owned {\n    return move value\n    move value\n}\n",
    );
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();

    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.nodes().iter().any(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Control(CheckedControl::Unreachable(_))
            )
        })
    }));
}

#[test]
fn unreachable_locals_remain_available_to_later_unreachable_source() {
    let fixture = Fixture::new(
        "func finish(): void {\n    return\n    let local = 1\n    let _ = local\n}\n",
    );
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();

    check_prepared_program(&input, prepared).unwrap();
}

#[test]
fn unreachable_source_still_enforces_structural_move_and_value_rules() {
    let invalid_move = Fixture::new(
        "struct Owned {\n    value: i32\n}\n\
         func invalid(value: &+Owned): void {\n    return\n    let _ = move value\n}\n",
    );
    let (input, prelude) = invalid_move.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0377");

    let implicit_move = Fixture::new(
        "struct Owned {\n    value: i32\n}\n\
         func invalid(value: Owned): void {\n    return\n    let _ = value\n}\n",
    );
    let (input, prelude) = implicit_move.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0371");

    let invalid_value = Fixture::new("func invalid(): void {\n    return\n    42\n    return\n}\n");
    let (input, prelude) = invalid_value.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0372");
}
