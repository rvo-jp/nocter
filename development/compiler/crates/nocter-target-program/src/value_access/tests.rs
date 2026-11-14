//! Real private source -> declarations -> accepted unit -> static instances.
use super::*;
use nocter_checking::value_access::{
    ArgumentAction, ExpressionKind, accepted::Unit, flow::resources::Disposal,
};
use nocter_declaration_lowering::access_declarations;
use nocter_model::{ValueCode, ValueFunctionOrigin, ValueInvocation};
use nocter_test_support::ValueSourceFixture;
use std::fmt::Write as _;
mod body_channels;
mod captured;
mod invocation_sources;
mod primitives;
mod storage_roots;
mod substitution;

#[test]
fn zero_capture_metadata_does_not_require_a_binding_for_a_trapping_element_projection() {
    let fixture = ValueSourceFixture::new(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/empty-captured-callable-array.nct"
    )));
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    let make = callable(graph.unit(), "make");
    let callback = graph
        .unit()
        .program()
        .codes()
        .get(make)
        .unwrap()
        .parameters()[0];
    let data = graph
        .unit()
        .program()
        .parameters()
        .get(callback)
        .unwrap()
        .data();
    let captured = graph
        .instances()
        .iter()
        .find(|(_, instance)| matches!(instance.key().code(), nocter_model::ValueCode::Closure(_)))
        .unwrap()
        .1;
    assert!(captured.key().witnesses().is_empty());
    assert!(!captured.data().contains_key(&data));
    assert!(
        graph
            .instances()
            .iter()
            .any(|(_, instance)| instance.key().code()
                == nocter_model::ValueCode::Declaration(make)
                && instance.key().witnesses().contains_key(&callback.into()))
    );
    assert_eq!(
        graph
            .instances()
            .iter()
            .filter(
                |(_, instance)| instance.key().code() == nocter_model::ValueCode::Declaration(make)
            )
            .count(),
        2
    );
    storage::prepare(
        &graph,
        nocter_runtime_contract::RuntimeAbiIdentity::Arm64DarwinV1,
        [],
    )
    .unwrap();
}

fn accepted(fixture: &ValueSourceFixture) -> Arc<Unit> {
    Arc::new(
        nocter_checking::value_access::accepted::SourceUnit::prepare(fixture.syntax())
            .unwrap()
            .into_unit(),
    )
}

#[test]
fn uninvoked_owned_capture_projects_its_payload_without_creating_a_code_instance() {
    let fixture = ValueSourceFixture::new(
        "func run(take input: error): void {\nlet action = (input^;): void {}\n}",
    );
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    assert_eq!(graph.instances().len(), 1);
    let instance = graph.instances().get(graph.root()).unwrap();
    assert!(instance.calls().is_empty());
    assert!(instance.data().values().any(|d| {
        d.payload()
            .is_some_and(|p| p.disposal() == Disposal::OwnedCapture)
    }));
}

#[test]
fn captured_code_invocation_selects_its_checked_body() {
    let fixture = ValueSourceFixture::new(
        "func run(): i32 {\nlet value = 1\nlet action = (value;): i32 { value }\naction()\n}",
    );
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    assert_eq!(graph.instances().len(), 2);
    let edge = graph
        .instances()
        .get(graph.root())
        .unwrap()
        .calls()
        .values()
        .next()
        .unwrap();
    assert!(matches!(
        graph
            .instances()
            .get(edge.target().instance().unwrap())
            .unwrap()
            .key()
            .code(),
        ValueCode::Closure(_)
    ));
}
fn callable(unit: &Unit, name: &str) -> AccessCodeId {
    unit.program()
        .resolve(unit.program().symbols().get(name).unwrap())
        .unwrap()
}

#[test]
fn a_required_expression_cannot_substitute_its_source_owners_runtime_code() {
    let fixture = ValueSourceFixture::new("func run(): i32 { 42 }");
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let code = ValueCode::Expression(nocter_model::ValueExpression {
        owner: root,
        slot: 0,
    });
    assert_eq!(code.source_owner(), root);
    assert_eq!(code.origin(), None);
    assert!(unit.code_body(code).is_none());
    let key = unit.program().bind_code(code, []).unwrap();
    let mut instances = nocter_model::ArenaBuilder::new();
    let root = instances.insert(Instance {
        key,
        calls: BTreeMap::new(),
        data: BTreeMap::new(),
        inputs: BTreeMap::new(),
        result: DataEvidence::Unproduced,
        prepared: BTreeMap::new(),
    });
    let graph = InstanceProgram {
        unit,
        root,
        instances: instances.finish(),
    };
    assert!(graph.body(root).is_none());
    assert!(crate::value_access::lifecycle::Program::prepare(Arc::new(graph)).is_err());
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
        .find(|(_, instance)| instance.key.code() == ValueCode::Declaration(callable))
        .unwrap()
}

#[test]
fn forwarded_formals_close_to_one_named_dispatch_without_rechecking_bodies() {
    let graph = close_case("a82-callback-forward-read.nct");
    assert_eq!(graph.instances().len(), 4);
    let increment = callable(graph.unit(), "increment");
    for name in ["forward", "apply"] {
        let (id, selected) = instance(&graph, name);
        assert_eq!(selected.key.witnesses().len(), 1);
        let witness = selected.key.witnesses().values().next().unwrap();
        assert_eq!(witness.code(), ValueCode::Declaration(increment));
        assert_eq!(witness.invocation(), ValueInvocation::Read);
        assert!(std::ptr::eq(
            graph.body(id).unwrap(),
            graph.unit().code_body(selected.key.code()).unwrap()
        ));
    }
    let (increment_id, _) = instance(&graph, "increment");
    assert_eq!(
        instance(&graph, "apply")
            .1
            .calls
            .values()
            .map(|call| call.target().instance().unwrap())
            .collect::<Vec<_>>(),
        [increment_id]
    );
}

#[test]
fn recursion_reuses_the_reserved_instance_instead_of_expanding_witness_terms() {
    let graph = close_case("a88-callback-recursive-forward.nct");
    assert_eq!(graph.instances().len(), 3);
    let (apply_id, apply) = instance(&graph, "apply");
    assert!(
        apply
            .calls
            .values()
            .any(|call| call.target().instance().unwrap() == apply_id)
    );
    assert_eq!(apply.calls.len(), 2);
}

#[test]
fn equal_signatures_keep_distinct_positions_and_targets() {
    let graph = close_case("a89-callback-two-witnesses.nct");
    let apply = instance(&graph, "apply").1;
    let mut targets = apply
        .calls
        .values()
        .map(|call| {
            graph
                .instances()
                .get(call.target().instance().unwrap())
                .unwrap()
                .key
                .code()
        })
        .collect::<Vec<_>>();
    targets.sort();
    let mut expected = vec![
        ValueCode::Declaration(callable(graph.unit(), "increment")),
        ValueCode::Declaration(callable(graph.unit(), "decrement")),
    ];
    expected.sort();
    assert_eq!(targets, expected);
    assert_eq!(apply.key.witnesses().len(), 2);
    assert_ne!(
        apply.key.witnesses().values().next(),
        apply.key.witnesses().values().nth(1)
    );
}

#[test]
fn adapters_specialize_payload_obligations_without_changing_source_move_semantics() {
    let graph = close_case("a84-callback-forward-adapter.nct");
    let (apply_id, apply) = instance(&graph, "apply");
    assert_eq!(
        apply.key.witnesses().values().next().unwrap().invocation(),
        ValueInvocation::Take
    );
    let body = graph.body(apply_id).unwrap();
    let (formal, parameter) = graph
        .unit()
        .program()
        .parameters()
        .iter()
        .find(|(_, p)| {
            p.owner()
                == match apply.key.code() {
                    ValueCode::Declaration(id) => id,
                    ValueCode::Closure(_) | ValueCode::Expression(_) => panic!("named body"),
                }
        })
        .unwrap();
    assert_eq!(
        graph.unit().program().data_copyable(parameter.data()),
        Some(false)
    );
    let payload = apply
        .data
        .get(&parameter.data())
        .unwrap()
        .payload()
        .unwrap();
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
            } if *id == formal.into() => Some(arguments),
            _ => None,
        })
        .unwrap();
    assert!(matches!(
        call[0].action,
        ArgumentAction::ReserveTransfer { .. }
    ));
    for (_, selected) in graph.instances().iter() {
        for evidence in selected.data.values() {
            let payload = evidence.payload().unwrap();
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
fn unknown_interrupted_result_contracts_do_not_acquire_payloads_or_instance_edges() {
    let unit = accepted(&ValueSourceFixture::corpus(
        "a137-dependent-result-unknown-input.nct",
    ));
    let root = callable(&unit, "invalid");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    assert_eq!(graph.instances().len(), 1);
    let instance = graph.instances().get(graph.root()).unwrap();
    assert!(instance.calls().is_empty());
    let absent = instance
        .data()
        .iter()
        .filter_map(|(&data, evidence)| match evidence {
            DataEvidence::Unproduced => Some(data),
            DataEvidence::Payload(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(absent.len(), 1);
    let Some(nocter_declarations::value_access::PayloadSchema::Function(witness)) =
        graph.unit().program().data_schema(absent[0])
    else {
        panic!("callable shape")
    };
    assert_eq!(
        graph
            .unit()
            .program()
            .function_contract(witness)
            .unwrap()
            .inputs
            .len(),
        1
    );
    assert!(flow::resources::payload(graph.unit().program(), absent[0]).is_err());
}

#[test]
fn dead_adapted_locals_keep_contract_only_data_through_instance_projection() {
    let source = ValueSourceFixture::new(
        "func forward(take action: func(i32): i32): func(i32): i32 { action^ }\nfunc run(): i32 {\nlet absent = forward(if true { return 1 } else { return 2 })\nlet adapted: once(i32): i32 = absent\nadapted(1)\n}",
    );
    let unit = accepted(&source);
    let root = callable(&unit, "run");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    assert_eq!(graph.instances().len(), 1);
    let instance = graph.instances().get(graph.root()).unwrap();
    assert!(instance.calls().is_empty());
    assert_eq!(
        instance
            .data()
            .values()
            .filter(|e| matches!(e, DataEvidence::Unproduced))
            .count(),
        2
    );
}

#[test]
fn interrupted_structural_shapes_do_not_gain_storage_or_callee_instances() {
    let fixture = ValueSourceFixture::new(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/structural-result-values.nct"
    )));
    let unit = accepted(&fixture);
    let root = callable(&unit, "interrupted");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    assert_eq!(graph.instances().len(), 1);
    let instance = graph.instances().get(graph.root()).unwrap();
    assert!(instance.calls().is_empty());
    let absent = instance
        .data()
        .iter()
        .filter_map(|(&data, evidence)| {
            matches!(evidence, DataEvidence::Unproduced).then_some(data)
        })
        .collect::<Vec<_>>();
    assert!(absent.iter().any(|&data| matches!(
        graph.unit().program().data_schema(data),
        Some(nocter_declarations::value_access::PayloadSchema::Array(_))
    )));
    assert!(
        absent
            .iter()
            .all(|&data| flow::resources::payload(graph.unit().program(), data).is_err())
    );
    storage::prepare(
        &graph,
        nocter_runtime_contract::RuntimeAbiIdentity::Arm64DarwinV1,
        [],
    )
    .unwrap();
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
            .filter(|(_, i)| i.key.code() == ValueCode::Declaration(apply))
            .count(),
        2
    );
    let calls = graph
        .instances()
        .get(graph.root())
        .unwrap()
        .calls
        .values()
        .map(|call| call.target().instance().unwrap())
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
    let formal = unit.program().codes().get(root).unwrap().parameters()[0];
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
    // The body map owns only its actual input and result. Shared descendants stay in the
    // canonical data graph instead of being flattened into a second source-type inventory.
    assert_eq!(graph.instances().get(graph.root()).unwrap().data.len(), 2);
    let prepared = storage::prepare(
        &graph,
        nocter_runtime_contract::RuntimeAbiIdentity::Arm64DarwinV1,
        [],
    )
    .unwrap();
    let runtime = prepared.environment();
    // 65 nominal schemas, i32, and unit. The conceptual tree has 2^64 leaves.
    assert_eq!(runtime.types().iter().count(), 67);
    let mut aggregates = 0;
    for (_, representation) in runtime.type_representations().iter() {
        let nocter_runtime_contract::RuntimeTypeRepresentation::ValueAggregate { fields } =
            representation
        else {
            panic!("record representation");
        };
        aggregates += 1;
        if fields.len() == 2 {
            assert_eq!(fields[0].ty(), fields[1].ty());
        } else {
            assert_eq!(fields.len(), 1);
        }
    }
    assert_eq!(aggregates, 65);
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
            .get(
                root.calls
                    .values()
                    .next()
                    .unwrap()
                    .target()
                    .instance()
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(apply.key.witnesses().len(), 1);
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
        .filter(|(_, i)| i.key.code() == ValueCode::Declaration(bounce))
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    for (index, (id, instance)) in instances.iter().enumerate() {
        assert_eq!(instance.calls.len(), 3);
        assert!(
            instance
                .calls
                .values()
                .any(|call| call.target().instance().unwrap() == instances[1 - index].0)
        );
        assert!(
            !instance
                .calls
                .values()
                .any(|call| call.target().instance().unwrap() == *id)
        );
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
                .any(|(_, instance)| instance.key.code() == ValueCode::Declaration(increment))
        );
        for (id, instance) in graph.instances().iter() {
            assert!(graph.body(id).unwrap().typed().results_closed());
            for evidence in instance.data().values() {
                let payload = evidence.payload().expect("produced payload contract");
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
        .filter(|(_, i)| i.key.code() == ValueCode::Declaration(forward))
        .map(|(_, i)| i.key.witnesses().values().next().unwrap().code())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        producers,
        std::collections::BTreeSet::from([
            ValueCode::Declaration(callable(graph.unit(), "increment")),
            ValueCode::Declaration(callable(graph.unit(), "decrement"))
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
            assert!(instance.data().contains_key(&body.typed().result_data()));
            assert!(instance.data().values().all(|evidence| {
                evidence
                    .payload()
                    .is_some_and(|payload| payload.disposal() != Disposal::FunctionEnvironment)
            }));
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
mod lifecycle;

#[test]
fn required_initializers_are_not_function_values_or_runtime_instance_roots() {
    let fixture = ValueSourceFixture::new(
        "const func calculate(): i32 { 42 }\nconst ANSWER: i32 = calculate()\nfunc run(): i32 { ANSWER }",
    );
    let unit = accepted(&fixture);
    let initializer = unit
        .program()
        .resolve_constant(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert!(
        matches!(InstanceProgram::prepare(unit.clone(), initializer), Err(Failure::InitializerRoot(id)) if id == initializer)
    );
    let run = callable(&unit, "run");
    let graph = InstanceProgram::prepare(unit, run).unwrap();
    assert_eq!(graph.instances().len(), 1);
    assert!(
        graph
            .instances()
            .get(graph.root())
            .unwrap()
            .calls()
            .is_empty()
    );
}

#[test]
fn empty_array_storage_has_identity_without_an_element_payload_edge() {
    for source in [
        "func run(): [i32; 0] { [] }",
        "struct Node { next: [Node; 0] }\nfunc run(): [Node; 0] { [] }",
    ] {
        let fixture = ValueSourceFixture::new(source);
        let unit = accepted(&fixture);
        let root = callable(&unit, "run");
        let graph = Arc::new(InstanceProgram::prepare(unit, root).unwrap());
        super::lifecycle::Program::prepare(graph.clone()).unwrap();
        let prepared = storage::prepare(
            &graph,
            nocter_runtime_contract::RuntimeAbiIdentity::Arm64DarwinV1,
            [],
        )
        .unwrap();
        let runtime = prepared.environment();
        let arrays: Vec<_> = runtime
            .types()
            .iter()
            .filter_map(|(data, kind)| match kind {
                nocter_runtime_contract::RuntimeType::EmptyArray { element } => {
                    Some((data, *element))
                }
                _ => None,
            })
            .collect();
        assert_eq!(arrays.len(), 1);
        let (data, element) = arrays[0];
        assert!(runtime.types().get(element).is_none());
        assert!(runtime.type_representations().get(data).is_none());
        assert!(
            matches!(graph.unit().program().data_schema(data.source().unwrap()), Some(nocter_declarations::value_access::PayloadSchema::Array(array)) if Some(array.element()) == element.source() && array.is_empty())
        );
    }
}

#[test]
fn nonempty_array_storage_keeps_one_homogeneous_element_and_count() {
    let fixture = ValueSourceFixture::new(
        "func pass(take values: [i32; 2]): [i32; 2] { values^ }\nfunc run(): [i32; 2] { pass([1, 2]) }",
    );
    let unit = accepted(&fixture);
    let root = callable(&unit, "run");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    let prepared = storage::prepare(
        &graph,
        nocter_runtime_contract::RuntimeAbiIdentity::Arm64DarwinV1,
        [],
    )
    .unwrap();
    let runtime = prepared.environment();
    let arrays: Vec<_> = runtime
        .types()
        .iter()
        .filter_map(|(id, shape)| match shape {
            nocter_runtime_contract::RuntimeType::FixedArray { element, length } => {
                Some((id, *element, *length))
            }
            _ => None,
        })
        .collect();
    assert_eq!(arrays.len(), 1);
    let (id, element, length) = arrays[0];
    assert_eq!(length, 2);
    assert_eq!(
        runtime.types().get(element),
        Some(&nocter_runtime_contract::RuntimeType::Primitive(
            nocter_runtime_contract::RuntimePrimitive::Signed(32)
        ))
    );
    assert!(runtime.type_representations().get(id).is_none());
}

mod anonymous_inputs;
