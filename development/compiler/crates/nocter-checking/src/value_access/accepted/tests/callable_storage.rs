//! Ordinary callable storage keeps exact contexts through retirement, receipt and aliases.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value;

const RECEIVED_OUTCOMES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/native/value-access/received-excluded-outcome-inputs.nct"
));

#[test]
fn received_outcome_inputs_preserve_application_specific_empty_domains() {
    use crate::value_access::{ExpressionKind, InstantiationFailure};
    use nocter_constant_evaluation::state::WorkBudget;
    use nocter_model::{ValueApplication, ValueField, ValueFunctionOrigin};
    use std::num::NonZeroU64;
    let source = RECEIVED_OUTCOMES;
    let runtime = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func");
    let unit = accepted(&runtime);
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let caller = program.bind_code(code("calculate").into(), []).unwrap();
    let forward = named(&unit, "forward");
    let local = forward
        .typed()
        .body()
        .inputs()
        .ordered()
        .next()
        .unwrap()
        .local;
    let incomplete = program.bind_code(code("forward").into(), []).unwrap();
    let budget = || WorkBudget::new(NonZeroU64::new(100_000).unwrap());
    assert_eq!(
        unit.instantiate_input_data(&incomplete, local, &mut budget()),
        Err(InstantiationFailure::Unresolved)
    );
    let mut empty = Vec::new();
    for (call, expression) in named(&unit, "calculate")
        .typed()
        .body()
        .expressions()
        .iter()
    {
        let ExpressionKind::Call {
            target: ValueFunctionOrigin::Declaration(target),
            ..
        } = expression.kind
        else {
            continue;
        };
        let frame = unit
            .instantiate_invocation(
                &caller,
                ValueApplication {
                    owner: caller.code(),
                    call,
                },
                &mut budget(),
            )
            .unwrap_or_else(|failure| {
                panic!(
                    "{} invocation: {failure:?}",
                    program.code_label(target).unwrap()
                )
            });
        if target != code("forward") {
            continue;
        }
        let received = frame.input_data(0).unwrap();
        assert!(
            !received.is_uninhabited(),
            "the carrier is an actual received value"
        );
        assert_eq!(
            unit.instantiate_input_data(&frame, local, &mut budget())
                .unwrap(),
            *received
        );
        let result = unit
            .instantiate_result_contract(&frame, &mut budget())
            .unwrap();
        let excluded = |key: &nocter_declarations::value_access::DataInstanceKey| {
            key.component(program, ValueField::Variant(true))
                .unwrap()
                .is_uninhabited()
        };
        assert_eq!(excluded(&result), excluded(received));
        unit.instantiate_value_data_map(&frame, &mut budget())
            .unwrap();
        assert_resources(&unit, &frame, forward);
        empty.push(excluded(received));
    }
    assert_eq!(empty, [true, false]);
    unit.instantiate_prepared_data(&caller, &mut budget())
        .unwrap();
    unit.instantiate_value_data_map(&caller, &mut budget())
        .unwrap();
    assert_resources(&unit, &caller, named(&unit, "calculate"));
}

#[test]
fn received_outcome_inputs_execute_through_required_forwarding_and_merges() {
    let source = RECEIVED_OUTCOMES;
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
    let conditional = source
        .replace(
            "forward(absent())",
            "forward(if flag { absent() } else { absent() })",
        )
        .replace("func calculate():", "func calculate(flag: bool):")
        .replace("= calculate()", "= calculate(true)")
        .replace("{ calculate() }", "{ calculate(false) }");
    assert_ne!(conditional, source);
    assert_eq!(answer(&conditional, "ANSWER"), Value::integer(42));
    for condition in ["true", "false"] {
        let fixed = conditional.replace("if flag", &format!("if {condition}"));
        assert_ne!(fixed, conditional);
        assert_eq!(answer(&fixed, "ANSWER"), Value::integer(42));
    }
}

#[test]
fn compound_input_results_preserve_selected_component_witnesses_and_actual_values() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-input-results.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn compound_input_results_require_the_original_received_context_and_recover_after_failure() {
    use crate::value_access::{ExpressionKind, InstantiationFailure};
    use nocter_constant_evaluation::state::WorkBudget;
    use nocter_model::{ValueApplication, ValueFunctionOrigin};
    use std::{collections::BTreeSet, num::NonZeroU64};
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-input-results.nct"
    ));
    let unit = accepted(source);
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let caller = program.bind_code(code("calculate").into(), []).unwrap();
    let mut contexts = BTreeSet::new();
    let mut calls = 0;
    let budget = |units| WorkBudget::new(NonZeroU64::new(units).unwrap());
    for (call, expression) in unit
        .code_body(caller.code())
        .unwrap()
        .typed()
        .body()
        .expressions()
        .iter()
    {
        let ExpressionKind::Call {
            target: ValueFunctionOrigin::Declaration(target),
            ..
        } = expression.kind
        else {
            continue;
        };
        if target != code("select_array") {
            continue;
        }
        calls += 1;
        let selected = unit
            .instantiate_invocation(
                &caller,
                ValueApplication {
                    owner: caller.code(),
                    call,
                },
                &mut budget(100_000),
            )
            .unwrap();
        assert!(selected.witnesses().is_empty());
        assert!(selected.has_compound_input_contexts());
        assert!(selected.input_data(0).is_some());
        assert!(selected.input_data(1).is_none());
        let incomplete = program.bind_code(selected.code(), []).unwrap();
        assert_eq!(
            unit.instantiate_result_contract(&incomplete, &mut budget(100_000)),
            Err(InstantiationFailure::Unresolved)
        );
        assert_eq!(
            unit.instantiate_result_contract(&selected, &mut budget(1)),
            Err(InstantiationFailure::WorkLimit)
        );
        let result = unit
            .instantiate_result_contract(&selected, &mut budget(100_000))
            .unwrap();
        assert!(result.function_representation(program).unwrap().is_some());
        contexts.insert(selected);
    }
    assert_eq!(
        calls, 3,
        "direct calls and the named function value share the same receiving contracts"
    );
    assert_eq!(
        contexts.len(),
        2,
        "same code and direct bindings retain distinct received compound contexts"
    );
}

#[test]
fn compound_input_results_preserve_transfer_access_and_const_obligations() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-input-results.nct"
    ));
    for (index, changed) in [
        source.replace("{ pair.0^ }", "{ pair.0 }"),
        source.replace("take actions:", "actions:"),
        source.replace(
            "{ select(pair^) }",
            "{ let result = select(pair^)\nlet _ = pair.1\nresult^ }",
        ),
        format!("func runtime(value: i32): i32 {{ value + 20 }}\n{source}").replace(
            "let first = select((wrap(addition(20)), 0))",
            "let first = select((runtime, 0))",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert_ne!(changed, source);
        let (sources, tree) = input(&changed);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert!(
            matches!(
                (index, &failure),
                (0 | 1, SourceFailure::Checking(Failure::Body { .. }))
                    | (2, SourceFailure::Checking(Failure::Flow { .. }))
                    | (3, SourceFailure::Checking(Failure::Const { .. }))
            ),
            "case {index}: {failure:?}"
        );
    }
}

#[test]
fn contextual_component_alias_writes_preserve_the_actual_containing_owner() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-replacement.nct"
    ));
    let runtime = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("if ANSWER == 42 { calculate() } else { 0 }", "calculate()")
        .replace("const func", "func");
    accepted(&runtime);
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn contextual_alias_writes_require_update_authority_and_exclusive_access() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-replacement.nct"
    ));
    for (index, changed) in [
        source.replace("edit selected =", "read selected ="),
        source.replace(
            "let second = selected(8)",
            "read held = selected\nselected = wrap(addition(1))\nlet second = held(8)",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert_ne!(changed, source);
        let (sources, tree) = input(&changed);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert!(
            matches!(
                (index, &failure),
                (0, SourceFailure::Checking(Failure::Body { .. }))
                    | (1, SourceFailure::Checking(Failure::Flow { .. }))
            ),
            "{failure:?}"
        );
    }
}

#[test]
fn mutable_array_inputs_preserve_selected_callback_state_across_forwarded_calls() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-mutable-array-inputs.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn mutable_array_inputs_preserve_bounds_and_writable_access_requirements() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-mutable-array-inputs.nct"
    ));
    let immutable = source.replace("var actions =", "let actions =");
    assert_ne!(immutable, source);
    let (sources, tree) = input(&immutable);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Body { .. }))
    ));
    let bad_index = source.replace(
        "let first = step(actions, 0, 1)",
        "let first = step(actions, 2, 1)",
    );
    assert_ne!(bad_index, source);
    let (sources, tree) = input(&bad_index);
    let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
    assert!(
        matches!(failure, SourceFailure::Checking(Failure::Evaluation { diagnostic, .. }) if diagnostic.rule() == crate::value_access::evaluation::Rule::Execution(nocter_constant_evaluation::state::Rule::IndexOutOfBounds))
    );
}

#[test]
fn compound_edits_preserve_received_contexts_and_caller_completion() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-edits.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn compound_edits_require_writable_storage_and_preserve_live_reference_conflicts() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-edits.nct"
    ));
    for (index, changed) in [
        source.replace("var pair =", "let pair ="),
        source.replace(
            "let first = update(pair)",
            "read held = pair.1\nlet first = update(pair)\nlet _ = held",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert_ne!(changed, source);
        let (sources, tree) = input(&changed);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert!(
            matches!(
                (index, &failure),
                (0, SourceFailure::Checking(Failure::Body { .. }))
                    | (1, SourceFailure::Checking(Failure::Flow { .. }))
            ),
            "{failure:?}"
        );
    }
}

#[test]
fn compound_edit_preparations_retire_only_explicitly_transferred_owners() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-edits.nct"
    ));
    let changed = source.replace(
        "let moved = forward(pair^)",
        "let moved = forward(pair^)\nlet _ = pair.1",
    );
    assert_ne!(changed, source);
    let (sources, tree) = input(&changed);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}

#[test]
fn completed_compound_input_reprojection_cannot_substitute_another_static_callable() {
    use crate::value_access::{
        ArgumentAction, ExpressionKind, Referent, ValueSourceBoundary, ValueSourceSelection,
        ValueSourceSlot,
    };
    use nocter_declarations::value_access::PayloadSchema;
    use nocter_model::{ValueField, ValueFunctionOrigin};
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-edits.nct"
    ));
    let source = source.replace(
        "let first = update(pair)",
        "let other = (addition(20), 21)\nlet first = update(pair)",
    );
    let unit = accepted(&source);
    let code = |name| {
        unit.program()
            .resolve(unit.program().symbols().get(name).unwrap())
            .unwrap()
    };
    let query = named(&unit, "calculate");
    let body = query.typed().body();
    let frame = unit
        .program()
        .bind_code(code("calculate").into(), [])
        .unwrap();
    let mut selected = None;
    for (point, block) in query.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            let Operation::CommitCall(call) = event.operation else {
                continue;
            };
            let ExpressionKind::Call {
                target: ValueFunctionOrigin::Declaration(target),
                arguments,
                ..
            } = &body.expressions().get(call).unwrap().kind
            else {
                continue;
            };
            if *target != code("update") {
                continue;
            }
            let ArgumentAction::ReserveUpdate(Referent::Place(place)) = &arguments[0].action else {
                continue;
            };
            let local = body.places().get(*place).unwrap().local;
            selected = Some((
                ValueSourceSelection {
                    boundary: ValueSourceBoundary {
                        point,
                        action: action + 1,
                    },
                    slot: ValueSourceSlot::Local(local),
                    path: nocter_model::ValuePath::default(),
                },
                body.locals().get(local).unwrap().data,
            ));
        }
    }
    let (selected, actual) = selected.unwrap();
    let wrong = body.locals().iter().find_map(|(_, local)| {
        let leaf = unit.program().component(local.data, ValueField::Tuple(0))?.data;
        let PayloadSchema::Function(witness) = unit.program().data_schema(leaf)? else { return None; };
        matches!(witness.origin, ValueFunctionOrigin::Closure(closure) if closure.owner.source_owner() == code("addition")).then_some(local.data)
    }).unwrap();
    assert_ne!(actual, wrong);
    let mut work = nocter_constant_evaluation::state::WorkBudget::new(
        std::num::NonZeroU64::new(1_000_000).unwrap(),
    );
    assert_eq!(
        unit.instantiate_stored_data(&frame, &selected, actual, &mut work)
            .unwrap()
            .data(),
        actual
    );
    assert_eq!(
        unit.instantiate_stored_data(&frame, &selected, wrong, &mut work),
        Err(crate::value_access::InstantiationFailure::Unresolved)
    );
}

#[test]
fn compound_inputs_preserve_each_received_callable_context() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-inputs.nct"
    ));
    let runtime = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("if ANSWER == 42 { calculate() } else { 0 }", "calculate()")
        .replace("const func", "func");
    accepted(&runtime);
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn compound_input_headers_reconcile_each_published_contract() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-inputs.nct"
    ));
    let (sources, tree) = input(source);
    let unit = SourceUnit::prepare(bound(&sources, &tree)).unwrap();
    assert!(unit.declarations().bind(bound(&sources, &tree)).is_ok());
    let changed = source.replace("pair: (func(i32): i32, i32)", "pair: (func(i32): i64, i32)");
    assert_ne!(changed, source);
    let (sources, tree) = input(&changed);
    assert!(unit.declarations().bind(bound(&sources, &tree)).is_err());
}

#[test]
fn compound_inputs_do_not_grant_copy_or_hide_runtime_only_calls() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-inputs.nct"
    ));
    for (index, changed) in [
        source.replace("receive(first^)", "receive(first)"),
        source.replace("let b = forward", "let _ = first.1\nlet b = forward"),
        source.replace("const func identity", "func identity"),
        source.replace(
            "twins((wrap(addition(20)), wrap(subtraction(50))))",
            "twins((wrap(addition(20)), 8))",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert_ne!(changed, source);
        let (sources, tree) = input(&changed);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        let correct = match (index, &failure) {
            (
                0 | 3,
                SourceFailure::Checking(Failure::Body {
                    failure: crate::value_access::Failure::Rejected(diagnostic),
                    ..
                }),
            ) => {
                diagnostic.rule()
                    == if index == 0 {
                        crate::value_access::Rule::MissingTransfer
                    } else {
                        crate::value_access::Rule::TypeMismatch
                    }
            }
            (1, SourceFailure::Checking(Failure::Flow { .. }))
            | (2, SourceFailure::Checking(Failure::Const { .. })) => true,
            _ => false,
        };
        assert!(correct, "{failure:?}");
    }
}

#[test]
fn empty_compound_inputs_never_supply_a_callable_element() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-inputs.nct"
    ));
    let changed = source.replace(
        "const func empty(take actions: [func(i32): i32; 0]): i32 { 42 }",
        "const func empty(take actions: [func(i32): i32; 0]): i32 { actions[0](22) }",
    );
    assert_ne!(changed, source);
    let (sources, tree) = input(&changed);
    let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
    assert!(
        matches!(failure, SourceFailure::Checking(Failure::Evaluation { diagnostic, .. }) if diagnostic.rule() == crate::value_access::evaluation::Rule::Execution(nocter_constant_evaluation::state::Rule::IndexOutOfBounds))
    );
}

#[test]
fn contextual_callable_replacement_preserves_retired_received_and_transferred_instances() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-callable-replacement.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}
#[test]
fn contextual_replacement_cannot_overwrite_a_still_used_reference() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-callable-replacement.nct"
    ));
    let changed = source.replace(
        "action = wrap(subtraction(50))",
        "read retained = action\n    action = wrap(subtraction(50))\n    let _ = retained(0)",
    );
    assert_ne!(changed, source);
    let (sources, tree) = input(&changed);
    let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
    assert!(
        matches!(failure, SourceFailure::Checking(Failure::Flow { .. })),
        "{failure:?}"
    );
}

#[test]
fn contextual_indexed_growth_keeps_the_original_selection_and_sibling_values() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-growth.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
    for (changed, expected) in [
        (
            source.replace(
                "pair.0[index] = wrap(addition(20))",
                "pair.0[index + 1] = wrap(addition(20))",
            ),
            crate::value_access::evaluation::Rule::Execution(
                nocter_constant_evaluation::state::Rule::IndexOutOfBounds,
            ),
        ),
        (
            source.replace("[wrap(identity)]", "[wrap(identity), wrap(identity)]"),
            crate::value_access::evaluation::Rule::UnresolvedContext,
        ),
    ] {
        assert_ne!(changed, source);
        let (sources, tree) = input(&changed);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert!(
            matches!(failure, SourceFailure::Checking(Failure::Evaluation { diagnostic, .. }) if diagnostic.rule() == expected)
        );
    }
}

#[test]
fn contextual_dynamic_holes_preserve_the_original_index_and_restoration_proof() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-dynamic-hole-growth.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
    let changed = source.replace("let saved_sibling = pair.1[index].0^", "var selected = index\n    let saved_sibling = pair.1[selected].0^\n    selected = 1 - selected");
    assert_ne!(changed, source);
    assert_eq!(answer(&changed, "ANSWER"), Value::integer(42));
}

#[test]
fn contextual_array_holes_preserve_saved_owners_and_remaining_elements() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-array-hole-growth.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn contextual_conditional_holes_preserve_both_original_value_paths() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-conditional-hole-growth.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn contextual_singleton_holes_preserve_saved_instances_and_initialized_element_fields() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-singleton-hole-growth.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn contextual_component_growth_preserves_sparse_holes_and_separate_saved_owners() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-growth-holes.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn contextual_component_growth_preserves_siblings_and_saved_instances() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-growth.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn contextual_component_replacement_preserves_sibling_views_and_saved_instances() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-replacement.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn contextual_indexed_replacement_preserves_containing_roots_and_sibling_views() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-replacement.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn contextual_indexed_receipts_share_original_numeric_facts_with_ownership() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-same-context.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn unique_indexed_contexts_do_not_elide_bounds_or_choose_a_multi_element_context() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-replacement.nct"
    ));
    for (changed, expected) in [
        (
            source.replace("calculate(0)", "calculate(1)"),
            crate::value_access::evaluation::Rule::Execution(
                nocter_constant_evaluation::state::Rule::IndexOutOfBounds,
            ),
        ),
        (
            source.replace(
                "[wrap(addition(20))]",
                "[wrap(addition(20)), wrap(addition(20))]",
            ),
            crate::value_access::evaluation::Rule::UnresolvedContext,
        ),
    ] {
        assert_ne!(changed, source);
        let (sources, tree) = input(&changed);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert!(
            matches!(&failure, SourceFailure::Checking(Failure::Evaluation { diagnostic, .. }) if diagnostic.rule() == expected),
            "{failure:?}"
        );
    }
}

#[test]
fn a_unique_coordinate_does_not_allow_overwriting_a_still_used_element_reference() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-replacement.nct"
    ));
    let changed = source
        .replace(
            "edit selected = pair.0[retained]",
            "read held = pair.0[retained]\n    edit selected = pair.0[retained]",
        )
        .replace(
            "let second = selected(8)",
            "let _ = held(20)\n    let second = selected(8)",
        );
    assert_ne!(changed, source);
    let (sources, tree) = input(&changed);
    let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
    assert!(
        matches!(failure, SourceFailure::Checking(Failure::Flow { .. })),
        "{failure:?}"
    );
}

#[test]
fn contextual_prefix_holes_preserve_required_evaluation_after_counter_mutation() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-prefix-hole-growth.nct"
    ));
    assert_eq!(
        super::initializers::answer(source, "ANSWER"),
        nocter_constant_evaluation::state::Value::integer(42)
    );
}
