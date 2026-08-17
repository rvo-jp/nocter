use nocter_declaration_lowering::lower_compile_unit_declarations;

use super::check_prepared_program;
use crate::prepare_program_checking;
use crate::test_support::Fixture;

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

#[test]
fn readonly_loan_ends_after_its_last_source_use() {
    check(
        "func inspect(value: &i32): void { return }\n\
         func valid(): void {\n\
             var value = 1\n\
             let read = &value\n\
             inspect(read)\n\
             let write = &+value\n\
             return\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn readwrite_borrow_rejects_a_readonly_loan_used_later() {
    let error = check(
        "func inspect(value: &i32): void { return }\n\
         func invalid(): void {\n\
             var value = 1\n\
             let read = &value\n\
             let write = &+value\n\
             inspect(read)\n\
             return\n\
         }\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0396");
    assert_eq!(error.source_diagnostic().unwrap().notes().len(), 1);
}

#[test]
fn multiple_readonly_loans_can_overlap() {
    check(
        "func inspect(value: &i32): void { return }\n\
         func valid(): void {\n\
             var value = 1\n\
             let first = &value\n\
             let second = &value\n\
             inspect(first)\n\
             inspect(second)\n\
             return\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn move_and_assignment_conflict_with_a_live_loan() {
    for source in [
        "struct Box { value: i32 }\n\
         func inspect(value: &Box): void { return }\n\
         func consume(value: Box): void { return }\n\
         func invalid(value: Box): void {\n\
             let read = &value\n\
             consume(move value)\n\
             inspect(read)\n\
             return\n\
         }\n",
        "func inspect(value: &i32): void { return }\n\
         func invalid(): void {\n\
             var value = 1\n\
             let read = &value\n\
             value = 2\n\
             inspect(read)\n\
             return\n\
         }\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0397");
    }
}

#[test]
fn named_fields_are_disjoint_but_the_same_field_conflicts() {
    check(
        "copy struct Pair { left: i32\n    right: i32\n}\n\
         func inspect(value: &i32): void { return }\n\
         func valid(): void {\n\
             var pair = Pair { left: 1, right: 2 }\n\
             let read = &pair.left\n\
             pair.right = 3\n\
             inspect(read)\n\
             return\n\
         }\n",
    )
    .unwrap();

    let error = check(
        "copy struct Pair { left: i32\n    right: i32\n}\n\
         func inspect(value: &i32): void { return }\n\
         func invalid(): void {\n\
             var pair = Pair { left: 1, right: 2 }\n\
             let read = &pair.left\n\
             pair.left = 3\n\
             inspect(read)\n\
             return\n\
         }\n",
    )
    .unwrap_err();
    assert_eq!(
        error.rule(),
        Some(crate::BodyRule::BorrowedPlaceMutation),
        "{error:?}"
    );
}

#[test]
fn receiver_derived_method_result_keeps_the_receiver_loan_live() {
    let error = check(
        "copy struct Box { value: i32 }\n\
         instance Box {\n\
             pub method &self.view(): &i32 { &self.value }\n\
         }\n\
         func inspect(value: &i32): void { return }\n\
         func invalid(): void {\n\
             var value = Box { value: 1 }\n\
             let view = value.view()\n\
             value.value = 2\n\
             inspect(view)\n\
             return\n\
         }\n",
    )
    .unwrap_err();

    assert_eq!(
        error.rule(),
        Some(crate::BodyRule::BorrowedPlaceMutation),
        "{error:?}"
    );
}

#[test]
fn condition_only_loan_ends_before_the_selected_body() {
    check(
        "func positive(value: &i32): bool { true }\n\
         func valid(): void {\n\
             var value = 1\n\
             if positive(&value) {\n\
                 value = 2\n\
             }\n\
             return\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn reborrow_through_readwrite_input_uses_the_same_external_loan() {
    check(
        "copy struct Box { value: i32 }\n\
         func inspect(value: &i32): void { return }\n\
         func valid(box: &+Box): void {\n\
             let read = &box.value\n\
             inspect(read)\n\
             box.value = 2\n\
             return\n\
         }\n",
    )
    .unwrap();

    let error = check(
        "copy struct Box { value: i32 }\n\
         func inspect(value: &i32): void { return }\n\
         func invalid(box: &+Box): void {\n\
             let read = &box.value\n\
             box.value = 2\n\
             inspect(read)\n\
             return\n\
         }\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0397");
}

#[test]
fn dynamic_indices_are_conservatively_overlapping() {
    let error = check(
        "func inspect(value: &i32): void { return }\n\
         func invalid(): void {\n\
             var values = [1, 2]\n\
             let read = &values[0]\n\
             values[1] = 3\n\
             inspect(read)\n\
             return\n\
         }\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0397");
}

#[test]
fn branch_last_use_ends_before_the_join() {
    check(
        "func inspect(value: &i32): void { return }\n\
         func valid(condition: bool): void {\n\
             var value = 1\n\
             let read = &value\n\
             if condition { inspect(read) }\n\
             value = 2\n\
             return\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn branch_mutation_conflicts_when_the_loan_is_used_after_the_join() {
    let error = check(
        "func inspect(value: &i32): void { return }\n\
         func invalid(condition: bool): void {\n\
             var value = 1\n\
             let read = &value\n\
             if condition { value = 2 }\n\
             inspect(read)\n\
             return\n\
         }\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0397");
}

#[test]
fn loop_iteration_loan_ends_at_its_last_use() {
    check(
        "func inspect(value: &i32): void { return }\n\
         func valid(condition: bool): void {\n\
             var value = 1\n\
             while condition {\n\
                 let read = &value\n\
                 inspect(read)\n\
                 value = 2\n\
             }\n\
             return\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn type_owned_drop_observes_borrow_fields_in_destruction_order() {
    let declarations = "struct Guard { value: &i32 }\n\
                        drop Guard(&+self) { let _ = self.value\n    return }\n";
    check(&format!(
        "{declarations}func valid(): void {{\n\
             let source = 1\n\
             let guard = Guard {{ value: &source }}\n\
             let _ = &guard\n\
             return\n\
         }}\n"
    ))
    .unwrap();

    let error = check(&format!(
        "{declarations}func invalid(seed: &i32): void {{\n\
             var guard = Guard {{ value: seed }}\n\
             let source = 1\n\
             guard.value = &source\n\
             return\n\
         }}\n"
    ))
    .unwrap_err();

    assert_eq!(
        error.rule(),
        Some(crate::BodyRule::BorrowedPlaceMutation),
        "{error:?}"
    );

    let error = check(&format!(
        "{declarations}func invalid_break(seed: &i32, condition: bool): void {{\n\
             while condition {{\n\
                 var guard = Guard {{ value: seed }}\n\
                 let source = 1\n\
                 guard.value = &source\n\
                 break\n\
             }}\n\
             return\n\
         }}\n"
    ))
    .unwrap_err();
    assert_eq!(
        error.rule(),
        Some(crate::BodyRule::BorrowedPlaceMutation),
        "{error:?}"
    );
}
