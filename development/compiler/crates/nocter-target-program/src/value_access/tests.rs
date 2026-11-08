//! Real private source -> declarations -> accepted unit -> static instances.
use super::*;
use nocter_checking::value_access::{
    ArgumentAction, ExpressionKind, accepted::Unit, flow::resources::Disposal,
};
use nocter_declaration_lowering::access_declarations;
use nocter_model::{ValueFunctionOrigin, ValueInvocation};
use nocter_test_support::ValueSourceFixture;
use std::fmt::Write as _;

fn accepted(fixture: &ValueSourceFixture) -> Arc<Unit> {
    let headers = access_declarations::Query::prepare(fixture.syntax()).unwrap();
    Arc::new(Unit::prepare(&headers.bind(fixture.syntax()).unwrap()).unwrap())
}
fn callable(unit: &Unit, name: &str) -> AccessCallableId {
    unit.program()
        .resolve(unit.program().symbols().get(name).unwrap())
        .unwrap()
}
fn close_case(name: &str) -> InstanceProgram {
    let unit = accepted(&ValueSourceFixture::corpus(name));
    let root = callable(&unit, "run");
    InstanceProgram::prepare(unit, root).unwrap()
}
fn instance<'a>(graph: &'a InstanceProgram, name: &str) -> (AccessInstanceId, &'a Instance) {
    let callable = callable(graph.unit(), name);
    graph
        .instances()
        .iter()
        .find(|(_, instance)| instance.key.callable == callable)
        .unwrap()
}

#[test]
fn forwarded_formals_close_to_one_named_dispatch_without_rechecking_bodies() {
    let graph = close_case("a82-callback-forward-read.nct");
    assert_eq!(graph.instances().len(), 4);
    let increment = callable(graph.unit(), "increment");
    for name in ["forward", "apply"] {
        let (id, selected) = instance(&graph, name);
        assert_eq!(selected.key.witnesses.len(), 1);
        let witness = selected.key.witnesses.values().next().unwrap();
        assert_eq!(witness.declaration, increment);
        assert_eq!(witness.invocation, ValueInvocation::Read);
        assert!(std::ptr::eq(
            graph.body(id).unwrap(),
            graph.unit().body(selected.key.callable).unwrap()
        ));
    }
    let (increment_id, _) = instance(&graph, "increment");
    assert_eq!(
        instance(&graph, "apply")
            .1
            .calls
            .values()
            .copied()
            .collect::<Vec<_>>(),
        [increment_id]
    );
}

#[test]
fn recursion_reuses_the_reserved_instance_instead_of_expanding_witness_terms() {
    let graph = close_case("a88-callback-recursive-forward.nct");
    assert_eq!(graph.instances().len(), 3);
    let (apply_id, apply) = instance(&graph, "apply");
    assert!(apply.calls.values().any(|&id| id == apply_id));
    assert_eq!(apply.calls.len(), 2);
}

#[test]
fn equal_signatures_keep_distinct_positions_and_targets() {
    let graph = close_case("a89-callback-two-witnesses.nct");
    let apply = instance(&graph, "apply").1;
    let mut targets = apply
        .calls
        .values()
        .map(|id| graph.instances().get(*id).unwrap().key.callable)
        .collect::<Vec<_>>();
    targets.sort();
    let mut expected = vec![
        callable(graph.unit(), "increment"),
        callable(graph.unit(), "decrement"),
    ];
    expected.sort();
    assert_eq!(targets, expected);
    assert_eq!(apply.key.witnesses.len(), 2);
    assert_ne!(
        apply.key.witnesses.values().next(),
        apply.key.witnesses.values().nth(1)
    );
}

#[test]
fn adapters_specialize_payload_obligations_without_changing_source_move_semantics() {
    let graph = close_case("a84-callback-forward-adapter.nct");
    let (apply_id, apply) = instance(&graph, "apply");
    assert_eq!(
        apply.key.witnesses.values().next().unwrap().invocation,
        ValueInvocation::Take
    );
    let body = graph.body(apply_id).unwrap();
    let (formal, parameter) = graph
        .unit()
        .program()
        .parameters()
        .iter()
        .find(|(_, p)| p.owner() == apply.key.callable)
        .unwrap();
    assert_eq!(
        graph.unit().program().data_copyable(parameter.data()),
        Some(false)
    );
    let payload = apply.data.get(&parameter.data()).unwrap();
    assert_eq!(payload.disposal(), Disposal::Trivial);
    assert_eq!(
        graph.unit().program().data_copyable(payload.data()),
        Some(true)
    );
    let call = body
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(_, expression)| match &expression.kind {
            ExpressionKind::Call {
                target: ValueFunctionOrigin::Parameter(id),
                arguments,
                ..
            } if *id == formal => Some(arguments),
            _ => None,
        })
        .unwrap();
    assert!(matches!(call[0].action, ArgumentAction::ReserveTransfer(_)));
    for (_, selected) in graph.instances().iter() {
        for payload in selected.data.values() {
            assert_ne!(payload.disposal(), Disposal::FunctionEnvironment);
            if let Some(nocter_declarations::value_access::PayloadSchema::Function(witness)) =
                graph.unit().program().data_schema(payload.data())
            {
                assert!(matches!(
                    witness.origin,
                    ValueFunctionOrigin::Declaration(_)
                ));
            }
        }
    }
}

#[test]
fn interrupted_inputs_do_not_make_uninvoked_code_reachable() {
    for case in [
        "a93-callback-interrupted-input.nct",
        "a96-callback-terminated-witness.nct",
    ] {
        let graph = close_case(case);
        assert_eq!(graph.instances().len(), 1);
        assert!(
            graph
                .instances()
                .get(graph.root())
                .unwrap()
                .calls
                .is_empty()
        );
    }
}

#[test]
fn repeated_concrete_bindings_share_one_instance_and_distinct_bindings_do_not() {
    let fixture = ValueSourceFixture::new(
        "func first(value: i32): i32 { value + 1 }\nfunc second(value: i32): i32 { value - 1 }\nfunc apply(action: func(i32): i32): i32 { action(1) }\nfunc run(): i32 { apply(first) + apply(second) + apply(first) }",
    );
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let apply = callable(&unit, "apply");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    assert_eq!(graph.instances().len(), 5);
    assert_eq!(
        graph
            .instances()
            .iter()
            .filter(|(_, i)| i.key.callable == apply)
            .count(),
        2
    );
    let calls = graph
        .instances()
        .get(graph.root())
        .unwrap()
        .calls
        .values()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0], calls[2]);
    assert_ne!(calls[0], calls[1]);
}

#[test]
fn body_validation_is_not_bypassed_for_unused_invalid_definitions() {
    let fixture = ValueSourceFixture::new(
        "func unused(action: func(i32): i32): void { let copy = action }\nfunc run(): i32 { 1 }",
    );
    let headers = access_declarations::Query::prepare(fixture.syntax()).unwrap();
    assert!(Unit::prepare(&headers.bind(fixture.syntax()).unwrap()).is_err());
}

#[test]
fn unbound_callback_roots_cannot_publish_a_partially_concrete_program() {
    let unit = accepted(&ValueSourceFixture::corpus(
        "a94-callback-take-cleanup-on-return.nct",
    ));
    let root = callable(&unit, "apply");
    let formal = unit.program().callables().get(root).unwrap().parameters()[0];
    assert_eq!(
        InstanceProgram::prepare(unit, root).unwrap_err(),
        Failure::UnboundRoot(formal)
    );
}

#[test]
fn data_closure_visits_shared_record_schemas_without_expanding_payload_trees() {
    let mut source = String::from("struct Leaf { value: i32 }\n");
    let mut previous = String::from("Leaf");
    for depth in 0..64 {
        let next = format!("Layer{depth}");
        writeln!(
            source,
            "struct {next} {{ first: {previous}\nsecond: {previous} }}"
        )
        .unwrap();
        previous = next;
    }
    write!(
        source,
        "func run(take input: {previous}): void {{ drop input }}"
    )
    .unwrap();
    let unit = accepted(&ValueSourceFixture::new(&source));
    let root = callable(&unit, "run");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    assert_eq!(graph.instances().len(), 1);
    // 65 nominal schemas, i32, and unit. The conceptual tree has 2^64 leaves.
    assert_eq!(graph.instances().get(graph.root()).unwrap().data.len(), 67);
}

#[test]
fn direct_and_value_invocations_keep_explicit_environment_shape() {
    use nocter_checking::value_access::CallKind;
    for (case, kind, offset) in [
        ("a77-callback-input.nct", CallKind::Direct, 0),
        ("a90-callback-higher-function-value.nct", CallKind::Value, 1),
    ] {
        let graph = close_case(case);
        let root = graph.instances().get(graph.root()).unwrap();
        assert_eq!(root.calls.len(), 1);
        let call = *root.calls.keys().next().unwrap();
        let ExpressionKind::Call {
            kind: selected,
            arguments,
            ..
        } = &graph
            .body(graph.root())
            .unwrap()
            .typed()
            .body()
            .expressions()
            .get(call)
            .unwrap()
            .kind
        else {
            panic!("selected call");
        };
        assert_eq!(*selected, kind);
        assert!(arguments[offset].binding.is_some());
        if offset == 1 {
            assert!(arguments[0].binding.is_none());
        }
        let apply = graph
            .instances()
            .get(*root.calls.values().next().unwrap())
            .unwrap();
        assert_eq!(apply.key.witnesses.len(), 1);
    }
}

#[test]
fn unreachable_but_well_typed_calls_do_not_enter_instance_reachability() {
    let fixture = ValueSourceFixture::new(
        "func identity(value: i32): i32 { value }
func apply(action: func(i32): i32): i32 { action(1) }
func run(): i32 { if false { apply(identity) } else { 42 } }",
    );
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    assert_eq!(graph.instances().len(), 1);
    assert!(
        graph
            .instances()
            .get(graph.root())
            .unwrap()
            .calls
            .is_empty()
    );
}

#[test]
fn recursive_witness_permutations_close_to_distinct_shared_instances() {
    let fixture = ValueSourceFixture::new(
        "func increment(value: i32): i32 { value + 1 }
func decrement(value: i32): i32 { value - 1 }
func bounce(first: func(i32): i32, second: func(i32): i32, done: bool): i32 {
    if done { first(1) + second(1) } else { bounce(second, first, true) }
}
func run(): i32 { bounce(increment, decrement, false) }",
    );
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let bounce = callable(&unit, "bounce");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    assert_eq!(graph.instances().len(), 5);
    let instances = graph
        .instances()
        .iter()
        .filter(|(_, i)| i.key.callable == bounce)
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    for (index, (id, instance)) in instances.iter().enumerate() {
        assert_eq!(instance.calls.len(), 3);
        assert!(
            instance
                .calls
                .values()
                .any(|&target| target == instances[1 - index].0)
        );
        assert!(!instance.calls.values().any(|target| target == id));
    }
}

#[test]
fn callable_results_enter_the_same_instance_graph_with_resolved_payloads() {
    for (case, expected) in [
        ("a99-callable-result-forward.nct", 4),
        ("a102-callable-result-recursive-anchor.nct", 3),
        ("a107-callable-result-callback-input.nct", 4),
    ] {
        let graph = close_case(case);
        assert_eq!(graph.instances().len(), expected);
        let increment = callable(graph.unit(), "increment");
        assert!(
            graph
                .instances()
                .iter()
                .any(|(_, instance)| instance.key.callable == increment)
        );
        for (id, instance) in graph.instances().iter() {
            assert!(graph.body(id).unwrap().typed().results_closed());
            for payload in instance.payloads().values() {
                assert_ne!(payload.disposal(), Disposal::FunctionEnvironment);
                assert!(
                    !matches!(graph.unit().program().data_schema(payload.data()), Some(nocter_declarations::value_access::PayloadSchema::Function(w)) if matches!(w.origin, ValueFunctionOrigin::Result(_)))
                );
            }
        }
    }
}

#[test]
fn interrupted_factory_calls_retain_types_without_reaching_factory_or_result_code() {
    let graph = close_case("a111-callable-result-interruption.nct");
    assert_eq!(graph.instances().len(), 1);
    assert!(
        graph
            .instances()
            .get(graph.root())
            .unwrap()
            .calls
            .is_empty()
    );
}

#[test]
fn returned_once_views_copy_only_after_the_source_result_proof() {
    use nocter_model::ValueInputOperation;
    let graph = close_case("a104-callable-result-once.nct");
    let query = graph.body(graph.root()).unwrap();
    let consuming = query
        .typed()
        .body()
        .expressions()
        .iter()
        .filter_map(|(_, expression)| match &expression.kind {
            ExpressionKind::Call {
                kind: nocter_checking::value_access::CallKind::Value,
                arguments,
                ..
            } => Some(&arguments[0]),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(consuming.len(), 2);
    for argument in consuming {
        assert_eq!(argument.operation, ValueInputOperation::Take);
        assert!(matches!(argument.action, ArgumentAction::CopyPlace(_)));
    }
}

#[test]
fn separate_result_applications_close_to_their_own_factory_and_callback_instances() {
    let graph = close_case("a123-distinct-result-applications.nct");
    assert_eq!(graph.instances().len(), 5);
    let forward = callable(graph.unit(), "forward");
    let producers = graph
        .instances()
        .iter()
        .filter(|(_, i)| i.key.callable == forward)
        .map(|(_, i)| i.key.witnesses.values().next().unwrap().declaration)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        producers,
        std::collections::BTreeSet::from([
            callable(graph.unit(), "increment"),
            callable(graph.unit(), "decrement")
        ])
    );
    let root = graph.instances().get(graph.root()).unwrap();
    assert_eq!(root.calls.len(), 4);
}

#[test]
fn dependent_result_relations_share_the_existing_instance_graph_and_payload_projection() {
    for (case, expected) in [
        ("a124-dependent-result-relay.nct", 4),
        ("a125-dependent-result-recursion.nct", 3),
        ("a126-dependent-result-permutation.nct", 4),
        ("a129-dependent-result-once-transfer.nct", 4),
        ("a132-dependent-result-callback-input.nct", 4),
        ("a135-dependent-result-value-factory.nct", 3),
        ("a136-dependent-result-mut.nct", 3),
    ] {
        let graph = close_case(case);
        assert_eq!(graph.instances().len(), expected, "{case}");
        for (id, instance) in graph.instances().iter() {
            let body = graph.body(id).unwrap();
            assert!(
                instance
                    .payloads()
                    .contains_key(&body.typed().result_data())
            );
            assert!(
                instance
                    .payloads()
                    .values()
                    .all(|payload| payload.disposal() != Disposal::FunctionEnvironment)
            );
        }
    }
}

#[test]
fn interrupted_dependent_application_adds_no_factory_or_callback_instance() {
    let graph = close_case("a131-dependent-result-interruption.nct");
    assert_eq!(graph.instances().len(), 1);
    assert!(
        graph
            .instances()
            .get(graph.root())
            .unwrap()
            .calls
            .is_empty()
    );
}
