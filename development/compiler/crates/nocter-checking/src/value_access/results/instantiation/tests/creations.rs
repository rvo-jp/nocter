//! Creation-snapshot substitution and conservative metadata failure.
use super::*;
use crate::value_access::results::ProducerShape;

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
