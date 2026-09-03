use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_source_index::{SemanticEntity, SourceRole};

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{CheckedOperation, PlaceProjection, prepare_program_checking};

#[test]
fn explicit_move_transfers_a_move_only_parameter_into_the_body_result() {
    let fixture = Fixture::new(
        "struct Owned {\n    value: i32\n}\n\
         func consume(value: Owned): Owned {\n    move value\n}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
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
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();

    check_prepared_program(&input, prepared).unwrap();
}

#[test]
fn second_use_after_move_reports_uninitialized_place() {
    let fixture = Fixture::new(
        "struct Owned {\n    value: i32\n}\n\
         func invalid(value: Owned): Owned {\n    let first = move value\n    move value\n}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn builtin_error_is_move_only() {
    let fixture = Fixture::new(
        "func invalid(value: error): void {\n    let first = move value\n    let _ = move value\n    drop first\n    return\n}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn fallible_value_is_move_only_even_when_its_success_payload_is_copyable() {
    let fixture = Fixture::new(
        "func invalid(value: i32!): void {\n    let first = move value\n    let _ = move value\n    drop first\n    return\n}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn explicit_move_rejects_copy_and_borrow_values_separately() {
    let copy = Fixture::new("func invalid(value: i32): i32 {\n    move value\n}\n");
    let input = copy.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0376");

    let borrow =
        Fixture::new("func invalid(value: &+i32): void {\n    let _ = move value\n    return\n}\n");
    let input = borrow.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0377");
}

#[test]
fn generic_named_field_move_uses_the_substituted_field_type() {
    let fixture = Fixture::new(
        "struct Owned {\n    value: i32\n}\n\
         struct Box<T> {\n    item: T\n}\n\
         func take(box: Box<Owned>): Owned {\n    move box.item\n}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();

    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| !body.places().is_empty())
        .unwrap();
    let (_, place) = body.places().iter().next().unwrap();
    let PlaceProjection::Field { field, .. } = place.projections()[0] else {
        panic!("named field move must retain its semantic field projection");
    };

    assert!(place.is_move_source());
    assert!(
        output
            .source_index()
            .bindings_for(SemanticEntity::Field(field))
            .iter()
            .any(|binding| binding.role() == SourceRole::Reference)
    );
}

#[test]
fn disjoint_named_fields_move_independently() {
    let fixture = Fixture::new(
        "struct Owned {\n    value: i32\n}\n\
         struct Pair {\n    first: Owned\n    second: Owned\n}\n\
         func split(pair: Pair): Owned {\n    let _ = move pair.first\n    move pair.second\n}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();

    check_prepared_program(&input, prepared).unwrap();
}

#[test]
fn disjoint_tuple_positions_move_independently() {
    let fixture = Fixture::new(
        "struct Owned { value: i32 }\n\
         func split(pair: (Owned, Owned)): Owned {\n\
             let _ = move pair.0\n\
             move pair.1\n\
         }\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();

    check_prepared_program(&input, prepared).unwrap();
}

#[test]
fn named_field_move_invalidates_that_field_and_its_parent() {
    for result in ["move pair.first", "move pair"] {
        let fixture = Fixture::new(&format!(
            "struct Owned {{\n    value: i32\n}}\n\
             struct Pair {{\n    first: Owned\n    second: Owned\n}}\n\
             func invalid(pair: Pair): void {{\n    let _ = move pair.first\n    let _ = {result}\n    return\n}}\n"
        ));
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
        let error = check_prepared_program(&input, prepared).unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
    }
}

#[test]
fn named_field_move_rejects_borrowed_and_unknown_fields() {
    let borrowed = Fixture::new(
        "struct Owned {\n    value: i32\n}\n\
         struct Box {\n    item: Owned\n}\n\
         func invalid(box: &+Box): Owned {\n    move box.item\n}\n",
    );
    let input = borrowed.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0377");

    let unknown = Fixture::new(
        "struct Owned {\n    value: i32\n}\n\
         struct Box {\n    item: Owned\n}\n\
         func invalid(box: Box): Owned {\n    move box.missing\n}\n",
    );
    let input = unknown.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0379");
}

#[test]
fn named_field_move_obeys_the_declaring_module_visibility_boundary() {
    let fixture = Fixture::with_child(
        "use ./child.{Box, Owned}\n\
         func invalid(box: Box): Owned {\n    move box.item\n}\n",
        "pub struct Owned {\n    value: i32\n}\n\
         pub struct Box {\n    item: Owned\n}\n",
    );
    let mut expected = None;
    for reverse in [false, true] {
        let input = fixture.input(reverse);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
        let error = check_prepared_program(&input, prepared).unwrap_err();
        let diagnostic = error.source_diagnostic().unwrap().clone();

        assert_eq!(diagnostic.code(), "E0380");
        if let Some(expected) = &expected {
            assert_eq!(&diagnostic, expected);
        } else {
            expected = Some(diagnostic);
        }
    }
}

#[test]
fn a_type_owned_drop_forbids_partial_move_with_a_related_location() {
    let fixture = Fixture::new(
        "struct Owned {\n    value: i32\n}\n\
         struct Box {\n    item: Owned\n}\n\
         drop Box(&+self) {\n    return\n}\n\
         func invalid(box: Box): Owned {\n    move box.item\n}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();
    let diagnostic = error.source_diagnostic().unwrap();

    assert_eq!(diagnostic.code(), "E0381");
    assert_eq!(diagnostic.notes().len(), 1);
}
