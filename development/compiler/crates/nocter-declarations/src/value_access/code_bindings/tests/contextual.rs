use super::*;
use nocter_model::{ValueClosure, ValueParameter};

struct Fixture {
    program: Program,
    creator: AccessCodeId,
    formal: ValueParameter,
    captured: FunctionWitness,
    nested: FunctionWitness,
    holder: FunctionWitness,
    branching: FunctionWitness,
    inputs: [ValueParameter; 2],
    callbacks: [FunctionWitness; 2],
}
fn fixture() -> Fixture {
    let (headers, [creator, _, plain, _, _]) = headers();
    let formal = headers.codes().get(creator).unwrap().parameters()[0].into();
    let data = headers
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(formal),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    let void = headers.data_type(BuiltinType::Void).unwrap();
    let contract = FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([]),
        result: void,
    };
    let mut body = headers.body_types(creator).unwrap();
    let captured = body.reserve_closure().unwrap();
    body.closure(captured, data, contract.clone()).unwrap();
    let holder = body.reserve_closure().unwrap();
    let inputs = [0, 1].map(|position| ValueParameter::Closure {
        owner: holder,
        position,
    });
    let input_data = [0, 1].map(|position| {
        body.closure_parameter(holder, position, contract.clone())
            .unwrap()
    });
    let nested = body.reserve_closure().unwrap();
    body.closure(nested, input_data[0], contract.clone())
        .unwrap();
    body.closure(
        holder,
        void,
        FunctionConstraint {
            inputs: input_data
                .map(|data| FunctionInput {
                    data,
                    operation: ValueInputOperation::Read,
                })
                .into(),
            ..contract.clone()
        },
    )
    .unwrap();
    let companion = body.reserve_closure().unwrap();
    let wrapped = body
        .closure(companion, input_data[1], contract.clone())
        .unwrap();
    let environment = body
        .tuple(&[
            body.program()
                .function_data(FunctionWitness {
                    origin: ValueFunctionOrigin::Closure(nested),
                    invocation: ValueInvocation::Read,
                })
                .unwrap(),
            wrapped,
        ])
        .unwrap();
    let branching = body.reserve_closure().unwrap();
    body.closure(branching, environment, contract.clone())
        .unwrap();
    let second = body.reserve_closure().unwrap();
    body.closure(second, void, contract).unwrap();
    let witness = |code: ValueClosure| FunctionWitness {
        origin: ValueFunctionOrigin::Closure(code),
        invocation: ValueInvocation::Read,
    };
    Fixture {
        program: body.finish(),
        creator,
        formal,
        captured: witness(captured),
        nested: witness(nested),
        holder: witness(holder),
        branching: witness(branching),
        inputs,
        callbacks: [plain.into(), witness(second)],
    }
}

#[test]
fn same_lexical_code_keeps_distinct_captured_callbacks_through_one_receiving_call() {
    let fixture = fixture();
    let program = &fixture.program;
    let contexts = fixture.callbacks.map(|callback| {
        let root = program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap();
        root.resolve(program, fixture.captured).unwrap()
    });
    assert_eq!(contexts[0].witness(), contexts[1].witness());
    assert_ne!(contexts[0], contexts[1]);
    let holder = program.concrete_witness(fixture.holder).unwrap();
    let key = program
        .bind_instance(
            &holder,
            fixture.inputs.into_iter().zip(contexts.iter().cloned()),
        )
        .unwrap();
    for (position, expected) in contexts.iter().enumerate() {
        let selected = FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(fixture.inputs[position]),
            invocation: ValueInvocation::Read,
        };
        assert_eq!(key.resolve(program, selected).unwrap(), *expected);
        let taken = key
            .resolve(
                program,
                FunctionWitness {
                    invocation: ValueInvocation::Take,
                    ..selected
                },
            )
            .unwrap();
        assert_eq!(taken.captures(), expected.captures());
        assert_eq!(taken.invocation(), ValueInvocation::Take);
    }
    let nested = key.resolve(program, fixture.nested).unwrap();
    let nested_key = program.bind_instance(&nested, []).unwrap();
    assert_eq!(
        nested_key
            .resolve(
                program,
                FunctionWitness {
                    origin: ValueFunctionOrigin::Parameter(fixture.inputs[0]),
                    invocation: ValueInvocation::Read
                }
            )
            .unwrap(),
        contexts[0]
    );
    let template = program
        .bind_template(
            fixture.creator.into(),
            [(
                fixture.formal,
                FunctionWitness {
                    origin: ValueFunctionOrigin::Parameter(fixture.inputs[0]),
                    invocation: ValueInvocation::Read,
                },
            )],
        )
        .unwrap();
    let closed = template.close(program, &key).unwrap();
    assert_eq!(closed.witnesses()[&fixture.formal], contexts[0]);
}

#[test]
fn a_bare_code_witness_cannot_erase_required_environment_bindings() {
    let fixture = fixture();
    let program = &fixture.program;
    assert_eq!(
        program.concrete_witness(fixture.captured),
        Err(CodeBindingError::Missing(fixture.formal))
    );
    assert_eq!(
        program.bind_code(fixture.captured.origin.code().unwrap(), []),
        Err(CodeBindingError::Missing(fixture.formal))
    );
    let actual = program.concrete_witness(fixture.callbacks[0]).unwrap();
    assert_eq!(
        program.bind_callable_environment(fixture.captured, []),
        Err(CodeBindingError::Missing(fixture.formal))
    );
    assert_eq!(
        program.bind_callable_environment(fixture.captured, [(fixture.inputs[0], actual.clone())]),
        Err(CodeBindingError::Unexpected(fixture.inputs[0]))
    );
    assert_eq!(
        program.bind_callable_environment(
            fixture.captured,
            [(fixture.formal, actual.clone()), (fixture.formal, actual)]
        ),
        Err(CodeBindingError::Duplicate(fixture.formal))
    );
}

#[test]
fn shared_contexts_compare_structurally_without_expanding_independent_equal_dags() {
    let fixture = fixture();
    let program = &fixture.program;
    let build = |callback| {
        let mut value = program.concrete_witness(callback).unwrap();
        for _ in 0..64 {
            value = program
                .bind_callable_environment(
                    fixture.branching,
                    fixture
                        .inputs
                        .into_iter()
                        .map(|parameter| (parameter, value.clone())),
                )
                .unwrap();
        }
        value
    };
    let first = build(fixture.callbacks[0]);
    let equal = build(fixture.callbacks[0]);
    let different = build(fixture.callbacks[1]);
    assert_eq!(first, equal);
    assert_eq!(first.cmp(&equal), std::cmp::Ordering::Equal);
    assert_eq!(first.cmp(&different), std::cmp::Ordering::Less);
    assert_eq!(different.cmp(&first), std::cmp::Ordering::Greater);
}
