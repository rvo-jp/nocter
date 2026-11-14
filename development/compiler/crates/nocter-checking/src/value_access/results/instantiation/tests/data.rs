//! Full carrier queries preserve actual component contexts across returns and updates.
use super::values::{bundle, factory_nodes, id, insert_graph};
use super::*;
use crate::value_access::{ValueSource, ValueSources, ir::LocalOrigin};
use nocter_declarations::value_access::InstanceSchema;
use nocter_model::{ValueField, ValueProjection};

#[test]
fn a_returned_carrier_can_supply_a_new_environment_without_flattening_its_component_contexts() {
    let fixture = fixture_with_shape(1, false);
    let program = &fixture.program;
    let Origin::Closure(outer) = fixture.captured.origin else {
        unreachable!()
    };
    let &wrapper = fixture.supporting.keys().next().unwrap();
    let wrapper_witness = FunctionWitness {
        origin: Origin::Closure(wrapper),
        invocation: ValueInvocation::Read,
    };
    let (environment, _) = creations::mixed_environment(&fixture, outer, wrapper_witness);
    let selected = program
        .bind_callable_data(fixture.captured, &environment)
        .unwrap();
    let frame = program.bind_instance(&selected, []).unwrap();
    let mut evidence = Evidence::default();
    insert_factory(&fixture, wrapper, wrapper_witness, outer, &mut evidence);
    insert_forwarder(&fixture, environment.data(), &mut evidence);
    let graph = ValueSources::test(
        frame.code(),
        vec![
            (
                program.function_data(fixture.root.into()),
                ValueSource::Function(fixture.root.into()),
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
                Some(environment.data()),
                ValueSource::CallResult(ValueApplication {
                    owner: frame.code(),
                    call: fixture.sites[0],
                }),
            ),
            (
                program.function_data(fixture.captured),
                ValueSource::Closure {
                    closure: outer,
                    environment: id(3),
                },
            ),
        ],
        BTreeMap::from([(
            fixture.sites[0],
            vec![bundle(
                0,
                &[1, 2],
                &[(fixture.root_formals[0], 0), (fixture.root_formals[1], 1)],
            )],
        )]),
        BTreeSet::new(),
    );
    insert_graph(&mut evidence, graph, &fixture.sites[..1]);
    assert_eq!(
        data(
            program,
            &evidence,
            &frame,
            id(3),
            environment.data(),
            &mut budget()
        )
        .unwrap(),
        environment
    );
    assert_eq!(
        value(program, &evidence, &frame, id(4), &mut budget()).unwrap(),
        selected
    );
    assert_eq!(
        data(
            program,
            &evidence,
            &frame,
            id(3),
            environment.data(),
            &mut WorkBudget::new(NonZeroU64::new(8).unwrap())
        ),
        Err(InstantiationFailure::WorkLimit)
    );
    assert_eq!(
        data(
            program,
            &evidence,
            &frame,
            id(3),
            environment.data(),
            &mut budget()
        )
        .unwrap(),
        environment
    );
}

fn insert_factory(
    fixture: &Fixture,
    wrapper: ValueClosure,
    witness: FunctionWitness,
    outer: ValueClosure,
    evidence: &mut Evidence,
) {
    let graph = ValueSources::test(
        fixture.creator.into(),
        factory_nodes(fixture, wrapper, witness, outer),
        BTreeMap::new(),
        BTreeSet::from([id(4)]),
    );
    insert_graph(evidence, graph, &[]);
}
fn insert_forwarder(fixture: &Fixture, carrier: TypeId, evidence: &mut Evidence) {
    let owner = fixture.root.into();
    let mut locals = ArenaBuilder::new();
    let mut nodes = vec![(
        fixture.program.function_data(fixture.creator.into()),
        ValueSource::Function(fixture.creator.into()),
    )];
    for parameter in fixture.root_formals {
        let ValueParameter::Declaration(input) = parameter else {
            panic!("named formal")
        };
        let witness = FunctionWitness {
            origin: Origin::Parameter(parameter),
            invocation: ValueInvocation::Read,
        };
        nodes.push((
            fixture.program.function_data(witness),
            ValueSource::Input {
                local: locals.insert(()),
                origin: LocalOrigin::Parameter(input),
            },
        ));
    }
    for site in &fixture.sites[..2] {
        nodes.push((
            None,
            ValueSource::CallResult(ValueApplication { owner, call: *site }),
        ));
    }
    nodes.push((
        Some(carrier),
        ValueSource::Aggregate(
            vec![(ValueField::Tuple(0), id(3)), (ValueField::Tuple(1), id(4))].into(),
        ),
    ));
    let graph = ValueSources::test(
        owner,
        nodes,
        BTreeMap::from([
            (
                fixture.sites[0],
                vec![bundle(0, &[1], &[(fixture.formal, 0)])],
            ),
            (
                fixture.sites[1],
                vec![bundle(0, &[2], &[(fixture.formal, 0)])],
            ),
        ]),
        BTreeSet::from([id(5)]),
    );
    insert_graph(evidence, graph, &fixture.sites[..2]);
}

#[test]
fn component_replacement_changes_only_the_new_snapshot_and_weak_updates_cannot_choose_a_context() {
    let fixture = fixture_with_shape(1, false);
    let program = &fixture.program;
    let Origin::Closure(outer) = fixture.captured.origin else {
        unreachable!()
    };
    let &wrapper = fixture.supporting.keys().next().unwrap();
    let wrapper_witness = FunctionWitness {
        origin: Origin::Closure(wrapper),
        invocation: ValueInvocation::Read,
    };
    let (environment, leaves) = creations::mixed_environment(&fixture, outer, wrapper_witness);
    let selected = program
        .bind_callable_data(fixture.captured, &environment)
        .unwrap();
    let frame = program.bind_instance(&selected, []).unwrap();
    let nodes = replacement_nodes(environment.data(), program.function_data(wrapper_witness));
    let graph = ValueSources::test(frame.code(), nodes, BTreeMap::new(), BTreeSet::new());
    let mut evidence = Evidence::default();
    insert_graph(&mut evidence, graph, &[]);
    let expected = program
        .instantiate_components(
            environment.data(),
            [0, 1].map(|index| (ValueField::Tuple(index), leaves[1].clone())),
        )
        .unwrap();
    assert_eq!(
        data(
            program,
            &evidence,
            &frame,
            id(2),
            environment.data(),
            &mut budget()
        )
        .unwrap(),
        expected
    );
    assert_eq!(
        data(
            program,
            &evidence,
            &frame,
            id(0),
            environment.data(),
            &mut budget()
        )
        .unwrap(),
        environment
    );
    assert_eq!(
        data(
            program,
            &evidence,
            &frame,
            id(3),
            environment.data(),
            &mut budget()
        ),
        Err(InstantiationFailure::Unresolved)
    );
    let InstanceSchema::Function(wanted) = leaves[1].schema(program).unwrap() else {
        panic!("function leaf")
    };
    assert_eq!(
        Query::new(program, &evidence, &mut budget())
            .solve(&Key::Data(
                frame,
                id(2),
                ValuePath::new([ValueProjection::Tuple(0)]),
                leaves[1].data()
            ))
            .unwrap(),
        Fact::Data(
            program
                .instantiate_function(leaves[1].data(), &wanted)
                .unwrap()
        )
    );
}

#[test]
fn an_unproduced_application_does_not_publish_even_declaration_closed_data() {
    let fixture = fixture();
    let program = &fixture.program;
    let frame = program
        .bind_code(
            fixture.creator.into(),
            [(fixture.formal, fixture.callbacks[0])],
        )
        .unwrap();
    let source = program.data_type(BuiltinType::I32).unwrap();
    let site = ValueApplication {
        owner: frame.code(),
        call: fixture.sites[0],
    };
    let graph = ValueSources::test(
        frame.code(),
        vec![(Some(source), ValueSource::CallResult(site))],
        BTreeMap::new(),
        BTreeSet::new(),
    );
    let mut evidence = Evidence::default();
    insert_graph(&mut evidence, graph, &[]);
    evidence
        .applications
        .insert(site, ApplicationEvidence::Unproduced);
    assert_eq!(
        data(program, &evidence, &frame, id(0), source, &mut budget()),
        Err(InstantiationFailure::Unresolved)
    );
}

pub(super) fn replacement_nodes(
    carrier: TypeId,
    element: Option<TypeId>,
) -> Vec<(Option<TypeId>, ValueSource)> {
    let mut locals = ArenaBuilder::new();
    vec![
        (
            Some(carrier),
            ValueSource::Input {
                local: locals.insert(()),
                origin: LocalOrigin::Capture,
            },
        ),
        (
            element,
            ValueSource::Select {
                source: id(0),
                path: ValuePath::new([ValueProjection::Tuple(1)]),
            },
        ),
        (
            Some(carrier),
            ValueSource::Replace {
                source: id(0),
                value: id(1),
                path: ValuePath::new([ValueProjection::Tuple(0)]),
                weak: false,
            },
        ),
        (
            Some(carrier),
            ValueSource::Replace {
                source: id(0),
                value: id(1),
                path: ValuePath::new([ValueProjection::Tuple(0)]),
                weak: true,
            },
        ),
    ]
}
