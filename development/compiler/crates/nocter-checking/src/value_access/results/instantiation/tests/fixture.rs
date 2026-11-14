//! Logical witness/creation fixtures, separate from source or executable admission.
use super::*;
use crate::value_access::results::{CreationRecipe, ProducerShape};

pub(super) struct Fixture {
    pub(super) program: Program,
    pub(super) creator: AccessCodeId,
    pub(super) root: AccessCodeId,
    pub(super) formal: ValueParameter,
    pub(super) root_formals: [ValueParameter; 2],
    pub(super) captured: FunctionWitness,
    pub(super) callbacks: [FunctionWitness; 2],
    pub(super) sites: [AccessExpressionId; 3],
    pub(super) supporting: BTreeMap<ValueClosure, CreationRecipe>,
    pub(super) array_shapes: Option<[TypeId; 2]>,
}
pub(super) fn fixture() -> Fixture {
    fixture_with_shape(0, false)
}
pub(super) fn fixture_with_shape(depth: usize, mixed_views: bool) -> Fixture {
    fixture_with_array_shapes(depth, mixed_views, false)
}
pub(super) fn fixture_with_array_shapes(depth: usize, mixed_views: bool, arrays: bool) -> Fixture {
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
    let mut body = header.body_types(creator).unwrap();
    let (environment, wrappers) = environment_shape(
        &mut body,
        formal,
        header.data_type(BuiltinType::I32).unwrap(),
        depth,
        mixed_views,
    );
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
    let array_shapes = arrays.then(|| {
        let element = body
            .program()
            .function_data(FunctionWitness {
                origin: Origin::Closure(wrappers[0]),
                invocation: ValueInvocation::Read,
            })
            .unwrap();
        [
            body.array(element, 2).unwrap(),
            body.array(element, 0).unwrap(),
        ]
    });
    let mut expressions = ArenaBuilder::new();
    let program = body.finish();
    let supporting = wrappers
        .into_iter()
        .map(|closure| {
            let environment = program.closure(closure).unwrap().environment();
            (
                closure,
                CreationRecipe {
                    owner: creator.into(),
                    expression: expressions.insert(()),
                    environment,
                    producers: ProducerShape::test_schema(&program, environment),
                },
            )
        })
        .collect();
    Fixture {
        program,
        creator,
        root,
        formal,
        root_formals,
        captured: FunctionWitness {
            origin: Origin::Closure(closure),
            invocation: ValueInvocation::Read,
        },
        callbacks,
        sites: std::array::from_fn(|_| expressions.insert(())),
        supporting,
        array_shapes,
    }
}
pub(super) fn budget() -> WorkBudget {
    WorkBudget::new(NonZeroU64::new(10_000).unwrap())
}
pub(super) fn captured(fixture: &Fixture, callback: FunctionWitness) -> ConcreteWitness {
    fixture
        .program
        .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
        .unwrap()
        .resolve(&fixture.program, fixture.captured)
        .unwrap()
}
pub(super) fn creation(
    fixture: &Fixture,
    terms: impl IntoIterator<Item = FunctionWitness>,
) -> CreationRecipe {
    let Origin::Closure(closure) = fixture.captured.origin else {
        unreachable!()
    };
    CreationRecipe {
        owner: fixture.creator.into(),
        expression: fixture.sites[0],
        environment: fixture.program.closure(closure).unwrap().environment(),
        producers: ProducerShape::test_terms(terms),
    }
}

fn environment_shape(
    body: &mut nocter_declarations::value_access::BodyTypes,
    formal: ValueParameter,
    result: TypeId,
    depth: usize,
    mixed_views: bool,
) -> (TypeId, Vec<ValueClosure>) {
    let read = body
        .program()
        .function_data(FunctionWitness {
            origin: Origin::Parameter(formal),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    if depth == 0 && !mixed_views {
        return (read, Vec::new());
    }
    let views = if mixed_views {
        vec![ValueInvocation::Read, ValueInvocation::Take]
    } else {
        vec![ValueInvocation::Read]
    };
    let mut wrappers = Vec::new();
    let mut components = Vec::new();
    for invocation in views {
        let data = body
            .program()
            .function_data(FunctionWitness {
                origin: Origin::Parameter(formal),
                invocation,
            })
            .unwrap();
        let closure = body.reserve_closure().unwrap();
        let data = body
            .closure(
                closure,
                data,
                nocter_declarations::value_access::FunctionConstraint {
                    invocation: ValueInvocation::Read,
                    inputs: Box::new([]),
                    result,
                },
            )
            .unwrap();
        wrappers.push(closure);
        components.push(data);
    }
    let mut environment = if components.len() == 1 {
        components[0]
    } else {
        body.tuple(&components).unwrap()
    };
    for _ in 0..depth {
        environment = body.tuple(&[environment, environment]).unwrap();
    }
    (environment, wrappers)
}
