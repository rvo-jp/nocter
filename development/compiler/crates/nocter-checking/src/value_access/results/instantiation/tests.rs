//! Logical metadata graphs exercise contexts not yet admitted by the source capture gate.
use super::*;
use crate::value_access::{
    results::{CallableEvidence, InvocationRecipe},
    tests::{bound, input},
};
use nocter_model::{
    AccessCodeId, AccessExpressionId, ArenaBuilder, BuiltinType, TypeId, ValueClosure,
    ValueParameter,
};
use std::{num::NonZeroU64, sync::Arc};

mod batches;
mod carrier_writes;
mod control;
mod creations;
mod data;
mod explicit_writes;
mod fixture;
mod stored;
mod values;
mod writes;
use fixture::*;

#[test]
fn equal_lexical_code_with_different_captured_contexts_cannot_select_an_arbitrary_frame() {
    let fixture = fixture();
    let program = &fixture.program;
    let values = fixture
        .callbacks
        .map(|callback| captured(&fixture, callback));
    assert_eq!(values[0].witness(), values[1].witness());
    assert_ne!(values[0], values[1]);
    let root = program.concrete_witness(fixture.root.into()).unwrap();
    let different = program
        .bind_instance(
            &root,
            fixture.root_formals.into_iter().zip(values.iter().cloned()),
        )
        .unwrap();
    let equal = program
        .bind_instance(
            &root,
            fixture
                .root_formals
                .into_iter()
                .map(|parameter| (parameter, values[0].clone())),
        )
        .unwrap();
    let site = ValueApplication {
        owner: fixture.root.into(),
        call: fixture.sites[0],
    };
    let mut evidence = Evidence::default();
    evidence.invocations.insert(
        site,
        Arc::new(InvocationRecipe {
            callees: fixture
                .root_formals
                .map(|parameter| FunctionWitness {
                    origin: Origin::Parameter(parameter),
                    invocation: ValueInvocation::Read,
                })
                .into(),
            inputs: BTreeMap::new(),
        }),
    );
    assert_eq!(
        call(program, &evidence, &different, site, &mut budget()),
        Err(InstantiationFailure::Unresolved)
    );
    let resolved = call(program, &evidence, &equal, site, &mut budget()).unwrap();
    assert_eq!(resolved, program.bind_instance(&values[0], []).unwrap());
}

#[test]
fn returned_closure_uses_its_actual_creation_frame_instead_of_the_outer_frames_binding() {
    let fixture = fixture();
    let program = &fixture.program;
    let outer = program
        .bind_code(
            fixture.creator.into(),
            [(fixture.formal, fixture.callbacks[0])],
        )
        .unwrap();
    let factory = ValueApplication {
        owner: fixture.creator.into(),
        call: fixture.sites[0],
    };
    let invoked = ValueApplication {
        owner: fixture.creator.into(),
        call: fixture.sites[1],
    };
    let mut evidence = Evidence::default();
    evidence
        .applications
        .insert(factory.into(), CallableEvidence::Witness(fixture.captured));
    evidence.results.insert(
        fixture.creator.into(),
        CallableEvidence::Witness(fixture.captured),
    );
    let Origin::Closure(closure) = fixture.captured.origin else {
        unreachable!()
    };
    evidence.creations.insert(
        closure,
        creation(
            &fixture,
            [FunctionWitness {
                origin: Origin::Parameter(fixture.formal),
                invocation: ValueInvocation::Read,
            }],
        ),
    );
    evidence.recipes.insert(
        ProducerTerm::Result {
            code: fixture.creator.into(),
            ordinal: 0,
        },
        ProducerRelation::Union(BTreeSet::from([fixture.captured])),
    );
    evidence.invocations.insert(
        factory,
        Arc::new(InvocationRecipe {
            callees: BTreeSet::from([fixture.creator.into()]),
            inputs: BTreeMap::from([(
                fixture.formal,
                BTreeSet::from([fixture.callbacks[1]]).into(),
            )]),
        }),
    );
    evidence.invocations.insert(
        invoked,
        Arc::new(InvocationRecipe {
            callees: BTreeSet::from([FunctionWitness {
                origin: Origin::Application(factory.into()),
                invocation: ValueInvocation::Read,
            }]),
            inputs: BTreeMap::new(),
        }),
    );
    let actual = call(program, &evidence, &outer, invoked, &mut budget()).unwrap();
    let expected = captured(&fixture, fixture.callbacks[1]);
    assert_eq!(actual, program.bind_instance(&expected, []).unwrap());
    assert_ne!(
        actual
            .resolve(
                program,
                FunctionWitness {
                    origin: Origin::Parameter(fixture.formal),
                    invocation: ValueInvocation::Read
                }
            )
            .unwrap(),
        outer.witnesses()[&fixture.formal]
    );
}

#[test]
fn adapting_a_concrete_witness_preserves_context_and_rejects_a_view_upgrade() {
    let fixture = fixture();
    let program = &fixture.program;
    let original = captured(&fixture, fixture.callbacks[0]);
    let edited = program
        .adapt_witness(&original, ValueInvocation::Edit)
        .unwrap();
    let consumed = program
        .adapt_witness(&edited, ValueInvocation::Take)
        .unwrap();
    assert_eq!(consumed.code(), original.code());
    assert_eq!(consumed.environment(), original.environment());
    assert!(
        program
            .adapt_witness(&consumed, ValueInvocation::Read)
            .is_err()
    );
}
