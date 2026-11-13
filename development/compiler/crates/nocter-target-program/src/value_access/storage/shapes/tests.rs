use super::*;
use nocter_declarations::value_access::{FunctionConstraint, FunctionWitness};
use nocter_model::{TypeId, ValueFunctionOrigin, ValueInvocation, ValueParameter};
use nocter_test_support::ValueSourceFixture;

struct Fixture {
    program: Program,
    keys: [DataInstanceKey; 2],
    frames: [nocter_declarations::value_access::CodeInstanceKey; 2],
    captured: TypeId,
    array: TypeId,
    empty: TypeId,
}
fn fixture() -> Fixture {
    let source = ValueSourceFixture::new(
        "func plain(): i32 { 41 }\nfunc creator(take action: func(): i32): i32 { action() }",
    );
    let query =
        nocter_declaration_lowering::access_declarations::Query::prepare(source.syntax()).unwrap();
    let context = query.bind(source.syntax()).unwrap();
    let headers = context.program();
    let code = |name| {
        headers
            .resolve_code(headers.symbols().get(name).unwrap())
            .unwrap()
    };
    let creator = code("creator");
    let plain = code("plain");
    let parameter: ValueParameter = headers.codes().get(creator).unwrap().parameters()[0].into();
    let formal = headers
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(parameter),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    let result = headers.data_type(BuiltinType::I32).unwrap();
    let error = headers.data_type(BuiltinType::Error).unwrap();
    let mut body = headers.body_types(creator).unwrap();
    let contract = FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([]),
        result,
    };
    let closure = body.reserve_closure().unwrap();
    let captured = body.closure(closure, formal, contract.clone()).unwrap();
    let second = body.reserve_closure().unwrap();
    body.closure(second, error, contract).unwrap();
    let callbacks = [
        plain.into(),
        FunctionWitness {
            origin: ValueFunctionOrigin::Closure(second),
            invocation: ValueInvocation::Read,
        },
    ];
    let array = body.array(captured, 2).unwrap();
    let tuple = body.tuple(&[captured, array]).unwrap();
    let empty = body.array(captured, 0).unwrap();
    let program = body.finish();
    let frames = callbacks.map(|callback| {
        program
            .bind_code(creator.into(), [(parameter, callback)])
            .unwrap()
    });
    let keys = frames
        .each_ref()
        .map(|frame| program.instantiate_data(tuple, frame).unwrap());
    Fixture {
        program,
        keys,
        frames,
        captured,
        array,
        empty,
    }
}

#[test]
fn contextual_native_materialization_keeps_each_nested_environment_and_reuses_actual_leaves() {
    let fixture = fixture();
    let program = &fixture.program;
    let mut builder = Builder::new(program);
    let roots = fixture
        .keys
        .each_ref()
        .map(|key| builder.intern(key.clone()).unwrap());
    assert_ne!(roots[0], roots[1]);
    assert!(
        roots
            .iter()
            .all(|id| matches!(id, RuntimeDataId::Instance(_)))
    );
    let leaves = fixture.keys.each_ref().map(|key| {
        builder
            .intern(key.project(program, fixture.captured).unwrap())
            .unwrap()
    });
    let arrays = fixture.keys.each_ref().map(|key| {
        builder
            .intern(key.project(program, fixture.array).unwrap())
            .unwrap()
    });
    let environments = fixture.keys.each_ref().map(|key| {
        let child = key.project(program, fixture.captured).unwrap();
        let environment = child.environment(program).unwrap().unwrap();
        builder.intern(environment).unwrap()
    });
    for (key, expected) in fixture.keys.iter().zip(leaves) {
        let child = key.project(program, fixture.captured).unwrap();
        let actual = child.function_representation(program).unwrap().unwrap();
        assert_eq!(builder.intern(actual).unwrap(), expected);
    }
    let prepared = builder.finish(RuntimeAbiIdentity::Arm64DarwinV1).unwrap();
    let runtime = prepared.environment();
    for (position, frame) in fixture.frames.iter().enumerate() {
        assert_eq!(
            prepared.data(program, frame, fixture.captured).unwrap(),
            leaves[position]
        );
        assert_eq!(
            prepared.data(program, frame, fixture.array).unwrap(),
            arrays[position]
        );
        assert_eq!(
            runtime.types().get(leaves[position]),
            Some(&RuntimeType::StaticCallable {
                environment: Some(environments[position])
            })
        );
        assert_eq!(
            runtime.types().get(arrays[position]),
            Some(&RuntimeType::FixedArray {
                element: leaves[position],
                length: 2
            })
        );
        let Some(RuntimeTypeRepresentation::ValueAggregate { fields }) =
            runtime.type_representations().get(roots[position])
        else {
            panic!("tuple storage");
        };
        assert_eq!(
            fields.iter().map(|field| field.ty()).collect::<Vec<_>>(),
            [leaves[position], arrays[position]]
        );
    }
    assert_ne!(environments[0], environments[1]);
    assert_eq!(
        runtime.types().get(environments[0]),
        Some(&RuntimeType::StaticCallable { environment: None })
    );
    let Some(RuntimeType::StaticCallable {
        environment: Some(error),
    }) = runtime.types().get(environments[1])
    else {
        panic!("owned error callback environment");
    };
    assert_eq!(
        runtime.types().get(*error),
        Some(&RuntimeType::Primitive(RuntimePrimitive::Error))
    );
}

#[test]
fn empty_contextual_array_keeps_only_source_element_metadata() {
    let fixture = fixture();
    let program = &fixture.program;
    let mut builder = Builder::new(program);
    let ids = fixture.keys.each_ref().map(|key| {
        builder
            .intern(key.project(program, fixture.empty).unwrap())
            .unwrap()
    });
    assert_eq!(ids[0], ids[1]);
    let prepared = builder.finish(RuntimeAbiIdentity::Arm64DarwinV1).unwrap();
    let runtime = prepared.environment();
    assert_eq!(runtime.types().iter().count(), 1);
    assert_eq!(
        runtime.types().get(ids[0]),
        Some(&RuntimeType::EmptyArray {
            element: RuntimeDataId::Source(fixture.captured)
        })
    );
    assert!(
        runtime
            .types()
            .get(RuntimeDataId::Source(fixture.captured))
            .is_none()
    );
    assert!(
        prepared
            .data(program, &fixture.frames[0], fixture.captured)
            .is_err()
    );
}

#[test]
fn prepared_lookup_does_not_materialize_a_missing_context() {
    let fixture = fixture();
    let program = &fixture.program;
    let mut builder = Builder::new(program);
    let first = builder.intern(fixture.keys[0].clone()).unwrap();
    let prepared = builder.finish(RuntimeAbiIdentity::Arm64DarwinV1).unwrap();
    assert_eq!(
        prepared
            .data(program, &fixture.frames[0], fixture.keys[0].data())
            .unwrap(),
        first
    );
    assert_eq!(
        prepared.data(program, &fixture.frames[1], fixture.keys[1].data()),
        Err(Failure::InvalidPayload(fixture.keys[1].data()))
    );
}

#[test]
fn nested_occurrences_of_one_lexical_closure_materialize_as_a_finite_graph() {
    let fixture = fixture();
    let program = &fixture.program;
    let frame = &fixture.frames[0];
    let parameter = *frame.witnesses().keys().next().unwrap();
    let mut selected = frame.witnesses()[&parameter].clone();
    let InstanceSchema::Function(wrapper) = fixture.keys[0]
        .project(program, fixture.captured)
        .unwrap()
        .schema(program)
        .unwrap()
    else {
        panic!("captured function");
    };
    for _ in 0..64 {
        selected = program
            .bind_callable_environment(wrapper.witness(), [(parameter, selected)])
            .unwrap();
    }
    let creator = program
        .concrete_witness(FunctionWitness {
            origin: frame.code().origin().unwrap(),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    let frame = program
        .bind_instance(&creator, [(parameter, selected)])
        .unwrap();
    let key = program.instantiate_data(fixture.captured, &frame).unwrap();
    let mut builder = Builder::new(program);
    let id = builder.intern(key).unwrap();
    let prepared = builder.finish(RuntimeAbiIdentity::Arm64DarwinV1).unwrap();
    assert_eq!(
        prepared.data(program, &frame, fixture.captured).unwrap(),
        id
    );
    assert_eq!(prepared.environment().types().iter().count(), 66);
    let mut next = Some(id);
    let mut count = 0;
    while let Some(id) = next {
        let Some(RuntimeType::StaticCallable { environment }) =
            prepared.environment().types().get(id)
        else {
            panic!("callable environment chain");
        };
        count += 1;
        next = *environment;
    }
    assert_eq!(count, 66);
}

#[test]
fn logical_payload_projection_keeps_context_without_replacing_source_operations() {
    use nocter_checking::value_access::flow::resources::{self, Disposal, Holder, Step};
    let fixture = fixture();
    let program = &fixture.program;
    let contract = resources::payload(program, fixture.keys[0].data()).unwrap();
    let source = Holder::Return.place();
    let step = Step::Dispose {
        source: source.clone(),
        payload: contract,
    };
    let projected = fixture.keys.each_ref().map(|key| {
        step.try_map_payloads(|payload| payload.instantiate(program, key))
            .unwrap()
    });
    assert_ne!(projected[0], projected[1]);
    for (position, projected) in projected.iter().enumerate() {
        let Step::Dispose {
            source: actual,
            payload,
        } = projected
        else {
            panic!("same disposal operation");
        };
        assert_eq!(actual, &source);
        assert_eq!(payload.data(), contract.data());
        assert_eq!(payload.disposal(), Disposal::Aggregate);
        assert_eq!(payload.instance(), &fixture.keys[position]);
    }
    let scalar = resources::payload(program, program.data_type(BuiltinType::I32).unwrap()).unwrap();
    assert!(scalar.instantiate(program, &fixture.keys[0]).is_err());
    assert_eq!(
        step,
        Step::Dispose {
            source,
            payload: contract
        }
    );
}

#[test]
fn instantiated_payload_environment_uses_the_actual_captured_value_context() {
    use nocter_checking::value_access::flow::resources::{self, Disposal};
    let fixture = fixture();
    let program = &fixture.program;
    let contract = resources::payload(program, fixture.captured).unwrap();
    let payloads = fixture.keys.each_ref().map(|key| {
        let key = key.project(program, fixture.captured).unwrap();
        contract.instantiate(program, &key).unwrap()
    });
    assert_eq!(payloads[0].data(), payloads[1].data());
    assert_ne!(payloads[0], payloads[1]);
    for payload in &payloads {
        assert_eq!(payload.disposal(), Disposal::OwnedCapture);
    }
    let first = payloads[0].environment(program).unwrap().unwrap();
    let second = payloads[1].environment(program).unwrap().unwrap();
    assert_eq!(first.disposal(), Disposal::Trivial);
    assert!(first.environment(program).unwrap().is_none());
    assert_eq!(second.disposal(), Disposal::OwnedCapture);
    let nested = second.environment(program).unwrap().unwrap();
    assert_eq!(nested.disposal(), Disposal::OwnedError);
    assert_eq!(program.data_kind(nested.data()), Some(BuiltinType::Error));
}
