//! Concrete frame, environment identity and validation boundaries.
use super::*;

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
        assert_eq!(taken.environment(), expected.environment());
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
    let formatted = format!("{first:?}");
    assert!(
        formatted.len() < 100_000,
        "shared contexts must not expand during diagnostics"
    );
    assert_ne!(formatted, format!("{different:?}"));
    assert_eq!(first, equal);
    assert_eq!(first.cmp(&equal), std::cmp::Ordering::Equal);
    assert_eq!(first.cmp(&different), std::cmp::Ordering::Less);
    assert_eq!(different.cmp(&first), std::cmp::Ordering::Greater);
}

#[test]
fn one_closure_environment_keeps_distinct_occurrences_without_a_flat_capture_map() {
    use nocter_model::ValueField;
    let fixture = fixture();
    let program = &fixture.program;
    let data = program.function_data(fixture.captured).unwrap();
    let frames = fixture.callbacks.map(|callback| {
        program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap()
    });
    let leaves = frames
        .each_ref()
        .map(|frame| program.instantiate_data(data, frame).unwrap());
    let environment = program
        .instantiate_components(
            fixture.pair,
            [
                (ValueField::Tuple(0), leaves[0].clone()),
                (ValueField::Tuple(1), leaves[1].clone()),
            ],
        )
        .unwrap();
    let mixed = program
        .bind_callable_data(fixture.mixed, &environment)
        .unwrap();
    assert_eq!(mixed.environment(), Some(&environment));
    let key = program
        .instantiate_function(program.function_data(fixture.mixed).unwrap(), &mixed)
        .unwrap();
    assert_eq!(key.environment(program).unwrap(), Some(environment.clone()));
    let frame = program.bind_instance(&mixed, []).unwrap();
    assert_eq!(frame.environment(), Some(&environment));
    assert_eq!(
        program
            .code_template(frame.code())
            .unwrap()
            .close(program, &frame)
            .unwrap(),
        frame
    );
    assert!(
        frame
            .resolve(
                program,
                FunctionWitness {
                    origin: ValueFunctionOrigin::Parameter(fixture.formal),
                    invocation: ValueInvocation::Read
                }
            )
            .is_err(),
        "an ambiguous name cannot replace an exact component context"
    );
    for (position, leaf) in leaves.iter().enumerate() {
        assert_eq!(
            frame
                .environment()
                .unwrap()
                .component(program, ValueField::Tuple(position))
                .unwrap(),
            *leaf
        );
    }
    let uniform = program.instantiate_data(fixture.pair, &frames[0]).unwrap();
    let first = frames[0]
        .resolve(
            program,
            FunctionWitness {
                origin: ValueFunctionOrigin::Parameter(fixture.formal),
                invocation: ValueInvocation::Read,
            },
        )
        .unwrap();
    assert_eq!(
        program.bind_callable_data(fixture.mixed, &uniform).unwrap(),
        program
            .bind_callable_environment(fixture.mixed, [(fixture.formal, first)])
            .unwrap()
    );
    assert_ne!(
        program.bind_callable_data(fixture.mixed, &uniform).unwrap(),
        mixed
    );
    let taken = program
        .adapt_witness(&mixed, ValueInvocation::Take)
        .unwrap();
    assert_eq!(taken.environment(), mixed.environment());
    assert!(
        program
            .adapt_witness(&taken, ValueInvocation::Read)
            .is_err()
    );
    assert!(
        program
            .bind_callable_data(fixture.callbacks[0], &environment)
            .is_err()
    );
    assert!(
        program
            .bind_callable_data(fixture.mixed, &leaves[0])
            .is_err()
    );
}

#[test]
fn environment_validation_rejects_missing_contexts_and_noncanonical_closed_edges() {
    use crate::value_access::{DataInstanceKey, data_instances::Context};
    use nocter_model::ValueField;
    use std::sync::Arc;
    let fixture = fixture();
    let program = &fixture.program;
    let frame = program
        .bind_code(
            fixture.creator.into(),
            [(fixture.formal, fixture.callbacks[0])],
        )
        .unwrap();
    let valid = program.instantiate_data(fixture.pair, &frame).unwrap();
    let child = valid.component(program, ValueField::Tuple(0)).unwrap();
    for fields in [
        BTreeMap::new(),
        BTreeMap::from([(ValueField::Tuple(0), child.clone())]),
        BTreeMap::from([
            (ValueField::Tuple(0), child.clone()),
            (
                ValueField::Tuple(1),
                DataInstanceKey {
                    data: child.data(),
                    context: Arc::new(Context::Closed),
                },
            ),
        ]),
    ] {
        let incomplete = DataInstanceKey {
            data: valid.data(),
            context: Arc::new(Context::Components(fields)),
        };
        assert_eq!(
            program.bind_callable_data(fixture.mixed, &incomplete),
            Err(CodeBindingError::InvalidWitness)
        );
    }
    let closed = program
        .instantiate_closed_data(program.data_type(BuiltinType::Void).unwrap())
        .unwrap();
    let invalid = DataInstanceKey {
        data: valid.data(),
        context: Arc::new(Context::Components(BTreeMap::from([
            (ValueField::Tuple(0), child.clone()),
            (ValueField::Tuple(1), child),
            (ValueField::Tuple(2), closed),
        ]))),
    };
    assert_eq!(
        program.bind_callable_data(fixture.mixed, &invalid),
        Err(CodeBindingError::InvalidWitness)
    );
    assert!(program.bind_callable_data(fixture.mixed, &valid).is_ok());
    let environment = program.instantiate_closed_data(fixture.empty).unwrap();
    assert!(environment.is_context_free());
    assert!(
        program.bind_callable_data(fixture.mixed, &valid).is_ok(),
        "invalid requests must not poison a valid retry"
    );
}

#[test]
fn frame_environment_projection_retains_each_actual_component_and_rejects_invalid_paths() {
    use nocter_model::{FieldPath, ValueField};
    let fixture = fixture();
    let program = &fixture.program;
    let frames = fixture.callbacks.map(|callback| {
        program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap()
    });
    assert!(
        program
            .instantiate_environment(&frames[0])
            .unwrap()
            .is_none()
    );
    let data = program.function_data(fixture.captured).unwrap();
    let values = frames
        .each_ref()
        .map(|frame| program.instantiate_data(data, frame).unwrap());
    let environment = program
        .instantiate_components(
            fixture.pair,
            values
                .iter()
                .enumerate()
                .map(|(position, key)| (ValueField::Tuple(position), key.clone())),
        )
        .unwrap();
    let selected = program
        .bind_callable_data(fixture.mixed, &environment)
        .unwrap();
    let frame = program.bind_instance(&selected, []).unwrap();
    let received = program.instantiate_environment(&frame).unwrap().unwrap();
    assert_eq!(received, environment);
    assert_eq!(
        received
            .project_path(program, &FieldPath::default())
            .unwrap(),
        received
    );
    for (position, expected) in values.iter().enumerate() {
        assert_eq!(
            received
                .project_path(program, &FieldPath::new([ValueField::Tuple(position)]))
                .unwrap(),
            *expected
        );
    }
    assert!(
        received
            .project_path(program, &FieldPath::new([ValueField::Tuple(2)]))
            .is_err()
    );
    assert!(
        received
            .project_path(
                program,
                &FieldPath::new([ValueField::Tuple(0), ValueField::Tuple(0)])
            )
            .is_err(),
        "a callable environment is not a source field"
    );
    let closed = program
        .bind_code(fixture.callbacks[1].origin.code().unwrap(), [])
        .unwrap();
    assert!(
        program
            .instantiate_environment(&closed)
            .unwrap()
            .unwrap()
            .is_context_free()
    );
    let empty = program.instantiate_closed_data(fixture.empty).unwrap();
    assert!(
        empty
            .project_path(program, &FieldPath::new([ValueField::Array(0)]))
            .is_err()
    );
}
