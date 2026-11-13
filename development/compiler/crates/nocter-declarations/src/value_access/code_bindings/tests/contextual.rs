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
    empty: nocter_model::TypeId,
    pair: nocter_model::TypeId,
    large: nocter_model::TypeId,
}
fn fixture() -> Fixture {
    fixture_with_depth(0)
}
fn fixture_with_depth(depth: usize) -> Fixture {
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
    let captured_data = body.closure(captured, data, contract.clone()).unwrap();
    let empty = body.array(captured_data, 0).unwrap();
    let mut pair = body.tuple(&[captured_data, captured_data]).unwrap();
    for _ in 0..depth {
        pair = body.tuple(&[pair, pair]).unwrap();
    }
    let large = body.array(pair, u64::MAX).unwrap();
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
        empty,
        pair,
        large,
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

#[test]
fn contextual_data_preserves_semantic_identity_and_each_captured_environment() {
    use crate::value_access::InstanceSchema;
    let fixture = fixture();
    let program = &fixture.program;
    let data = program.function_data(fixture.captured).unwrap();
    let frames = fixture.callbacks.map(|callback| {
        program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap()
    });
    let instances = frames
        .each_ref()
        .map(|frame| program.instantiate_data(data, frame).unwrap());
    assert_eq!(instances[0].data(), data);
    assert_eq!(instances[1].data(), data);
    assert_ne!(instances[0], instances[1]);
    for (position, instance) in instances.iter().enumerate() {
        let InstanceSchema::Function(selected) = instance.schema(program).unwrap() else {
            panic!("function instance");
        };
        assert_eq!(
            selected,
            frames[position].resolve(program, fixture.captured).unwrap()
        );
        let environment = instance.environment(program).unwrap().unwrap();
        let InstanceSchema::Function(actual) = environment.schema(program).unwrap() else {
            panic!("captured input");
        };
        assert_eq!(actual.witness(), fixture.callbacks[position]);
    }
    let void = program.data_type(BuiltinType::Void).unwrap();
    assert_eq!(
        program.instantiate_data(void, &frames[0]),
        program.instantiate_data(void, &frames[1])
    );
}

#[test]
fn contextual_projection_does_not_flatten_two_occurrences_of_the_same_lexical_code() {
    use crate::value_access::InstanceSchema;
    let fixture = fixture();
    let program = &fixture.program;
    let contexts = fixture.callbacks.map(|callback| {
        let root = program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap();
        root.resolve(program, fixture.captured).unwrap()
    });
    let holder = program.concrete_witness(fixture.holder).unwrap();
    let frame = program
        .bind_instance(
            &holder,
            fixture.inputs.into_iter().zip(contexts.iter().cloned()),
        )
        .unwrap();
    let data = program.function_data(fixture.branching).unwrap();
    let instance = program.instantiate_data(data, &frame).unwrap();
    let environment = instance.environment(program).unwrap().unwrap();
    let InstanceSchema::Aggregate(schema) = environment.schema(program).unwrap() else {
        panic!("two components");
    };
    for (position, component) in schema.components().enumerate() {
        let child = environment.component(program, component.field).unwrap();
        let received = child.environment(program).unwrap().unwrap();
        let InstanceSchema::Function(actual) = received.schema(program).unwrap() else {
            panic!("captured callback");
        };
        assert_eq!(actual, contexts[position]);
    }
}

#[test]
fn empty_carrier_metadata_does_not_manufacture_a_positive_child_or_pending_producer() {
    use crate::value_access::InstanceSchema;
    let fixture = fixture();
    let program = &fixture.program;
    let root = program
        .bind_code(fixture.callbacks[0].origin.code().unwrap(), [])
        .unwrap();
    let empty = program.instantiate_data(fixture.empty, &root).unwrap();
    let InstanceSchema::Array(array) = empty.schema(program).unwrap() else {
        panic!("empty array");
    };
    assert!(array.is_empty());
    assert!(
        empty
            .component(program, nocter_model::ValueField::Array(0))
            .is_err()
    );
    let factory = program
        .resolve_code(program.symbols().get("factory").unwrap())
        .unwrap();
    let pending = program.codes().get(factory).unwrap().result();
    assert_eq!(
        program.instantiate_data(pending, &root),
        Err(CodeBindingError::InvalidWitness)
    );
}

#[test]
fn data_components_require_a_real_source_field_and_do_not_expose_function_backing() {
    use crate::value_access::InstanceSchema;
    use nocter_model::ValueField;
    let fixture = fixture();
    let program = &fixture.program;
    let frame = program
        .bind_code(
            fixture.creator.into(),
            [(fixture.formal, fixture.callbacks[0])],
        )
        .unwrap();
    let data = program.function_data(fixture.captured).unwrap();
    let function = program.instantiate_data(data, &frame).unwrap();
    assert!(function.environment(program).unwrap().is_some());
    for field in [
        ValueField::Tuple(0),
        ValueField::Variant(true),
        ValueField::Array(0),
    ] {
        assert_eq!(
            function.component(program, field),
            Err(CodeBindingError::InvalidWitness),
            "function backing is not a source field"
        );
    }
    let holder = program.concrete_witness(fixture.holder).unwrap();
    let values = fixture.callbacks.map(|callback| {
        program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap()
            .resolve(program, fixture.captured)
            .unwrap()
    });
    let frame = program
        .bind_instance(&holder, fixture.inputs.into_iter().zip(values))
        .unwrap();
    let branching = program
        .instantiate_data(program.function_data(fixture.branching).unwrap(), &frame)
        .unwrap();
    let environment = branching.environment(program).unwrap().unwrap();
    let InstanceSchema::Aggregate(schema) = environment.schema(program).unwrap() else {
        panic!("aggregate environment");
    };
    for component in schema.components() {
        assert_eq!(
            environment
                .component(program, component.field)
                .unwrap()
                .data(),
            component.data
        );
    }
    assert_eq!(
        environment.component(program, ValueField::Tuple(schema.len())),
        Err(CodeBindingError::InvalidWitness)
    );
}

#[test]
fn closed_data_construction_does_not_borrow_bindings_or_manufacture_empty_elements() {
    use nocter_model::ValueField;
    let fixture = fixture();
    let program = &fixture.program;
    let function = program.function_data(fixture.captured).unwrap();
    assert_eq!(
        program.instantiate_closed_data(function),
        Err(CodeBindingError::InvalidWitness)
    );
    let empty = program.instantiate_closed_data(fixture.empty).unwrap();
    assert!(empty.is_context_free());
    assert_eq!(
        empty.component(program, ValueField::Array(0)),
        Err(CodeBindingError::InvalidWitness)
    );
    let integer = program.data_type(BuiltinType::I32).unwrap();
    assert_eq!(
        program.instantiate_closed_data(integer).unwrap().data(),
        integer
    );
    let plain = program
        .instantiate_closed_data(program.function_data(fixture.callbacks[0]).unwrap())
        .unwrap();
    assert!(plain.is_context_free());
    assert!(plain.environment(program).unwrap().is_none());
}

#[test]
fn component_instances_preserve_two_contexts_of_the_identical_source_type() {
    use nocter_model::ValueField;
    let fixture = fixture();
    let program = &fixture.program;
    let frames = fixture.callbacks.map(|callback| {
        program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap()
    });
    let data = program.function_data(fixture.captured).unwrap();
    let leaves = frames
        .each_ref()
        .map(|frame| program.instantiate_data(data, frame).unwrap());
    assert_eq!(leaves[0].data(), leaves[1].data());
    assert_ne!(leaves[0], leaves[1]);
    let mixed = program
        .instantiate_components(
            fixture.pair,
            [
                (ValueField::Tuple(0), leaves[0].clone()),
                (ValueField::Tuple(1), leaves[1].clone()),
            ],
        )
        .unwrap();
    for (position, expected) in leaves.iter().enumerate() {
        assert_eq!(
            mixed
                .component(program, ValueField::Tuple(position))
                .unwrap(),
            *expected
        );
    }
    let uniform = program
        .instantiate_components(
            fixture.pair,
            [
                (ValueField::Tuple(1), leaves[0].clone()),
                (ValueField::Tuple(0), leaves[0].clone()),
            ],
        )
        .unwrap();
    assert_eq!(
        uniform,
        program.instantiate_data(fixture.pair, &frames[0]).unwrap()
    );
    assert_ne!(mixed, uniform);
    assert!(
        program
            .instantiate_components(fixture.pair, [(ValueField::Tuple(0), leaves[0].clone())])
            .is_err()
    );
    assert!(
        program
            .instantiate_components(
                fixture.pair,
                [
                    (ValueField::Tuple(0), leaves[0].clone()),
                    (ValueField::Tuple(0), leaves[1].clone()),
                ]
            )
            .is_err()
    );
    assert!(
        program
            .instantiate_components(fixture.empty, [(ValueField::Array(0), leaves[0].clone())])
            .is_err()
    );
    assert!(program.instantiate_components(data, []).is_err());
    assert!(
        program
            .instantiate_components(
                fixture.pair,
                [
                    (
                        ValueField::Tuple(0),
                        program
                            .instantiate_closed_data(program.data_type(BuiltinType::Void).unwrap())
                            .unwrap()
                    ),
                    (ValueField::Tuple(1), leaves[1].clone()),
                ]
            )
            .is_err()
    );
}

#[test]
fn component_contexts_share_schema_dags_and_compare_without_expanding_cardinality() {
    use nocter_model::ValueField;
    let fixture = fixture_with_depth(64);
    let array = fixture.large;
    let program = fixture.program;
    let frames = fixture.callbacks.map(|callback| {
        program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap()
    });
    let left = program.instantiate_data(array, &frames[0]).unwrap();
    let equal = program.instantiate_data(array, &frames[0]).unwrap();
    let right = program.instantiate_data(array, &frames[1]).unwrap();
    assert_eq!(left, equal);
    assert_ne!(left, right);
    assert_eq!(left.cmp(&right), right.cmp(&left).reverse());
    let element = left
        .component(&program, ValueField::Array(u64::MAX - 1))
        .unwrap();
    let assembled = program
        .instantiate_components(array, [(ValueField::Array(0), element)])
        .unwrap();
    assert_eq!(left, assembled);
    assert!(
        left.component(&program, ValueField::Array(u64::MAX))
            .is_err()
    );
}

#[test]
fn function_occurrence_construction_keeps_context_without_guessing_an_outer_frame() {
    use crate::value_access::InstanceSchema;
    let fixture = fixture();
    let program = &fixture.program;
    let data = program.function_data(fixture.captured).unwrap();
    let formal = program
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(fixture.formal),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    let actuals = fixture.callbacks.map(|callback| {
        program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap()
            .resolve(program, fixture.captured)
            .unwrap()
    });
    let instances = actuals
        .each_ref()
        .map(|actual| program.instantiate_function(data, actual).unwrap());
    assert_ne!(instances[0], instances[1]);
    for (position, actual) in actuals.iter().enumerate() {
        let formal_instance = program.instantiate_function(formal, actual).unwrap();
        assert_eq!(formal_instance.data(), formal);
        assert_eq!(
            formal_instance.function_representation(program).unwrap(),
            Some(instances[position].clone())
        );
        let InstanceSchema::Function(received) = formal_instance.schema(program).unwrap() else {
            panic!("function");
        };
        assert_eq!(&received, actual);
        let frame = program
            .bind_code(
                fixture.creator.into(),
                [(fixture.formal, fixture.callbacks[position])],
            )
            .unwrap();
        assert_eq!(
            instances[position],
            program.instantiate_data(data, &frame).unwrap()
        );
    }
    let plain_data = program.function_data(fixture.callbacks[0]).unwrap();
    let wrong = program
        .concrete_witness(
            program
                .resolve_code(program.symbols().get("wrong").unwrap())
                .unwrap()
                .into(),
        )
        .unwrap();
    assert!(matches!(
        program.instantiate_function(formal, &wrong),
        Err(CodeBindingError::Contract(FunctionMismatch::Result { .. }))
    ));
    assert!(
        program
            .instantiate_function(plain_data, &actuals[0])
            .is_err(),
        "equal signatures do not replace concrete code"
    );
    assert!(
        program
            .instantiate_function(program.data_type(BuiltinType::Void).unwrap(), &actuals[0])
            .is_err()
    );
    let factory = program
        .resolve_code(program.symbols().get("factory").unwrap())
        .unwrap();
    assert!(
        program
            .instantiate_function(program.codes().get(factory).unwrap().result(), &actuals[0])
            .is_err(),
        "a producer contract alone does not identify a supplied occurrence"
    );
    let taken = program
        .adapt_witness(&actuals[0], ValueInvocation::Take)
        .unwrap();
    assert!(program.instantiate_function(data, &taken).is_err());
    let taken_data = program
        .function_data(FunctionWitness {
            invocation: ValueInvocation::Take,
            ..fixture.captured
        })
        .unwrap();
    let weakened = program
        .instantiate_function(taken_data, &actuals[0])
        .unwrap();
    let InstanceSchema::Function(selected) = weakened.schema(program).unwrap() else {
        panic!("function");
    };
    assert_eq!(selected, taken);
}
