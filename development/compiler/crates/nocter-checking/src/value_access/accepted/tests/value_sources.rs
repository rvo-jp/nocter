//! Source SSA records exact definition/selection provenance without executing values.
use super::*;
use crate::value_access::{
    CallKind, ExpressionKind, ValueSource, ValueSourceId, ValueSources, ir::LocalOrigin,
};
use nocter_model::{ValueApplication, ValueCode, ValuePath, ValueProjection};
use std::collections::BTreeSet;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/native/value-access/capture-producers.nct"
));
fn code(unit: &Unit, name: &str) -> ValueCode {
    unit.program()
        .resolve_code(unit.program().symbols().get(name).unwrap())
        .unwrap()
        .into()
}
fn formation(
    unit: &Unit,
    owner: ValueCode,
) -> (nocter_model::ValueClosure, &ValueSources, ValueSourceId) {
    let graph = unit.value_sources(owner).unwrap();
    let closure = unit
        .code_body(owner)
        .unwrap()
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(_, expression)| match expression.kind {
            ExpressionKind::Closure { closure, .. } => Some(closure),
            _ => None,
        })
        .unwrap();
    (closure, graph, graph.creation(closure).unwrap())
}
fn edges(value: &ValueSource) -> Vec<ValueSourceId> {
    match value {
        ValueSource::Merge { inputs, .. } | ValueSource::Alternatives(inputs) => {
            inputs.iter().copied().collect()
        }
        ValueSource::Select { source, .. }
        | ValueSource::Adapt { source }
        | ValueSource::Index { source, .. }
        | ValueSource::CallWrite { source, .. } => vec![*source],
        ValueSource::Closure { environment, .. } => vec![*environment],
        ValueSource::Aggregate(fields) => fields.iter().map(|(_, value)| *value).collect(),
        ValueSource::Repeat { seed } => vec![*seed],
        ValueSource::Replace { source, value, .. } => vec![*source, *value],
        ValueSource::Empty
        | ValueSource::Input { .. }
        | ValueSource::Function(_)
        | ValueSource::CallResult(_)
        | ValueSource::Opaque => vec![],
    }
}
fn reachable_calls(graph: &ValueSources, root: ValueSourceId) -> BTreeSet<ValueApplication> {
    let mut pending = vec![root];
    let mut seen = BTreeSet::new();
    let mut calls = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let node = graph.node(id).unwrap();
        if let ValueSource::CallResult(site) = node.value {
            calls.insert(site);
        }
        pending.extend(edges(&node.value));
    }
    calls
}
fn factories(unit: &Unit, owner: ValueCode) -> Vec<ValueApplication> {
    let maker = code(unit, "make").origin().unwrap();
    unit.code_body(owner)
        .unwrap()
        .typed()
        .body()
        .expressions()
        .iter()
        .filter_map(|(call, expression)| {
            matches!(&expression.kind, ExpressionKind::Call { target, .. } if *target == maker)
                .then_some(ValueApplication { owner, call })
        })
        .collect()
}

#[test]
fn repeated_element_updates_preserve_declaration_closed_callable_metadata() {
    let unit = accepted(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/const-named-values.nct"
    )));
    let owner = code(&unit, "closure_containers");
    let frame = unit.program().bind_code(owner, []).unwrap();
    let graph = unit.value_sources(owner).unwrap();
    let body = unit.code_body(owner).unwrap().typed().body();
    let mut count = 0;
    for (call, _) in body.expressions().iter() {
        if graph.call(call).is_none() {
            continue;
        }
        let mut work = nocter_constant_evaluation::state::WorkBudget::new(
            std::num::NonZeroU64::new(10_000).unwrap(),
        );
        unit.instantiate_invocation(&frame, ValueApplication { owner, call }, &mut work)
            .unwrap_or_else(|failure| panic!("{call:?}: {failure:?}"));
        count += 1;
    }
    assert_eq!(count, 7);
}

#[test]
fn original_source_values_publish_canonical_data_without_publishing_empty_sources() {
    let unit = accepted(SOURCE);
    let mut checked = 0;
    for name in [
        "captured_components",
        "prepared_snapshot",
        "repeated_creation",
    ] {
        let owner = code(&unit, name);
        let frame = unit.program().bind_code(owner, []).unwrap();
        let graph = unit.value_sources(owner).unwrap();
        for (value, node) in graph.nodes() {
            if !matches!(
                node.value,
                ValueSource::Aggregate(_)
                    | ValueSource::Repeat { .. }
                    | ValueSource::CallResult(_)
                    | ValueSource::Replace { .. }
            ) {
                continue;
            }
            let Some(source) = node.data else {
                continue;
            };
            let expected = unit.program().instantiate_closed_data(source).unwrap();
            let mut work = nocter_constant_evaluation::state::WorkBudget::new(
                std::num::NonZeroU64::new(10_000).unwrap(),
            );
            assert_eq!(
                unit.instantiate_value_data(&frame, value, source, &mut work)
                    .unwrap(),
                expected
            );
            checked += 1;
        }
        let (empty, _) = graph
            .nodes()
            .find(|(_, node)| node.value == ValueSource::Empty)
            .unwrap();
        let data = unit
            .program()
            .data_type(nocter_model::BuiltinType::I32)
            .unwrap();
        let mut work = nocter_constant_evaluation::state::WorkBudget::new(
            std::num::NonZeroU64::new(10_000).unwrap(),
        );
        assert_eq!(
            unit.instantiate_value_data(&frame, empty, data, &mut work),
            Err(crate::value_access::InstantiationFailure::Unresolved)
        );
    }
    assert!(checked >= 10);
}
#[test]
fn a_formation_retains_prior_definitions_without_absorbing_a_later_write() {
    let unit = accepted(SOURCE);
    let owner = code(&unit, "captured_components");
    let (_, graph, capture) = formation(&unit, owner);
    assert_eq!(graph.owner(), owner);
    let calls = factories(&unit, owner);
    assert_eq!(calls.len(), 4);
    assert_eq!(
        reachable_calls(graph, capture),
        calls[..3].iter().copied().collect()
    );
    assert!(!reachable_calls(graph, capture).contains(&calls[3]));
    assert!(graph.nodes().any(|(_, node)| matches!(&node.value, ValueSource::Replace { path, weak: false, .. } if path.fields() == [ValueProjection::Tuple(0)])));
}
fn captured_paths(graph: &ValueSources, root: ValueSourceId) -> BTreeSet<ValuePath<()>> {
    let mut pending = vec![(root, ValuePath::default())];
    let mut seen = BTreeSet::new();
    let mut paths = BTreeSet::new();
    while let Some((id, path)) = pending.pop() {
        if !seen.insert((id, path.clone())) {
            continue;
        }
        match &graph.node(id).unwrap().value {
            ValueSource::Input {
                origin: LocalOrigin::Capture,
                ..
            } => {
                paths.insert(path);
            }
            ValueSource::Select {
                source,
                path: prefix,
            } => pending.push((*source, prefix.concat(&path))),
            value => pending.extend(edges(value).into_iter().map(|id| (id, path.clone()))),
        }
    }
    paths
}
#[test]
fn received_capture_components_keep_distinct_paths_before_prepared_invocation() {
    let unit = accepted(SOURCE);
    let (closure, _, _) = formation(&unit, code(&unit, "captured_components"));
    let owner = ValueCode::Closure(closure);
    let graph = unit.value_sources(owner).unwrap();
    let body = unit.code_body(owner).unwrap().typed().body();
    let mut paths = BTreeSet::new();
    for (id, expression) in body.expressions().iter() {
        if !matches!(
            expression.kind,
            ExpressionKind::Call {
                kind: CallKind::Value,
                ..
            }
        ) {
            continue;
        }
        let bundles = graph.call(id).unwrap();
        assert_eq!(bundles.len(), 1);
        paths.extend(captured_paths(graph, bundles[0].callee));
    }
    assert_eq!(
        paths,
        [0, 1]
            .map(|index| ValuePath::new([ValueProjection::Tuple(index)]))
            .into()
    );
}
#[test]
fn loop_sources_close_through_sparse_merge_edges_without_unrolling_iterations() {
    let unit = accepted(SOURCE);
    let owner = code(&unit, "repeated_creation");
    let (_, graph, capture) = formation(&unit, owner);
    let calls = factories(&unit, owner);
    assert_eq!(calls.len(), 2);
    assert_eq!(reachable_calls(graph, capture), calls.into_iter().collect());
    let mut merges = BTreeSet::new();
    for (_, node) in graph.nodes() {
        if let ValueSource::Merge { point, slot, .. } = node.value {
            assert!(merges.insert((point, slot)));
        }
    }
    let body = unit.code_body(owner).unwrap();
    let slots = body.typed().body().locals().len() + body.typed().body().expressions().len();
    assert!(merges.len() <= body.graph().points().len() * slots);
}
#[test]
fn repetitions_retain_one_seed_source_independently_of_declared_cardinality() {
    let count = |text| {
        let unit = accepted(text);
        let graph = unit.value_sources(code(&unit, "run")).unwrap();
        assert_eq!(
            graph
                .nodes()
                .filter(|(_, node)| matches!(node.value, ValueSource::Repeat { .. }))
                .count(),
            1
        );
        graph.nodes().len()
    };
    assert_eq!(
        count("func run(): void { let _ = [0; 1] }"),
        count("func run(): void { let _ = [0; 18446744073709551615] }")
    );
}

#[test]
fn prepared_arguments_keep_their_source_before_later_argument_effects() {
    let unit = accepted(SOURCE);
    let owner = code(&unit, "prepared_snapshot");
    let graph = unit.value_sources(owner).unwrap();
    let calls = factories(&unit, owner);
    assert_eq!(calls.len(), 2);
    let target = code(&unit, "consume_prepared").origin().unwrap();
    let call = unit.code_body(owner).unwrap().typed().body().expressions().iter().find_map(|(id, expression)| {
        matches!(&expression.kind, ExpressionKind::Call { target: callee, .. } if *callee == target).then_some(id)
    }).unwrap();
    let bundles = graph.call(call).unwrap();
    assert_eq!(bundles.len(), 1);
    assert_eq!(
        reachable_calls(graph, bundles[0].arguments[0]),
        BTreeSet::from([calls[0]])
    );
    let answer = unit
        .program()
        .resolve_constant(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(answer),
        Some(&nocter_constant_evaluation::state::Value::integer(42).freeze())
    );
}
#[test]
fn a_value_graph_cannot_be_rebound_after_same_typed_source_effects_change() {
    let (sources, tree) = input(SOURCE);
    let syntax = bound(&sources, &tree);
    let headers = declarations::Query::prepare(syntax).unwrap();
    let context = headers.bind(syntax).unwrap();
    let unit = Unit::prepare(&context).unwrap();
    let original = unit
        .value_sources(code(&unit, "captured_components"))
        .unwrap();
    assert!(original.nodes().any(|(_, node)| matches!(&node.value, ValueSource::Replace { path, .. } if path.fields() == [ValueProjection::Tuple(0)])));
    let changed = SOURCE.replace("alias.0 = make(44)", "alias.1 = make(44)");
    let (sources, tree) = input(&changed);
    let current = headers.bind(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        unit.bind(&current),
        Err(crate::value_access::Failure::ChangedBody)
    ));
    let fresh = Unit::prepare(&current).unwrap();
    assert!(fresh.value_sources(code(&fresh, "captured_components")).unwrap().nodes().any(|(_, node)| matches!(&node.value, ValueSource::Replace { path, .. } if path.fields() == [ValueProjection::Tuple(1)])));
}

#[test]
fn original_action_locations_and_erased_array_writes_do_not_supply_numeric_proofs() {
    let unit = accepted(
        "func run(index: usize): void { var items = [1, 2]
items[index] = 3 }",
    );
    let owner = code(&unit, "run");
    let graph = unit.value_sources(owner).unwrap();
    let body = unit.code_body(owner).unwrap();
    let mut array_writes = 0;
    for (_, node) in graph.nodes() {
        if let Some(origin) = node.origin {
            assert!(
                body.graph()
                    .points()
                    .get(origin.point)
                    .unwrap()
                    .actions
                    .get(origin.action)
                    .is_some()
            );
        }
        if let ValueSource::Replace { path, weak, .. } = &node.value
            && path
                .fields()
                .iter()
                .any(|field| matches!(field, ValueProjection::Array(())))
        {
            assert!(*weak);
            array_writes += 1;
            let origin = node.origin.unwrap();
            assert!(matches!(
                body.graph().points().get(origin.point).unwrap().actions[origin.action].operation,
                Operation::Assign { .. }
            ));
        }
    }
    assert_eq!(array_writes, 1);
}
#[test]
fn a_computed_edit_input_writes_its_temporary_without_reinterpreting_a_source_place() {
    let unit = accepted(
        "func touch(edit value: i32): void { value += 1 }
func run(edit value: i32): void { touch(value)
touch(40 + 2) }",
    );
    let owner = code(&unit, "run");
    let graph = unit.value_sources(owner).unwrap();
    let body = unit.code_body(owner).unwrap().typed().body();
    let computed = body
        .expressions()
        .iter()
        .find_map(|(id, expression)| match &expression.kind {
            ExpressionKind::Call { arguments, .. }
                if matches!(
                    arguments[0].action,
                    crate::value_access::ArgumentAction::AccessOwnedValue
                ) =>
            {
                Some(id)
            }
            _ => None,
        })
        .unwrap();
    let bundles = graph.call(computed).unwrap();
    assert_eq!(bundles.len(), 1);
    let bundle = &bundles[0];
    assert!(
        matches!(unit.code_body(owner).unwrap().graph().points().get(bundle.origin.point).unwrap().actions[bundle.origin.action].operation, Operation::CommitCall(id) if id == computed)
    );
    assert!(graph.nodes().any(|(_, node)| matches!(&node.value, ValueSource::CallWrite { call, source, path, .. } if *call == computed && *source == bundle.arguments[0] && path.is_root())));
    assert_eq!(
        graph
            .nodes()
            .filter(|(_, node)| matches!(node.value, ValueSource::CallWrite { .. }))
            .count(),
        2
    );
}
