use super::*;

fn build(reverse: bool) -> Program {
    let symbols = SymbolTable::from_spellings(["alpha", "beta", "input"]);
    let mut builder = Builder::new(symbols.clone());
    let a = builder.reserve(symbols.get("alpha").unwrap());
    let b = builder.reserve(symbols.get("beta").unwrap());
    for id in if reverse { [b, a] } else { [a, b] } {
        builder
            .define(
                id,
                [ParameterDefinition {
                    name: symbols.get("input").unwrap(),
                    data: BuiltinType::Error.into(),
                    operation: ValueInputOperation::Edit,
                }],
                BuiltinType::Error,
            )
            .unwrap();
    }
    builder.finish().unwrap()
}
#[test]
fn definition_order_preserves_operation_and_data_identities() {
    let a = build(false);
    let b = build(true);
    assert_eq!(a.callables, b.callables);
    assert_eq!(a.parameters, b.parameters);
    assert_eq!(a.namespace, b.namespace);
    for (id, callable) in a.callables.iter() {
        assert_eq!(a.data_kind(callable.result()), Some(BuiltinType::Error));
        let parameter = a.parameters.get(callable.parameters()[0]).unwrap();
        assert_eq!(parameter.owner(), id);
        assert_eq!(parameter.operation(), ValueInputOperation::Edit);
        assert_eq!(parameter.data(), callable.result());
    }
}
#[test]
fn each_input_operation_is_independent_of_result_data_and_copy_evidence() {
    for data in [
        BuiltinType::Bool,
        BuiltinType::I32,
        BuiltinType::Void,
        BuiltinType::Error,
    ] {
        let symbols = SymbolTable::from_spellings(["f", "read", "edit", "take"]);
        let mut builder = Builder::new(symbols.clone());
        let id = builder.reserve(symbols.get("f").unwrap());
        builder
            .define(
                id,
                [
                    ParameterDefinition {
                        name: symbols.get("read").unwrap(),
                        data: data.into(),
                        operation: ValueInputOperation::Read,
                    },
                    ParameterDefinition {
                        name: symbols.get("edit").unwrap(),
                        data: data.into(),
                        operation: ValueInputOperation::Edit,
                    },
                    ParameterDefinition {
                        name: symbols.get("take").unwrap(),
                        data: data.into(),
                        operation: ValueInputOperation::Take,
                    },
                ],
                data,
            )
            .unwrap();
        let program = builder.finish().unwrap();
        let callable = program.callables().get(id).unwrap();
        assert_eq!(program.data_kind(callable.result()), Some(data));
        assert_eq!(
            program.data_copyable(callable.result()),
            Some(data != BuiltinType::Error)
        );
        let operations: Vec<_> = callable
            .parameters()
            .iter()
            .map(|id| program.parameters().get(*id).unwrap().operation())
            .collect();
        assert_eq!(
            operations,
            [
                ValueInputOperation::Read,
                ValueInputOperation::Edit,
                ValueInputOperation::Take
            ]
        );
    }
}
#[test]
fn incomplete_or_double_definition_never_publishes_a_program() {
    let symbols = SymbolTable::from_spellings(["f"]);
    let mut builder = Builder::new(symbols.clone());
    let id = builder.reserve(symbols.get("f").unwrap());
    builder.define(id, [], BuiltinType::Void).unwrap();
    assert!(matches!(
        builder.define(id, [], BuiltinType::I32),
        Err(DefinitionError::AlreadyDefined)
    ));
    let program = builder.finish().unwrap();
    assert_eq!(
        program.data_kind(program.callables().get(id).unwrap().result()),
        Some(BuiltinType::Void)
    );
    let mut builder = Builder::new(symbols.clone());
    builder.reserve(symbols.get("f").unwrap());
    assert!(
        builder
            .finish()
            .unwrap_err()
            .iter()
            .any(|v| v.rule == DeclarationRule::MissingDefinition)
    );
}
#[test]
fn duplicate_names_and_unsupported_data_do_not_cross_freeze() {
    let symbols = SymbolTable::from_spellings(["f", "x"]);
    let mut builder = Builder::new(symbols.clone());
    let f = builder.reserve(symbols.get("f").unwrap());
    let parameter = ParameterDefinition {
        name: symbols.get("x").unwrap(),
        data: BuiltinType::I32.into(),
        operation: ValueInputOperation::Read,
    };
    builder
        .define(f, [parameter, parameter], BuiltinType::F64)
        .unwrap();
    let violations = builder.finish().unwrap_err();
    assert!(
        violations
            .iter()
            .any(|v| matches!(v.rule, DeclarationRule::DuplicateParameter { first: 0 }))
    );
    assert!(violations.iter().any(|v| matches!(
        v.rule,
        DeclarationRule::UnsupportedDataType {
            data: BuiltinType::F64
        }
    )));
}
