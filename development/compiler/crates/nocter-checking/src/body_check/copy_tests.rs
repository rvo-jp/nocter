use nocter_declaration_lowering::lower_compile_unit_declarations;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{CheckedOperation, prepare_program_checking};

#[test]
fn copy_struct_specialization_uses_substituted_field_copyability() {
    let fixture = Fixture::new(
        "copy struct Box<T> {\n    value: T\n}\n\
         func duplicate(value: Box<i32>): Box<i32> {\n    value\n}\n",
    );
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();

    assert!(
        output.program().types().iter().all(|(ty, _)| output
            .program()
            .copyabilities()
            .get(ty)
            .is_some())
    );
    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.nodes()
            .iter()
            .any(|(_, node)| matches!(node.operation(), CheckedOperation::Copy(_)))
    }));
}

#[test]
fn copy_struct_specialization_remains_move_only_for_move_only_argument() {
    let fixture = Fixture::new(
        "struct Owned {\n    value: i32\n}\n\
         copy struct Box<T> {\n    value: T\n}\n\
         func duplicate(value: Box<Owned>): Box<Owned> {\n    value\n}\n",
    );
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0371");
}

#[test]
fn callable_copy_requirement_supplies_the_generic_body_proof() {
    let fixture = Fixture::new("func duplicate<T>(value: T): T where copy T {\n    value\n}\n");
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();

    check_prepared_program(&input, prepared).unwrap();
}

#[test]
fn unconstrained_generic_parameter_is_not_implicitly_copied() {
    let fixture = Fixture::new("func duplicate<T>(value: T): T {\n    value\n}\n");
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0371");
}

#[test]
fn payloadless_enum_is_copyable_without_a_marker() {
    let fixture = Fixture::new(
        "enum Choice {\n    yes\n    no\n}\n\
         func duplicate(value: Choice): Choice {\n    value\n}\n",
    );
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();

    check_prepared_program(&input, prepared).unwrap();
}

#[test]
fn readonly_and_readwrite_borrows_have_distinct_copyability() {
    let readonly = Fixture::new("func duplicate(value: &i32): &i32 from value {\n    value\n}\n");
    let (input, prelude) = readonly.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    check_prepared_program(&input, prepared).unwrap();

    let readwrite =
        Fixture::new("func duplicate(value: &+i32): &+i32 from value {\n    value\n}\n");
    let (input, prelude) = readwrite.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0371");
}
