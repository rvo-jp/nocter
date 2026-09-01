use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_syntax::NodeKind;
use nocter_toolchain_contract::StandardDeclarationRole;

use super::check_prepared_program;
use crate::test_support::{Fixture, StandardRoleInput, with_standard_roles};
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

#[test]
fn concrete_generic_aggregate_uses_its_substituted_destruction_effect() {
    check(
        "struct Owned {}\n\
         noalloc drop Owned(&+self) { return }\n\
         struct Wrapper<T> { value: T }\n\
         noalloc func consume(value: Wrapper<Owned>): void { return }\n",
    )
    .unwrap();
}

#[test]
fn enum_residual_effect_excludes_the_transferred_payload() {
    check(&format!(
        "{TEXT_DECLARATIONS}\n\
         struct Transferred {{}}\n\
         drop Transferred(&+self) {{ let _ = Text \"drop\"\n return }}\n\
         struct Retained {{}}\n\
         noalloc drop Retained(&+self) {{ return }}\n\
         enum Pair {{ values(first: Transferred, second: Retained) }}\n\
         noalloc drop Pair(&+self) {{ return }}\n\
         noalloc func take(first: Transferred, second: Retained): Transferred {{\n\
             return match Pair.values(move first, move second) {{\n\
                 Pair.values(item, _) {{ move item }}\n\
             }}\n\
         }}\n"
    ))
    .unwrap();
}

#[test]
fn compiler_selected_allocation_request_is_a_positive_effect_seed() {
    let fixture = Fixture::with_standard(
        "",
        "pub func request(size: usize): usize { return size }\nnoalloc func invalid(): usize { return request(1) }\n",
    );
    let input = with_standard_roles(
        fixture.input(false),
        vec![StandardRoleInput::new(
            StandardDeclarationRole::AllocationRequest,
            fixture.standard_declaration_token(NodeKind::FunctionDeclaration, "request"),
        )],
    );
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));
}
