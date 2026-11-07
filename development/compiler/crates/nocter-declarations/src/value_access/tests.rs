use super::*;

fn build(reverse_definitions: bool) -> Program {
    let symbols = SymbolTable::from_spellings(["alpha", "beta", "input"]);
    let mut builder = Builder::new(symbols.clone());
    let first = builder.reserve(symbols.get("alpha").unwrap());
    let second = builder.reserve(symbols.get("beta").unwrap());
    let order = if reverse_definitions {
        [second, first]
    } else {
        [first, second]
    };
    for id in order {
        builder
            .define(
                id,
                [ParameterDefinition {
                    name: symbols.get("input").unwrap(),
                    data: BuiltinType::I32,
                    mode: ValueAccessMode::Update,
                }],
                BuiltinType::I32,
                ValueAccessMode::Update,
            )
            .unwrap();
    }
    builder.finish().unwrap()
}

#[test]
fn definition_order_cannot_reassign_parameter_or_origin_identities() {
    let first = build(false);
    let second = build(true);
    assert_eq!(first.callables, second.callables);
    assert_eq!(first.parameters, second.parameters);
    assert_eq!(first.namespace, second.namespace);
    assert_eq!(first.symbols, second.symbols);
    for (id, callable) in first.callables.iter() {
        assert_eq!(callable.result.mode(), ValueAccessMode::Update);
        assert_eq!(
            callable.referent,
            ResultReferent::Inputs(callable.parameters.clone())
        );
        assert_eq!(
            first
                .parameters
                .get(callable.parameters[0])
                .unwrap()
                .owner(),
            id
        );
    }
}

#[test]
fn result_bounds_keep_permissions_and_only_eligible_caller_referents() {
    for data in [
        BuiltinType::Bool,
        BuiltinType::I32,
        BuiltinType::Void,
        BuiltinType::Error,
    ] {
        for result_mode in [
            ValueAccessMode::Read,
            ValueAccessMode::Update,
            ValueAccessMode::Own,
        ] {
            let symbols = SymbolTable::from_spellings(["f", "read", "edit", "owned", "other"]);
            let mut builder = Builder::new(symbols.clone());
            let id = builder.reserve(symbols.get("f").unwrap());
            builder
                .define(
                    id,
                    [
                        ParameterDefinition {
                            name: symbols.get("read").unwrap(),
                            data,
                            mode: ValueAccessMode::Read,
                        },
                        ParameterDefinition {
                            name: symbols.get("edit").unwrap(),
                            data,
                            mode: ValueAccessMode::Update,
                        },
                        ParameterDefinition {
                            name: symbols.get("owned").unwrap(),
                            data,
                            mode: ValueAccessMode::Own,
                        },
                        ParameterDefinition {
                            name: symbols.get("other").unwrap(),
                            data: if data == BuiltinType::I32 {
                                BuiltinType::Bool
                            } else {
                                BuiltinType::I32
                            },
                            mode: ValueAccessMode::Update,
                        },
                    ],
                    data,
                    result_mode,
                )
                .unwrap();
            let program = builder.finish().unwrap();
            let callable = program.callables().get(id).unwrap();
            assert_eq!(callable.result().mode(), result_mode);
            assert_eq!(callable.result().data(), program.data_type(data).unwrap());
            assert_eq!(
                program.data_copyable(callable.result().data()),
                Some(data != BuiltinType::Error)
            );
            assert_eq!(program.data_type(BuiltinType::U64), None);
            let expected = match result_mode {
                ValueAccessMode::Own => ResultReferent::Value,
                ValueAccessMode::Read if data != BuiltinType::Error => ResultReferent::Value,
                ValueAccessMode::Read => ResultReferent::Inputs(callable.parameters()[..2].into()),
                ValueAccessMode::Update => {
                    ResultReferent::Inputs(callable.parameters()[1..2].into())
                }
            };
            assert_eq!(callable.result_referent(), &expected);
        }
    }
}

#[test]
fn empty_and_alternative_bounds_do_not_invent_an_independent_update() {
    let symbols = SymbolTable::from_spellings(["f", "left", "right"]);
    for count in 0..=2 {
        let mut builder = Builder::new(symbols.clone());
        let id = builder.reserve(symbols.get("f").unwrap());
        let parameters = ["left", "right"]
            .into_iter()
            .take(count)
            .map(|name| ParameterDefinition {
                name: symbols.get(name).unwrap(),
                data: BuiltinType::I32,
                mode: ValueAccessMode::Update,
            })
            .collect::<Vec<_>>();
        builder
            .define(id, parameters, BuiltinType::I32, ValueAccessMode::Update)
            .unwrap();
        let program = builder.finish().unwrap();
        let callable = program.callables.get(id).unwrap();
        assert_eq!(
            callable.referent,
            ResultReferent::Inputs(callable.parameters.clone())
        );
        assert_eq!(callable.parameters.len(), count);
    }
}

#[test]
fn incomplete_duplicate_shadowing_and_unsupported_headers_never_freeze() {
    let symbols = SymbolTable::from_spellings(["f", "p", "bool"]);
    let mut builder = Builder::new(symbols.clone());
    let missing = builder.reserve(symbols.get("f").unwrap());
    let duplicate = builder.reserve(symbols.get("f").unwrap());
    let reserved = builder.reserve(symbols.get("bool").unwrap());
    builder
        .define(
            duplicate,
            [
                ParameterDefinition {
                    name: symbols.get("p").unwrap(),
                    data: BuiltinType::I32,
                    mode: ValueAccessMode::Read,
                },
                ParameterDefinition {
                    name: symbols.get("p").unwrap(),
                    data: BuiltinType::I32,
                    mode: ValueAccessMode::Own,
                },
                ParameterDefinition {
                    name: symbols.get("f").unwrap(),
                    data: BuiltinType::U64,
                    mode: ValueAccessMode::Own,
                },
            ],
            BuiltinType::I32,
            ValueAccessMode::Read,
        )
        .unwrap();
    builder
        .define(reserved, [], BuiltinType::I32, ValueAccessMode::Read)
        .unwrap();
    assert_eq!(
        builder.define(reserved, [], BuiltinType::I32, ValueAccessMode::Update),
        Err(DefinitionError::AlreadyDefined)
    );
    let violations = builder.finish().unwrap_err();
    assert!(violations.contains(&DeclarationViolation {
        callable: missing,
        position: HeaderPosition::Name,
        rule: DeclarationRule::MissingDefinition
    }));
    assert!(violations.iter().any(
        |v| matches!(v.rule, DeclarationRule::DuplicateCallable { first } if first == missing)
    ));
    assert!(
        violations
            .iter()
            .any(|v| matches!(v.rule, DeclarationRule::DuplicateParameter { first: 0 }))
    );
    assert!(
        violations
            .iter()
            .any(|v| matches!(v.rule, DeclarationRule::ReservedBuiltinName))
    );
    assert!(
        violations
            .iter()
            .any(|v| matches!(v.rule, DeclarationRule::ParameterShadowsCallable { .. }))
    );
    assert!(violations.iter().any(|v| matches!(
        v.rule,
        DeclarationRule::UnsupportedDataType {
            data: BuiltinType::U64
        }
    )));
}
