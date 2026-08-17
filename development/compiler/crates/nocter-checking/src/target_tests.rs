use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::CompilationTarget;

use crate::test_support::Fixture;
use crate::{PreparationError, check_prepared_program, prepare_program_checking};

#[test]
fn inactive_target_bodies_never_enter_checked_program_construction() {
    let fixture = Fixture::new(
        "#target: \"arm64-darwin\"\n\
         func platform(): i32 { 1 }\n\
         #target: \"x64-linux\"\n\
         func platform(): MissingType { unresolved_name }\n",
    );
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    assert_eq!(program.target(), CompilationTarget::Arm64Darwin);

    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let checked = check_prepared_program(&input, prepared).unwrap();

    assert_eq!(
        checked.program().graph().target(),
        CompilationTarget::Arm64Darwin
    );
    assert_eq!(checked.program().bodies().len(), 1);
}

#[test]
fn checked_program_preparation_rejects_a_different_target_snapshot() {
    let fixture = Fixture::new("func platform(): i32 { 1 }\n");
    let (declaration_input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&declaration_input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let (checking_input, _) = fixture.input(false);
    let checking_input = checking_input.with_target(CompilationTarget::X64Linux);

    assert!(matches!(
        prepare_program_checking(&checking_input, program, source_index),
        Err(PreparationError::TargetMismatch {
            input: CompilationTarget::X64Linux,
            program: CompilationTarget::Arm64Darwin,
        })
    ));
}
