//! Ordinary callable storage keeps exact contexts through retirement, receipt and aliases.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value;

const RECEIVED_OUTCOMES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/native/value-access/received-excluded-outcome-inputs.nct"
));

const JOINED_SIBLINGS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/value-contexts/contextual-joined-sibling-growth.nct"
));
fn joined_runtime_unit() -> Unit {
    let (definitions, _) = JOINED_SIBLINGS
        .split_once("\nconst ANSWER:")
        .expect("required fixture entry");
    accepted(&format!("{definitions}\nfunc run(): i32 {{ 0 }}\n"))
}

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

#[test]
fn contextual_mixed_holes_preserve_distinct_prefix_and_point_coordinates() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-mixed-hole-growth.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn contextual_record_holes_preserve_nominal_fields_and_restore_saved_owners() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-record-hole-growth.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn contextual_optional_siblings_preserve_original_tags_and_received_contexts() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-optional-sibling-growth.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn positive_source_result_domains_keep_both_original_creation_contexts() {
    use nocter_constant_evaluation::state::WorkBudget;
    use nocter_declarations::value_access::InstanceSchema;
    use std::num::NonZeroU64;
    let unit = joined_runtime_unit();
    let program = unit.program();
    let code = program
        .resolve(program.symbols().get("maybe").unwrap())
        .unwrap();
    let frame = program.bind_code(code.into(), []).unwrap();
    let mut work = WorkBudget::new(NonZeroU64::new(100_000).unwrap());
    let domain = unit.instantiate_result_contract(&frame, &mut work).unwrap();
    let primary = domain
        .component(program, nocter_model::ValueField::Variant(true))
        .unwrap();
    let InstanceSchema::FunctionAlternatives(choices) = primary.schema(program).unwrap() else {
        panic!("positive source choices")
    };
    assert_eq!(choices.len(), 2);
    assert_eq!(choices[0].code(), choices[1].code());
    assert_ne!(choices[0].environment(), choices[1].environment());
    assert!(primary.function_representation(program).is_err());
    let mut exhausted = WorkBudget::new(NonZeroU64::new(1).unwrap());
    assert_eq!(
        unit.instantiate_result_contract(&frame, &mut exhausted),
        Err(crate::value_access::InstantiationFailure::WorkLimit)
    );
    let mut healthy = WorkBudget::new(NonZeroU64::new(100_000).unwrap());
    assert_eq!(
        unit.instantiate_result_contract(&frame, &mut healthy)
            .unwrap(),
        domain
    );
}

#[test]
fn invocation_domains_keep_original_preparations_and_each_candidate_environment() {
    use crate::value_access::{CallKind, ExpressionKind, InstantiationFailure};
    use nocter_constant_evaluation::state::WorkBudget;
    use nocter_model::ValueApplication;
    use std::num::NonZeroU64;
    let unit = std::sync::Arc::new(joined_runtime_unit());
    let program = unit.program();
    let code = program
        .resolve(program.symbols().get("calculate").unwrap())
        .unwrap();
    let caller = program.bind_code(code.into(), []).unwrap();
    let body = unit.code_body(caller.code()).unwrap();
    let call = body
        .typed()
        .body()
        .expressions()
        .iter()
        .filter_map(|(call, expression)| {
            matches!(
                expression.kind,
                ExpressionKind::Call {
                    kind: CallKind::Value,
                    ..
                }
            )
            .then_some(call)
        })
        .last()
        .unwrap();
    let site = ValueApplication {
        owner: caller.code(),
        call,
    };
    let budget = || WorkBudget::new(NonZeroU64::new(100_000).unwrap());
    assert_eq!(
        unit.instantiate_invocation(&caller, site, &mut budget()),
        Err(InstantiationFailure::Unresolved)
    );
    let domain = unit
        .instantiate_invocation_domain(&caller, site, &mut budget())
        .unwrap();
    assert_retained_invocation_domain(&unit, &caller, site, &domain);
    assert_eq!(domain.caller(), &caller);
    assert_eq!(domain.site(), site);
    assert_eq!(domain.cases().len(), 2);
    let cases = domain.cases();
    assert_eq!(cases[0].source(), cases[1].source());
    assert_eq!(cases[0].callee().code(), cases[1].callee().code());
    assert_ne!(
        cases[0].callee().environment(),
        cases[1].callee().environment()
    );
    for case in cases {
        assert_eq!(case.frame().code(), case.callee().code());
        assert_eq!(case.frame().environment(), case.callee().environment());
        assert_eq!(case.source().kind, CallKind::Value);
        assert_eq!(case.source().arguments.len(), 2);
        let action = &body
            .graph()
            .points()
            .get(case.source().origin.point)
            .unwrap()
            .actions[case.source().origin.action];
        assert_eq!(
            action.operation,
            crate::value_access::flow::Operation::CommitCall(call)
        );
    }
    let mut exhausted = WorkBudget::new(NonZeroU64::new(1).unwrap());
    assert_eq!(
        unit.instantiate_invocation_domain(&caller, site, &mut exhausted),
        Err(InstantiationFailure::WorkLimit)
    );
    assert_eq!(
        unit.instantiate_invocation_domain(&caller, site, &mut budget())
            .unwrap(),
        domain
    );
    assert_invocation_domain_budget(&unit, &caller, site, &domain);
    let root = program
        .resolve(program.symbols().get("run").unwrap())
        .unwrap();
    let root = program.bind_code(root.into(), []).unwrap();
    let foreign = ValueApplication {
        owner: root.code(),
        call,
    };
    assert_eq!(
        unit.instantiate_invocation_domain(&caller, foreign, &mut budget()),
        Err(InstantiationFailure::WrongOwner)
    );
    assert_eq!(
        unit.instantiate_invocation_domain(&root, foreign, &mut budget()),
        Err(InstantiationFailure::MissingCall)
    );
}

fn assert_invocation_domain_budget(
    unit: &Unit,
    caller: &nocter_declarations::value_access::CodeInstanceKey,
    site: nocter_model::ValueApplication,
    expected: &crate::value_access::InvocationDomain,
) {
    use crate::value_access::InstantiationFailure;
    use nocter_constant_evaluation::state::WorkBudget;
    use std::num::NonZeroU64;
    let mut low = 1;
    let mut high = 100_000;
    while low < high {
        let middle = low + (high - low) / 2;
        let mut work = WorkBudget::new(NonZeroU64::new(middle).unwrap());
        match unit.instantiate_invocation_domain(caller, site, &mut work) {
            Ok(actual) => {
                assert_eq!(&actual, expected);
                high = middle;
            }
            Err(InstantiationFailure::WorkLimit) => low = middle + 1,
            Err(other) => panic!("unexpected metadata failure: {other:?}"),
        }
    }
    assert!(low > 1);
    let mut late = WorkBudget::new(NonZeroU64::new(low - 1).unwrap());
    assert_eq!(
        unit.instantiate_invocation_domain(caller, site, &mut late),
        Err(InstantiationFailure::WorkLimit)
    );
    let mut exact = WorkBudget::new(NonZeroU64::new(low).unwrap());
    assert_eq!(
        &unit
            .instantiate_invocation_domain(caller, site, &mut exact)
            .unwrap(),
        expected
    );
}

#[test]
fn original_expression_merges_retain_common_receipts_without_replacing_actual_definitions() {
    use nocter_declarations::value_access::InstanceSchema;
    use nocter_model::ValueField;
    let budget = || {
        nocter_constant_evaluation::state::WorkBudget::new(
            std::num::NonZeroU64::new(100_000).unwrap(),
        )
    };
    let unit = joined_runtime_unit();
    let program = unit.program();
    let code = program
        .resolve(program.symbols().get("maybe").unwrap())
        .unwrap();
    let frame = program.bind_code(code.into(), []).unwrap();
    let values = unit
        .instantiate_value_data_map(&frame, &mut budget())
        .unwrap();
    let graph = unit.value_sources(frame.code()).unwrap();
    let expression = graph
        .nodes()
        .find_map(|(_, node)| {
            let crate::value_access::ValueSource::Merge {
                slot: crate::value_access::ValueSourceSlot::Expression(expression),
                ..
            } = node.value
            else {
                return None;
            };
            values.receipt(expression).map(|_| expression)
        })
        .unwrap();
    let receipt = values.receipt(expression).unwrap();
    let primary = receipt
        .component(program, ValueField::Variant(true))
        .unwrap();
    let InstanceSchema::FunctionAlternatives(choices) = primary.schema(program).unwrap() else {
        panic!("complete choice receipt")
    };
    assert_eq!(choices.len(), 2);
    assert_ne!(choices[0].environment(), choices[1].environment());
    let result = unit
        .instantiate_result_contract(&frame, &mut budget())
        .unwrap();
    assert!(receipt.has_same_representation(program, &result));
    let mut limited =
        nocter_constant_evaluation::state::WorkBudget::new(std::num::NonZeroU64::new(1).unwrap());
    assert!(
        unit.instantiate_value_data_map(&frame, &mut limited)
            .is_err()
    );
    let retried = unit
        .instantiate_value_data_map(&frame, &mut budget())
        .unwrap();
    assert_eq!(retried.receipt(expression), Some(receipt));
}

fn assert_retained_invocation_domain(
    unit: &std::sync::Arc<Unit>,
    caller: &nocter_declarations::value_access::CodeInstanceKey,
    site: nocter_model::ValueApplication,
    domain: &crate::value_access::InvocationDomain,
) {
    use crate::value_access::InstantiationFailure;
    use nocter_constant_evaluation::state::WorkBudget;
    use std::num::NonZeroU64;
    let budget = || WorkBudget::new(NonZeroU64::new(100_000).unwrap());
    let retained = unit.instance_metadata(caller, &mut budget()).unwrap();
    assert_eq!(
        retained.invocation_domain(site, &mut budget()).unwrap(),
        *domain
    );
    assert_eq!(
        retained.invocation(site, &mut budget()),
        Err(InstantiationFailure::Unresolved)
    );
    let mut exhausted = WorkBudget::new(NonZeroU64::new(1).unwrap());
    assert_eq!(
        retained.invocation_domain(site, &mut exhausted),
        Err(InstantiationFailure::WorkLimit)
    );
    assert_eq!(
        retained.invocation_domain(site, &mut budget()).unwrap(),
        *domain
    );
}

#[test]
fn callable_record_fields_preserve_their_original_named_source() {
    assert_eq!(
        answer(
            "struct Handler { callback: func(i32): i32 }
        const func identity(value: i32): i32 { value }
        const func calculate(): i32 {
            let holder = Handler { callback: identity }
            holder.callback(42)
        }
        const ANSWER: i32 = calculate()",
            "ANSWER"
        ),
        Value::integer(42)
    );
}

#[test]
fn callable_field_contracts_do_not_grant_runtime_functions_const_capability() {
    for required in [false, true] {
        let source = format!(
            "struct Handler {{ callback: func(i32): i32 }}
            func runtime(value: i32): i32 {{ value }}
            const func calculate(): i32 {{ let holder = Handler {{ callback: runtime }}
                holder.callback(42) }}
            {}",
            if required {
                "const ANSWER: i32 = calculate()"
            } else {
                ""
            }
        );
        let fixture = nocter_test_support::ValueSourceFixture::new(&source);
        let Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) =
            SourceUnit::prepare(fixture.syntax())
        else {
            panic!("a type contract cannot grant const capability: required={required}");
        };
        assert!(matches!(
            diagnostic.rule(),
            crate::value_access::constant::Rule::RuntimeCall(_)
        ));
    }
}

#[test]
fn callable_record_fields_preserve_owned_captures_through_factory_results() {
    assert_eq!(answer(
        "struct Item { value: i32 }
        struct Handler { callback: func(i32): i32 }
        const func make(offset: i32): Handler {
            let item = Item { value: offset }
            Handler { callback: (item^; value: i32): i32 { item.value + value } }
        }
        const func calculate(): i32 {
            let first = make(40)
            let second = make(41)
            if first.callback(2) == 42 && second.callback(1) == 42 && first.callback(1) == 41 { 42 } else { 0 }
        }
        const ANSWER: i32 = calculate()", "ANSWER"), Value::integer(42));
}

#[test]
fn generic_unqualified_field_invocations_do_not_bypass_const_definition_checks() {
    for body in [
        "let holder = Handler { callback: runtime }\nholder.callback(42)",
        "let action = (): i32 { let holder = Handler { callback: runtime }\nholder.callback(42) }\naction()",
    ] {
        let source = format!(
            "struct Handler {{ callback: func(i32): i32 }}\nfunc runtime(value: i32): i32 {{ value }}\nconst func calculate(unused: func(i32): i32): i32 {{ {body} }}"
        );
        let fixture = nocter_test_support::ValueSourceFixture::new(&source);
        let Err(SourceFailure::Checking(Failure::Const { .. })) =
            SourceUnit::prepare(fixture.syntax())
        else {
            panic!("unqualified generic field contexts cannot grant const capability");
        };
    }
}

#[test]
fn callable_field_carriers_keep_sources_through_inputs_results_and_partial_transfer() {
    assert_eq!(
        answer(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/native/value-access/callable-record-fields.nct"
            )),
            "ANSWER"
        ),
        Value::integer(42)
    );
}

#[test]
fn callable_field_factory_edits_invalidate_closed_caller_contexts() {
    let source = "struct Handler { callback: func(i32): i32 }
        func identity(value: i32): i32 { value }
        func shifted(value: i32): i32 { value + 1 }
        func make(): Handler { Handler { callback: identity } }
        func caller(): i32 { let holder = make()\nholder.callback(41) }
        func unrelated(): i32 { 0 }";
    let fixture = nocter_test_support::ValueSourceFixture::new(source);
    let headers =
        nocter_declaration_lowering::access_declarations::Query::prepare(fixture.syntax()).unwrap();
    let context = headers.bind(fixture.syntax()).unwrap();
    let unit = Unit::prepare(&context).unwrap();
    let id = unit
        .program()
        .resolve_code(unit.program().symbols().get("caller").unwrap())
        .unwrap();
    let query = unit.body(id).unwrap();
    for (changed, reusable) in [
        (
            source.replace("callback: identity", "callback: shifted"),
            false,
        ),
        (
            source.replace("unrelated(): i32 { 0 }", "unrelated(): i32 { 1 }"),
            true,
        ),
    ] {
        let fixture = nocter_test_support::ValueSourceFixture::new(&changed);
        let context = headers.bind(fixture.syntax()).unwrap();
        assert_eq!(query.bind(&context).is_ok(), reusable);
    }
}

#[test]
fn callable_field_const_contracts_remain_checked_in_dormant_source_paths() {
    for body in [
        "if false { holder.callback(42) } else { 0 }",
        "loop {}\nholder.callback(42)",
        "holder.callback(if true { return 0 } else { return 1 })",
    ] {
        let source = format!(
            "struct Handler {{ callback: func(i32): i32 }}\nfunc runtime(value: i32): i32 {{ value }}\nconst func calculate(): i32 {{ let holder = Handler {{ callback: runtime }}\n{body} }}"
        );
        accepted(&source.replace("const func", "func"));
        let fixture = nocter_test_support::ValueSourceFixture::new(&source);
        match SourceUnit::prepare(fixture.syntax()) {
            Err(SourceFailure::Checking(Failure::Const { .. })) => {}
            Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) => {
                assert_eq!(
                    diagnostic.rule(),
                    crate::value_access::evaluation::Rule::UnresolvedContext
                );
            }
            _ => panic!("expected a source-backed const rejection: {body}"),
        }
    }
}

const SELECTED_CONTEXT_RESULTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/value-contexts/selected-contextual-results.nct"
));

#[test]
fn selected_calls_return_and_forward_original_contextual_field_values() {
    assert_eq!(
        answer(SELECTED_CONTEXT_RESULTS, "ANSWER"),
        Value::integer(42)
    );
}

#[test]
fn selected_call_results_keep_every_candidate_and_the_original_receiving_definition() {
    use crate::value_access::{CallKind, ExpressionKind, InstantiationFailure, ValueSourceSlot};
    use nocter_constant_evaluation::state::{Limits, WorkBudget};
    let (definitions, _) = SELECTED_CONTEXT_RESULTS
        .split_once("\nconst ANSWER:")
        .unwrap();
    let unit = accepted(&definitions.replace("const func", "func"));
    let program = unit.program();
    let code = program
        .resolve_code(program.symbols().get("calculate").unwrap())
        .unwrap();
    let frame = program.bind_code(code.into(), []).unwrap();
    let body = unit.code_body(frame.code()).unwrap();
    let call = body
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(id, expression)| {
            (matches!(
                expression.kind,
                ExpressionKind::Call {
                    kind: CallKind::Value,
                    ..
                }
            ) && expression
                .ty
                .access()
                .is_some_and(|access| program.record_identity(access.data()).is_some()))
            .then_some(id)
        })
        .unwrap();
    let site = nocter_model::ValueApplication {
        owner: frame.code(),
        call,
    };
    let budget = || WorkBudget::new(Limits::default().steps());
    assert_eq!(
        unit.instantiate_invocation(&frame, site, &mut budget()),
        Err(InstantiationFailure::Unresolved)
    );
    let domain = unit
        .instantiate_invocation_domain(&frame, site, &mut budget())
        .unwrap();
    assert_eq!(domain.cases().len(), 2);
    let values = unit
        .instantiate_value_data_map(&frame, &mut budget())
        .unwrap();
    for case in domain.cases() {
        let result = unit
            .instantiate_result_contract(case.frame(), &mut budget())
            .unwrap();
        assert!(!result.is_context_free());
        let receiving = values
            .definition(case.source().origin, ValueSourceSlot::Expression(call))
            .unwrap()
            .instance();
        assert!(receiving.has_same_representation(program, &result));
    }
}

#[test]
fn selected_contextual_results_reject_a_nonuniform_candidate_without_an_internal_error() {
    let source = SELECTED_CONTEXT_RESULTS
        .replace(
            "const func first()",
            "const func identity(value: i32): i32 { value }\nconst func first()",
        )
        .replace(
            "const func second(): Handler { make(41) }",
            "const func second(): Handler { Handler { callback: identity } }",
        );
    let (definitions, _) = source.split_once("\nconst ANSWER:").unwrap();
    accepted(&definitions.replace("const func", "func"));
    let fixture = nocter_test_support::ValueSourceFixture::new(&source);
    let Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) =
        SourceUnit::prepare(fixture.syntax())
    else {
        panic!("a nonuniform contextual result needs an original-source rejection");
    };
    assert_eq!(
        diagnostic.rule(),
        crate::value_access::evaluation::Rule::UnresolvedContext
    );
}

#[test]
fn contextual_definition_checks_obey_configured_limits_and_share_one_final_scope() {
    let common = "struct Handler { callback: func(i32): i32 }\nconst func identity(value: i32): i32 { value }\n";
    let definition = "const func first(): i32 { let holder = Handler { callback: identity }\nholder.callback(42) }";
    let single = nocter_test_support::ValueSourceFixture::new(&format!("{common}{definition}"));
    let double = nocter_test_support::ValueSourceFixture::new(&format!(
        "{common}{definition}\n{}",
        definition.replace("first()", "second()")
    ));
    let single_limit = minimum_definition_steps(&single);
    let double_limit = minimum_definition_steps(&double);
    assert!(single_limit > 1);
    assert!(double_limit > single_limit);
    assert!(!definition_check_with_steps(&double, single_limit));
    assert!(definition_check_with_steps(&double, double_limit));
    assert!(!definition_check_with_steps(&single, single_limit - 1));
    assert!(definition_check_with_steps(&single, single_limit));
    let runtime = nocter_test_support::ValueSourceFixture::new(
        &format!("{common}{definition}").replace("const func", "func"),
    );
    assert!(definition_check_with_steps(&runtime, 1));
}

fn minimum_definition_steps(source: &nocter_test_support::ValueSourceFixture) -> u64 {
    let mut low = 1;
    let mut high = 10_000;
    assert!(definition_check_with_steps(source, high));
    while low < high {
        let middle = low + (high - low) / 2;
        if definition_check_with_steps(source, middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

fn definition_check_with_steps(
    source: &nocter_test_support::ValueSourceFixture,
    steps: u64,
) -> bool {
    let limits = nocter_constant_evaluation::CompileTimeEvaluationLimits::new(
        std::num::NonZeroU64::new(steps).unwrap(),
        std::num::NonZeroU32::new(64).unwrap(),
    );
    match SourceUnit::prepare_with_limits(source.syntax(), limits) {
        Ok(_) => true,
        Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) => {
            assert_eq!(
                diagnostic.rule(),
                crate::value_access::evaluation::Rule::ConditionWorkLimit
            );
            false
        }
        Err(failure) => panic!("unexpected definition-check failure: {failure:?}"),
    }
}

#[test]
fn higher_order_field_calls_bind_the_original_selected_physical_formals() {
    use nocter_constant_evaluation::state::{Limits, WorkBudget};
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/higher-order-callable-field-values.nct"
    ));
    let unit = accepted(source);
    assert_eq!(unit.program().parameters().iter().count(), 2);
    let called = source.replace("    42\n", "    runner.callback(identity)\n");
    let unit = accepted(&called);
    let program = unit.program();
    let code = |name| {
        program
            .resolve_code(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let frame = program.bind_code(code("run").into(), []).unwrap();
    let body = unit.code_body(frame.code()).unwrap();
    let call = body
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(id, expression)| {
            matches!(
                expression.kind,
                crate::value_access::ExpressionKind::Call { .. }
            )
            .then_some(id)
        })
        .unwrap();
    let selected = unit
        .instantiate_invocation(
            &frame,
            nocter_model::ValueApplication {
                owner: frame.code(),
                call,
            },
            &mut WorkBudget::new(Limits::default().steps()),
        )
        .unwrap();
    assert_eq!(selected.code(), code("invoke").into());
    let parameter = program.codes().get(code("invoke")).unwrap().parameters()[0];
    assert_eq!(
        program.callable_input_parameters(selected.code()).unwrap(),
        BTreeMap::from([(0, parameter.into())])
    );
    assert_eq!(selected.witnesses().len(), 1);
    assert_eq!(
        selected.witnesses().get(&parameter.into()).unwrap().code(),
        code("identity").into()
    );
}

const HIGHER_ORDER_FIELD_CALLS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/native/value-access/higher-order-callable-field-invocations.nct"
));

#[test]
fn higher_order_fields_preserve_original_callback_ordinals_owned_environments_and_transfer() {
    assert_eq!(
        answer(HIGHER_ORDER_FIELD_CALLS, "ANSWER"),
        Value::integer(42)
    );
}

#[test]
fn higher_order_field_callback_failures_preserve_original_argument_provenance() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/higher-order-callable-field-values.nct"
    ));
    let source = source
        .replace("    42\n", "    runner.callback(identity)\n")
        .replace("func ", "const func ")
        .replace("const func identity", "func identity");
    for required in [false, true] {
        let source = if required {
            format!("{source}\nconst ANSWER: i32 = run()")
        } else {
            source.clone()
        };
        let (sources, tree) = input(&source);
        let Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) =
            SourceUnit::prepare(bound(&sources, &tree))
        else {
            panic!("runtime callbacks cannot gain const capability");
        };
        assert!(matches!(
            diagnostic.rule(),
            crate::value_access::constant::Rule::RuntimeCall(_)
        ));
        let note = diagnostic
            .source()
            .notes()
            .iter()
            .find(|note| note.message().contains("argument supplies"))
            .unwrap();
        assert_eq!(
            sources
                .get(tree.source())
                .unwrap()
                .text_at(note.origin().span().range())
                .unwrap(),
            "identity"
        );
    }
}

#[test]
fn higher_order_field_inputs_report_callback_signature_mismatches_at_source() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/higher-order-callable-field-values.nct"
    ));
    let source = source
        .replace("    42\n", "    runner.callback(identity)\n")
        .replace(
            "func identity(value: i32): i32 { value }",
            "func identity(value: i32): bool { true }",
        );
    let fixture = nocter_test_support::ValueSourceFixture::new(&source);
    let Err(SourceFailure::Checking(Failure::Body {
        failure: crate::value_access::Failure::Rejected(diagnostic),
        ..
    })) = SourceUnit::prepare(fixture.syntax())
    else {
        panic!("a callback signature mismatch must reject at source");
    };
    assert_eq!(diagnostic.rule(), crate::value_access::Rule::TypeMismatch);
}

#[test]
fn callable_field_results_keep_type_contracts_separate_from_original_return_producers() {
    use crate::value_access::{ExpressionKind, ValueSource};
    use nocter_constant_evaluation::state::{Limits, WorkBudget};
    use nocter_declarations::value_access::{InstanceSchema, PayloadSchema};
    let (definitions, _) = HIGHER_ORDER_FIELD_CALLS
        .split_once("\nconst ANSWER:")
        .unwrap();
    let unit = accepted(&definitions.replace("const func", "func"));
    let program = unit.program();
    let calculate = program
        .resolve_code(program.symbols().get("calculate").unwrap())
        .unwrap();
    let make = program
        .resolve_code(program.symbols().get("make_callback").unwrap())
        .unwrap();
    let frame = program.bind_code(calculate.into(), []).unwrap();
    let body = unit.code_body(frame.code()).unwrap();
    let mut count = 0;
    for (call, expression) in body.typed().body().expressions().iter() {
        if !matches!(
            expression.kind,
            ExpressionKind::Call {
                target: nocter_model::ValueFunctionOrigin::Contract(_),
                ..
            }
        ) {
            continue;
        }
        let data = expression.ty.access().unwrap().data();
        let Some(PayloadSchema::Function(contract)) = program.data_schema(data) else {
            continue;
        };
        assert!(matches!(
            contract.origin,
            nocter_model::ValueFunctionOrigin::Contract(_)
        ));
        let source = unit
            .value_sources(frame.code())
            .unwrap()
            .nodes()
            .find_map(|(id, node)| {
                (node.expression == Some(call) && matches!(node.value, ValueSource::CallResult(_)))
                    .then_some(id)
            })
            .unwrap();
        let returned = unit
            .instantiate_value_data(
                &frame,
                source,
                data,
                &mut WorkBudget::new(Limits::default().steps()),
            )
            .unwrap();
        let InstanceSchema::Function(actual) = returned.schema(program).unwrap() else {
            panic!("original returned callback");
        };
        assert!(
            matches!(actual.code(), nocter_model::ValueCode::Closure(closure) if closure.owner == make.into())
        );
        count += 1;
    }
    assert_eq!(count, 2);
}

#[test]
fn callable_field_results_do_not_grant_returned_runtime_values_const_capability() {
    let source = "struct Factory { callback: func(): func(i32): i32 }\nfunc runtime(value: i32): i32 { value }\nconst func make(): func(i32): i32 { runtime }\nconst func run(): i32 { let factory = Factory { callback: make }\nlet action = factory.callback()\naction(42) }";
    for required in [false, true] {
        let source = if required {
            format!("{source}\nconst ANSWER: i32 = run()")
        } else {
            source.to_owned()
        };
        let (sources, tree) = input(&source);
        let Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) =
            SourceUnit::prepare(bound(&sources, &tree))
        else {
            panic!("returned runtime values must retain their missing const capability");
        };
        assert!(matches!(
            diagnostic.rule(),
            crate::value_access::constant::Rule::RuntimeCall(_)
        ));
        assert_eq!(
            sources
                .get(tree.source())
                .unwrap()
                .text_at(diagnostic.source().primary().span().range())
                .unwrap(),
            "action(42)"
        );
    }
}

const COMPOUND_FIELD_INPUTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/native/value-access/compound-callable-field-inputs.nct"
));

#[test]
fn compound_field_inputs_preserve_absence_tuple_coordinates_and_array_captures() {
    assert_eq!(answer(COMPOUND_FIELD_INPUTS, "ANSWER"), Value::integer(42));
}

#[test]
fn absent_compound_field_inputs_do_not_grant_runtime_callbacks_const_capability() {
    let source = COMPOUND_FIELD_INPUTS
        .replace("const func identity", "func identity")
        .replace(
            "optional_runner.callback(make(40))",
            "optional_runner.callback(identity)",
        );
    let fixture = nocter_test_support::ValueSourceFixture::new(&source);
    let Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) =
        SourceUnit::prepare(fixture.syntax())
    else {
        panic!("inactive constraints cannot grant a supplied runtime callback const capability");
    };
    assert!(matches!(
        diagnostic.rule(),
        crate::value_access::constant::Rule::RuntimeCall(_)
    ));
}

#[test]
fn zero_length_compound_inputs_retain_their_declared_callback_signature() {
    let source = COMPOUND_FIELD_INPUTS.replace(
        "const func empty(take values: [func(i32): i32; 0])",
        "const func empty(take values: [func(i32): bool; 0])",
    );
    assert_ne!(source, COMPOUND_FIELD_INPUTS);
    let fixture = nocter_test_support::ValueSourceFixture::new(&source);
    let Err(SourceFailure::Checking(Failure::Body {
        failure: crate::value_access::Failure::Rejected(diagnostic),
        ..
    })) = SourceUnit::prepare(fixture.syntax())
    else {
        panic!("zero-byte representation cannot erase a callback signature mismatch");
    };
    assert_eq!(diagnostic.rule(), crate::value_access::Rule::TypeMismatch);
}

const NESTED_CALLABLE_HEADERS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/native/value-access/nested-callable-headers.nct"
));

#[test]
fn nested_headers_keep_original_named_captured_and_returned_higher_order_contexts() {
    assert_eq!(
        answer(NESTED_CALLABLE_HEADERS, "ANSWER"),
        Value::integer(42)
    );
    let interrupted = NESTED_CALLABLE_HEADERS.replace(
        "    consume(second^)",
        "    forward(0, second^, if true { return 42 } else { return 0 })",
    );
    assert_ne!(interrupted, NESTED_CALLABLE_HEADERS);
    assert_eq!(answer(&interrupted, "ANSWER"), Value::integer(42));
}

#[test]
fn nested_header_contracts_preserve_source_mismatch_paths_and_actual_const_failures() {
    let source = "const func identity(value: i32): i32 { value }\nconst func apply(action: func(i32): i32): i32 { action(42) }\nconst func forward(action: func(func(i32): i32): i32): i32 { action(identity) }\nconst ANSWER: i32 = forward(apply)";
    let mismatch = source.replace("func(func(i32): i32): i32", "func(once(i32): i32): i32");
    let fixture = nocter_test_support::ValueSourceFixture::new(&mismatch);
    let Err(SourceFailure::Checking(Failure::Body {
        failure: BodyFailure::Rejected(diagnostic),
        ..
    })) = SourceUnit::prepare(fixture.syntax())
    else {
        panic!("nested invocation contracts are invariant");
    };
    assert_eq!(diagnostic.rule(), crate::value_access::Rule::TypeMismatch);
    assert!(
        diagnostic
            .source()
            .message()
            .contains("nested callable contract")
    );
    let runtime = source.replace("const func identity", "func identity");
    let fixture = nocter_test_support::ValueSourceFixture::new(&runtime);
    let Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) =
        SourceUnit::prepare(fixture.syntax())
    else {
        panic!("a type contract grants no const capability");
    };
    assert!(matches!(
        diagnostic.rule(),
        crate::value_access::constant::Rule::RuntimeCall(_)
    ));
    assert!(diagnostic.source().notes().iter().any(|note| {
        let range = note.origin().span().range();
        &runtime[range.start().get() as usize..range.end().get() as usize] == "identity"
    }));
}
