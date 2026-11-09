//! Real checked owned environments close through one instance arena and explicit call channels.
use super::*;
use nocter_checking::value_access::{CallKind, LocalOrigin};
use nocter_model::ValueInputOperation as Op;

fn close(source: &str) -> InstanceProgram {
    let unit = accepted(&ValueSourceFixture::new(source));
    let root = callable(&unit, "run");
    InstanceProgram::prepare(unit, root).unwrap()
}
fn root(graph: &InstanceProgram) -> &Instance {
    graph.instances().get(graph.root()).unwrap()
}
fn assert_closed(graph: &InstanceProgram) {
    for (id, instance) in graph.instances().iter() {
        let body = graph.body(id).unwrap();
        assert_eq!(body.typed().code(), instance.key().code());
        assert!(std::ptr::eq(body.typed().program(), graph.unit().program()));
        assert!(instance.data().values().all(|d| {
            d.payload()
                .is_none_or(|p| p.disposal() != Disposal::FunctionEnvironment)
        }));
        for call in instance.calls().values() {
            let target = graph.instances().get(call.target()).unwrap();
            let body = graph.body(call.target()).unwrap();
            assert_eq!(
                target.data()[&body.typed().result_data()].payload(),
                Some(call.result())
            );
            assert_eq!(
                call.inputs().len(),
                body.typed().body().inputs().explicit().len()
            );
        }
    }
}

#[test]
fn complete_owned_capture_corpus_closes_without_retyping_or_symbolic_cleanup() {
    for (case, count) in [
        ("a138-owned-capture-read.nct", 3),
        ("a139-owned-capture-edit.nct", 2),
        ("a140-owned-capture-take.nct", 2),
        ("a141-owned-capture-partial.nct", 2),
        ("a142-owned-capture-result.nct", 5),
        ("a143-owned-capture-interruption.nct", 1),
        ("a144-owned-capture-dead-move.nct", 3),
        ("a145-owned-capture-copy.nct", 2),
        ("a146-owned-capture-copy-once.nct", 2),
        ("a147-owned-capture-input-operations.nct", 2),
    ] {
        let graph = close_case(case);
        assert_eq!(graph.instances().len(), count, "{case}");
        assert_closed(&graph);
    }
}

#[test]
fn binding_an_uninvoked_capture_does_not_instantiate_its_code() {
    let graph = close(
        "func ignore(action: func(): void): void {}\nfunc run(take input: error): void {\nlet action = (input;): void {}\nignore(action)\n}",
    );
    assert_eq!(graph.instances().len(), 2);
    assert!(
        graph
            .instances()
            .iter()
            .all(|(_, i)| matches!(i.key().code(), ValueCode::Declaration(_)))
    );
    let ignored = instance(&graph, "ignore").1;
    assert!(matches!(
        ignored.key().witnesses().values().next().unwrap().code(),
        ValueCode::Closure(_)
    ));
    assert!(ignored.calls().is_empty());
    assert_closed(&graph);
}

#[test]
fn environment_and_explicit_edit_take_inputs_have_independent_frozen_channels() {
    let graph = close_case("a147-owned-capture-input-operations.nct");
    let call = root(&graph).calls().values().next().unwrap();
    assert_eq!(call.kind(), CallKind::Value);
    let body = graph.body(call.target()).unwrap().typed();
    assert_eq!(body.source_owner(), callable(graph.unit(), "run"));
    assert!(matches!(body.code(), ValueCode::Closure(_)));
    let Environment::Capture {
        argument,
        supplied,
        input,
        payload,
        exit,
        ..
    } = call.environment()
    else {
        panic!("captured input")
    };
    assert_eq!(argument, 0);
    assert_eq!(supplied, Op::Read);
    assert_eq!(input.operation, Op::Read);
    assert_eq!(Some(input), body.body().inputs().environment());
    assert_eq!(
        body.body().locals().get(input.local).unwrap().origin,
        LocalOrigin::Capture
    );
    assert_eq!(
        graph.unit().program().data_kind(payload.data()),
        Some(nocter_model::BuiltinType::I32)
    );
    assert_eq!(exit, EnvironmentExit::Retained);
    let inputs = call.inputs();
    assert_eq!(
        (inputs[0].argument, inputs[0].input.operation),
        (1, Op::Edit)
    );
    assert_eq!(
        (inputs[1].argument, inputs[1].input.operation),
        (2, Op::Take)
    );
    assert_eq!(inputs[0].input, body.body().inputs().explicit()[0]);
    assert_eq!(inputs[1].input, body.body().inputs().explicit()[1]);
}

#[test]
fn one_checked_body_is_shared_across_read_update_and_consuming_views() {
    let graph = close(
        "func observe(input: error): void {}\nfunc run(take input: error): void {\nlet action = (input;): void { observe(input) }\naction()\nvar update: mut(): void = action\nupdate()\nlet finish: once(): void = update\nfinish()\n}",
    );
    assert_eq!(graph.instances().len(), 3);
    let calls = root(&graph).calls().values().collect::<Vec<_>>();
    assert_eq!(calls.len(), 3);
    assert!(calls.iter().all(|call| call.target() == calls[0].target()));
    for (call, expected) in calls.iter().zip([Op::Read, Op::Edit, Op::Take]) {
        let Environment::Capture {
            supplied,
            input,
            exit,
            payload,
            ..
        } = call.environment()
        else {
            panic!("capture")
        };
        assert_eq!(supplied, expected);
        assert_eq!(input.operation, Op::Read);
        assert_eq!(
            exit,
            if expected == Op::Take {
                EnvironmentExit::DisposeBeforeResult(payload)
            } else {
                EnvironmentExit::Retained
            }
        );
    }
    assert_closed(&graph);
}

#[test]
fn consuming_adapter_cleans_a_read_environment_before_delivering_the_answer() {
    let graph = close(
        "func observe(input: error): void {}\nfunc apply(take action: once(): void): void { action() }\nfunc run(take input: error): void {\nlet action = (input;): void { observe(input) }\napply(action)\n}",
    );
    let apply = instance(&graph, "apply").1;
    let call = apply.calls().values().next().unwrap();
    let Environment::Capture {
        supplied,
        input,
        exit,
        payload,
        ..
    } = call.environment()
    else {
        panic!("capture")
    };
    assert_eq!(supplied, Op::Take);
    assert_eq!(input.operation, Op::Read);
    assert_eq!(payload.disposal(), Disposal::OwnedError);
    assert_eq!(exit, EnvironmentExit::DisposeBeforeResult(payload));
    assert_eq!(call.result().disposal(), Disposal::Trivial);
    assert_closed(&graph);
}

#[test]
fn consuming_adapter_cleans_the_post_update_environment() {
    let graph = close(
        "func reset(edit target: error, take replacement: error): void { target = replacement }\nfunc apply(take action: once(take error): void, take replacement: error): void { action(replacement) }\nfunc run(take input: error, take replacement: error): void {\nlet action = (input; take next: error): void { reset(input, next) }\napply(action, replacement)\n}",
    );
    let call = instance(&graph, "apply").1.calls().values().next().unwrap();
    let Environment::Capture {
        supplied,
        input,
        exit,
        payload,
        ..
    } = call.environment()
    else {
        panic!("capture")
    };
    assert_eq!(supplied, Op::Take);
    assert_eq!(input.operation, Op::Edit);
    assert_eq!(exit, EnvironmentExit::DisposeBeforeResult(payload));
    assert_eq!(call.inputs()[0].input.operation, Op::Take);
    assert_closed(&graph);
}

#[test]
fn consuming_body_alone_owns_full_or_partial_environment_cleanup() {
    for case in [
        "a140-owned-capture-take.nct",
        "a141-owned-capture-partial.nct",
    ] {
        let graph = close_case(case);
        let call = root(&graph).calls().values().next().unwrap();
        let Environment::Capture {
            supplied,
            input,
            exit,
            ..
        } = call.environment()
        else {
            panic!("capture")
        };
        assert_eq!(supplied, Op::Take);
        assert_eq!(input.operation, Op::Take);
        assert_eq!(exit, EnvironmentExit::BodyOwned);
        assert_eq!(call.result().disposal(), Disposal::OwnedError);
    }
}

#[test]
fn runtime_environment_values_do_not_duplicate_code_instances() {
    let graph = close(
        "func make(value: i32): func(i32): i32 { (value; number: i32): i32 { value + number } }\nfunc run(): i32 {\nlet first = make(1)\nlet second = make(2)\nfirst(0) + second(0)\n}",
    );
    assert_eq!(graph.instances().len(), 3);
    let targets = root(&graph)
        .calls()
        .values()
        .map(Invocation::target)
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 4);
    assert_eq!(targets[0], targets[1]);
    assert_eq!(targets[2], targets[3]);
    assert_ne!(targets[0], targets[2]);
    assert_closed(&graph);
}

#[test]
fn equal_signatures_keep_lexical_producers_distinct_through_generic_inputs() {
    let graph = close(
        "func apply(action: func(): i32): i32 { action() }\nfunc run(): i32 {\nlet value = 1\nlet first = (value;): i32 { value }\nlet second = (value;): i32 { value }\napply(first) + apply(second) + apply(first)\n}",
    );
    assert_eq!(graph.instances().len(), 5);
    let targets = root(&graph)
        .calls()
        .values()
        .map(Invocation::target)
        .collect::<Vec<_>>();
    assert_eq!(targets[0], targets[2]);
    assert_ne!(targets[0], targets[1]);
    let selected = |id| {
        graph
            .instances()
            .get(id)
            .unwrap()
            .key()
            .witnesses()
            .values()
            .next()
            .unwrap()
            .code()
    };
    assert_ne!(selected(targets[0]), selected(targets[1]));
    assert_closed(&graph);
}

#[test]
fn named_and_captured_recursion_share_the_same_reserved_instance_graph() {
    let fixture = ValueSourceFixture::new(
        "func driver(done: bool): i32 {\nlet value = 1\nlet action = (value;): i32 { driver(true) }\nif done { 0 } else { action() }\n}",
    );
    let unit = accepted(&fixture);
    let root = callable(&unit, "driver");
    let graph = InstanceProgram::prepare(unit, root).unwrap();
    assert_eq!(graph.instances().len(), 2);
    let captured = graph
        .instances()
        .get(graph.root())
        .unwrap()
        .calls()
        .values()
        .next()
        .unwrap()
        .target();
    assert_eq!(
        graph
            .instances()
            .get(captured)
            .unwrap()
            .calls()
            .values()
            .next()
            .unwrap()
            .target(),
        graph.root()
    );
    let bridge = graph
        .instances()
        .get(captured)
        .unwrap()
        .calls()
        .values()
        .next()
        .unwrap();
    assert_eq!(bridge.kind(), CallKind::Direct);
    assert_eq!(bridge.environment(), Environment::None);
    assert_eq!(bridge.inputs()[0].argument, 0);
    assert_eq!(
        bridge.inputs()[0].input,
        graph
            .body(graph.root())
            .unwrap()
            .typed()
            .body()
            .inputs()
            .explicit()[0]
    );
    assert_closed(&graph);
}

#[test]
fn recursive_callback_forwarding_preserves_the_captured_witness() {
    let graph = close(
        "func apply(action: func(i32): i32, depth: i32): i32 {\nif depth == 0 { action(0) } else { apply(action, depth - 1) }\n}\nfunc run(): i32 {\nlet value = 4\nlet action = (value; number: i32): i32 { value + number }\napply(action, 2)\n}",
    );
    assert_eq!(graph.instances().len(), 3);
    let (apply_id, apply) = instance(&graph, "apply");
    assert!(apply.calls().values().any(|call| call.target() == apply_id));
    assert!(matches!(
        apply.key().witnesses().values().next().unwrap().code(),
        ValueCode::Closure(_)
    ));
    assert_closed(&graph);
}
