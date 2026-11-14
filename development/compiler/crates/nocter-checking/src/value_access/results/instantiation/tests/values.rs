//! Original value occurrences select full contexts; metadata graphs confer no source admission.
use super::*;
use crate::value_access::{
    CallKind, CallValueSources, ValueSource, ValueSourceId, ValueSources, flow, ir::LocalOrigin,
};
use nocter_model::{ValueField, ValueProjection};

fn id(index: usize) -> ValueSourceId {
    ValueSourceId::test(index)
}
fn bundle(
    callee: usize,
    arguments: &[usize],
    parameters: &[(ValueParameter, usize)],
) -> CallValueSources {
    let mut points = ArenaBuilder::new();
    CallValueSources {
        kind: CallKind::Direct,
        invocation: ValueInvocation::Read,
        parameters: parameters.iter().copied().collect(),
        origin: flow::Location {
            point: points.insert(()),
            action: 0,
        },
        callee: id(callee),
        arguments: arguments.iter().copied().map(id).collect(),
    }
}
fn insert_graph(evidence: &mut Evidence, graph: ValueSources, calls: &[AccessExpressionId]) {
    // The public call query still requires a matched original commit. Deliberately empty legacy
    // recipes ensure that an occurrence-backed query cannot silently use a code-only fallback.
    for &call in calls {
        if graph.call(call).is_some() {
            evidence.invocations.insert(
                ValueApplication {
                    owner: graph.owner(),
                    call,
                },
                Arc::new(InvocationRecipe {
                    callees: BTreeSet::new(),
                    inputs: BTreeMap::new(),
                }),
            );
        }
    }
    evidence.value_sources.insert(graph.owner(), graph);
}

#[test]
fn received_components_preserve_different_contexts_and_ambiguous_values_cannot_choose_one() {
    let fixture = fixture_with_shape(1, false);
    let program = &fixture.program;
    let Origin::Closure(outer) = fixture.captured.origin else {
        unreachable!()
    };
    let &wrapper = fixture.supporting.keys().next().unwrap();
    let wrapper = FunctionWitness {
        origin: Origin::Closure(wrapper),
        invocation: ValueInvocation::Read,
    };
    let (environment, leaves) = creations::mixed_environment(&fixture, outer, wrapper);
    let actual = program
        .bind_callable_data(fixture.captured, &environment)
        .unwrap();
    let frame = program.bind_instance(&actual, []).unwrap();
    let mut locals = ArenaBuilder::new();
    let nodes = vec![
        (
            Some(environment.data()),
            ValueSource::Input {
                local: locals.insert(()),
                origin: LocalOrigin::Capture,
            },
        ),
        (
            program.function_data(wrapper),
            ValueSource::Select {
                source: id(0),
                path: ValuePath::new([ValueProjection::Tuple(0)]),
            },
        ),
        (
            program.function_data(wrapper),
            ValueSource::Select {
                source: id(0),
                path: ValuePath::new([ValueProjection::Tuple(1)]),
            },
        ),
        (
            program.function_data(wrapper),
            ValueSource::Alternatives(BTreeSet::from([id(1), id(2)])),
        ),
        (
            program.function_data(wrapper),
            ValueSource::CallWrite {
                call: fixture.sites[0],
                argument: 0,
                source: id(1),
                path: ValuePath::default(),
            },
        ),
    ];
    let graph = ValueSources::test(
        frame.code(),
        nodes,
        BTreeMap::from([
            (fixture.sites[0], vec![bundle(1, &[], &[])]),
            (fixture.sites[1], vec![bundle(2, &[], &[])]),
        ]),
        BTreeSet::new(),
    );
    let mut evidence = Evidence::default();
    insert_graph(&mut evidence, graph, &fixture.sites[..2]);
    for (position, leaf) in leaves.iter().enumerate() {
        let nocter_declarations::value_access::InstanceSchema::Function(expected) =
            leaf.schema(program).unwrap()
        else {
            panic!("function leaf")
        };
        assert_eq!(
            value(program, &evidence, &frame, id(position + 1), &mut budget()).unwrap(),
            expected
        );
        let site = ValueApplication {
            owner: frame.code(),
            call: fixture.sites[position],
        };
        assert_eq!(
            call(program, &evidence, &frame, site, &mut budget()).unwrap(),
            program.bind_instance(&expected, []).unwrap()
        );
    }
    for index in [3, 4] {
        assert_eq!(
            value(program, &evidence, &frame, id(index), &mut budget()),
            Err(InstantiationFailure::Unresolved)
        );
    }
    assert_eq!(
        value(
            program,
            &evidence,
            &frame,
            id(1),
            &mut WorkBudget::new(NonZeroU64::new(1).unwrap())
        ),
        Err(InstantiationFailure::WorkLimit)
    );
    assert!(value(program, &evidence, &frame, id(1), &mut budget()).is_ok());
}

#[test]
fn original_call_bundles_feed_distinct_returned_environments_into_one_new_creation() {
    let fixture = fixture_with_shape(1, false);
    let program = &fixture.program;
    let frame = program
        .bind_code(
            fixture.creator.into(),
            [(fixture.formal, fixture.callbacks[0])],
        )
        .unwrap();
    let Origin::Closure(outer) = fixture.captured.origin else {
        unreachable!()
    };
    let &wrapper = fixture.supporting.keys().next().unwrap();
    let wrapper_witness = FunctionWitness {
        origin: Origin::Closure(wrapper),
        invocation: ValueInvocation::Read,
    };
    let nodes = factory_nodes(&fixture, wrapper, wrapper_witness, outer);
    let graph = ValueSources::test(
        frame.code(),
        nodes,
        BTreeMap::from([
            (
                fixture.sites[0],
                vec![bundle(1, &[2], &[(fixture.formal, 0)])],
            ),
            (
                fixture.sites[1],
                vec![bundle(1, &[3], &[(fixture.formal, 0)])],
            ),
        ]),
        BTreeSet::from([id(4)]),
    );
    let mut evidence = Evidence::default();
    insert_graph(&mut evidence, graph, &fixture.sites[..2]);
    let (environment, _) = creations::mixed_environment(&fixture, outer, wrapper_witness);
    let expected = program
        .bind_callable_data(fixture.captured, &environment)
        .unwrap();
    assert_eq!(
        value(program, &evidence, &frame, id(8), &mut budget()).unwrap(),
        expected
    );
    assert_eq!(
        value(
            program,
            &evidence,
            &frame,
            id(8),
            &mut WorkBudget::new(NonZeroU64::new(8).unwrap())
        ),
        Err(InstantiationFailure::WorkLimit)
    );
    assert_eq!(
        value(program, &evidence, &frame, id(8), &mut budget()).unwrap(),
        expected
    );
}

fn factory_nodes(
    fixture: &Fixture,
    wrapper: ValueClosure,
    wrapper_witness: FunctionWitness,
    outer: ValueClosure,
) -> Vec<(Option<TypeId>, ValueSource)> {
    let program = &fixture.program;
    let frame_code = ValueCode::Declaration(fixture.creator);
    let parameter_data = program.closure(wrapper).unwrap().environment();
    let mut locals = ArenaBuilder::new();
    vec![
        (
            Some(parameter_data),
            ValueSource::Input {
                local: locals.insert(()),
                origin: LocalOrigin::Parameter(match fixture.formal {
                    ValueParameter::Declaration(parameter) => parameter,
                    ValueParameter::Closure { .. } => panic!("named parameter"),
                }),
            },
        ),
        (
            program.function_data(fixture.creator.into()),
            ValueSource::Function(fixture.creator.into()),
        ),
        (
            program.function_data(fixture.callbacks[0]),
            ValueSource::Function(fixture.callbacks[0]),
        ),
        (
            program.function_data(fixture.callbacks[1]),
            ValueSource::Function(fixture.callbacks[1]),
        ),
        (
            program.function_data(wrapper_witness),
            ValueSource::Closure {
                closure: wrapper,
                environment: id(0),
            },
        ),
        (
            program.function_data(wrapper_witness),
            ValueSource::CallResult(ValueApplication {
                owner: frame_code,
                call: fixture.sites[0],
            }),
        ),
        (
            program.function_data(wrapper_witness),
            ValueSource::CallResult(ValueApplication {
                owner: frame_code,
                call: fixture.sites[1],
            }),
        ),
        (
            Some(program.closure(outer).unwrap().environment()),
            ValueSource::Aggregate(
                vec![(ValueField::Tuple(0), id(5)), (ValueField::Tuple(1), id(6))].into(),
            ),
        ),
        (
            program.function_data(fixture.captured),
            ValueSource::Closure {
                closure: outer,
                environment: id(7),
            },
        ),
    ]
}

#[test]
fn homogeneous_arrays_require_one_context_and_empty_arrays_supply_no_element_context() {
    let fixture = fixture_with_array_shapes(1, false, true);
    let Origin::Closure(outer) = fixture.captured.origin else {
        unreachable!()
    };
    let &wrapper = fixture.supporting.keys().next().unwrap();
    let wrapper_witness = FunctionWitness {
        origin: Origin::Closure(wrapper),
        invocation: ValueInvocation::Read,
    };
    let element = fixture.program.function_data(wrapper_witness).unwrap();
    let [array, empty] = fixture.array_shapes.unwrap();
    let program = &fixture.program;
    let frame = program
        .bind_code(
            fixture.creator.into(),
            [(fixture.formal, fixture.callbacks[0])],
        )
        .unwrap();
    let mut nodes = factory_nodes(&fixture, wrapper, wrapper_witness, outer);
    nodes.extend([
        (Some(array), ValueSource::Repeat { seed: id(5) }),
        (
            Some(array),
            ValueSource::Aggregate(
                vec![(ValueField::Array(0), id(5)), (ValueField::Array(1), id(6))].into(),
            ),
        ),
        (Some(empty), ValueSource::Repeat { seed: id(999) }),
    ]);
    let graph = ValueSources::test(
        frame.code(),
        nodes,
        BTreeMap::from([
            (
                fixture.sites[0],
                vec![bundle(1, &[2], &[(fixture.formal, 0)])],
            ),
            (
                fixture.sites[1],
                vec![bundle(1, &[3], &[(fixture.formal, 0)])],
            ),
        ]),
        BTreeSet::from([id(4)]),
    );
    let mut evidence = Evidence::default();
    insert_graph(&mut evidence, graph, &fixture.sites[..2]);
    let (_, leaves) = creations::mixed_environment(&fixture, outer, wrapper_witness);
    let expected = program
        .instantiate_components(array, [(ValueField::Array(0), leaves[0].clone())])
        .unwrap();
    for (index, target, path, expected) in [
        (9, array, ValuePath::default(), Fact::Data(expected)),
        (10, array, ValuePath::default(), Fact::Unavailable),
        (
            11,
            empty,
            ValuePath::default(),
            Fact::Data(program.instantiate_closed_data(empty).unwrap()),
        ),
        (
            11,
            element,
            ValuePath::new([ValueProjection::Array(())]),
            Fact::Unavailable,
        ),
    ] {
        assert_eq!(
            Query::new(program, &evidence, &mut budget())
                .solve(&Key::Data(frame.clone(), id(index), path, target))
                .unwrap(),
            expected
        );
    }
}
