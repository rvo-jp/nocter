use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::{CallableExecution, ProvenanceOrigin};

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{CheckedOperation, CleanupTarget, prepare_program_checking};

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
        "async func produce(): i32 { 1 }\n\
         async func consume(): i32 {\n\
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
        (
            "func bad(value: future i32): i32 { await value }\n",
            "E0415",
        ),
        ("async func bad(): i32 { await 1 }\n", "E0416"),
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), code);
    }
}

#[test]
fn awaiting_discards_pending_captures_but_keeps_result_provenance() {
    let output = check(
        "async func choose(kept: &i32, pending_only: &i32): &i32 from kept { kept }\n\
         async func relay(kept: &i32, pending_only: &i32): &i32 from kept {\n\
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
        "async func produce(value: &i32): i32 { 1 }\n\
         func bad(): (future i32)? {\n\
             let local = 1\n\
             produce(&local)\n\
         }\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0395");
}

#[test]
fn suspension_retains_frame_internal_loans_and_preserves_external_input_loans() {
    let output = check(
        "async func consume(value: &i32): void { return }\n\
         async func valid(): void {\n\
             let local = 1\n\
             let pending = consume(&local)\n\
             await pending\n\
             return\n\
         }\n",
    )
    .unwrap();
    let program = output.program();
    let (body, await_node) = program
        .bodies()
        .iter()
        .find_map(|(body, checked)| {
            checked.nodes().iter().find_map(|(node, checked)| {
                matches!(checked.operation(), CheckedOperation::Await(_)).then_some((body, node))
            })
        })
        .expect("awaiting body");
    assert!(matches!(
        program
            .loans()
            .body(body)
            .unwrap()
            .suspension_storage(await_node),
        Some([crate::SuspensionStorage::Local(_)])
    ));

    check(
        "async func ready(): void { return }\n\
         async func valid(input: &i32): i32 {\n\
             let pending = ready()\n\
             await pending\n\
             let _ = input\n\
             1\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn direct_await_retains_loans_created_while_evaluating_its_computation() {
    let output = check(
        "async func consume(value: &i32): void { return }\n\
         async func valid(): void {\n\
             let local = 1\n\
             await consume(&local)\n\
             return\n\
         }\n",
    )
    .unwrap();
    let program = output.program();
    let (body, await_node) = program
        .bodies()
        .iter()
        .find_map(|(body, checked)| {
            checked.nodes().iter().find_map(|(node, checked)| {
                matches!(checked.operation(), CheckedOperation::Await(_)).then_some((body, node))
            })
        })
        .expect("direct await");

    assert!(matches!(
        program
            .loans()
            .body(body)
            .unwrap()
            .suspension_storage(await_node),
        Some([crate::SuspensionStorage::Local(_)])
    ));
}

#[test]
fn nested_await_and_outer_failure_propagation_keep_their_type_layers() {
    check(
        "async func value(): i32! { 1 }\n\
         async func nested(): future i32! { value() }\n\
         async func flatten(): i32! { await await nested()? }\n",
    )
    .unwrap();
}

#[test]
fn an_immediate_nested_closure_cannot_inherit_its_owners_suspension_authority() {
    let error = check(
        "async func outer(): i32 {\n\
             let callback = (pending: future i32): i32 { await pending }\n\
             1\n\
         }\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0415");
}

#[test]
fn awaited_results_provide_context_for_generic_call_inference() {
    check(
        "async func produce<T>(): T { loop {} }\n\
         async func value(): i32 { await produce() }\n\
         async func nested<T>(): future T { produce() }\n\
         async func flattened(): i32 { await await nested() }\n",
    )
    .unwrap();
}

#[test]
fn awaited_outcomes_provide_payload_context_for_generic_call_inference() {
    check(
        "async func produce<T>(): T! { loop {} }\n\
         async func value(): i32! { await produce()? }\n",
    )
    .unwrap();
}

#[test]
fn awaiting_an_immediate_generic_transfer_does_not_reclassify_the_callable() {
    check(
        "async func ready(): i32 { 1 }\n\
         func transfer<T>(item: T): T { move item }\n\
         async func consume_ready(): i32 {\n\
             let pending = ready()\n\
             await transfer(move pending)\n\
         }\n",
    )
    .unwrap();

    check(
        "async func ready(): i32! { 1 }\n\
         func transfer<T>(item: T): T { move item }\n\
         async func consume_ready(): i32! {\n\
             let pending = ready()\n\
             await transfer(move pending)?\n\
         }\n",
    )
    .unwrap();

    check(
        "func unreachable_value<T>(): T { loop {} }\n\
         async func inferred_computation(): i32 { await unreachable_value() }\n",
    )
    .unwrap();
}

#[test]
fn await_records_the_active_ownership_cleanup_for_cancellation() {
    let output = check(
        "struct Resource {}\n\
         drop Resource(&+self) { return }\n\
         async func ready(): void { return }\n\
         async func keep(value: Resource): void {\n\
             await ready()\n\
             let _ = &value\n\
             return\n\
         }\n",
    )
    .unwrap();
    let (body, await_node) = output
        .program()
        .bodies()
        .iter()
        .find_map(|(_, body)| {
            body.nodes()
                .iter()
                .find_map(|(node, value)| {
                    matches!(value.operation(), CheckedOperation::Await(_)).then_some(node)
                })
                .map(|node| (body, node))
        })
        .unwrap();
    let actions = body.cleanups().cancellation_actions(await_node).unwrap();

    assert!(actions.iter().any(|action| {
        matches!(
            action.target(),
            CleanupTarget::Path(path)
                if matches!(path.root(), crate::PlaceRoot::Parameter(_))
        )
    }));
}
