use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::ProvenanceOrigin;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{AmbientStorageDependence, prepare_program_checking};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

#[test]
fn inferred_callable_contract_tracks_only_the_returned_aggregate_field() {
    let output = check(
        "struct Pair {\n    left: &i32\n    right: &i32\n}\n\
         func select(left: &i32, right: &i32): &i32 {\n\
             let pair = Pair { left: left, right: right }\n\
             pair.left\n\
         }\n",
    )
    .unwrap();
    let (callable, declaration) = output
        .program()
        .graph()
        .declarations()
        .callables()
        .iter()
        .next()
        .unwrap();
    let provenance = output
        .program()
        .provenance()
        .callables()
        .get(callable)
        .unwrap();

    assert_eq!(
        provenance.origins(),
        &[ProvenanceOrigin::Parameter(declaration.parameters()[0])]
    );
    assert_eq!(provenance.ambient(), AmbientStorageDependence::Independent);
}

#[test]
fn inferred_provenance_flows_through_static_calls() {
    let output = check(
        "func identity(value: &i32): &i32 { value }\n\
         func relay(value: &i32): &i32 { identity(value) }\n",
    )
    .unwrap();
    for (callable, declaration) in output.program().graph().declarations().callables().iter() {
        let provenance = output
            .program()
            .provenance()
            .callables()
            .get(callable)
            .unwrap();
        assert_eq!(
            provenance.origins(),
            &[ProvenanceOrigin::Parameter(declaration.parameters()[0])]
        );
    }
}

#[test]
fn local_and_owned_parameter_borrows_cannot_escape() {
    for source in [
        "func bad(value: i32): &i32 { &value }\n",
        "func bad(value: i32): &i32 { let local = value\n&local }\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0395");
    }
}

#[test]
fn explicit_result_contract_is_an_upper_bound_checked_against_the_body() {
    let error = check(
        "func choose(left: &i32, right: &i32): &i32 from left {\n\
             right\n\
         }\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0395");
}

#[test]
fn a_receiver_derived_result_cannot_escape_a_temporary_receiver() {
    let error = check(
        "struct Box { value: i32 }\n\
         instance Box {\n\
             pub method &self.view(): &i32 { &self.value }\n\
         }\n\
         func bad(): &i32 { Box { value: 1 }.view() }\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0395");
}

#[test]
fn temporary_receiver_storage_cannot_enter_a_named_binding() {
    let error = check(
        "struct Box { value: i32 }\n\
         instance Box {\n\
             pub method &self.view(): &i32 { &self.value }\n\
         }\n\
         func bad(): void {\n\
             let view = Box { value: 1 }.view()\n\
             let _ = view\n\
             return\n\
         }\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0398");
}

#[test]
fn local_storage_cannot_leave_a_nested_block() {
    for source in [
        "func bad(seed: i32): void {\n\
             var view = &seed\n\
             if true {\n\
                 let local = 1\n\
                 view = &local\n\
             }\n\
             let _ = view\n\
             return\n\
         }\n",
        "func bad(): void {\n\
             let view = if true {\n\
                 let local = 1\n\
                 &local\n\
             } else {\n\
                 let other = 2\n\
                 &other\n\
             }\n\
             let _ = view\n\
             return\n\
         }\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0398");
    }
}

#[test]
fn outer_storage_can_be_borrowed_by_an_inner_binding() {
    check(
        "func valid(): void {\n\
             let outer = 1\n\
             if true {\n\
                 let view = &outer\n\
                 let _ = view\n\
             }\n\
             return\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn borrowed_pattern_payload_preserves_the_subject_origin() {
    let output = check(
        "enum Choice {\n    one(item: i32)\n}\n\
         func value(choice: &Choice): &i32 {\n\
             match choice {\n\
                 Choice.one(item) { item }\n\
             }\n\
         }\n",
    )
    .unwrap();
    let (callable, declaration) = output
        .program()
        .graph()
        .declarations()
        .callables()
        .iter()
        .next()
        .unwrap();

    assert_eq!(
        output
            .program()
            .provenance()
            .callables()
            .get(callable)
            .unwrap()
            .origins(),
        &[ProvenanceOrigin::Parameter(declaration.parameters()[0])]
    );
}

#[test]
fn loop_break_and_backedge_states_participate_in_inference() {
    let output = check(
        "func select(left: &i32, right: &i32, repeat: bool): &i32 {\n\
             var selected = left\n\
             while repeat {\n\
                 selected = right\n\
                 break\n\
             }\n\
             selected\n\
         }\n",
    )
    .unwrap();
    let (callable, declaration) = output
        .program()
        .graph()
        .declarations()
        .callables()
        .iter()
        .next()
        .unwrap();

    assert_eq!(
        output
            .program()
            .provenance()
            .callables()
            .get(callable)
            .unwrap()
            .origins(),
        &[
            ProvenanceOrigin::Parameter(declaration.parameters()[0]),
            ProvenanceOrigin::Parameter(declaration.parameters()[1]),
        ]
    );
}

#[test]
fn inferred_conformance_result_cannot_exceed_the_interface_contract() {
    let error = check(
        "pub interface Select {\n\
             pub method &self.pick(other: &i32): &i32 from self\n\
         }\n\
         struct Holder { value: i32 }\n\
         conform Select for Holder {\n\
             method &self.pick(other: &i32): &i32 { other }\n\
         }\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0395");
}
