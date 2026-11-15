//! Concrete callable contexts through source, common native storage and actual cleanup.
use super::*;

#[test]
fn private_contextual_outcome_widening_selects_actual_receiving_layouts() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-widened-outcome-results.nct"
    ));
    let machine = machine(source);
    assert_differently_sized_transport_helper(&machine);
    let package = TempPackage::new();
    execute_native_status(&image(&machine), &package.0, "contextual-widening", 42);
}

#[test]
fn private_contextual_outcome_widening_retires_actual_received_error_graphs() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-widened-outcome-errors.nct"
    )));
    assert_differently_sized_transport_helper(&machine);
    let package = TempPackage::new();
    execute_native_status(
        &image(&machine),
        &package.0,
        "contextual-widening-errors",
        42,
    );
}

#[test]
fn private_closed_outcome_widening_transports_original_tags_and_payloads() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/widened-outcome-results.nct"
    ));
    let package = TempPackage::new();
    let machine = machine(source);
    assert_differently_sized_transport_helper(&machine);
    execute_native_status(&image(&machine), &package.0, "widened-outcome-results", 42);
    let assertion = source.replace(
        "func run(): i32 { calculate() }",
        "func run(): i32 { let value = choose(false)\nlet action = value^!\naction(2) }",
    );
    assert_ne!(assertion, source);
    execute_native_signal(
        &compile(&assertion),
        &package.0,
        "widened-absence-assertion",
        5,
    );
}

#[test]
fn private_closed_outcome_widening_preserves_carrier_fields_and_array_strides() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/widened-outcome-carriers.nct"
    ));
    let machine = machine(source);
    assert_differently_sized_transport_helper(&machine);
    let package = TempPackage::new();
    execute_native_status(&image(&machine), &package.0, "widened-outcome-carriers", 42);
}

#[test]
fn private_closed_outcome_widening_moves_actual_error_graphs_once() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/widened-outcome-results-errors.nct"
    ));
    let machine = trusted_error_machine(source);
    assert_differently_sized_transport_helper(&machine);
    let package = TempPackage::new();
    execute_native_status(&image(&machine), &package.0, "widened-outcome-errors", 42);
}

fn assert_differently_sized_transport_helper(machine: &nocter_machine::MachineProgram) {
    assert!(
        machine.functions().any(|(_, function)| {
            let parameters = function.body().parameters();
            if parameters.len() != 2 {
                return false;
            }
            let first = function.body().stack(parameters[0]).unwrap();
            let second = function.body().stack(parameters[1]).unwrap();
            first.carrier() == nocter_machine::MachineValueCarrier::Address
                && second.carrier() == nocter_machine::MachineValueCarrier::Address
                && machine.layouts().get(first.ty()).unwrap().size()
                    != machine.layouts().get(second.ty()).unwrap().size()
        }),
        "the real executable must retain a helper for distinct source and destination layouts"
    );
}

#[test]
fn private_producerless_outcomes_preserve_absence_without_callable_payloads() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/producerless-outcome-results.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "producerless-outcome-results",
        42,
    );
}

#[test]
fn private_producerless_outcomes_dispose_actual_failure_graphs() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/producerless-outcome-results-errors.nct"
    )));
    let package = TempPackage::new();
    execute_native_status(
        &image(&machine),
        &package.0,
        "producerless-outcome-errors",
        42,
    );
}

#[test]
fn private_producerless_outcome_guards_preserve_recovery_and_propagation() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/producerless-outcome-guards.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "producerless-outcome-guards",
        42,
    );
    let present = source.replace(
        "const func absent(): (func(): i32)? { none }",
        "const func live(): i32 { 7 }\nconst func absent(): (func(): i32)? { live }",
    );
    assert_ne!(source, present);
    execute_native_status(
        &compile(&present),
        &package.0,
        "inhabited-outcome-guards",
        0,
    );
}

#[test]
fn private_producerless_outcome_guards_preserve_actual_error_cleanup() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/producerless-outcome-guards-errors.nct"
    )));
    let package = TempPackage::new();
    execute_native_status(
        &image(&machine),
        &package.0,
        "producerless-outcome-guards-errors",
        42,
    );
}

#[test]
fn private_excluded_callable_assertions_keep_the_required_traps() {
    let package = TempPackage::new();
    for (index, binding) in [
        "let action = value^!",
        "read action = value!",
        "edit action = value!",
    ]
    .into_iter()
    .enumerate()
    {
        let source = format!(
            "func absent(): (func(): i32)? {{ none }}\nfunc run(): i32 {{ var value = absent()\n{binding}\naction() }}"
        );
        execute_native_signal(
            &compile(&source),
            &package.0,
            &format!("excluded-optional-assertion-{index}"),
            5,
        );
        let source = format!(
            "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\nfunc failed(): (func(): i32)! {{ throw context_error(create_failure(\"failed\", \"leaf\"), \"outer\") }}\nfunc run(): i32 {{ var value = failed()\n{binding}\naction() }}"
        );
        let machine = trusted_error_machine(&source);
        execute_native_signal(
            &image(&machine),
            &package.0,
            &format!("excluded-failure-assertion-{index}"),
            5,
        );
    }
    let source = "func absent(): (func(): i32)? { none }\nfunc run(): i32 { let value = absent()\nlet action = (if true { return 42 } else { value^ })!\naction() }";
    execute_native_status(
        &compile(source),
        &package.0,
        "exit-before-excluded-assertion",
        42,
    );
}

#[test]
fn private_excluded_result_components_preserve_adjacent_callable_contexts() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-excluded-result-components.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "contextual-excluded-components",
        42,
    );
}

#[test]
fn private_outcome_results_preserve_callable_values_and_absence() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-outcome-results.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "outcome-results", 42);
}

#[test]
fn private_outcome_results_dispose_actual_captures_and_propagated_errors() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-outcome-results-errors.nct"
    )));
    let package = TempPackage::new();
    execute_native_status(&image(&machine), &package.0, "outcome-result-errors", 42);
}

#[test]
fn private_empty_compound_results_preserve_descriptor_gaps_without_element_values() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/empty-compound-results.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "empty-compound-results", 42);
}

#[test]
fn private_empty_compound_results_dispose_the_seed_and_the_returned_error_graphs() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/empty-compound-results-errors.nct"
    )));
    let package = TempPackage::new();
    execute_native_status(&image(&machine), &package.0, "empty-result-errors", 42);
}

#[test]
fn private_compound_results_preserve_actual_tuple_and_array_environments() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-results.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "compound-results", 42);
}

#[test]
fn private_compound_results_dispose_actual_returned_and_remaining_error_graphs() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-results-errors.nct"
    )));
    let package = TempPackage::new();
    execute_native_status(&image(&machine), &package.0, "compound-results-errors", 42);
}

#[test]
fn private_compound_input_results_keep_selected_callbacks_after_container_cleanup() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-input-results.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "compound-input-results", 42);
}

#[test]
fn private_compound_input_results_dispose_remaining_and_returned_error_graphs() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-input-results-errors.nct"
    )));
    let package = TempPackage::new();
    execute_native_status(
        &image(&machine),
        &package.0,
        "compound-input-results-errors",
        42,
    );
}

#[test]
fn private_mutable_array_inputs_keep_independent_selected_state() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-mutable-array-inputs.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "mutable-array-inputs", 42);
}

#[test]
fn private_mutable_array_inputs_dispose_actual_remaining_error_graphs() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-mutable-array-input-errors.nct"
    )));
    let package = TempPackage::new();
    execute_native_status(
        &image(&machine),
        &package.0,
        "mutable-array-input-errors",
        42,
    );
}

#[test]
fn private_mutable_array_inputs_keep_original_bounds_effects() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-mutable-array-inputs.nct"
    ));
    let runtime = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("if ANSWER == 42 { calculate() } else { 0 }", "calculate()")
        .replace("const func", "func");
    let changed = runtime.replace(
        "let first = step(actions, 0, 1)",
        "let first = step(actions, 2, 1)",
    );
    assert_ne!(changed, runtime);
    let package = TempPackage::new();
    execute_native_signal(
        &compile(&changed),
        &package.0,
        "mutable-array-input-bounds",
        5,
    );
}

#[test]
fn private_compound_edits_preserve_caller_values_after_completion() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-edits.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "compound-edits", 42);
}

#[test]
fn private_compound_edits_keep_actual_error_graph_cleanup() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-edits-errors.nct"
    )));
    let package = TempPackage::new();
    execute_native_status(&image(&machine), &package.0, "compound-edit-errors", 42);
}

#[test]
fn private_compound_inputs_keep_distinct_received_callable_contexts() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-inputs.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "compound-input-contexts", 42);
}

#[test]
fn private_compound_inputs_dispose_actual_received_error_graphs() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-input-errors.nct"
    )));
    let package = TempPackage::new();
    execute_native_status(&image(&machine), &package.0, "compound-input-errors", 42);
}

#[test]
fn private_compound_inputs_keep_empty_array_bounds_failures() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-inputs.nct"
    ));
    let runtime = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("if ANSWER == 42 { calculate() } else { 0 }", "calculate()")
        .replace("const func", "func");
    let changed = runtime.replace(
        "func empty(take actions: [func(i32): i32; 0]): i32 { 42 }",
        "func empty(take actions: [func(i32): i32; 0]): i32 { actions[0](22) }",
    );
    assert_ne!(changed, runtime);
    let package = TempPackage::new();
    execute_native_signal(&compile(&changed), &package.0, "compound-empty-bounds", 5);
}

#[test]
fn private_contextual_indexed_replacement_uses_the_original_numeric_projection() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-same-context.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "contextual-indexed-numeric",
        42,
    );
}

#[test]
fn private_contextual_indexed_replacement_rejects_shifted_sibling_geometry() {
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-replacement.nct"
    ));
    let initial = original.replace("[wrap(addition(20))]", "[wrap(identity)]");
    assert_ne!(initial, original);
    let source = initial.replace(
        "if ANSWER == 42 { calculate(0) } else { 0 }",
        "calculate(0)",
    );
    assert_ne!(source, initial);
    let fixture = ValueSourceFixture::new(&source);
    let unit = Arc::new(SourceUnit::prepare(fixture.syntax()).unwrap().into_unit());
    assert!(matches!(
        machine_unit(unit),
        Err(nocter_machine::value_access::ExecutableFailure::InvalidBody(_))
    ));
}

#[test]
fn private_contextual_indexed_replacement_keeps_actual_bounds_failures() {
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-replacement.nct"
    ));
    let source = original.replace(
        "func run(): i32 { if ANSWER == 42 { calculate(0) } else { 0 } }",
        "func run(): i32 { calculate(1) }",
    );
    assert_ne!(source, original);
    let package = TempPackage::new();
    execute_native_signal(
        &compile(&source),
        &package.0,
        "contextual-indexed-bounds",
        5,
    );
}

#[test]
fn private_contextual_indexed_replacement_disposes_actual_retired_saved_and_received_errors() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-replacement-errors.nct"
    )));
    let package = TempPackage::new();
    execute_native_status(
        &image(&machine),
        &package.0,
        "contextual-indexed-errors",
        42,
    );
}

#[test]
fn private_contextual_indexed_replacement_preserves_containing_roots_and_sibling_views() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-replacement.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "contextual-indexed-replacement",
        42,
    );
}

#[test]
fn private_formal_capture_keeps_distinct_received_callback_environments() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/formal-captured-callbacks.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "formal-captured-callbacks",
        42,
    );
}

#[test]
fn private_formal_capture_disposes_actual_owned_error_environments() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/formal-captured-errors.nct"
    )));
    assert!(
        machine
            .functions()
            .any(
                |(_, function)| function.body().operations().any(|(_, operation)| matches!(
                    operation.kind(),
                    nocter_machine::MachineOperationKind::ReleaseError { .. }
                ))
            )
    );
    let package = TempPackage::new();
    execute_native_status(&image(&machine), &package.0, "formal-captured-errors", 42);
}

#[test]
fn private_contextual_tuple_results_keep_the_actual_anonymous_input_environments() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-tuple-results.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "contextual-tuple-results", 42);
}

#[test]
fn private_context_bound_callable_arrays_preserve_elements_and_zero_count_seed_cleanup() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-callable-arrays.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "contextual-callable-arrays",
        42,
    );
}

#[test]
fn private_contextual_callable_arrays_dispose_owned_error_elements_and_zero_count_seeds() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-callable-array-errors.nct"
    ));
    // Qualify each cleanup independently: the other consumer cannot supply its release proof.
    for (name, source) in [
        (
            "element",
            source.replace("let unused = owning()\n    discard(unused^)", ""),
        ),
        (
            "zero-seed",
            source.replace(
                "let answer = apply(active^, 40)",
                "discard(active^)\n    let answer = 42",
            ),
        ),
    ] {
        let machine = trusted_error_machine(&source);
        assert!(
            machine
                .functions()
                .any(
                    |(_, function)| function.body().operations().any(|(_, operation)| matches!(
                        operation.kind(),
                        nocter_machine::MachineOperationKind::ReleaseError { .. }
                    ))
                ),
            "{name}"
        );
        let package = TempPackage::new();
        execute_native_status(
            &image(&machine),
            &package.0,
            &format!("contextual-array-errors-{name}"),
            42,
        );
    }
}

#[test]
fn private_contextual_component_transfers_keep_container_contexts_and_disjoint_sibling_views() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-transfers.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "contextual-component-transfers",
        42,
    );
}

#[test]
fn private_contextual_component_transfers_destroy_actual_selected_and_remaining_error_graphs() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-errors.nct"
    )));
    let package = TempPackage::new();
    execute_native_status(
        &image(&machine),
        &package.0,
        "contextual-component-errors",
        42,
    );
}

#[test]
fn private_contextual_callable_replacement_preserves_retired_received_and_transferred_instances() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-callable-replacement.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "contextual-callable-replacement",
        42,
    );
}

#[test]
fn private_contextual_callable_replacement_disposes_retired_received_and_saved_error_graphs() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-callable-replacement-errors.nct"
    )));
    let package = TempPackage::new();
    execute_native_status(
        &image(&machine),
        &package.0,
        "contextual-callable-replacement-errors",
        42,
    );
}

#[test]
fn private_contextual_component_replacement_preserves_sibling_views_and_saved_instances() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-replacement.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "contextual-component-replacement",
        42,
    );
}

#[test]
fn private_contextual_component_replacement_disposes_actual_retired_saved_and_received_errors() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-replacement-errors.nct"
    )));
    let package = TempPackage::new();
    execute_native_status(
        &image(&machine),
        &package.0,
        "contextual-component-replacement-errors",
        42,
    );
}

#[test]
fn private_contextual_component_replacement_rejects_shifted_sibling_geometry() {
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-replacement.nct"
    ));
    let source = original
        .replace("(wrap(addition(20)), 2)", "(wrap(identity), 2)")
        .replace(
            "func run(): i32 { if ANSWER == 42 { calculate() } else { 0 } }",
            "func run(): i32 { calculate() }",
        );
    assert_ne!(source, original);
    let fixture = ValueSourceFixture::new(&source);
    let unit = Arc::new(SourceUnit::prepare(fixture.syntax()).unwrap().into_unit());
    assert!(matches!(
        machine_unit(unit),
        Err(nocter_machine::value_access::ExecutableFailure::InvalidBody(_))
    ));
}

#[test]
fn private_contextual_closures_keep_selected_contracts_in_runtime_instances() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-closures.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "contextual-closures", 42);
}
