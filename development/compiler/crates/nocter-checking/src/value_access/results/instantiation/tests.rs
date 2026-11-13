//! Logical metadata graphs exercise contexts not yet admitted by the source capture gate.
use super::*;
use crate::value_access::{
    results::{ApplicationEvidence, InvocationRecipe},
    tests::{bound, input},
};
use nocter_model::{AccessCodeId, AccessExpressionId, ArenaBuilder, BuiltinType, ValueParameter};
use std::{num::NonZeroU64, sync::Arc};

struct Fixture {
    program: Program,
    creator: AccessCodeId,
    root: AccessCodeId,
    formal: ValueParameter,
    root_formals: [ValueParameter; 2],
    captured: FunctionWitness,
    callbacks: [FunctionWitness; 2],
    sites: [AccessExpressionId; 2],
}
fn fixture() -> Fixture {
    let (sources, tree) = input(
        "func first(): i32 { 41 }\nfunc second(): i32 { 42 }\nfunc creator(take action: func(): i32): func(): i32 { first }\nfunc root(left: func(): i32, right: func(): i32): i32 { 0 }",
    );
    let syntax = bound(&sources, &tree);
    let query = nocter_declaration_lowering::access_declarations::Query::prepare(syntax).unwrap();
    let context = query.bind(syntax).unwrap();
    let header = context.program();
    let code = |name| header.resolve(header.symbols().get(name).unwrap()).unwrap();
    let creator = code("creator");
    let root = code("root");
    let callbacks = ["first", "second"].map(|name| code(name).into());
    let parameter = header.codes().get(creator).unwrap().parameters()[0];
    let formal = parameter.into();
    let root_formals = std::array::from_fn(|position| {
        header.codes().get(root).unwrap().parameters()[position].into()
    });
    let environment = header.parameters().get(parameter).unwrap().data();
    let mut body = header.body_types(creator).unwrap();
    let closure = body.reserve_closure().unwrap();
    body.closure(
        closure,
        environment,
        nocter_declarations::value_access::FunctionConstraint {
            invocation: ValueInvocation::Read,
            inputs: Box::new([]),
            result: header.data_type(BuiltinType::I32).unwrap(),
        },
    )
    .unwrap();
    let mut expressions = ArenaBuilder::new();
    Fixture {
        program: body.finish(),
        creator,
        root,
        formal,
        root_formals,
        captured: FunctionWitness {
            origin: Origin::Closure(closure),
            invocation: ValueInvocation::Read,
        },
        callbacks,
        sites: [expressions.insert(()), expressions.insert(())],
    }
}
fn budget() -> WorkBudget {
    WorkBudget::new(NonZeroU64::new(10_000).unwrap())
}
fn captured(fixture: &Fixture, callback: FunctionWitness) -> ConcreteWitness {
    fixture
        .program
        .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
        .unwrap()
        .resolve(&fixture.program, fixture.captured)
        .unwrap()
}

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
        .insert(factory, ApplicationEvidence::Witness(fixture.captured));
    evidence.producers.insert(fixture.creator, fixture.captured);
    evidence.recipes.insert(
        ProducerTerm::Result(fixture.creator.into()),
        ProducerRelation::Union(BTreeSet::from([fixture.captured])),
    );
    evidence.invocations.insert(
        factory,
        Arc::new(InvocationRecipe {
            callees: BTreeSet::from([fixture.creator.into()]),
            inputs: BTreeMap::from([(fixture.formal, BTreeSet::from([fixture.callbacks[1]]))]),
        }),
    );
    evidence.invocations.insert(
        invoked,
        Arc::new(InvocationRecipe {
            callees: BTreeSet::from([FunctionWitness {
                origin: Origin::Application(factory),
                invocation: ValueInvocation::Read,
            }]),
            inputs: BTreeMap::new(),
        }),
    );
    let actual = call(program, &evidence, &outer, invoked, &mut budget()).unwrap();
    let expected = captured(&fixture, fixture.callbacks[1]);
    assert_eq!(actual, program.bind_instance(&expected, []).unwrap());
    assert_ne!(
        actual.captures()[&fixture.formal],
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
    assert_eq!(consumed.captures(), original.captures());
    assert!(
        program
            .adapt_witness(&consumed, ValueInvocation::Read)
            .is_err()
    );
}
