//! Concrete callable contexts through source, common native storage and actual cleanup.
use super::*;

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
