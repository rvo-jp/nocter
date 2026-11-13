use super::*;
use crate::value_access::{
    CodeKind, FunctionDefinition, FunctionInput, ParameterDefinition, Reservations, ValueDefinition,
};
use nocter_model::{
    AccessCodeId, BuiltinType, CompileTimeGuarantee, SymbolTable, ValueInputOperation,
};

fn headers() -> (Program, [AccessCodeId; 5]) {
    let symbols =
        SymbolTable::from_spellings(["read", "edit", "plain", "wrong", "factory", "action"]);
    let mut reservations = Reservations::new(symbols.clone());
    let ids = [
        reservations.reserve(symbols.get("read").unwrap(), 1),
        reservations.reserve(symbols.get("edit").unwrap(), 1),
        reservations.reserve(symbols.get("plain").unwrap(), 0),
        reservations.reserve(symbols.get("wrong").unwrap(), 0),
        reservations.reserve(symbols.get("factory").unwrap(), 0),
    ];
    let mut builder = reservations.finish().unwrap();
    let kind = CodeKind::Function(CompileTimeGuarantee::RuntimeOnly);
    for (id, invocation, operation) in [
        (ids[0], ValueInvocation::Read, ValueInputOperation::Read),
        (ids[1], ValueInvocation::Edit, ValueInputOperation::Edit),
    ] {
        builder
            .define(
                id,
                kind,
                [ParameterDefinition {
                    name: symbols.get("action").unwrap(),
                    operation,
                    data: ValueDefinition::Function(FunctionDefinition {
                        invocation,
                        inputs: Box::new([]),
                        result: BuiltinType::Void.into(),
                    }),
                }],
                BuiltinType::Void,
            )
            .unwrap();
    }
    builder.define(ids[2], kind, [], BuiltinType::Void).unwrap();
    builder.define(ids[3], kind, [], BuiltinType::Bool).unwrap();
    builder
        .define(
            ids[4],
            kind,
            [],
            ValueDefinition::Function(FunctionDefinition {
                invocation: ValueInvocation::Read,
                inputs: Box::new([]),
                result: BuiltinType::Void.into(),
            }),
        )
        .unwrap();
    (builder.finish().unwrap(), ids)
}
#[test]
fn instance_bindings_are_exact_canonical_contract_views_without_code_body_capability() {
    let (program, [read, edit, plain, wrong, factory]) = headers();
    let read_parameter = program.codes().get(read).unwrap().parameters()[0].into();
    let edit_parameter = program.codes().get(edit).unwrap().parameters()[0].into();
    let actual = FunctionWitness::from(plain);
    assert_eq!(
        program.bind_code(read.into(), []),
        Err(InstanceBindingError::Missing(read_parameter))
    );
    assert_eq!(
        program.bind_code(read.into(), [(edit_parameter, actual)]),
        Err(InstanceBindingError::Unexpected(edit_parameter))
    );
    assert_eq!(
        program.bind_code(
            read.into(),
            [(read_parameter, actual), (read_parameter, actual)]
        ),
        Err(InstanceBindingError::Duplicate(read_parameter))
    );
    assert!(matches!(
        program.bind_code(read.into(), [(read_parameter, wrong.into())]),
        Err(InstanceBindingError::Contract(
            FunctionMismatch::Result { .. }
        ))
    ));
    let key = program
        .bind_code(edit.into(), [(edit_parameter, actual)])
        .unwrap();
    let edited = FunctionWitness {
        invocation: ValueInvocation::Edit,
        ..actual
    };
    assert_eq!(
        key,
        program
            .bind_code(edit.into(), [(edit_parameter, edited)])
            .unwrap()
    );
    assert_eq!(
        key.witnesses()[&edit_parameter].invocation(),
        ValueInvocation::Edit
    );
    assert!(matches!(
        key.resolve(
            &program,
            FunctionWitness {
                origin: ValueFunctionOrigin::Parameter(edit_parameter),
                invocation: ValueInvocation::Read
            }
        ),
        Err(InstanceBindingError::Contract(
            FunctionMismatch::Invocation { .. }
        ))
    ));
    assert_eq!(
        key.resolve(
            &program,
            FunctionWitness {
                origin: ValueFunctionOrigin::Parameter(edit_parameter),
                invocation: ValueInvocation::Take
            }
        )
        .unwrap()
        .invocation(),
        ValueInvocation::Take
    );
    assert!(matches!(
        program.bind_code(read.into(), [(read_parameter, edited)]),
        Err(InstanceBindingError::Contract(
            FunctionMismatch::Invocation { .. }
        ))
    ));
    assert_eq!(
        program.concrete_witness(FunctionWitness {
            origin: ValueFunctionOrigin::Result(factory),
            invocation: ValueInvocation::Read
        }),
        Err(InstanceBindingError::InvalidWitness)
    );
    assert_eq!(
        program.codes().get(plain).unwrap().compile_time(),
        CompileTimeGuarantee::RuntimeOnly
    );
}
#[test]
fn anonymous_binding_coverage_uses_explicit_input_positions_and_keeps_expression_roots_separate() {
    let (program, [root, _, plain, ..]) = headers();
    let mut body = program.body_types(root).unwrap();
    let owner = body.reserve_closure().unwrap();
    let void = program.data_type(BuiltinType::Void).unwrap();
    let integer = program.data_type(BuiltinType::I32).unwrap();
    let callback = body
        .closure_parameter(
            owner,
            1,
            FunctionConstraint {
                invocation: ValueInvocation::Read,
                inputs: Box::new([]),
                result: void,
            },
        )
        .unwrap();
    body.closure(
        owner,
        void,
        FunctionConstraint {
            invocation: ValueInvocation::Read,
            inputs: vec![
                FunctionInput {
                    data: integer,
                    operation: ValueInputOperation::Read,
                },
                FunctionInput {
                    data: callback,
                    operation: ValueInputOperation::Take,
                },
            ]
            .into(),
            result: void,
        },
    )
    .unwrap();
    let program = body.finish();
    let formal = ValueParameter::Closure { owner, position: 1 };
    let key = program
        .bind_code(ValueCode::Closure(owner), [(formal, plain.into())])
        .unwrap();
    assert_eq!(
        key.witnesses()[&formal].code(),
        ValueCode::Declaration(plain)
    );
    assert!(matches!(
        program.bind_code(
            ValueCode::Closure(owner),
            [(ValueParameter::Closure { owner, position: 0 }, plain.into())]
        ),
        Err(InstanceBindingError::Unexpected(_))
    ));
    let expression = nocter_model::ValueExpression {
        owner: root,
        slot: 0,
    };
    let frame = program
        .bind_code(ValueCode::Expression(expression), [])
        .unwrap();
    assert_eq!(frame.code(), ValueCode::Expression(expression));
    assert!(ValueCode::Expression(expression).origin().is_none());
}
