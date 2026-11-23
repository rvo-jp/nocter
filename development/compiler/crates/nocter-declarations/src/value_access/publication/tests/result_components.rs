//! Result contracts retain declaration-owned leaves without publishing a returned value.
use super::*;
use crate::value_access::{
    CodeBindingError, FunctionDefinition, FunctionWitness, HeaderPosition, PayloadSchema,
};
use nocter_model::{
    ValueCallableResult, ValueField, ValueFunctionOrigin, ValuePath, ValueProjection,
};

fn callable(result: BuiltinType) -> DataReference {
    DataReference::Function(Box::new(FunctionDefinition {
        invocation: ValueInvocation::Read,
        inputs: Box::new([]),
        result: result.into(),
    }))
}
fn result() -> DataReference {
    DataReference::Tuple(TupleData::new(
        callable(BuiltinType::I32),
        DataReference::Array {
            element: std::sync::Arc::new(callable(BuiltinType::I32)),
            length: nocter_model::UsizeTerm::Value(2),
        },
        [],
    ))
}
fn fixture(reverse: bool) -> (Program, [AccessCodeId; 2], AccessCodeId) {
    let symbols = SymbolTable::from_spellings(["first", "second", "callback"]);
    let mut reservations = Reservations::new(symbols.clone());
    let ids = ["first", "second"].map(|name| reservations.reserve(symbols.get(name).unwrap(), 0));
    let callback = reservations.reserve(symbols.get("callback").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    for id in ids {
        builder
            .define(
                id,
                CodeKind::Function(CompileTimeGuarantee::Evaluatable),
                [],
                result(),
            )
            .unwrap();
    }
    builder
        .define(
            callback,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::I32,
        )
        .unwrap();
    let mut publication = builder.into_publication();
    let order = if reverse { [ids[1], ids[0]] } else { ids };
    for id in order {
        publication.publish(id).unwrap();
    }
    (publication.finish().unwrap(), ids, callback)
}

#[test]
fn compound_result_contracts_are_distinct_deterministic_and_not_runtime_inputs_or_values() {
    let mut orders = Vec::new();
    for reverse in [false, true] {
        let (program, ids, callback) = fixture(reverse);
        let supplied = program.concrete_witness(callback.into()).unwrap();
        assert!(program.parameters().is_empty());
        let mut origins = Vec::new();
        let mut roots = Vec::new();
        for id in ids {
            let code = program.codes().get(id).unwrap();
            assert!(code.parameters().is_empty());
            assert_eq!(code.result_contracts().len(), 2);
            assert_result_paths(&program, id);
            roots.push(code.result());
            let first = program
                .component(code.result(), ValueField::Tuple(0))
                .unwrap()
                .data;
            let array = program
                .component(code.result(), ValueField::Tuple(1))
                .unwrap()
                .data;
            let second = program.component(array, ValueField::Array(1)).unwrap().data;
            assert_ne!(first, second);
            for (ordinal, data) in [first, second].into_iter().enumerate() {
                let Some(PayloadSchema::Function(witness)) = program.data_schema(data) else {
                    panic!("result leaf");
                };
                let expected =
                    ValueFunctionOrigin::Result(ValueCallableResult { owner: id, ordinal });
                assert_eq!(witness.origin, expected);
                assert_eq!(
                    program.function_contract(witness).as_ref(),
                    Some(&code.result_contracts()[ordinal])
                );
                assert_eq!(program.function_data(witness), Some(data));
                assert_eq!(
                    program.concrete_witness(witness),
                    Err(CodeBindingError::InvalidWitness)
                );
                assert_eq!(
                    program.instantiate_function(data, &supplied),
                    Err(CodeBindingError::InvalidWitness)
                );
                origins.push(witness.origin);
            }
            for ordinal in [2, usize::MAX] {
                let witness = FunctionWitness {
                    origin: ValueFunctionOrigin::Result(ValueCallableResult { owner: id, ordinal }),
                    invocation: ValueInvocation::Read,
                };
                assert_eq!(program.function_contract(witness), None);
                assert_eq!(program.function_data(witness), None);
                assert_eq!(
                    program.callable_result_path(ValueCallableResult { owner: id, ordinal }),
                    None
                );
            }
            assert_eq!(program.data_copyable(code.result()), Some(false));
            assert!(!program.owned_payload_supported(code.result()));
            assert!(program.instantiate_closed_data(code.result()).is_err());
            assert_eq!(
                program.existing_result_value_reference(id, &result().into()),
                Some(code.result())
            );
            assert_eq!(program.existing_data_reference(&result()), None);
            let changed = DataReference::Tuple(TupleData::new(
                callable(BuiltinType::I64),
                DataReference::Array {
                    element: std::sync::Arc::new(callable(BuiltinType::I32)),
                    length: nocter_model::UsizeTerm::Value(2),
                },
                [],
            ));
            assert_eq!(
                program.existing_result_value_reference(id, &changed.into()),
                None
            );
        }
        assert_ne!(roots[0], roots[1]);
        orders.push(origins);
    }
    assert_eq!(orders[0], orders[1]);
}

fn assert_result_paths(program: &Program, owner: AccessCodeId) {
    for (ordinal, path) in [
        ValuePath::new([ValueProjection::Tuple(0)]),
        ValuePath::new([ValueProjection::Tuple(1), ValueProjection::Array(())]),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            program.callable_result_path(ValueCallableResult { owner, ordinal }),
            Some(path)
        );
    }
}

#[test]
fn result_path_lookup_terminates_through_recursive_empty_nominal_schemas() {
    let symbols = SymbolTable::from_spellings(["factory", "Node", "next"]);
    let mut reservations = Reservations::new(symbols.clone());
    let owner = reservations.reserve(symbols.get("factory").unwrap(), 0);
    let node = reservations.reserve_record(symbols.get("Node").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            node,
            false,
            [FieldDefinition {
                name: symbols.get("next").unwrap(),
                data: DataReference::Array {
                    element: std::sync::Arc::new(DataReference::Nominal(node)),
                    length: nocter_model::UsizeTerm::Value(0),
                },
            }],
        )
        .unwrap();
    builder
        .define(
            owner,
            CodeKind::Function(CompileTimeGuarantee::Evaluatable),
            [],
            DataReference::Tuple(TupleData::new(
                callable(BuiltinType::I32),
                DataReference::Nominal(node),
                [],
            )),
        )
        .unwrap();
    let program = builder.finish().unwrap();
    assert_eq!(
        program.callable_result_path(ValueCallableResult { owner, ordinal: 0 }),
        Some(ValuePath::new([ValueProjection::Tuple(0)])),
    );
}

#[test]
fn result_descriptor_paths_preserve_outcomes_and_empty_array_metadata_without_values() {
    for length in [None, Some(0), Some(u64::MAX)] {
        let symbols = SymbolTable::from_spellings(["factory"]);
        let mut reservations = Reservations::new(symbols.clone());
        let owner = reservations.reserve(symbols.get("factory").unwrap(), 0);
        let mut builder = reservations.finish().unwrap();
        let (data, path) = if let Some(length) = length {
            let data = DataReference::Optional {
                payload: std::sync::Arc::new(DataReference::Fallible(std::sync::Arc::new(
                    DataReference::Array {
                        element: std::sync::Arc::new(callable(BuiltinType::I32)),
                        length: length.into(),
                    },
                ))),
                layers: std::num::NonZeroUsize::new(2).unwrap(),
            };
            (
                data.into(),
                ValuePath::new([
                    ValueProjection::Variant(true),
                    ValueProjection::Variant(true),
                    ValueProjection::Variant(true),
                    ValueProjection::Array(()),
                ]),
            )
        } else {
            (
                crate::value_access::ValueDefinition::Function(FunctionDefinition {
                    invocation: ValueInvocation::Read,
                    inputs: Box::new([]),
                    result: BuiltinType::I32.into(),
                }),
                ValuePath::default(),
            )
        };
        builder
            .define(
                owner,
                CodeKind::Function(CompileTimeGuarantee::Evaluatable),
                [],
                data,
            )
            .unwrap();
        let mut headers = builder.into_publication();
        let selected = ValueCallableResult { owner, ordinal: 0 };
        assert_eq!(headers.snapshot().callable_result_path(selected), None);
        headers.publish(owner).unwrap();
        let program = headers.finish().unwrap();
        let count = program.types.store().type_count();
        assert_eq!(program.callable_result_path(selected), Some(path.clone()));
        let root = program.codes().get(owner).unwrap().result();
        assert_eq!(
            program
                .project_data_by(root, &path, |(), length| length > 0)
                .is_some(),
            length != Some(0)
        );
        assert_eq!(program.types.store().type_count(), count);
        assert_eq!(
            program.instantiate_closed_data(root).is_ok(),
            length == Some(0)
        );
    }
}

#[test]
fn invalid_result_slots_and_initializer_positions_publish_nothing() {
    for case in [0, 2, 3] {
        let symbols = SymbolTable::from_spellings(["factory"]);
        let mut reservations = Reservations::new(symbols.clone());
        let id = reservations.reserve(symbols.get("factory").unwrap(), 0);
        let mut builder = reservations.finish().unwrap();
        let leaf = match case {
            0 => callable(BuiltinType::F64),
            _ => callable(BuiltinType::I32),
        };
        let data = DataReference::Array {
            element: std::sync::Arc::new(leaf),
            length: nocter_model::UsizeTerm::Value(0),
        };
        let kind = match case {
            2 => CodeKind::ConstantInitializer,
            3 => CodeKind::StaticInitializer,
            _ => CodeKind::Function(CompileTimeGuarantee::Evaluatable),
        };
        builder.define(id, kind, [], data).unwrap();
        let mut publication = builder.into_publication();
        let before = publication.snapshot();
        let failure = publication.publish(id).unwrap_err();
        let HeaderFailure::Rejected(violations) = failure else {
            panic!("invalid header");
        };
        assert!(
            violations
                .iter()
                .all(|v| v.position == HeaderPosition::Result)
        );
        assert!(violations.iter().any(|v| match case {
            0 => matches!(
                v.rule,
                DeclarationRule::UnsupportedDataType {
                    data: BuiltinType::F64
                }
            ),
            _ => v.rule == DeclarationRule::UnsupportedCallableData,
        }));
        let after = publication.snapshot();
        assert!(after.preserves_headers(&before));
        assert!(!after.header_ready(id));
        assert!(after.parameters().is_empty());
        let witness = FunctionWitness {
            origin: ValueFunctionOrigin::Result(id.into()),
            invocation: ValueInvocation::Read,
        };
        assert_eq!(after.function_contract(witness), None);
        assert_eq!(after.function_data(witness), None);
    }
}
