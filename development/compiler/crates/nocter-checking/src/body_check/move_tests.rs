use nocter_declaration_lowering::lower_compile_unit_declarations;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{CheckedOperation, prepare_program_checking};

#[test]
fn explicit_move_transfers_a_move_only_parameter_into_the_body_result() {
    let fixture = Fixture::new(
        "struct Owned {\n    value: i32\n}\n\
         func consume(value: Owned): Owned {\n    move value\n}\n",
    );
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();

    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.nodes()
            .iter()
            .any(|(_, node)| matches!(node.operation(), CheckedOperation::Move(_)))
    }));
}

#[test]
fn moved_local_is_initialized_before_its_own_transfer() {
    let fixture = Fixture::new(
        "struct Owned {\n    value: i32\n}\n\
         func consume(value: Owned): Owned {\n    let local = move value\n    move local\n}\n",
    );
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();

    check_prepared_program(&input, prepared).unwrap();
}

#[test]
fn second_use_after_move_reports_uninitialized_place() {
    let fixture = Fixture::new(
        "struct Owned {\n    value: i32\n}\n\
         func invalid(value: Owned): Owned {\n    let first = move value\n    move value\n}\n",
    );
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn explicit_move_rejects_copy_and_borrow_values_separately() {
    let copy = Fixture::new("func invalid(value: i32): i32 {\n    move value\n}\n");
    let (input, prelude) = copy.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0376");

    let borrow =
        Fixture::new("func invalid(value: &+i32): void {\n    let _ = move value\n    return\n}\n");
    let (input, prelude) = borrow.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0377");
}
