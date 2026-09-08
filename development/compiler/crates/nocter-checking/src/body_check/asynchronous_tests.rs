use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::{CallableExecution, ProvenanceOrigin};

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{CheckedOperation, prepare_program_checking};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

#[test]
fn await_is_a_checked_consuming_operation_with_the_inner_type() {
    let output = check(
        "func produce(): async i32 { 1 }\n\
         func consume(): async i32 {\n\
             let pending = produce()\n\
             await pending\n\
         }\n",
    )
    .unwrap();
    let program = output.program();
    let await_node = program
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .find_map(|(_, node)| match node.operation() {
            CheckedOperation::Await(await_) => Some((node, *await_)),
            _ => None,
        })
        .expect("checked await");

    assert_eq!(
        program.types().get(await_node.0.ty()),
        Some(&nocter_model::TypeKind::Builtin(
            nocter_model::BuiltinType::I32
        ))
    );
    assert!(program.bodies().iter().any(|(_, body)| {
        body.nodes()
            .get(await_node.1.computation())
            .is_some_and(|node| matches!(node.operation(), CheckedOperation::Move(_)))
    }));
}

#[test]
fn await_is_confined_to_deferred_bodies_and_requires_an_async_operand() {
    for (source, code) in [
        ("func bad(value: async i32): i32 { await value }\n", "E0415"),
        ("func bad(): async i32 { await 1 }\n", "E0416"),
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), code);
    }
}

#[test]
fn awaiting_discards_pending_captures_but_keeps_result_provenance() {
    let output = check(
        "func choose(kept: &i32, pending_only: &i32): async &i32 from kept { kept }\n\
         func relay(kept: &i32, pending_only: &i32): async &i32 from kept {\n\
             let pending = choose(kept, pending_only)\n\
             await pending\n\
         }\n",
    )
    .unwrap();
    let graph = output.program().graph();
    let (relay, declaration) = graph
        .declarations()
        .callables()
        .iter()
        .find(|(_, declaration)| {
            matches!(declaration.execution(), CallableExecution::Deferred { .. })
                && declaration.parameters().len() == 2
                && declaration.name() == graph.symbols().get("relay")
        })
        .expect("relay declaration");
    let provenance = output
        .program()
        .provenance()
        .callables()
        .get(relay)
        .unwrap();

    assert_eq!(
        provenance.origins(),
        &[ProvenanceOrigin::Parameter(declaration.parameters()[0])]
    );
}

#[test]
fn a_pending_computation_cannot_escape_borrowed_local_storage() {
    let error = check(
        "func produce(value: &i32): async i32 { 1 }\n\
         func bad(): (async i32)? {\n\
             let local = 1\n\
             produce(&local)\n\
         }\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0395");
}

#[test]
fn suspension_rejects_frame_internal_loans_but_preserves_external_input_loans() {
    let error = check(
        "func consume(value: &i32): async void { return }\n\
         func bad(): async void {\n\
             let local = 1\n\
             let pending = consume(&local)\n\
             await pending\n\
             return\n\
         }\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0417");

    check(
        "func ready(): async void { return }\n\
         func valid(input: &i32): async i32 {\n\
             let pending = ready()\n\
             await pending\n\
             let _ = input\n\
             1\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn nested_await_and_outer_failure_propagation_keep_their_type_layers() {
    check(
        "func value(): async i32! { 1 }\n\
         func nested(): async async i32! { value() }\n\
         func flatten(): async i32! { await await nested()? }\n",
    )
    .unwrap();
}

#[test]
fn an_immediate_nested_closure_cannot_inherit_its_owners_suspension_authority() {
    let error = check(
        "func outer(): async i32 {\n\
             let callback = (pending: async i32): i32 { await pending }\n\
             1\n\
         }\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0415");
}

#[test]
fn awaited_results_provide_context_for_generic_call_inference() {
    check(
        "func produce<T>(): async T { loop {} }\n\
         func value(): async i32 { await produce() }\n\
         func nested<T>(): async async T { produce() }\n\
         func flattened(): async i32 { await await nested() }\n",
    )
    .unwrap();
}

#[test]
fn awaited_outcomes_provide_payload_context_for_generic_call_inference() {
    check(
        "func produce<T>(): async T! { loop {} }\n\
         func value(): async i32! { await produce()? }\n",
    )
    .unwrap();
}

#[test]
fn awaiting_an_immediate_generic_transfer_does_not_reclassify_the_callable() {
    check(
        "func ready(): async i32 { 1 }\n\
         func transfer<T>(item: T): T { move item }\n\
         func consume_ready(): async i32 {\n\
             let pending = ready()\n\
             await transfer(move pending)\n\
         }\n",
    )
    .unwrap();

    check(
        "func ready(): async i32! { 1 }\n\
         func transfer<T>(item: T): T { move item }\n\
         func consume_ready(): async i32! {\n\
             let pending = ready()\n\
             await transfer(move pending)?\n\
         }\n",
    )
    .unwrap();

    check(
        "func unreachable_value<T>(): T { loop {} }\n\
         func inferred_computation(): async i32 { await unreachable_value() }\n",
    )
    .unwrap();
}
