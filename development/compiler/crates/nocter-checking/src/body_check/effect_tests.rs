use nocter_declaration_lowering::lower_compile_unit_declarations;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{BodyRule, prepare_program_checking};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

const TEXT_DECLARATIONS: &str = r#"
struct Text {}
construct Text {
    pub literal ""(text: &str): Self { return Self {} }
}
"#;

#[test]
fn direct_allocation_violates_noalloc() {
    let error = check(&format!(
        "{TEXT_DECLARATIONS}\nnoalloc func invalid(): Text {{ return Text \"value\" }}\n"
    ))
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0411");
}

#[test]
fn allocation_effects_propagate_through_source_backed_calls() {
    let error = check(&format!(
        "{TEXT_DECLARATIONS}\nfunc allocate(): Text {{ return Text \"value\" }}\nnoalloc func invalid(): Text {{ return allocate() }}\n"
    ))
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));
}

#[test]
fn source_backed_unmarked_helpers_can_be_proven_allocation_free() {
    check(
        "func helper(value: i32): i32 { return value + 1 }\nnoalloc func valid(value: i32): i32 { return helper(value) }\n",
    )
    .unwrap();
}

#[test]
fn allocation_free_mutual_recursion_reaches_the_least_fixed_point() {
    check(
        "noalloc func even(value: i32): bool {\n    if value == 0 { return true }\n    return odd(value - 1)\n}\nnoalloc func odd(value: i32): bool {\n    if value == 0 { return false }\n    return even(value - 1)\n}\n",
    )
    .unwrap();
}

#[test]
fn invoked_closure_effects_are_distinct_from_closure_creation() {
    check(
        "noalloc func valid(value: i32): i32 {\n    let callback: noalloc func(i32): i32 = (item) { item + 1 }\n    return callback(value)\n}\n",
    )
    .unwrap();

    let error = check(&format!(
        "{TEXT_DECLARATIONS}\nnoalloc func invalid(): Text {{\n    let callback: noalloc func(): Text = () {{ Text \"value\" }}\n    return callback()\n}}\n"
    ))
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));
}

#[test]
fn implicit_destruction_participates_in_the_same_effect_graph() {
    let error = check(&format!(
        "{TEXT_DECLARATIONS}\nstruct Owned {{}}\ndrop Owned(&+self) {{ let _ = Text \"drop\"\n return }}\nnoalloc func invalid(): void {{\n    let value = Owned {{}}\n    return\n}}\n"
    ))
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));

    let error = check(&format!(
        "{TEXT_DECLARATIONS}\nstruct Owned {{}}\nnoalloc drop Owned(&+self) {{ let _ = Text \"drop\"\n return }}\n"
    ))
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));
}
