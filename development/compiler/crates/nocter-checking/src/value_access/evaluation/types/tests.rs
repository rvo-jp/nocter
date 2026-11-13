use super::*;
use crate::value_access::tests::{bound, input};
use nocter_declarations::value_access::{FunctionConstraint, FunctionWitness};
use nocter_model::{BuiltinType, ValueFunctionOrigin, ValueInvocation, ValueParameter};

struct Fixture {
    program: Program,
    creator: nocter_model::AccessCodeId,
    parameter: ValueParameter,
    formal: TypeId,
    captured: TypeId,
    tuple: TypeId,
    empty: TypeId,
    callbacks: [FunctionWitness; 2],
    wrapper: FunctionWitness,
}
fn fixture() -> Fixture {
    let (sources, tree) = input(
        "func first(): i32 { 41 }\nfunc second(): i32 { 42 }\nfunc creator(take action: func(): i32): i32 { action() }",
    );
    let syntax = bound(&sources, &tree);
    let headers = nocter_declaration_lowering::access_declarations::Query::prepare(syntax).unwrap();
    let context = headers.bind(syntax).unwrap();
    let program = context.program();
    let code = |name| {
        program
            .resolve_code(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let creator = code("creator");
    let callbacks = ["first", "second"].map(|name| code(name).into());
    let parameter = program.codes().get(creator).unwrap().parameters()[0].into();
    let formal = program
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(parameter),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    let integer = program.data_type(BuiltinType::I32).unwrap();
    let mut body = program.body_types(creator).unwrap();
    let closure = body.reserve_closure().unwrap();
    let captured = body
        .closure(
            closure,
            formal,
            FunctionConstraint {
                invocation: ValueInvocation::Read,
                inputs: Box::new([]),
                result: integer,
            },
        )
        .unwrap();
    let array = body.array(captured, 2).unwrap();
    let tuple = body.tuple(&[captured, array]).unwrap();
    let empty = body.array(captured, 0).unwrap();
    let wrapper = body.reserve_closure().unwrap();
    body.closure(
        wrapper,
        tuple,
        FunctionConstraint {
            invocation: ValueInvocation::Read,
            inputs: Box::new([]),
            result: integer,
        },
    )
    .unwrap();
    let wrapper = FunctionWitness {
        origin: ValueFunctionOrigin::Closure(wrapper),
        invocation: ValueInvocation::Read,
    };
    Fixture {
        program: body.finish(),
        creator,
        parameter,
        formal,
        captured,
        tuple,
        empty,
        callbacks,
        wrapper,
    }
}

#[test]
fn contextual_shapes_separate_carriers_and_share_the_same_actual_callable_leaf() {
    let fixture = fixture();
    let program = &fixture.program;
    let frames = fixture.callbacks.map(|callback| {
        program
            .bind_code(fixture.creator.into(), [(fixture.parameter, callback)])
            .unwrap()
    });
    let mut types = Types::default();
    assert!(!types.supports(program, fixture.captured).unwrap());
    let actual = frames
        .each_ref()
        .map(|frame| types.get_in(program, frame, fixture.captured).unwrap());
    assert_ne!(actual[0], actual[1]);
    let carriers = frames
        .each_ref()
        .map(|frame| types.get_in(program, frame, fixture.tuple).unwrap());
    assert_ne!(carriers[0], carriers[1]);
    for (position, frame) in frames.iter().enumerate() {
        let leaf = types.get_in(program, frame, fixture.formal).unwrap();
        let named = types
            .get(
                program,
                program.function_data(fixture.callbacks[position]).unwrap(),
            )
            .unwrap();
        assert_eq!(
            leaf, named,
            "argument and directly selected value need the same evaluator type"
        );
        let Type::CapturedFunction { environment, .. } = types.shapes[actual[position]].ty else {
            panic!("captured environment");
        };
        assert_eq!(environment, leaf);
        assert!(types.shapes[carriers[position]].supported);
        assert!(!types.shapes[carriers[position]].serializable);
    }
    let empty = frames
        .each_ref()
        .map(|frame| types.get_in(program, frame, fixture.empty).unwrap());
    assert_eq!(empty[0], empty[1]);
    assert!(types.shapes[empty[0]].supported);
    assert!(types.shapes[empty[0]].serializable);
    assert!(
        !types.supports(program, fixture.captured).unwrap(),
        "closed instances must not strengthen the unbound body"
    );
}

#[test]
fn the_same_lexical_code_with_nested_owned_contexts_has_a_finite_shape_walk() {
    let fixture = fixture();
    let program = &fixture.program;
    let Some(PayloadSchema::Function(captured)) = program.data_schema(fixture.captured) else {
        panic!("closure");
    };
    let mut current = program.concrete_witness(fixture.callbacks[0]).unwrap();
    for _ in 0..64 {
        current = program
            .bind_callable_environment(captured, [(fixture.parameter, current)])
            .unwrap();
    }
    let frame = program
        .bind_code(
            fixture.creator.into(),
            [(fixture.parameter, fixture.callbacks[0])],
        )
        .unwrap();
    let mut types = Types::default();
    let mut index = types
        .get_in(
            program,
            &program.bind_instance(&current, []).unwrap(),
            fixture.captured,
        )
        .unwrap();
    for _ in 0..64 {
        let Type::CapturedFunction { environment, .. } = types.shapes[index].ty else {
            panic!("distinct finite environment");
        };
        index = environment;
    }
    assert!(matches!(types.shapes[index].ty, Type::NamedFunction(_)));
    assert_eq!(types.shapes.len(), 65);
    let canonical = types.get_in(program, &frame, fixture.formal).unwrap();
    assert_eq!(canonical, index);
}

#[test]
fn evaluator_carriers_keep_mixed_component_contexts_without_strengthening_raw_types() {
    let fixture = fixture();
    let program = &fixture.program;
    let frames = fixture.callbacks.map(|callback| {
        program
            .bind_code(fixture.creator.into(), [(fixture.parameter, callback)])
            .unwrap()
    });
    let first = program
        .instantiate_data(fixture.captured, &frames[0])
        .unwrap();
    let uniform = program.instantiate_data(fixture.tuple, &frames[1]).unwrap();
    let array = uniform.component(program, ValueField::Tuple(1)).unwrap();
    let mixed = program
        .instantiate_components(
            fixture.tuple,
            [(ValueField::Tuple(0), first), (ValueField::Tuple(1), array)],
        )
        .unwrap();
    let mut types = Types::default();
    let id = types
        .get_key(program, &ShapeKey::instantiated(program, mixed).unwrap())
        .unwrap();
    let actual = frames
        .each_ref()
        .map(|frame| types.get_in(program, frame, fixture.captured).unwrap());
    let Type::Record(fields) = &types.shapes[id].ty else {
        panic!("record");
    };
    assert_eq!(fields[0], actual[0]);
    let Type::Array { element, length } = types.shapes[fields[1]].ty else {
        panic!("array");
    };
    assert_eq!(length, 2);
    assert_eq!(element, actual[1]);
    assert_ne!(actual[0], actual[1]);
    assert!(types.shapes[id].supported);
    assert!(!types.shapes[id].serializable);
    assert!(!types.supports(program, fixture.tuple).unwrap());
}

#[test]
fn an_environment_input_uses_its_complete_frame_instead_of_reconstructing_type_bindings() {
    let fixture = fixture();
    let program = &fixture.program;
    let frames = fixture.callbacks.map(|callback| {
        program
            .bind_code(fixture.creator.into(), [(fixture.parameter, callback)])
            .unwrap()
    });
    let roots = frames
        .each_ref()
        .map(|frame| program.instantiate_data(fixture.tuple, frame).unwrap());
    let mixed = program
        .instantiate_components(
            fixture.tuple,
            [
                (
                    ValueField::Tuple(0),
                    roots[0].component(program, ValueField::Tuple(0)).unwrap(),
                ),
                (
                    ValueField::Tuple(1),
                    roots[1].component(program, ValueField::Tuple(1)).unwrap(),
                ),
            ],
        )
        .unwrap();
    let frame = program
        .bind_instance(
            &program.bind_callable_data(fixture.wrapper, &mixed).unwrap(),
            [],
        )
        .unwrap();
    assert!(
        program.instantiate_data(fixture.tuple, &frame).is_err(),
        "the two captured occurrences have no single free-parameter binding"
    );
    let mut types = Types::default();
    let expected = types.get_instance(program, mixed).unwrap();
    assert_eq!(
        types
            .get_environment(program, &frame, fixture.tuple)
            .unwrap(),
        expected
    );
    assert!(
        types
            .get_environment(program, &frame, fixture.captured)
            .is_err()
    );
    assert!(
        types
            .get_environment(program, &frames[0], fixture.tuple)
            .is_err()
    );
    assert!(!types.supports(program, fixture.tuple).unwrap());
}
