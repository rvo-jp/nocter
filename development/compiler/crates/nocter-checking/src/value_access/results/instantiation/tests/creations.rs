//! Creation-snapshot substitution and conservative metadata failure.
use super::*;
use crate::value_access::results::ProducerShape;
use nocter_model::ValueField;

#[test]
fn actual_capture_snapshot_overrides_an_outer_binding_and_missing_or_ambiguous_creation_rejects() {
    let fixture = fixture();
    let program = &fixture.program;
    let frame = program
        .bind_code(
            fixture.creator.into(),
            [(fixture.formal, fixture.callbacks[0])],
        )
        .unwrap();
    let site = ValueApplication {
        owner: frame.code(),
        call: fixture.sites[1],
    };
    let mut evidence = Evidence::default();
    evidence.invocations.insert(
        site,
        Arc::new(InvocationRecipe {
            callees: BTreeSet::from([fixture.captured]),
            inputs: BTreeMap::new(),
        }),
    );
    assert_eq!(
        call(program, &evidence, &frame, site, &mut budget()),
        Err(InstantiationFailure::Unresolved),
        "the outer formal is not a creation snapshot"
    );
    let Origin::Closure(closure) = fixture.captured.origin else {
        unreachable!()
    };
    evidence
        .creations
        .insert(closure, creation(&fixture, [fixture.callbacks[1]]));
    let expected = program
        .bind_instance(&captured(&fixture, fixture.callbacks[1]), [])
        .unwrap();
    assert_eq!(
        call(program, &evidence, &frame, site, &mut budget()).unwrap(),
        expected
    );
    assert_eq!(
        call(
            program,
            &evidence,
            &frame,
            site,
            &mut WorkBudget::new(NonZeroU64::new(1).unwrap())
        ),
        Err(InstantiationFailure::WorkLimit)
    );
    assert_eq!(
        call(program, &evidence, &frame, site, &mut budget()).unwrap(),
        expected
    );
    evidence
        .creations
        .insert(closure, creation(&fixture, fixture.callbacks));
    assert_eq!(
        call(program, &evidence, &frame, site, &mut budget()),
        Err(InstantiationFailure::Unresolved),
        "different possible captured values cannot select an arbitrary context"
    );
}

#[test]
fn a_dependent_closure_term_in_a_foreign_frame_cannot_reuse_that_frames_capture_names() {
    let fixture = fixture();
    let program = &fixture.program;
    let frame = program
        .bind_instance(&captured(&fixture, fixture.callbacks[0]), [])
        .unwrap();
    let site = ValueApplication {
        owner: frame.code(),
        call: fixture.sites[1],
    };
    let Origin::Closure(closure) = fixture.captured.origin else {
        unreachable!()
    };
    let mut evidence = Evidence::default();
    evidence
        .creations
        .insert(closure, creation(&fixture, [fixture.callbacks[1]]));
    evidence.invocations.insert(
        site,
        Arc::new(InvocationRecipe {
            callees: BTreeSet::from([fixture.captured]),
            inputs: BTreeMap::new(),
        }),
    );
    assert_eq!(
        call(program, &evidence, &frame, site, &mut budget()),
        Err(InstantiationFailure::Unresolved)
    );
}

#[test]
fn simultaneous_capture_views_preserve_one_concrete_context() {
    let fixture = fixture_with_shape(0, true);
    let program = &fixture.program;
    let frame = program
        .bind_code(
            fixture.creator.into(),
            [(fixture.formal, fixture.callbacks[0])],
        )
        .unwrap();
    let site = ValueApplication {
        owner: frame.code(),
        call: fixture.sites[1],
    };
    let Origin::Closure(closure) = fixture.captured.origin else {
        unreachable!()
    };
    let environment = program.closure(closure).unwrap().environment();
    let mut recipe = creation(&fixture, []);
    recipe.producers = ProducerShape::test_schema(program, environment);
    let mut evidence = Evidence {
        creations: fixture.supporting.clone(),
        ..Evidence::default()
    };
    evidence.creations.insert(closure, recipe);
    evidence.invocations.insert(
        site,
        Arc::new(InvocationRecipe {
            callees: BTreeSet::from([fixture.captured]),
            inputs: BTreeMap::new(),
        }),
    );
    assert_eq!(
        call(program, &evidence, &frame, site, &mut budget()).unwrap(),
        program
            .bind_instance(&captured(&fixture, fixture.callbacks[0]), [])
            .unwrap()
    );
}

#[test]
fn shared_capture_shapes_spend_metadata_work_without_expanding_two_to_the_depth_leaves() {
    const DEPTH: usize = 64;
    let fixture = fixture_with_shape(DEPTH, false);
    let program = &fixture.program;
    let frame = program
        .bind_code(
            fixture.creator.into(),
            [(fixture.formal, fixture.callbacks[0])],
        )
        .unwrap();
    let site = ValueApplication {
        owner: frame.code(),
        call: fixture.sites[1],
    };
    let Origin::Closure(closure) = fixture.captured.origin else {
        unreachable!()
    };
    let environment = program.closure(closure).unwrap().environment();
    let mut recipe = creation(&fixture, []);
    recipe.producers = ProducerShape::test_schema(program, environment);
    let mut evidence = Evidence {
        creations: fixture.supporting.clone(),
        ..Evidence::default()
    };
    evidence.creations.insert(closure, recipe);
    evidence.invocations.insert(
        site,
        Arc::new(InvocationRecipe {
            callees: BTreeSet::from([fixture.captured]),
            inputs: BTreeMap::new(),
        }),
    );
    let expected = program
        .bind_instance(&captured(&fixture, fixture.callbacks[0]), [])
        .unwrap();
    assert_eq!(
        call(program, &evidence, &frame, site, &mut budget()).unwrap(),
        expected
    );
    let mut limited = WorkBudget::new(NonZeroU64::new(u64::try_from(DEPTH).unwrap()).unwrap());
    assert_eq!(
        call(program, &evidence, &frame, site, &mut limited),
        Err(InstantiationFailure::WorkLimit)
    );
    assert_eq!(
        call(program, &evidence, &frame, site, &mut budget()).unwrap(),
        expected
    );
}

#[test]
fn one_creation_snapshot_keeps_two_calls_to_the_same_closure_with_different_contexts() {
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
    let (&wrapper, wrapper_recipe) = fixture.supporting.iter().next().unwrap();
    let wrapper_witness = FunctionWitness {
        origin: Origin::Closure(wrapper),
        invocation: ValueInvocation::Read,
    };
    let sites = fixture.sites.map(|call| ValueApplication {
        owner: frame.code(),
        call,
    });
    let mut evidence = snapshot_evidence(&fixture, outer, wrapper_witness, sites);
    assert_eq!(wrapper_recipe.owner, frame.code());
    let (environment, expected_leaves) = mixed_environment(&fixture, outer, wrapper_witness);
    let expected = program
        .bind_instance(
            &program
                .bind_callable_data(fixture.captured, &environment)
                .unwrap(),
            [],
        )
        .unwrap();
    assert_eq!(
        call(program, &evidence, &frame, sites[2], &mut budget()).unwrap(),
        expected
    );
    assert_eq!(
        call(
            program,
            &evidence,
            &frame,
            sites[2],
            &mut WorkBudget::new(NonZeroU64::new(8).unwrap())
        ),
        Err(InstantiationFailure::WorkLimit)
    );
    assert_eq!(
        call(program, &evidence, &frame, sites[2], &mut budget()).unwrap(),
        expected
    );
    for (position, key) in expected_leaves.iter().enumerate() {
        assert_eq!(
            expected
                .environment()
                .unwrap()
                .component(program, ValueField::Tuple(position))
                .unwrap(),
            *key
        );
    }
    evidence.invocations.remove(&sites[1]);
    assert_eq!(
        call(program, &evidence, &frame, sites[2], &mut budget()),
        Err(InstantiationFailure::Unresolved)
    );
}

fn snapshot_evidence(
    fixture: &Fixture,
    outer: ValueClosure,
    wrapper_witness: FunctionWitness,
    sites: [ValueApplication; 3],
) -> Evidence {
    let mut evidence = Evidence {
        creations: fixture.supporting.clone(),
        ..Evidence::default()
    };
    for (site, callback) in sites[..2].iter().zip(fixture.callbacks) {
        evidence.invocations.insert(
            *site,
            Arc::new(InvocationRecipe {
                callees: BTreeSet::from([fixture.creator.into()]),
                inputs: BTreeMap::from([(fixture.formal, BTreeSet::from([callback]).into())]),
            }),
        );
    }
    evidence.recipes.insert(
        ProducerTerm::Result {
            code: fixture.creator.into(),
            ordinal: 0,
        },
        ProducerRelation::Union(BTreeSet::from([wrapper_witness])),
    );
    let mut outer_recipe = creation(fixture, []);
    outer_recipe.producers =
        ProducerShape::test_components(sites[..2].iter().enumerate().map(|(position, &site)| {
            (
                ValueField::Tuple(position),
                FunctionWitness {
                    origin: Origin::Application(site.into()),
                    invocation: ValueInvocation::Read,
                },
            )
        }));
    evidence.creations.insert(outer, outer_recipe);
    evidence.invocations.insert(
        sites[2],
        Arc::new(InvocationRecipe {
            callees: BTreeSet::from([fixture.captured]),
            inputs: BTreeMap::new(),
        }),
    );
    evidence
}

pub(super) fn mixed_environment(
    fixture: &Fixture,
    outer: ValueClosure,
    wrapper_witness: FunctionWitness,
) -> (
    nocter_declarations::value_access::DataInstanceKey,
    [nocter_declarations::value_access::DataInstanceKey; 2],
) {
    let program = &fixture.program;
    let expected_leaves = fixture.callbacks.map(|callback| {
        let frame = program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap();
        program
            .instantiate_data(program.function_data(wrapper_witness).unwrap(), &frame)
            .unwrap()
    });
    let environment = program
        .instantiate_components(
            program.closure(outer).unwrap().environment(),
            expected_leaves
                .iter()
                .enumerate()
                .map(|(position, key)| (ValueField::Tuple(position), key.clone())),
        )
        .unwrap();
    (environment, expected_leaves)
}
