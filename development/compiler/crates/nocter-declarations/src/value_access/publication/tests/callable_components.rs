//! A compound input's callable constraints do not create physical input slots or erase witnesses.
use super::*;
use crate::value_access::{FunctionDefinition, InputBindingError, PayloadSchema, ValueDefinition};
use nocter_model::{ValueField, ValueFunctionOrigin, ValueParameter};

fn callable() -> DataReference {
    DataReference::Function(Box::new(FunctionDefinition {
        invocation: ValueInvocation::Read,
        inputs: Box::new([]),
        result: BuiltinType::I32.into(),
    }))
}

fn fixture(reverse: bool) -> (Program, AccessCodeId, [AccessCodeId; 2]) {
    let symbols = SymbolTable::from_spellings(["receive", "first", "second", "pair"]);
    let mut reservations = Reservations::new(symbols.clone());
    let receiver = reservations.reserve(symbols.get("receive").unwrap(), 1);
    let callbacks =
        ["first", "second"].map(|name| reservations.reserve(symbols.get(name).unwrap(), 0));
    let mut definitions = reservations.finish().unwrap();
    definitions
        .define(
            receiver,
            CodeKind::Function(CompileTimeGuarantee::Evaluatable),
            [ParameterDefinition {
                binding: crate::value_access::ParameterBinding::Name(symbols.get("pair").unwrap()),
                operation: nocter_model::ValueInputOperation::Take,
                data: DataReference::Tuple(TupleData::new(
                    callable(),
                    DataReference::Array {
                        element: std::sync::Arc::new(callable()),
                        length: 2,
                    },
                    [],
                ))
                .into(),
            }],
            BuiltinType::I32,
        )
        .unwrap();
    for callback in callbacks {
        definitions
            .define(
                callback,
                CodeKind::Function(CompileTimeGuarantee::Evaluatable),
                [],
                BuiltinType::I32,
            )
            .unwrap();
    }
    let mut publication = definitions.into_publication();
    if reverse {
        for callback in callbacks.into_iter().rev() {
            publication.publish(callback).unwrap();
        }
    }
    publication.publish(receiver).unwrap();
    (publication.finish().unwrap(), receiver, callbacks)
}

#[test]
fn compound_callable_leaves_have_stable_distinct_contracts_without_extra_parameters() {
    for reverse in [false, true] {
        let (program, receiver, _) = fixture(reverse);
        let parameters = program.codes().get(receiver).unwrap().parameters();
        assert_eq!(parameters.len(), 1);
        assert_eq!(program.parameters().len(), 1);
        let input = program.parameters().get(parameters[0]).unwrap();
        assert!(input.contract().is_none());
        let first = program
            .component(input.data(), ValueField::Tuple(0))
            .unwrap()
            .data;
        let array = program
            .component(input.data(), ValueField::Tuple(1))
            .unwrap()
            .data;
        let second = program.component(array, ValueField::Array(1)).unwrap().data;
        assert_ne!(first, second);
        for (ordinal, data) in [first, second].into_iter().enumerate() {
            let parameter = ValueParameter::Component {
                input: parameters[0].into(),
                ordinal,
            };
            let Some(PayloadSchema::Function(witness)) = program.data_schema(data) else {
                panic!("formal leaf")
            };
            assert_eq!(witness.origin, ValueFunctionOrigin::Parameter(parameter));
            assert_eq!(program.parameter_owner(parameter), Some(receiver.into()));
            assert_eq!(
                program.function_contract(witness).unwrap().result,
                program.data_type(BuiltinType::I32).unwrap()
            );
            assert!(program.parameter_label(parameter).unwrap().contains("pair"));
            assert_eq!(
                program
                    .code_template(receiver.into())
                    .unwrap()
                    .resolve(&program, witness),
                Ok(witness)
            );
        }
        assert!(
            program
                .parameter_contract(ValueParameter::Component {
                    input: parameters[0].into(),
                    ordinal: 2
                })
                .is_none()
        );
        assert_eq!(program.data_copyable(input.data()), Some(false));
        assert!(!program.owned_payload_supported(input.data()));
        assert!(
            program
                .code_template(receiver.into())
                .unwrap()
                .witnesses()
                .is_empty()
        );
        let recipe = DataReference::Tuple(TupleData::new(
            callable(),
            DataReference::Array {
                element: std::sync::Arc::new(callable()),
                length: 2,
            },
            [],
        ));
        assert_eq!(
            program.existing_input_value_reference(parameters[0], &recipe.clone().into()),
            Some(input.data())
        );
        assert_eq!(program.existing_data_reference(&recipe), None);
    }
}

#[test]
fn compound_input_instances_retain_each_nested_witness_without_direct_bindings() {
    let (program, receiver, callbacks) = fixture(false);
    let parameter = program.codes().get(receiver).unwrap().parameters()[0];
    let data = program.parameters().get(parameter).unwrap().data();
    let first = program.component(data, ValueField::Tuple(0)).unwrap().data;
    let array = program.component(data, ValueField::Tuple(1)).unwrap().data;
    let element = program.component(array, ValueField::Array(0)).unwrap().data;
    let selected = program.concrete_witness(receiver.into()).unwrap();
    let mut frames = Vec::new();
    for order in [callbacks, [callbacks[1], callbacks[0]]] {
        let witnesses = order.map(|code| program.concrete_witness(code.into()).unwrap());
        let fields = [
            (
                ValueField::Tuple(0),
                program.instantiate_function(first, &witnesses[0]).unwrap(),
            ),
            (
                ValueField::Tuple(1),
                program
                    .instantiate_components(
                        array,
                        [(
                            ValueField::Array(0),
                            program
                                .instantiate_function(element, &witnesses[1])
                                .unwrap(),
                        )],
                    )
                    .unwrap(),
            ),
        ];
        let key = program.instantiate_components(data, fields).unwrap();
        let frame = program
            .bind_instance_with_inputs(&selected, [], [(0, key.clone())])
            .unwrap();
        assert_eq!(frame.input_data(0), Some(&key));
        assert!(frame.witnesses().is_empty());
        assert_eq!(
            key.component(&program, ValueField::Tuple(0))
                .unwrap()
                .function_representation(&program)
                .unwrap()
                .unwrap()
                .data(),
            program.function_data(witnesses[0].witness()).unwrap()
        );
        frames.push(frame);
    }
    assert_ne!(frames[0], frames[1]);
    assert_eq!(
        program.bind_instance_with_inputs(&selected, [], []),
        Err(InputBindingError::Missing(0))
    );
}

#[test]
fn raw_root_callable_data_requires_an_explicit_value_contract() {
    for input in [false, true] {
        let symbols = SymbolTable::from_spellings(["run", "value"]);
        let mut reservations = Reservations::new(symbols.clone());
        let run = reservations.reserve(symbols.get("run").unwrap(), usize::from(input));
        let mut builder = reservations.finish().unwrap();
        let parameters = if input {
            vec![ParameterDefinition {
                binding: crate::value_access::ParameterBinding::Name(symbols.get("value").unwrap()),
                operation: nocter_model::ValueInputOperation::Read,
                data: callable().into(),
            }]
        } else {
            vec![]
        };
        let result: ValueDefinition = if input {
            BuiltinType::Void.into()
        } else {
            callable().into()
        };
        builder
            .define(
                run,
                CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
                parameters,
                result,
            )
            .unwrap();
        let mut publication = builder.into_publication();
        let before = publication.snapshot();
        assert!(
            matches!(publication.publish(run), Err(HeaderFailure::Rejected(rules)) if rules.iter().any(|v| v.rule == DeclarationRule::UnsupportedCallableData))
        );
        let after = publication.snapshot();
        assert!(!after.header_ready(run));
        assert!(after.preserves_headers(&before));
        assert!(after.parameters().is_empty());
    }
}

#[test]
fn callable_component_signatures_validate_their_data_before_publication() {
    for data in [BuiltinType::F64, BuiltinType::Char] {
        let symbols = SymbolTable::from_spellings(["run", "value"]);
        let mut reservations = Reservations::new(symbols.clone());
        let run = reservations.reserve(symbols.get("run").unwrap(), 1);
        let mut builder = reservations.finish().unwrap();
        let invalid = DataReference::Function(Box::new(FunctionDefinition {
            invocation: ValueInvocation::Read,
            inputs: Box::new([(nocter_model::ValueInputOperation::Read, data.into())]),
            result: BuiltinType::I32.into(),
        }));
        builder
            .define(
                run,
                CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
                [ParameterDefinition {
                    binding: crate::value_access::ParameterBinding::Name(
                        symbols.get("value").unwrap(),
                    ),
                    operation: nocter_model::ValueInputOperation::Take,
                    data: DataReference::Array {
                        element: std::sync::Arc::new(invalid),
                        length: 0,
                    }
                    .into(),
                }],
                BuiltinType::Void,
            )
            .unwrap();
        let mut publication = builder.into_publication();
        assert!(
            matches!(publication.publish(run), Err(HeaderFailure::Rejected(rules)) if rules.iter().any(|v| v.rule == DeclarationRule::UnsupportedDataType { data }))
        );
        assert!(publication.snapshot().parameters().is_empty());
    }
}
