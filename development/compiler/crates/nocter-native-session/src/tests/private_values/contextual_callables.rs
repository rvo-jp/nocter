//! Concrete callable contexts through source, common native storage and actual cleanup.
use super::*;

#[test]
fn private_guarded_nested_optionals_preserve_tags_and_owned_callback_cleanup() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/value-guarded-nested-outcomes.nct"
    ));
    let runtime = source.replace("const func", "func");
    let package = TempPackage::new();
    for tuple in [false, true] {
        let (source, action) = if tuple {
            (
                runtime
                    .replace("((func(i32): i32)?)?", "(i32, (func(i32): i32)?)?")
                    .replace(" { value^ }", " { (35, value^) }")
                    .replace("let action = inner^", "let action = inner.1^")
                    .replace("action(35)", "action(inner.0)"),
                "inner.1",
            )
        } else {
            (runtime.clone(), "inner")
        };
        for (parameter, inner, selected) in [
            (
                "take value",
                "let inner = value^",
                format!("let action = {action}^"),
            ),
            (
                "value",
                "read inner = value",
                format!("read action = {action}"),
            ),
            (
                "edit value",
                "edit inner = value",
                format!("edit action = {action}"),
            ),
        ] {
            let source = source
                .replace("inspect(take value", &format!("inspect({parameter}"))
                .replace("let inner = value^", inner)
                .replace(&format!("let action = {action}^"), &selected);
            execute_native_status(&compile(&source), &package.0, "guarded-nested-inputs", 42);
            let owned = source.replace("if present { runtime }", "if present { make() }");
            assert_ne!(owned, source);
            let owned = format!(
                "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\nfunc make(): func(i32): i32 {{ let reason = context_error(create_failure(\"nested\", \"leaf\"), \"outer\")\n(reason^; value: i32) {{ runtime(value) }} }}\n{owned}"
            );
            execute_native_status(
                &image(&trusted_error_machine(&owned)),
                &package.0,
                "guarded-nested-errors",
                42,
            );
        }
    }
}

#[test]
fn private_guarded_tuple_components_preserve_sibling_independence_and_actual_captures() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/value-guarded-compound-outcomes.nct"
    ));
    let runtime = source.replace("const func", "func");
    let package = TempPackage::new();
    for nested in [false, true] {
        let (source, value) = if nested {
            (
                runtime
                    .replace(
                        "take values: ((func(i32): i32)?, (func(i32): i32)?, i32)",
                        "take values: (i32, ((func(i32): i32)?, (func(i32): i32)?, i32))",
                    )
                    .replace("values.1^", "values.1.1^")
                    .replace("values.2", "values.1.2")
                    .replace(
                        "inspect((produce(true), produce(false), 42))",
                        "inspect((9, (produce(true), produce(false), 42)))",
                    )
                    .replace(
                        "inspect((produce(false), produce(true), 0))",
                        "inspect((9, (produce(false), produce(true), 0)))",
                    ),
                "values.1.1",
            )
        } else {
            (runtime.clone(), "values.1")
        };
        for (parameter, extraction) in [
            ("take values", format!("let action = {value}^")),
            ("values", format!("read action = {value}")),
            ("edit values", format!("edit action = {value}")),
        ] {
            let source = source
                .replace("inspect(take values", &format!("inspect({parameter}"))
                .replace(&format!("let action = {value}^"), &extraction);
            execute_native_status(&compile(&source), &package.0, "guarded-tuple-inputs", 42);
            let owned = source.replace("if present { runtime }", "if present { make() }");
            assert_ne!(owned, source);
            let owned = format!(
                "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\nfunc make(): func(i32): i32 {{ let reason = context_error(create_failure(\"tuple\", \"leaf\"), \"outer\")\n(reason^; value: i32) {{ runtime(value) }} }}\n{owned}"
            );
            execute_native_status(
                &image(&trusted_error_machine(&owned)),
                &package.0,
                "guarded-tuple-errors",
                42,
            );
        }
    }
}

#[test]
fn private_guarded_captured_callbacks_remain_valid_for_both_tags_and_owned_environments() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/value-guarded-captured-outcomes.nct"
    ));
    let runtime = source.replace("const func", "func");
    let package = TempPackage::new();
    for (parameter, extraction) in [
        ("take value", "let action = value^"),
        ("value", "read action = value"),
        ("edit value", "edit action = value"),
    ] {
        let source = runtime
            .replace("inspect(take value", &format!("inspect({parameter}"))
            .replace("let action = value^", extraction);
        execute_native_status(
            &compile(&source),
            &package.0,
            "guarded-captured-outcomes",
            42,
        );
        let owned = source.replace("    let offset = 7", "    let reason = context_error(create_failure(\"captured\", \"leaf\"), \"outer\")\n    let offset = 7").replace("(offset; value: i32)", "(reason^, offset; value: i32)");
        assert_ne!(owned, source);
        let owned = format!(
            "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\n{owned}"
        );
        execute_native_status(
            &image(&trusted_error_machine(&owned)),
            &package.0,
            "guarded-captured-errors",
            42,
        );
    }
}

#[test]
fn private_contextual_compound_constraints_preserve_inferred_inputs_and_actual_error_cleanup() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-constraints.nct"
    ));
    let runtime = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func");
    let package = TempPackage::new();
    execute_native_status(
        &compile(&runtime),
        &package.0,
        "contextual-compound-inputs",
        42,
    );
    let owned = runtime.replace(
        "    apply(addition)",
        "    if apply(addition) == 42 && apply(failed()) == 42 { 42 } else { 0 }",
    );
    assert_ne!(owned, runtime);
    let owned = format!(
        "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\nfunc failed(): (func(i32): i32)! {{ throw context_error(create_failure(\"failed\", \"leaf\"), \"outer\") }}\n{owned}"
    );
    execute_native_status(
        &image(&trusted_error_machine(&owned)),
        &package.0,
        "contextual-compound-failure",
        42,
    );
}

#[test]
fn private_captured_nullable_inputs_preserve_input_modes_and_environment_storage() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/captured-compound-inputs.nct"
    ));
    let runtime = source
        .replace(
            "const ANSWER: i32 = calculate(false) + calculate(true) - 42",
            "",
        )
        .replace("const func", "func");
    let package = TempPackage::new();
    for (parameter, binding) in [
        ("take value", "let action = value^"),
        ("value", "read action = value"),
        ("edit value", "edit action = value"),
    ] {
        let source = runtime
            .replace("take value", parameter)
            .replace("let action = value^", binding);
        execute_native_status(&compile(&source), &package.0, "captured-nullable-input", 42);
        let owned = source.replace("if present { addition }", "if present { callback() }");
        assert_ne!(owned, source);
        let owned = format!(
            "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\nfunc callback(): func(i32): i32 {{ let reason = context_error(create_failure(\"callback\", \"leaf\"), \"outer\")\n(reason^; value: i32) {{ value + 7 }} }}\n{owned}"
        );
        execute_native_status(
            &image(&trusted_error_machine(&owned)),
            &package.0,
            "captured-nullable-owned-input",
            42,
        );
    }
}

#[test]
fn private_selected_loop_exit_results_preserve_returned_environments_and_error_owners() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-loop-exit-results.nct"
    ));
    assert_selected_return_contexts(source, "selected-loop-exit-results");
}

#[test]
fn private_selected_normal_returns_preserve_callable_contexts_and_error_owners() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-return-boundaries.nct"
    ));
    assert_selected_return_contexts(source, "selected-normal-returns");
}

fn assert_selected_return_contexts(source: &str, name: &str) {
    let runtime = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func");
    let package = TempPackage::new();
    for (parameter, binding) in [
        ("take value", "let ignored = value^"),
        ("value", "read ignored = value"),
        ("edit value", "edit ignored = value"),
    ] {
        let source = runtime
            .replace("select(take value", &format!("select({parameter}"))
            .replace("let ignored = value^", binding);
        assert!(source.contains(&format!("func select({parameter}:")));
        let source = if parameter == "take value" {
            source
        } else {
            source.replace("drop ignored", "")
        };
        execute_native_status(&compile(&source), &package.0, name, 42);
    }
    let owned = runtime.replace("wrap(identity)", "wrap(initial())")
        .replace("(number; value: i32)", "(reason^, number; value: i32)")
        .replace("func addition(number: i32): func(i32): i32 {",
            "func addition(number: i32): func(i32): i32 { let reason = context_error(create_failure(\"returned\", \"leaf\"), \"outer\")");
    let owned = format!(
        "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\nfunc initial(): func(i32): i32 {{ let reason = context_error(create_failure(\"retired\", \"leaf\"), \"outer\")\n(reason^; value: i32) {{ value }} }}\n{owned}"
    );
    execute_native_status(
        &image(&trusted_error_machine(&owned)),
        &package.0,
        &format!("{name}-owned-errors"),
        42,
    );
}

#[test]
fn private_received_loop_exits_preserve_selected_contexts_and_error_cleanup() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-loop-exit-contexts.nct"
    ));
    let runtime = source
        .replace("const ANSWER: i32 = choose(absent())", "")
        .replace("const func", "func");
    let package = TempPackage::new();
    for (parameter, binding) in [
        ("take value", "let ignored = value^"),
        ("value", "read ignored = value"),
        ("edit value", "edit ignored = value"),
    ] {
        let source = runtime
            .replace("choose(take value", &format!("choose({parameter}"))
            .replace("let ignored = value^", binding);
        let source = if parameter == "take value" {
            source
        } else {
            source.replace("drop ignored", "")
        };
        for receiver in [
            "action(2)",
            "read retained = action\n    retained(2)",
            "let retained = action^\n    retained(2)",
        ] {
            let source = source.replace("    action(2)", &format!("    {receiver}"));
            execute_native_status(
                &compile(&source),
                &package.0,
                "received-loop-exit-contexts",
                42,
            );
        }
    }
    let owned = runtime.replace("wrap(identity)", "wrap(initial())")
        .replace("(number; value: i32)", "(reason^, number; value: i32)")
        .replace("func addition(number: i32): func(i32): i32 {",
            "func addition(number: i32): func(i32): i32 { let reason = context_error(create_failure(\"received\", \"leaf\"), \"outer\")");
    let owned = format!(
        "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\nfunc initial(): func(i32): i32 {{ let reason = context_error(create_failure(\"retired\", \"leaf\"), \"outer\")\n(reason^; value: i32) {{ value }} }}\n{owned}"
    );
    for receiver in [
        "action(2)",
        "read retained = action\n    retained(2)",
        "let retained = action^\n    retained(2)",
    ] {
        let source = owned.replace("    action(2)", &format!("    {receiver}"));
        execute_native_status(
            &image(&trusted_error_machine(&source)),
            &package.0,
            "received-loop-exit-error-cleanup",
            42,
        );
    }
}

#[test]
fn private_contextual_absent_callable_arrays_preserve_supplied_environments_and_order() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-absent-callable-arrays.nct"
    ));
    let source = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func");
    let package = TempPackage::new();
    execute_native_status(
        &compile(&source),
        &package.0,
        "contextual-absent-callable-arrays",
        42,
    );
    let source = source
        .replace(
            "func addition(number: i32)",
            "func addition(take reason: error, number: i32)",
        )
        .replace("(number; value: i32)", "(reason^, number; value: i32)");
    let source = [0, 10, 40].into_iter().fold(source, |source, number| {
        source.replace(
            &format!("addition({number})"),
            &format!(
                "addition(context_error(create_failure(\"held\", \"leaf\"), \"outer\"), {number})"
            ),
        )
    });
    let source = format!(
        "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\n{source}"
    );
    execute_native_status(
        &image(&trusted_error_machine(&source)),
        &package.0,
        "contextual-absent-owned-error-arrays",
        42,
    );
}

#[test]
fn private_optional_runtime_callbacks_preserve_absent_and_present_execution() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/value-guarded-callable-outcomes.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "optional-runtime-callbacks",
        42,
    );
}

#[test]
fn private_indexed_empty_received_domains_keep_bounds_traps() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-indexed-outcome-control.nct"
    ));
    let source = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func");
    let package = TempPackage::new();
    for name in ["inspect", "inspect_read", "inspect_edit"] {
        let prefix = format!("{name}([(none, addition(100), 2), (none, addition(200), 3)], ");
        let invalid = source.replace(&format!("{prefix}0)"), &format!("{prefix}2)"));
        assert_ne!(invalid, source);
        execute_native_signal(
            &compile(&invalid),
            &package.0,
            &format!("indexed-received-{name}-bounds"),
            5,
        );
    }
}

#[test]
fn private_indexed_received_outcomes_keep_selected_and_remaining_error_owners() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-indexed-outcome-control.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "indexed-received-outcomes",
        42,
    );
    let source = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func")
        .replace(
            "func addition(number: i32)",
            "func addition(take reason: error, number: i32)",
        )
        .replace("(number; value: i32)", "(reason^, number; value: i32)");
    let source = ["number", "100", "200"].into_iter().fold(source, |source, number| source.replace(&format!("addition({number})"), &format!("addition(context_error(create_failure(\"held\", \"leaf\"), \"outer\"), {number})")));
    let source = format!(
        "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\n{source}"
    );
    execute_native_status(
        &image(&trusted_error_machine(&source)),
        &package.0,
        "indexed-received-error-environments",
        42,
    );
}

#[test]
fn private_compound_received_failures_preserve_staged_and_adjacent_error_owners() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-compound-failure-control.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &image(&trusted_error_machine(source)),
        &package.0,
        "compound-received-failure-control",
        42,
    );
}

#[test]
fn private_compound_received_assertions_keep_the_original_traps() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-compound-outcome-control.nct"
    ));
    let source = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func");
    let package = TempPackage::new();
    for name in ["asserted", "asserted_read", "asserted_edit"] {
        let absent = source.replace(
            &format!("{name}((present(29), addition(100), 7))"),
            &format!("{name}((absent(), addition(100), 7))"),
        );
        assert_ne!(absent, source);
        execute_native_signal(
            &compile(&absent),
            &package.0,
            &format!("compound-{name}-absent"),
            5,
        );
    }
}

#[test]
fn private_compound_received_outcomes_keep_adjacent_owners_and_scalar_offsets() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-compound-outcome-control.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "compound-received-outcomes",
        42,
    );
    let source = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func")
        .replace(
            "func addition(number: i32)",
            "func addition(take reason: error, number: i32)",
        )
        .replace("(number; value: i32)", "(reason^, number; value: i32)")
        .replace(
            "addition(number)",
            "addition(context_error(create_failure(\"held\", \"leaf\"), \"outer\"), number)",
        )
        .replace(
            "addition(100)",
            "addition(context_error(create_failure(\"adjacent\", \"leaf\"), \"outer\"), 100)",
        );
    let source = format!(
        "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\n{source}"
    );
    execute_native_status(
        &image(&trusted_error_machine(&source)),
        &package.0,
        "compound-received-error-environments",
        42,
    );
}

#[test]
fn private_nested_received_outcomes_preserve_layers_and_owned_environment_cleanup() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-nested-outcome-control.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "nested-received-outcomes", 42);
    let source = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func")
        .replace(
            "func addition(number: i32)",
            "func addition(take reason: error, number: i32)",
        )
        .replace("(number; value: i32)", "(reason^, number; value: i32)")
        .replace(
            "addition(number)",
            "addition(context_error(create_failure(\"held\", \"leaf\"), \"outer\"), number)",
        );
    let source = format!(
        "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\n{source}"
    );
    execute_native_status(
        &image(&trusted_error_machine(&source)),
        &package.0,
        "nested-received-error-environments",
        42,
    );
}

#[test]
fn private_received_recovery_merges_preserve_captured_payloads_and_error_owners() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-outcome-recovery-merge.nct"
    ));
    let source = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func");
    let package = TempPackage::new();
    execute_native_status(&compile(&source), &package.0, "received-recovery-merge", 42);
    let source = source
        .replace(
            "func present(number: i32)",
            "func present(take reason: error, number: i32)",
        )
        .replace("(number; value: i32)", "(reason^, number; value: i32)");
    let source = [21, 19, 39].into_iter().fold(source, |source, number| {
        source.replace(
            &format!("present({number})"),
            &format!(
                "present(context_error(create_failure(\"held\", \"leaf\"), \"outer\"), {number})"
            ),
        )
    });
    let source = format!(
        "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\n{source}"
    );
    execute_native_status(
        &image(&trusted_error_machine(&source)),
        &package.0,
        "received-recovery-error-merge",
        42,
    );
}

#[test]
fn private_received_outcome_propagation_preserves_domains_and_owned_errors() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-outcome-propagation.nct"
    ));
    let source = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func");
    let package = TempPackage::new();
    for (index, (parameter, binding)) in [
        ("take value", "let action = value^?"),
        ("value", "read action = value?"),
        ("edit value", "edit action = value?"),
    ]
    .into_iter()
    .enumerate()
    {
        let source = source
            .replace("propagate(take value", &format!("propagate({parameter}"))
            .replace("let action = value^?", binding);
        execute_native_status(
            &compile(&source),
            &package.0,
            &format!("received-optional-propagation-{index}"),
            42,
        );
    }
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-failure-propagation.nct"
    ));
    execute_native_status(
        &image(&trusted_error_machine(source)),
        &package.0,
        "received-failure-propagation",
        42,
    );
}

#[test]
fn private_received_callable_assertions_preserve_traps_and_positive_inputs() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-outcome-assertions.nct"
    ));
    let source = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func");
    let package = TempPackage::new();
    for (index, (parameter, binding)) in [
        ("take value", "let action = value^!"),
        ("value", "read action = value!"),
        ("edit value", "edit action = value!"),
    ]
    .into_iter()
    .enumerate()
    {
        let present = source
            .replace("take value", parameter)
            .replace("let action = value^!", binding);
        execute_native_status(
            &compile(&present),
            &package.0,
            &format!("received-assertion-present-{index}"),
            42,
        );
        let absent = present.replace("inspect(present())", "inspect(absent())");
        execute_native_signal(
            &compile(&absent),
            &package.0,
            &format!("received-assertion-absent-{index}"),
            5,
        );
    }
}

#[test]
fn private_received_outcome_control_keeps_each_input_domain() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-outcome-control.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "received-outcome-control", 42);
}

#[test]
fn private_constructed_compound_absence_keeps_owned_error_environments() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/constructed-outcome-environments.nct"
    ));
    let runtime = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func")
        .replace(
            "wrap(addition(20))",
            "wrap(owning(context_error(create_failure(\"failure\", \"leaf\"), \"outer\"), 20))",
        );
    let source = format!(
        "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\nfunc owning(take reason: error, number: i32): func(i32): i32 {{ (reason^, number; value: i32) {{ number + value }} }}\n{runtime}"
    );
    let machine = trusted_error_machine(&source);
    let package = TempPackage::new();
    execute_native_status(
        &image(&machine),
        &package.0,
        "constructed-compound-error-environments",
        42,
    );
}

#[test]
fn private_constructed_absence_preserves_positive_inactive_environments() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/constructed-outcome-environments.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "constructed-outcome-environments",
        42,
    );
}

#[test]
fn private_constructed_absence_survives_original_result_equations() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/constructed-outcome-results.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "constructed-outcome-results",
        42,
    );
}

#[test]
fn private_lifted_callable_inputs_preserve_actual_code_and_outcome_layers() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/lifted-callable-outcome-inputs.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "lifted-callable-inputs", 42);
}

#[test]
fn private_constructed_absence_closes_actual_receiving_domains() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/constructed-outcome-inputs.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "constructed-outcome-inputs",
        42,
    );
}

#[test]
fn private_received_outcomes_dispose_forwarded_failure_and_capture_owners() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-excluded-outcome-errors.nct"
    )));
    let package = TempPackage::new();
    execute_native_status(&image(&machine), &package.0, "received-outcome-errors", 42);
}

#[test]
fn private_received_outcomes_preserve_empty_and_positive_input_domains() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-excluded-outcome-inputs.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "received-outcome-inputs", 42);
    let conditional = source
        .replace(
            "forward(absent())",
            "forward(if flag { absent() } else { absent() })",
        )
        .replace("func calculate():", "func calculate(flag: bool):")
        .replace("= calculate()", "= calculate(true)")
        .replace("{ calculate() }", "{ calculate(false) }");
    assert_ne!(conditional, source);
    execute_native_status(
        &compile(&conditional),
        &package.0,
        "received-outcome-merge",
        42,
    );
    for condition in ["true", "false"] {
        let fixed = conditional.replace("if flag", &format!("if {condition}"));
        assert_ne!(fixed, conditional);
        execute_native_status(
            &compile(&fixed),
            &package.0,
            &format!("received-outcome-fixed-{condition}"),
            42,
        );
    }
}

#[test]
fn private_contextual_carrier_widening_preserves_positive_supplied_contexts() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-widened-outcome-carriers.nct"
    ));
    let machine = machine(source);
    assert_differently_sized_transport_helper(&machine);
    let package = TempPackage::new();
    execute_native_status(
        &image(&machine),
        &package.0,
        "contextual-carrier-widening",
        42,
    );
}

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
        let constructed = source.replace(
            "func absent(): (func(): i32)? { none }",
            "func forward(take value: (func(): i32)?): (func(): i32)? { value^ }\nfunc absent(): (func(): i32)? { forward(none) }",
        );
        execute_native_signal(
            &compile(&constructed),
            &package.0,
            &format!("constructed-optional-assertion-{index}"),
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
fn private_contextual_indexed_growth_keeps_the_original_selection_and_sibling_values() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-growth.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "indexed-growth", 42);
    let counted = source
        .replace("var pair =", "var count: usize = 0\n    var pair =")
        .replace("pair.0[index]", "pair.0[select(count, index)]")
        .replace("if small_sibling", "if count == 10 && small_sibling");
    let counted = format!(
        "const func select(edit count: usize, index: usize): usize {{ count += 1\nindex }}\n{counted}"
    );
    execute_native_status(&compile(&counted), &package.0, "indexed-growth-once", 42);
    let trapped = source
        .replace("const ANSWER: i32 = calculate(0)", "")
        .replace(
            "if ANSWER == 42 { calculate(0) } else { 0 }",
            "calculate(0)",
        )
        .replace(
            "pair.0[index] = wrap(addition(20))",
            "pair.0[shifted(index)] = wrap(addition(20))",
        );
    let trapped = format!("const func shifted(index: usize): usize {{ index + 1 }}\n{trapped}");
    execute_native_signal(&compile(&trapped), &package.0, "indexed-growth-bounds", 5);
}

#[test]
fn private_contextual_indexed_growth_preserves_actual_owned_capture_and_sibling_cleanup() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-growth-errors.nct"
    ));
    let source = source
        .replace("(1, wrap(identity),", "(1, [wrap(identity)],")
        .replace("pair.1", "pair.1[0]");
    let package = TempPackage::new();
    execute_native_status(
        &image(&trusted_error_machine(&source)),
        &package.0,
        "indexed-growth-errors",
        42,
    );
}

#[test]
fn private_contextual_prefix_holes_preserve_original_endpoint_and_remaining_storage() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-prefix-hole-growth.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "prefix-hole-growth", 42);
    let singleton = source
        .replace(
            "[wrap(addition(2)), wrap(addition(7))]",
            "[wrap(addition(2))]",
        )
        .replace(" && calculate(2) == 42", "");
    assert_ne!(singleton, source);
    execute_native_status(
        &compile(&singleton),
        &package.0,
        "prefix-hole-singleton",
        42,
    );
    let empty_storage = source.replace(
        "[wrap(addition(2)), wrap(addition(7))]",
        "[(); 18446744073709551615]",
    );
    assert_ne!(empty_storage, source);
    execute_native_status(
        &compile(&empty_storage),
        &package.0,
        "prefix-hole-zero-bytes",
        42,
    );
    let trapped = source.replace("func run(): i32 { if ANSWER == 42 && calculate(0) == 42 && calculate(1) == 42 && calculate(2) == 42 { 42 } else { 0 } }", "func run(): i32 { calculate(3) }");
    assert_ne!(trapped, source);
    execute_native_signal(&compile(&trapped), &package.0, "prefix-hole-bounds", 5);
}

#[test]
fn private_contextual_prefix_holes_preserve_actual_owned_error_cleanup() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-prefix-hole-errors.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &image(&trusted_error_machine(source)),
        &package.0,
        "prefix-hole-errors",
        42,
    );
}

#[test]
fn private_contextual_dynamic_holes_use_the_saved_original_index_during_relocation() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-dynamic-hole-growth.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "dynamic-hole-growth", 42);
    let mutated = source.replace("let saved_sibling = pair.1[index].0^", "var selected = index\n    let saved_sibling = pair.1[selected].0^\n    selected = 1 - selected");
    assert_ne!(mutated, source);
    execute_native_status(&compile(&mutated), &package.0, "dynamic-hole-snapshot", 42);
    let conditional = source
        .replace(
            "calculate(index: usize, amount: i32)",
            "calculate(index: usize, amount: i32, withdraw: bool)",
        )
        .replace(
            "let saved_sibling = pair.1[index].0^",
            "if withdraw { let withdrawn = pair.1[index].0^ }",
        )
        .replace("    pair.1[index].0 = saved_sibling^\n", "")
        .replace("    let restored = pair.1[index].0(amount)\n", "")
        .replace("pair.1[0].0(40) == 42 && ", "")
        .replace("restored == 42 && ", "")
        .replace("pair.1[1].0(35) == 42 && ", "")
        .replace("calculate(0, 40)", "calculate(0, 40, false)")
        .replace("calculate(1, 35)", "calculate(1, 35, true)");
    execute_native_status(
        &compile(&conditional),
        &package.0,
        "conditional-dynamic-hole",
        42,
    );
    let trapped = source.replace("const ANSWER: i32 = if calculate(0, 40) == 42 && calculate(1, 35) == 42 { 42 } else { 0 }", "")
        .replace("if ANSWER == 42 && calculate(0, 40) == 42 && calculate(1, 35) == 42 { 42 } else { 0 }", "calculate(2, 40)");
    assert_ne!(trapped, source);
    execute_native_signal(&compile(&trapped), &package.0, "dynamic-hole-bounds", 5);
}

#[test]
fn private_contextual_dynamic_holes_preserve_original_coordinates_and_actual_error_cleanup() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-dynamic-hole-errors.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &image(&trusted_error_machine(source)),
        &package.0,
        "dynamic-hole-errors",
        42,
    );
}

#[test]
fn private_contextual_array_holes_skip_zero_byte_storage_at_maximum_cardinality() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-growth.nct"
    ));
    let source = source
        .replace(
            "(wrap(identity), 2)",
            "(wrap(identity), [(); 18446744073709551615])",
        )
        .replace(
            "let empty =",
            "let saved_unit = pair.1[0]^\n    let empty =",
        )
        .replace("= pair.1\n", "= pair.1[1]\n")
        .replace("small_sibling == 2", "small_sibling == ()")
        .replace("large_sibling == 2", "large_sibling == ()")
        .replace("pair.1 == 2", "pair.1[18446744073709551614] == ()")
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("if ANSWER == 42 { calculate() } else { 0 }", "calculate()")
        .replace("let shrunk =", "pair.1[0] = saved_unit^\n    let shrunk =");
    let package = TempPackage::new();
    execute_native_status(
        &compile(&source),
        &package.0,
        "array-zero-storage-relocation",
        42,
    );
}

#[test]
fn private_contextual_array_holes_preserve_saved_owners_and_remaining_elements() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-array-hole-growth.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "array-hole-growth", 42);
}

#[test]
fn private_contextual_conditional_holes_select_their_original_owning_alternative() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-conditional-hole-growth.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "conditional-hole-growth", 42);
}

#[test]
fn private_contextual_conditional_holes_preserve_actual_branch_and_capture_cleanup() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-conditional-hole-errors.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &image(&trusted_error_machine(source)),
        &package.0,
        "conditional-hole-errors",
        42,
    );
}

#[test]
fn private_contextual_singleton_holes_preserve_saved_instances_and_initialized_element_fields() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-singleton-hole-growth.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "singleton-hole-growth", 42);
}

#[test]
fn private_contextual_component_growth_preserves_sparse_holes_and_restores_saved_owners() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-growth-holes.nct"
    ));
    let package = TempPackage::new();
    for scenario in 0..3 {
        let source = if scenario == 1 {
            source
                .replace(
                    "let saved_sibling = pair.1.0^",
                    "let saved_sibling = pair.1.0^\n    let saved_last = pair.2^",
                )
                .replace("let restored =", "pair.2 = saved_last^\n    let restored =")
        } else if scenario == 2 {
            let mut changed = source
                .replace(
                    "var pair = (wrap(identity),",
                    "var pair = ((wrap(identity), 7),",
                )
                .replace("pair.0(", "pair.0.0(")
                .replace("pair.0^", "pair.0.0^")
                .replace(
                    "let empty =",
                    "let saved_target = pair.0.1^\n    let empty =",
                )
                .replace("if held ==", "if saved_target == 7 && held ==");
            for value in ["addition(20)", "subtraction(50)", "addition(1)", "identity"] {
                let original = format!("pair.0 = wrap({value})");
                assert!(changed.contains(&original));
                changed = changed.replace(&original, &format!("pair.0 = (wrap({value}), 7)"));
            }
            changed
        } else {
            source.to_owned()
        };
        execute_native_status(&compile(&source), &package.0, "component-growth-holes", 42);
    }
}

#[test]
fn private_contextual_component_growth_preserves_siblings_and_saved_instances() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-growth.nct"
    ));
    let package = TempPackage::new();
    for nested in [false, true] {
        let source = if nested {
            source
                .replace("(wrap(identity), 2)", "(1, (wrap(identity), 2), 3)")
                .replace("pair.0", "pair.1.0")
                .replace("pair.1 ==", "pair.1.1 ==")
                .replace("= pair.1\n", "= pair.1.1\n")
                .replace(
                    "if small_sibling",
                    "if pair.0 == 1 && pair.2 == 3 && small_sibling",
                )
        } else {
            source
                .replace(
                    "let empty =",
                    "read prior = pair.1\n    let initial = prior\n    let empty =",
                )
                .replace("if small_sibling", "if initial == 2 && small_sibling")
        };
        execute_native_status(
            &compile(&source),
            &package.0,
            "contextual-component-growth",
            42,
        );
    }
}

#[test]
fn private_contextual_component_growth_moves_owned_siblings_and_disposes_actual_errors() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-growth-errors.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &image(&trusted_error_machine(source)),
        &package.0,
        "component-growth-errors",
        42,
    );
}

#[test]
fn private_contextual_component_growth_moves_only_initialized_siblings_and_preserves_saved_errors()
{
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-growth-errors.nct"
    ));
    let changed = source
        .replace(
            "let empty =",
            "let saved_sibling = pair.2.0^\n    let empty =",
        )
        .replace("inspect(pair.2.0)", "inspect(saved_sibling)");
    assert_ne!(changed, source);
    let package = TempPackage::new();
    execute_native_status(
        &image(&trusted_error_machine(&changed)),
        &package.0,
        "component-growth-saved-error",
        42,
    );
}

#[test]
fn private_contextual_array_holes_preserve_saved_and_remaining_error_graphs() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-growth-errors.nct"
    ));
    let original = "(context_error(create_failure(\"sibling\", \"leaf\"), \"outer\"), 2)";
    assert!(source.contains(original));
    for length in [1, 2, 3] {
        let value = "context_error(create_failure(\"sibling\", \"leaf\"), \"outer\")";
        let elements = std::iter::repeat_n(value, length)
            .collect::<Vec<_>>()
            .join(", ");
        let replacement = format!("([{elements}], 2)");
        let changed = source
            .replace(original, &replacement)
            .replace(
                "let empty =",
                "let saved_sibling = pair.2.0[0]^\n    let empty =",
            )
            .replace("inspect(pair.2.0)", "inspect(saved_sibling)");
        let unit = trusted_error_unit(&changed);
        let package = TempPackage::new();
        execute_native_status(
            &image(&machine_unit(unit).unwrap()),
            &package.0,
            "array-saved-error",
            42,
        );
    }
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

#[test]
fn private_contextual_mixed_prefix_and_point_holes_keep_independent_original_coordinates() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-mixed-hole-growth.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "mixed-hole-growth", 42);
    let mutated = source
        .replace(
            "    var cursor: usize = 0",
            "    var selection = index\n    var cursor: usize = 0",
        )
        .replace(
            "    let selected = pair.1[index].1^",
            "    let selected = pair.1[selection].1^\n    selection = 1 - selection",
        );
    assert_ne!(mutated, source);
    execute_native_status(&compile(&mutated), &package.0, "mixed-hole-snapshot", 42);
}

#[test]
fn private_contextual_mixed_holes_preserve_withdrawn_and_remaining_error_graphs() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-mixed-hole-errors.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &image(&trusted_error_machine(source)),
        &package.0,
        "mixed-hole-errors",
        42,
    );
}

#[test]
fn private_contextual_record_holes_preserve_initialized_fields_and_saved_owners() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-record-hole-growth.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "record-hole-growth", 42);
}

#[test]
fn private_contextual_record_holes_preserve_actual_saved_and_remaining_error_graphs() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-record-hole-errors.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &image(&trusted_error_machine(source)),
        &package.0,
        "record-hole-errors",
        42,
    );
}

#[test]
fn private_contextual_optional_siblings_keep_original_tags_and_contexts_across_relocation() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-optional-sibling-growth.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "optional-sibling-growth", 42);
}

#[test]
fn private_contextual_outcome_siblings_preserve_active_payloads_and_actual_error_cleanup() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-outcome-sibling-errors.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &image(&trusted_error_machine(source)),
        &package.0,
        "outcome-sibling-errors",
        42,
    );
}

#[test]
fn private_complete_fallible_cleanup_selects_actual_record_callback_and_error_graphs() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/complete-fallible-cleanup.nct"
    ));
    let machine = trusted_error_machine(source);
    let helpers: Vec<_> = machine
        .functions()
        .filter(|(_, function)| {
            let body = function.body();
            body.parameters()
                .first()
                .and_then(|&id| body.stack(id))
                .is_some_and(|slot| {
                    matches!(
                        machine.layouts().get(slot.ty()).unwrap().kind(),
                        nocter_machine::MachineLayoutKind::Outcome { .. }
                    )
                })
        })
        .collect();
    assert_eq!(helpers.len(), 2);
    for (_, helper) in helpers {
        assert_eq!(
            helper
                .body()
                .operations()
                .filter(|(_, operation)| matches!(
                    operation.kind(),
                    nocter_machine::MachineOperationKind::Call(_)
                ))
                .count(),
            2
        );
    }
    let package = TempPackage::new();
    execute_native_status(
        &image(&machine),
        &package.0,
        "complete-fallible-cleanup",
        42,
    );
}

#[test]
fn private_positive_callable_domains_return_store_and_retire_original_runtime_choices() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/value-contexts/contextual-joined-sibling-growth.nct"
    ));
    let (definitions, _) = source.split_once("\nconst ANSWER:").unwrap();
    let source = format!(
        "{}\nfunc run(): i32 {{ let first = maybe(false)\nlet second = maybe(true)\n42 }}",
        definitions.replace("const func", "func")
    );
    let plain = machine(&source);
    assert_positive_constructors(&plain);
    let package = TempPackage::new();
    execute_native_status(
        &image(&plain),
        &package.0,
        "positive-callable-retirement",
        42,
    );
    let owned = owned_choice_source(&source);
    let owned = trusted_error_machine(&owned);
    assert_positive_constructors(&owned);
    assert_positive_cleanup(&owned);
    execute_native_status(&image(&owned), &package.0, "positive-owned-retirement", 42);
}

fn assert_positive_constructors(machine: &nocter_machine::MachineProgram) {
    let constructors = machine
        .functions()
        .filter(|(_, function)| {
            let body = function.body();
            let parameters = body.parameters();
            if parameters.len() != 2 {
                return false;
            }
            let source = body.stack(parameters[0]).unwrap();
            let target = body.stack(parameters[1]).unwrap();
            matches!(
                machine.layouts().get(source.ty()).unwrap().kind(),
                nocter_machine::MachineLayoutKind::StaticCallable { .. }
            ) && matches!(
                machine.layouts().get(target.ty()).unwrap().kind(),
                nocter_machine::MachineLayoutKind::CallableAlternatives { .. }
            )
        })
        .count();
    assert!(
        constructors >= 2,
        "both original cases need tagged construction: {constructors}"
    );
}

#[test]
fn private_bare_positive_callable_domains_retain_and_retire_complete_runtime_cases() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/value-contexts/contextual-joined-sibling-growth.nct"
    ));
    let (definitions, _) = source.split_once("\nconst func calculate").unwrap();
    let source = format!(
        "{}\nfunc run(): i32 {{ let first = maybe(false)\nlet second = maybe(true)\n42 }}",
        definitions.replace("const func", "func").replace(
            "func maybe(present: bool): (func(i32): i32)?",
            "func maybe(present: bool): func(i32): i32"
        )
    );
    let program = machine(&source);
    assert_positive_constructors(&program);
    let package = TempPackage::new();
    execute_native_status(&image(&program), &package.0, "bare-positive-retirement", 42);
    let owned = trusted_error_machine(&owned_choice_source(&source));
    assert_positive_constructors(&owned);
    assert_positive_cleanup(&owned);
    execute_native_status(
        &image(&owned),
        &package.0,
        "bare-positive-owned-retirement",
        42,
    );
}

fn owned_choice_source(source: &str) -> String {
    assert!(source.contains("struct Item { value: i32 }"));
    assert_eq!(source.matches("Item { value: value }").count(), 2);
    let source = source.replace("struct Item { value: i32 }", "struct Item { value: i32\nreason: error }")
        .replace("Item { value: value }", "Item { value: value, reason: context_error(create_failure(\"choice\", \"leaf\"), \"outer\") }");
    format!(
        "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\n{source}"
    )
}

fn assert_positive_cleanup(owned: &nocter_machine::MachineProgram) {
    assert!(owned.functions().any(|(_, function)| {
        let body = function.body();
        body.parameters()
            .first()
            .and_then(|&id| body.stack(id))
            .is_some_and(|slot| {
                matches!(
                    owned.layouts().get(slot.ty()).unwrap().kind(),
                    nocter_machine::MachineLayoutKind::CallableAlternatives { .. }
                )
            })
            && body.blocks().any(|(_, block)| {
                matches!(
                    block.terminator(),
                    nocter_machine::MachineTerminator::SwitchValue { .. }
                )
            })
    }));
    assert!(
        owned
            .functions()
            .flat_map(|(_, function)| function.body().operations())
            .any(|(_, operation)| matches!(
                operation.kind(),
                nocter_machine::MachineOperationKind::ReleaseError { .. }
            ))
    );
}

#[test]
fn private_positive_callable_domains_invoke_only_the_original_runtime_case() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/value-contexts/contextual-joined-sibling-growth.nct"
    ));
    let (definitions, _) = source.split_once("\nconst ANSWER:").unwrap();
    let source = format!(
        "{}\nfunc run(): i32 {{ if calculate(false) == 42 && calculate(true) == 42 {{ 42 }} else {{ 0 }} }}",
        definitions.replace("const func", "func")
    );
    let plain = machine(&source);
    assert_positive_constructors(&plain);
    assert_positive_invocation(&plain);
    let package = TempPackage::new();
    execute_native_status(
        &image(&plain),
        &package.0,
        "positive-callable-invocation",
        42,
    );
    let owned = trusted_error_machine(&owned_choice_source(&source));
    assert_positive_constructors(&owned);
    assert_positive_cleanup(&owned);
    execute_native_status(&image(&owned), &package.0, "positive-owned-invocation", 42);
}

fn assert_positive_invocation(machine: &nocter_machine::MachineProgram) {
    use nocter_machine::{MachineCallTarget, MachineOperationKind, MachineTerminator};
    assert!(
        machine.functions().any(|(_, function)| {
            let body = function.body();
            body.blocks().any(|(_, block)| {
                let MachineTerminator::SwitchValue {
                    cases, fallback, ..
                } = block.terminator()
                else {
                    return false;
                };
                if cases.len() < 2
                    || !matches!(
                        body.block(fallback.block()).unwrap().terminator(),
                        MachineTerminator::Unreachable
                    )
                {
                    return false;
                }
                let targets: Option<std::collections::BTreeSet<_>> = cases
                    .iter()
                    .map(|case| {
                        body.block(case.target().block())
                            .unwrap()
                            .operations()
                            .iter()
                            .find_map(|&id| {
                                let MachineOperationKind::Call(call) =
                                    body.operation(id).unwrap().kind()
                                else {
                                    return None;
                                };
                                let MachineCallTarget::Direct(target) = call.target() else {
                                    return None;
                                };
                                Some(*target)
                            })
                    })
                    .collect();
                targets.is_some_and(|targets| targets.len() == cases.len())
            })
        }),
        "each runtime tag must select its own body, with invalid tags rejected"
    );
}

#[test]
fn private_positive_callable_domains_preserve_edit_and_take_explicit_inputs() {
    let machine = trusted_error_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/positive-callable-inputs.nct"
    )));
    assert_positive_constructors(&machine);
    assert_positive_invocation(&machine);
    let package = TempPackage::new();
    execute_native_status(&image(&machine), &package.0, "positive-callable-inputs", 42);
}

#[test]
fn private_positive_callable_domains_evaluate_read_inputs_once_for_bare_and_optional_values() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/value-contexts/contextual-joined-sibling-growth.nct"
    ));
    let (definitions, _) = source.split_once("\nconst func calculate").unwrap();
    for optional in [false, true] {
        let mut definitions = definitions.replace("const func", "func");
        if !optional {
            definitions = definitions.replace(
                "func maybe(present: bool): (func(i32): i32)?",
                "func maybe(present: bool): func(i32): i32",
            );
        }
        let fallback = if optional {
            " otherwise { return 0 }"
        } else {
            ""
        };
        let source = format!(
            "{definitions}\nfunc advance(edit count: i32): i32 {{ count += 1\ncount }}\nfunc invoke(present: bool): i32 {{\nvar count = 0\nlet candidate = maybe(present)\nlet callback = candidate^{fallback}\nlet result = callback(advance(count))\nif count != 1 {{ return 0 }}\nif present {{ if result == 3 {{ 42 }} else {{ 0 }} }} else {{ if result == 1 {{ 42 }} else {{ 0 }} }}\n}}\nfunc run(): i32 {{ if invoke(false) == 42 && invoke(true) == 42 {{ 42 }} else {{ 0 }} }}"
        );
        let plain = machine(&source);
        assert_positive_constructors(&plain);
        assert_positive_invocation(&plain);
        let package = TempPackage::new();
        execute_native_status(&image(&plain), &package.0, "positive-read-input-once", 42);
        let owned = trusted_error_machine(&owned_choice_source(&source));
        assert_positive_invocation(&owned);
        assert_positive_cleanup(&owned);
        execute_native_status(
            &image(&owned),
            &package.0,
            "positive-owned-read-input-once",
            42,
        );
    }
}

#[test]
fn private_positive_callable_domains_widen_nested_choices_before_selected_invocation() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/value-contexts/contextual-joined-sibling-growth.nct"
    ));
    let (definitions, _) = source.split_once("\nconst func maybe").unwrap();
    let source = format!(
        "{}\nfunc choose(which: i32): func(i32): i32 {{\nif which == 0 {{ wrap(identity) }} else {{ if which == 1 {{ wrap(addition(2)) }} else {{ wrap(subtraction(50)) }} }}\n}}\nfunc invoke(which: i32): i32 {{\nlet callback = choose(which)\nif which == 0 {{ callback(42) }} else {{ if which == 1 {{ callback(40) }} else {{ callback(8) }} }}\n}}\nfunc run(): i32 {{ if invoke(0) == 42 && invoke(1) == 42 && invoke(2) == 42 {{ 42 }} else {{ 0 }} }}",
        definitions.replace("const func", "func")
    );
    let plain = machine(&source);
    assert_positive_invocation(&plain);
    assert_positive_widening(&plain);
    let package = TempPackage::new();
    execute_native_status(&image(&plain), &package.0, "positive-nested-invocation", 42);
    let owned = trusted_error_machine(&owned_choice_source(&source));
    assert_positive_invocation(&owned);
    assert_positive_widening(&owned);
    assert_positive_cleanup(&owned);
    execute_native_status(
        &image(&owned),
        &package.0,
        "positive-owned-nested-invocation",
        42,
    );
}

fn assert_positive_widening(machine: &nocter_machine::MachineProgram) {
    assert!(
        machine.functions().any(|(_, function)| {
            let body = function.body();
            let [source, target] = body.parameters() else {
                return false;
            };
            let cases = |parameter| {
                let data = body.stack(parameter)?.ty();
                let nocter_machine::MachineLayoutKind::CallableAlternatives { cases, .. } =
                    machine.layouts().get(data)?.kind()
                else {
                    return None;
                };
                Some(cases.len())
            };
            cases(*source) == Some(2) && cases(*target) == Some(3)
        }),
        "nested original choices need an actual two-case to three-case transport"
    );
}

#[test]
fn private_callable_record_fields_preserve_names_captures_forwarding_and_owned_cleanup() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/callable-record-fields.nct"
    ));
    let (definitions, _) = source.split_once("\nconst ANSWER:").unwrap();
    let runtime = format!(
        "{}\nfunc run(): i32 {{ calculate() }}",
        definitions.replace("const func", "func")
    );
    let package = TempPackage::new();
    execute_native_status(&compile(&runtime), &package.0, "callable-record-fields", 42);
    let owned = record_capture_with_owned_error(&runtime);
    let machine = trusted_error_machine(&owned);
    assert!(
        machine
            .functions()
            .flat_map(|(_, function)| function.body().operations())
            .any(|(_, operation)| matches!(
                operation.kind(),
                nocter_machine::MachineOperationKind::ReleaseError { .. }
            ))
    );
    execute_native_status(
        &image(&machine),
        &package.0,
        "callable-record-fields-errors",
        42,
    );
}

#[test]
fn private_selected_calls_return_forward_and_transfer_actual_contextual_fields() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/value-contexts/selected-contextual-results.nct"
    ));
    let (definitions, _) = source.split_once("\nconst ANSWER:").unwrap();
    let runtime = format!(
        "{}\nfunc run(): i32 {{ if calculate(false) == 42 && calculate(true) == 42 {{ 42 }} else {{ 0 }} }}",
        definitions.replace("const func", "func")
    );
    let package = TempPackage::new();
    execute_native_status(
        &compile(&runtime),
        &package.0,
        "selected-contextual-results",
        42,
    );
    let machine = trusted_error_machine(&record_capture_with_owned_error(&runtime));
    assert!(
        machine
            .functions()
            .flat_map(|(_, function)| function.body().operations())
            .any(|(_, operation)| matches!(
                operation.kind(),
                nocter_machine::MachineOperationKind::ReleaseError { .. }
            ))
    );
    execute_native_status(
        &image(&machine),
        &package.0,
        "selected-contextual-result-errors",
        42,
    );
}

fn record_capture_with_owned_error(source: &str) -> String {
    let formation = "    Handler { callback: (item^;";
    assert_eq!(source.matches(formation).count(), 1);
    let source = source.replace(formation, "    let reason = context_error(create_failure(\"field\", \"owned capture\"), \"outer\")\n    Handler { callback: (item^, reason^;");
    format!(
        "primitive func create_failure(code: str, message: str): error\nprimitive func context_error(take value: error, message: str): error\n{source}"
    )
}
