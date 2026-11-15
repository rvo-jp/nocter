//! Ordinary callable storage keeps exact contexts through retirement, receipt and aliases.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value;

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
            "let first = pair.0[index](20)",
            "read held = pair.0[index]\n    let first = held(20)",
        )
        .replace(
            "let second = pair.0[index](8)",
            "let _ = held(20)\n    let second = pair.0[index](8)",
        );
    assert_ne!(changed, source);
    let (sources, tree) = input(&changed);
    let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
    assert!(
        matches!(failure, SourceFailure::Checking(Failure::Flow { .. })),
        "{failure:?}"
    );
}
