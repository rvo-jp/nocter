//! Abstract signature conformance retains declaration-owned identities and exact nested views.
use super::*;

#[test]
fn abstract_nested_signatures_match_without_merging_contract_ids() {
    let (headers, [root, _, _]) = super::application_headers();
    let integer = headers.data_type(BuiltinType::I32).unwrap();
    let mut body = headers.body_types(root).unwrap();
    let left = body.callable_contract(0, signature(integer)).unwrap();
    let right = body.callable_contract(1, signature(integer)).unwrap();
    let contract = |data| FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([FunctionInput {
            data,
            operation: ValueInputOperation::Read,
        }]),
        result: integer,
    };
    let actual = body.callable_contract(2, contract(left)).unwrap();
    let receiving = body.callable_contract(3, contract(right)).unwrap();
    let program = body.finish();
    let wanted = witness(&program, receiving);
    let before = program.types.store().iter().collect::<Vec<_>>();
    assert_ne!(left, right);
    assert!(
        program
            .constrain_function(actual, &program.function_contract(wanted).unwrap())
            .is_err()
    );
    assert_eq!(
        program.constrain_callable_contract(actual, wanted),
        Ok(actual)
    );
    assert_eq!(
        program.constrain_declared_function(actual, &program.function_contract(wanted).unwrap()),
        Ok(actual)
    );
    assert!(program.function_binding(actual, receiving));
    assert!(program.concrete_witness(witness(&program, actual)).is_err());
    assert!(program.concrete_witness(wanted).is_err());
    assert!(program.instantiate_closed_data(receiving).is_err());
    assert_eq!(program.data_copyable(receiving), Some(false));
    assert_eq!(program.types.store().iter().collect::<Vec<_>>(), before);
}

#[test]
fn nested_callable_views_and_input_operations_remain_invariant() {
    use crate::value_access::{FunctionMismatch, FunctionPosition};
    let (headers, [root, _, _]) = super::application_headers();
    let integer = headers.data_type(BuiltinType::I32).unwrap();
    let mut body = headers.body_types(root).unwrap();
    let contract = |invocation, operation| FunctionConstraint {
        invocation,
        inputs: Box::new([FunctionInput {
            data: integer,
            operation,
        }]),
        result: integer,
    };
    let left = body
        .callable_contract(
            0,
            contract(ValueInvocation::Read, ValueInputOperation::Read),
        )
        .unwrap();
    let view = body
        .callable_contract(
            1,
            contract(ValueInvocation::Take, ValueInputOperation::Read),
        )
        .unwrap();
    let edit = body
        .callable_contract(
            2,
            contract(ValueInvocation::Read, ValueInputOperation::Edit),
        )
        .unwrap();
    let outer = |data| FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([FunctionInput {
            data,
            operation: ValueInputOperation::Read,
        }]),
        result: integer,
    };
    let actual = body.callable_contract(3, outer(left)).unwrap();
    let view = body.callable_contract(4, outer(view)).unwrap();
    let edit = body.callable_contract(5, outer(edit)).unwrap();
    let program = body.finish();
    for (receiving, view_mismatch) in [(view, true), (edit, false)] {
        let failure = program
            .constrain_callable_contract(actual, witness(&program, receiving))
            .unwrap_err();
        let declared = program
            .constrain_declared_function(
                actual,
                &program
                    .function_contract(witness(&program, receiving))
                    .unwrap(),
            )
            .unwrap_err();
        assert_eq!(declared, failure);
        assert_eq!(failure.path.as_ref(), [FunctionPosition::Input(0)]);
        assert!(if view_mismatch {
            matches!(
                failure.mismatch,
                FunctionMismatch::ContractInvocation { .. }
            )
        } else {
            matches!(
                failure.mismatch,
                FunctionMismatch::Input { position: 0, .. }
            )
        });
    }
}

#[test]
fn nested_concrete_code_identity_cannot_be_erased_by_abstract_contract_binding() {
    use crate::value_access::FunctionPosition;
    let (headers, [root, first, second]) = super::application_headers();
    let void = headers.data_type(BuiltinType::Void).unwrap();
    let left = headers.function_data(first.into()).unwrap();
    let right = headers.function_data(second.into()).unwrap();
    let mut body = headers.body_types(root).unwrap();
    let signature = |data| FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([FunctionInput {
            data,
            operation: ValueInputOperation::Read,
        }]),
        result: void,
    };
    let actual = body.callable_contract(0, signature(left)).unwrap();
    let receiving = body.callable_contract(1, signature(right)).unwrap();
    let program = body.finish();
    let failure = program
        .constrain_callable_contract(actual, witness(&program, receiving))
        .unwrap_err();
    assert_eq!(failure.path.as_ref(), [FunctionPosition::Input(0)]);
    assert!(!program.function_binding(actual, receiving));
}

#[test]
fn nested_contract_conformance_preserves_deep_shared_signatures_without_expansion() {
    let (headers, [root, _, _]) = super::application_headers();
    let mut left = headers.data_type(BuiltinType::I32).unwrap();
    let mut right = left;
    let mut body = headers.body_types(root).unwrap();
    for depth in 0..1_000 {
        let mut contract = |data| {
            let pair = body.tuple(&[data, data]).unwrap();
            let array = body.array(pair, u64::MAX).unwrap();
            FunctionConstraint {
                invocation: ValueInvocation::Read,
                inputs: Box::new([FunctionInput {
                    data: array,
                    operation: ValueInputOperation::Read,
                }]),
                result: data,
            }
        };
        let lhs = contract(left);
        let rhs = contract(right);
        left = body.callable_contract(depth * 2, lhs).unwrap();
        right = body.callable_contract(depth * 2 + 1, rhs).unwrap();
    }
    let program = body.finish();
    assert_ne!(left, right);
    assert_eq!(
        program.constrain_callable_contract(left, witness(&program, right)),
        Ok(left)
    );
    assert_eq!(
        program.constrain_declared_function(
            left,
            &program.function_contract(witness(&program, right)).unwrap()
        ),
        Ok(left)
    );
    assert!(program.instantiate_closed_data(left).is_err());
}

#[test]
fn named_higher_order_inputs_conform_to_nested_type_contracts_without_rebinding_formal_ids() {
    let symbols = SymbolTable::from_spellings(["invoke", "action"]);
    let mut slots = Reservations::new(symbols.clone());
    let code = slots.reserve(symbols.get("invoke").unwrap(), 1);
    let mut builder = slots.finish().unwrap();
    builder
        .define(
            code,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [ParameterDefinition {
                name: symbols.get("action").unwrap(),
                operation: ValueInputOperation::Read,
                data: ValueDefinition::Function(FunctionDefinition {
                    invocation: ValueInvocation::Read,
                    inputs: Box::new([(ValueInputOperation::Read, BuiltinType::I32.into())]),
                    result: BuiltinType::I32.into(),
                }),
            }],
            BuiltinType::I32,
        )
        .unwrap();
    let headers = builder.finish().unwrap();
    let integer = headers.data_type(BuiltinType::I32).unwrap();
    let mut body = headers.body_types(code).unwrap();
    let inner = body
        .callable_contract(
            0,
            FunctionConstraint {
                invocation: ValueInvocation::Read,
                inputs: Box::new([FunctionInput {
                    data: integer,
                    operation: ValueInputOperation::Read,
                }]),
                result: integer,
            },
        )
        .unwrap();
    let receiving = body
        .callable_contract(
            1,
            FunctionConstraint {
                invocation: ValueInvocation::Read,
                inputs: Box::new([FunctionInput {
                    data: inner,
                    operation: ValueInputOperation::Read,
                }]),
                result: integer,
            },
        )
        .unwrap();
    let program = body.finish();
    let actual = program.function_data(code.into()).unwrap();
    assert!(program.function_binding(actual, receiving));
    let formal = program.codes().get(code).unwrap().parameters()[0];
    assert_ne!(program.parameters().get(formal).unwrap().data(), inner);
    let original = program.concrete_witness(code.into()).unwrap();
    let bound = program.instantiate_function(receiving, &original).unwrap();
    let crate::value_access::InstanceSchema::Function(retained) = bound.schema(&program).unwrap()
    else {
        panic!("retained named witness");
    };
    assert_eq!(retained, original);
}

#[test]
fn zero_array_signature_edges_still_compare_their_callable_contracts() {
    use crate::value_access::{FunctionMismatch, FunctionPosition};
    let (headers, [root, _, _]) = super::application_headers();
    let integer = headers.data_type(BuiltinType::I32).unwrap();
    let boolean = headers.data_type(BuiltinType::Bool).unwrap();
    let mut body = headers.body_types(root).unwrap();
    let left = body.callable_contract(0, signature(integer)).unwrap();
    let right = body.callable_contract(1, signature(boolean)).unwrap();
    let left = body.array(left, 0).unwrap();
    let right = body.array(right, 0).unwrap();
    let outer = |data| FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([FunctionInput {
            data,
            operation: ValueInputOperation::Read,
        }]),
        result: integer,
    };
    let actual = body.callable_contract(2, outer(left)).unwrap();
    let receiving = body.callable_contract(3, outer(right)).unwrap();
    let program = body.finish();
    let failure = program
        .constrain_callable_contract(actual, witness(&program, receiving))
        .unwrap_err();
    assert_eq!(
        failure.path.as_ref(),
        [
            FunctionPosition::Input(0),
            FunctionPosition::Component(nocter_model::ValueProjection::Array(())),
            FunctionPosition::Result
        ]
    );
    assert_eq!(
        failure.mismatch,
        FunctionMismatch::Data {
            actual: integer,
            expected: boolean
        }
    );
}

#[test]
fn equally_shaped_nominal_records_remain_distinct_inside_type_contracts() {
    use crate::value_access::FunctionPosition;
    let symbols = SymbolTable::from_spellings(["Left", "Right", "root"]);
    let mut slots = Reservations::new(symbols.clone());
    let left = slots.reserve_record(symbols.get("Left").unwrap(), 0);
    let right = slots.reserve_record(symbols.get("Right").unwrap(), 0);
    let root = slots.reserve(symbols.get("root").unwrap(), 0);
    let mut builder = slots.finish().unwrap();
    for record in [left, right] {
        builder.define_record(record, false, []).unwrap();
    }
    builder
        .define(
            root,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    let headers = builder.finish().unwrap();
    let void = headers.data_type(BuiltinType::Void).unwrap();
    let mut body = headers.body_types(root).unwrap();
    let contract = |record| FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([FunctionInput {
            data: headers.record_type_identity(record).unwrap(),
            operation: ValueInputOperation::Read,
        }]),
        result: void,
    };
    let actual = body.callable_contract(0, contract(left)).unwrap();
    let receiving = body.callable_contract(1, contract(right)).unwrap();
    let program = body.finish();
    let failure = program
        .constrain_callable_contract(actual, witness(&program, receiving))
        .unwrap_err();
    assert_eq!(failure.path.as_ref(), [FunctionPosition::Input(0)]);
}
