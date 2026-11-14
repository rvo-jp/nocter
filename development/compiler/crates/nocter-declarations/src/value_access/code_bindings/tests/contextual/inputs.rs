//! Received structural contexts remain distinct from direct parameters and captured environments.
use super::*;
use crate::value_access::InputBindingError;
use nocter_model::{ValueField, ValueParameter};

fn contexts(fixture: &Fixture) -> [crate::value_access::DataInstanceKey; 2] {
    let program = &fixture.program;
    let data = program.function_data(fixture.captured).unwrap();
    let leaves = fixture.callbacks.map(|callback| {
        let frame = program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap();
        program.instantiate_data(data, &frame).unwrap()
    });
    [0, 1].map(|position| {
        program
            .instantiate_components(
                fixture.pair,
                [
                    (ValueField::Tuple(0), leaves[position].clone()),
                    (ValueField::Tuple(1), leaves[1].clone()),
                ],
            )
            .unwrap()
    })
}
fn direct(fixture: &Fixture) -> ValueParameter {
    let ValueFunctionOrigin::Closure(owner) = fixture.receiver.origin else {
        panic!("receiver")
    };
    ValueParameter::Closure { owner, position: 1 }
}

#[test]
fn received_compound_contexts_distinguish_equal_code_environments_and_direct_bindings() {
    let fixture = fixture();
    let program = &fixture.program;
    let selected = program.concrete_witness(fixture.receiver).unwrap();
    assert_eq!(
        program.compound_input_contracts(fixture.receiver).unwrap(),
        std::collections::BTreeMap::from([(0, fixture.pair)])
    );
    let callback = program.concrete_witness(fixture.callbacks[0]).unwrap();
    let values = contexts(&fixture);
    assert_eq!(values[0].data(), values[1].data());
    assert_ne!(values[0], values[1]);
    let frames = values.each_ref().map(|value| {
        program
            .bind_instance_with_inputs(
                &selected,
                [(direct(&fixture), callback.clone())],
                [(0, value.clone())],
            )
            .unwrap()
    });
    assert_ne!(frames[0], frames[1]);
    assert_eq!(frames[0].code(), frames[1].code());
    assert_eq!(frames[0].witnesses(), frames[1].witnesses());
    assert_eq!(frames[0].environment(), frames[1].environment());
    for (frame, value) in frames.iter().zip(&values) {
        assert_eq!(frame.input_data(0), Some(value));
        for index in [1, 2, 3, usize::MAX] {
            assert!(frame.input_data(index).is_none());
        }
        assert_eq!(
            frame.resolve(program, fixture.captured),
            Err(CodeBindingError::Missing(fixture.formal))
        );
    }
    let template = program.code_template(selected.code()).unwrap();
    assert!(
        template
            .close(program, &frames[0])
            .unwrap()
            .input_data(0)
            .is_none()
    );
    assert_eq!(
        program
            .bind_instance_with_inputs(
                &selected,
                [(direct(&fixture), callback)],
                [(0, values[0].clone())]
            )
            .unwrap(),
        frames[0]
    );
    assert!(!program.owned_payload_supported(fixture.pair));
    assert_eq!(program.data_copyable(fixture.pair), Some(false));
}

#[test]
fn compound_input_binding_rejects_missing_duplicate_surplus_and_foreign_schema_contexts() {
    let fixture = fixture();
    let program = &fixture.program;
    let selected = program.concrete_witness(fixture.receiver).unwrap();
    let callback = program.concrete_witness(fixture.callbacks[0]).unwrap();
    let [value, _] = contexts(&fixture);
    let bind = |inputs| {
        program.bind_instance_with_inputs(&selected, [(direct(&fixture), callback.clone())], inputs)
    };
    assert_eq!(bind(vec![]), Err(InputBindingError::Missing(0)));
    assert_eq!(
        bind(vec![(0, value.clone()), (0, value.clone())]),
        Err(InputBindingError::Duplicate(0))
    );
    for index in [1, 2, 3, usize::MAX] {
        assert_eq!(
            bind(vec![(0, value.clone()), (index, value.clone())]),
            Err(InputBindingError::Unexpected(index))
        );
    }
    let wrong = program
        .instantiate_closed_data(program.data_type(BuiltinType::Void).unwrap())
        .unwrap();
    assert_eq!(bind(vec![(0, wrong)]), Err(InputBindingError::Schema(0)));
    assert_eq!(
        program.bind_instance_with_inputs(&selected, [], [(0, value)]),
        Err(InputBindingError::Code(CodeBindingError::Missing(direct(
            &fixture
        ))))
    );
}

#[test]
fn incomplete_or_noncanonical_nested_input_contexts_cannot_enter_a_frame() {
    use crate::value_access::{DataInstanceKey, data_instances::Context};
    use std::{collections::BTreeMap, sync::Arc};
    let fixture = fixture();
    let program = &fixture.program;
    let selected = program.concrete_witness(fixture.receiver).unwrap();
    let callback = program.concrete_witness(fixture.callbacks[0]).unwrap();
    let [value, _] = contexts(&fixture);
    let first = value.component(program, ValueField::Tuple(0)).unwrap();
    let invalid = [
        DataInstanceKey {
            data: fixture.pair,
            context: Arc::new(Context::Closed),
        },
        DataInstanceKey {
            data: fixture.pair,
            context: Arc::new(Context::Components(BTreeMap::new())),
        },
        DataInstanceKey {
            data: fixture.pair,
            context: Arc::new(Context::Components(BTreeMap::from([(
                ValueField::Tuple(0),
                first,
            )]))),
        },
    ];
    for value in invalid {
        assert_eq!(
            program.bind_instance_with_inputs(
                &selected,
                [(direct(&fixture), callback.clone())],
                [(0, value)]
            ),
            Err(InputBindingError::Schema(0))
        );
    }
}

#[test]
fn context_free_calls_need_no_input_keys_and_keep_the_existing_canonical_identity() {
    let fixture = fixture();
    let program = &fixture.program;
    let selected = program.concrete_witness(fixture.callbacks[0]).unwrap();
    assert_eq!(
        program
            .bind_instance_with_inputs(&selected, [], [])
            .unwrap(),
        program.bind_instance(&selected, []).unwrap()
    );
}
