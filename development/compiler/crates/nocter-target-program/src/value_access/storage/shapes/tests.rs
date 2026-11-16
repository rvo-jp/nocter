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
    optional: TypeId,
    optional_integer: TypeId,
    fallible: TypeId,
    wrapper: FunctionWitness,
}

#[test]
fn uninhabited_metadata_is_not_a_materialized_callable_or_backing() {
    let fixture = fixture();
    let program = &fixture.program;
    let empty = program
        .instantiate_uninhabited_data(fixture.captured)
        .unwrap();
    let mut builder = Builder::new(program);
    assert!(matches!(
        builder.intern(empty),
        Err(Failure::InvalidPayload(_))
    ));
    assert!(builder.ids.is_empty());
    assert!(builder.pending.is_empty());
    assert_eq!(builder.next, 0);
    assert!(builder.intern(fixture.keys[0].clone()).is_ok());
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
    let optional = body.optional(captured).unwrap();
    let optional_integer = body.optional(result).unwrap();
    let fallible = body.fallible(captured).unwrap();
    let wrapper = body.reserve_closure().unwrap();
    body.closure(
        wrapper,
        tuple,
        FunctionConstraint {
            invocation: ValueInvocation::Read,
            inputs: Box::new([]),
            result,
        },
    )
    .unwrap();
    let wrapper = FunctionWitness {
        origin: ValueFunctionOrigin::Closure(wrapper),
        invocation: ValueInvocation::Read,
    };
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
        optional,
        optional_integer,
        fallible,
        wrapper,
    }
}

#[test]
fn outcome_storage_retains_excluded_primary_metadata_without_materializing_a_callable() {
    let fixture = fixture();
    let program = &fixture.program;
    let empty = program
        .instantiate_uninhabited_data(fixture.captured)
        .unwrap();
    let optional = program
        .instantiate_components(
            fixture.optional,
            [(ValueField::Variant(true), empty.clone())],
        )
        .unwrap();
    let error = program.data_type(BuiltinType::Error).unwrap();
    let fallible = program
        .instantiate_components(
            fixture.fallible,
            [
                (ValueField::Variant(true), empty),
                (
                    ValueField::Variant(false),
                    program.instantiate_closed_data(error).unwrap(),
                ),
            ],
        )
        .unwrap();
    let mut builder = Builder::new(program);
    let a = builder.intern(optional).unwrap();
    let absent_integer = program
        .instantiate_components(
            fixture.optional_integer,
            [(
                ValueField::Variant(true),
                program
                    .instantiate_uninhabited_data(program.data_type(BuiltinType::I32).unwrap())
                    .unwrap(),
            )],
        )
        .unwrap();
    assert_ne!(absent_integer.data(), fixture.optional);
    assert_eq!(
        builder.intern(absent_integer).unwrap(),
        a,
        "empty runtime domains do not retain a counterfeit callable identity"
    );
    let b = builder.intern(fallible).unwrap();
    let runtime = builder
        .finish(RuntimeAbiIdentity::Arm64DarwinV1)
        .unwrap()
        .into_environment();
    let Some(RuntimeType::Optional(primary)) = runtime.types().get(a) else {
        panic!("optional");
    };
    assert_eq!(
        runtime.types().get(*primary),
        Some(&RuntimeType::Uninhabited)
    );
    assert_eq!(
        runtime.types().get(b),
        Some(&RuntimeType::Fallible(*primary))
    );
    assert!(
        !runtime
            .types()
            .iter()
            .any(|(_, kind)| matches!(kind, RuntimeType::StaticCallable { .. }))
    );
    let unsupported = program
        .instantiate_components(
            fixture.fallible,
            [
                (
                    ValueField::Variant(true),
                    fixture.keys[0]
                        .component(program, ValueField::Tuple(0))
                        .unwrap(),
                ),
                (
                    ValueField::Variant(false),
                    program.instantiate_uninhabited_data(error).unwrap(),
                ),
            ],
        )
        .unwrap();
    let mut builder = Builder::new(program);
    assert!(builder.intern(unsupported).is_err());
    assert!(builder.ids.is_empty());
    assert!(builder.pending.is_empty());
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
            .intern(key.component(program, ValueField::Tuple(0)).unwrap())
            .unwrap()
    });
    let arrays = fixture.keys.each_ref().map(|key| {
        builder
            .intern(key.component(program, ValueField::Tuple(1)).unwrap())
            .unwrap()
    });
    let environments = fixture.keys.each_ref().map(|key| {
        let child = key.component(program, ValueField::Tuple(0)).unwrap();
        let environment = child.environment(program).unwrap().unwrap();
        builder.intern(environment).unwrap()
    });
    for (key, expected) in fixture.keys.iter().zip(leaves) {
        let child = key.component(program, ValueField::Tuple(0)).unwrap();
        let actual = child.function_representation(program).unwrap().unwrap();
        assert_eq!(builder.intern(actual).unwrap(), expected);
    }
    let prepared = builder.finish(RuntimeAbiIdentity::Arm64DarwinV1).unwrap();
    let runtime = prepared.environment();
    for (position, key) in fixture.keys.iter().enumerate() {
        let leaf = key.component(program, ValueField::Tuple(0)).unwrap();
        let array = key.component(program, ValueField::Tuple(1)).unwrap();
        assert_eq!(leaf.data(), fixture.captured);
        assert_eq!(array.data(), fixture.array);
        assert_eq!(prepared.instance(program, &leaf).unwrap(), leaves[position]);
        assert_eq!(
            prepared.instance(program, &array).unwrap(),
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
    let closed = program.instantiate_closed_data(fixture.empty).unwrap();
    let id = builder.intern(closed.clone()).unwrap();
    for frame in &fixture.frames {
        let bound = program.instantiate_data(fixture.empty, frame).unwrap();
        assert_eq!(bound, closed);
        assert_eq!(builder.intern(bound).unwrap(), id);
    }
    let prepared = builder.finish(RuntimeAbiIdentity::Arm64DarwinV1).unwrap();
    let runtime = prepared.environment();
    assert_eq!(runtime.types().iter().count(), 1);
    assert_eq!(
        runtime.types().get(id),
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
            .instance(
                program,
                &fixture.keys[0]
                    .component(program, ValueField::Tuple(0))
                    .unwrap()
            )
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
    assert_eq!(prepared.instance(program, &fixture.keys[0]).unwrap(), first);
    assert_eq!(
        prepared.instance(program, &fixture.keys[1]),
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
        .component(program, ValueField::Tuple(0))
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
    let id = builder.intern(key.clone()).unwrap();
    let prepared = builder.finish(RuntimeAbiIdentity::Arm64DarwinV1).unwrap();
    assert_eq!(prepared.instance(program, &key).unwrap(), id);
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
        let key = key.component(program, ValueField::Tuple(0)).unwrap();
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

#[test]
fn mixed_component_contexts_keep_their_native_environment_and_cleanup_metadata() {
    use nocter_checking::value_access::flow::resources;
    let fixture = fixture();
    let program = &fixture.program;
    let first = fixture.keys[0]
        .component(program, ValueField::Tuple(0))
        .unwrap();
    let second = fixture.keys[1]
        .component(program, ValueField::Tuple(1))
        .unwrap();
    let mixed = program
        .instantiate_components(
            fixture.keys[0].data(),
            [
                (ValueField::Tuple(0), first.clone()),
                (ValueField::Tuple(1), second.clone()),
            ],
        )
        .unwrap();
    assert!(fixture.keys.iter().all(|key| key != &mixed));
    let mut builder = Builder::new(program);
    let root = builder.intern(mixed.clone()).unwrap();
    let first_id = builder.intern(first.clone()).unwrap();
    let array_id = builder.intern(second.clone()).unwrap();
    let second_leaf = second.component(program, ValueField::Array(1)).unwrap();
    let second_id = builder.intern(second_leaf.clone()).unwrap();
    assert_ne!(first_id, second_id);
    let prepared = builder.finish(RuntimeAbiIdentity::Arm64DarwinV1).unwrap();
    let runtime = prepared.environment();
    assert_eq!(
        runtime.types().get(array_id),
        Some(&RuntimeType::FixedArray {
            element: second_id,
            length: 2
        })
    );
    let Some(RuntimeTypeRepresentation::ValueAggregate { fields }) =
        runtime.type_representations().get(root)
    else {
        panic!("aggregate");
    };
    assert_eq!(fields[0].ty(), first_id);
    assert_eq!(fields[1].ty(), array_id);
    let payload = resources::payload(program, mixed.data())
        .unwrap()
        .instantiate(program, &mixed)
        .unwrap();
    assert_eq!(payload.instance(), &mixed);
    let left = resources::payload(program, first.data())
        .unwrap()
        .instantiate(program, &first)
        .unwrap()
        .environment(program)
        .unwrap()
        .unwrap();
    let right = resources::payload(program, second_leaf.data())
        .unwrap()
        .instantiate(program, &second_leaf)
        .unwrap()
        .environment(program)
        .unwrap()
        .unwrap();
    assert_ne!(left, right);
    assert!(left.environment(program).unwrap().is_none());
    let owned_error = right.environment(program).unwrap().unwrap();
    assert_eq!(
        program.data_kind(owned_error.data()),
        Some(BuiltinType::Error)
    );
}

#[test]
fn complete_callable_environment_keeps_mixed_contexts_through_storage_and_payload_projection() {
    use nocter_checking::value_access::flow::resources;
    let fixture = fixture();
    let program = &fixture.program;
    let environment = program
        .instantiate_components(
            fixture.keys[0].data(),
            [
                (
                    ValueField::Tuple(0),
                    fixture.keys[0]
                        .component(program, ValueField::Tuple(0))
                        .unwrap(),
                ),
                (
                    ValueField::Tuple(1),
                    fixture.keys[1]
                        .component(program, ValueField::Tuple(1))
                        .unwrap(),
                ),
            ],
        )
        .unwrap();
    let witness = program
        .bind_callable_data(fixture.wrapper, &environment)
        .unwrap();
    let key = program
        .instantiate_function(program.function_data(fixture.wrapper).unwrap(), &witness)
        .unwrap();
    let mut builder = Builder::new(program);
    let root = builder.intern(key.clone()).unwrap();
    let expected = builder.intern(environment.clone()).unwrap();
    let prepared = builder.finish(RuntimeAbiIdentity::Arm64DarwinV1).unwrap();
    assert_eq!(
        prepared.environment().types().get(root),
        Some(&RuntimeType::StaticCallable {
            environment: Some(expected)
        })
    );
    let payload = resources::payload(program, key.data())
        .unwrap()
        .instantiate(program, &key)
        .unwrap();
    assert_eq!(
        payload.environment(program).unwrap().unwrap().instance(),
        &environment
    );
}

#[test]
fn positive_callable_storage_retains_every_complete_environment_in_canonical_order() {
    let fixture = fixture();
    let program = &fixture.program;
    let keys = fixture
        .keys
        .iter()
        .map(|key| key.component(program, ValueField::Tuple(0)).unwrap())
        .collect::<Vec<_>>();
    let domain = program
        .join_data_domains(&keys[0], &keys[1], || Ok::<_, ()>(()))
        .unwrap();
    let cases = domain.function_representations(program).unwrap().unwrap();
    let creator = program
        .resolve_code(program.symbols().get("creator").unwrap())
        .unwrap();
    let parameter = program.codes().get(creator).unwrap().parameters()[0];
    let formal = program.parameters().get(parameter).unwrap().data();
    let received = domain.reproject(program, formal).unwrap();
    let array = program
        .instantiate_components(fixture.array, [(ValueField::Array(0), domain.clone())])
        .unwrap();
    assert_eq!(cases.len(), 2);
    assert!(domain.function_representation(program).is_err());
    assert!(domain.environment(program).is_err());
    let prepared = super::super::prepare_data(
        program,
        RuntimeAbiIdentity::Arm64DarwinV1,
        [
            domain.clone(),
            received.clone(),
            array.clone(),
            fixture.keys[0].clone(),
            fixture.keys[1].clone(),
        ],
    )
    .unwrap();
    let id = prepared.instance(program, &domain).unwrap();
    assert_eq!(prepared.instance(program, &received).unwrap(), id);
    assert_eq!(
        prepared
            .environment()
            .types()
            .get(prepared.instance(program, &array).unwrap()),
        Some(&RuntimeType::FixedArray {
            element: id,
            length: 2
        })
    );
    let expected = cases
        .iter()
        .map(|case| prepared.instance(program, case).unwrap())
        .collect::<Box<[_]>>();
    assert_ne!(expected[0], expected[1]);
    let runtime = prepared.environment();
    assert_eq!(
        runtime.types().get(id),
        Some(&RuntimeType::CallableAlternatives(expected.clone()))
    );
    let environments = expected
        .iter()
        .map(|case| match runtime.types().get(*case).unwrap() {
            RuntimeType::StaticCallable { environment } => *environment,
            _ => panic!("complete static case"),
        })
        .collect::<Vec<_>>();
    assert_ne!(environments[0], environments[1]);
    assert!(
        runtime
            .types()
            .primitive(RuntimePrimitive::Unsigned(8))
            .is_some()
    );
    for (index, (case, expected)) in cases.iter().zip(expected.iter()).enumerate() {
        let mapping = runtime.callable_case_targets(*expected, id).unwrap();
        assert_eq!(mapping.len(), 1);
        assert_eq!(mapping[0].data(), *expected);
        assert_eq!(mapping[0].target(), u64::try_from(index).unwrap());
        let key = case.environment(program).unwrap().unwrap();
        assert_eq!(
            runtime.types().get(*expected),
            Some(&RuntimeType::StaticCallable {
                environment: Some(prepared.instance(program, &key).unwrap()),
            })
        );
    }
    let reverse = program
        .join_data_domains(&keys[1], &keys[0], || Ok::<_, ()>(()))
        .unwrap();
    assert_eq!(prepared.instance(program, &reverse).unwrap(), id);
}

#[test]
fn finite_domain_storage_closes_the_runtime_owned_tag_primitive_at_257_cases() {
    use std::fmt::Write as _;
    let mut source = String::new();
    for index in 0..257 {
        writeln!(source, "func case_{index}(): i32 {{ {index} }}").unwrap();
    }
    source.push_str("func creator(take action: func(): i32): i32 { action() }");
    let fixture = ValueSourceFixture::new(&source);
    let query =
        nocter_declaration_lowering::access_declarations::Query::prepare(fixture.syntax()).unwrap();
    let context = query.bind(fixture.syntax()).unwrap();
    let program = context.program();
    let creator = program
        .resolve_code(program.symbols().get("creator").unwrap())
        .unwrap();
    let formal = program
        .parameters()
        .get(program.codes().get(creator).unwrap().parameters()[0])
        .unwrap()
        .data();
    let actuals = (0..257).map(|index| {
        let code = program
            .resolve_code(program.symbols().get(&format!("case_{index}")).unwrap())
            .unwrap();
        program.concrete_witness(code.into()).unwrap()
    });
    let domain = program
        .instantiate_function_alternatives(formal, actuals)
        .unwrap();
    let prepared =
        super::super::prepare_data(program, RuntimeAbiIdentity::Arm64DarwinV1, [domain.clone()])
            .unwrap();
    let id = prepared.instance(program, &domain).unwrap();
    let types = prepared.environment().types();
    assert!(types.primitive(RuntimePrimitive::Unsigned(16)).is_some());
    assert!(types.primitive(RuntimePrimitive::Unsigned(8)).is_none());
    let Some(RuntimeType::CallableAlternatives(cases)) = types.get(id) else {
        panic!("complete domain")
    };
    assert_eq!(cases.len(), 257);
    for case in cases {
        assert!(matches!(
            types.get(*case),
            Some(RuntimeType::StaticCallable { environment: None })
        ));
    }
}
