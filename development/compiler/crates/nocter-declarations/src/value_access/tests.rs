use super::*;
use nocter_model::ValueInvocation;

#[test]
fn one_closed_namespace_preserves_declaration_roles_before_and_after_type_publication() {
    let symbols = SymbolTable::from_spellings(["Item", "run"]);
    let mut reservations = Reservations::new(symbols.clone());
    let item_name = symbols.get("Item").unwrap();
    let run_name = symbols.get("run").unwrap();
    let item = reservations.reserve_record(item_name);
    let run = reservations.reserve(run_name);
    let mut builder = reservations.finish().unwrap();
    // A forward record reference needs no invented TypeId or complete record definition.
    assert_eq!(
        builder.resolve_data(item_name),
        Ok(DataReference::Record(item))
    );
    assert_eq!(
        builder.resolve_data(run_name),
        Err(DeclarationRule::UnknownDataName { name: run_name })
    );
    builder.define_record(item, false, []).unwrap();
    builder
        .define(
            run,
            CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            DataReference::Record(item),
        )
        .unwrap();
    let program = builder.finish().unwrap();
    assert_eq!(program.resolve_record(item_name), Some(item));
    assert_eq!(program.resolve(run_name), Some(run));
    assert_eq!(program.resolve_record(run_name), None);
    assert_eq!(program.resolve_code(item_name), None);
    assert_eq!(program.type_named("run"), None);
    assert_eq!(
        program.type_named("Item"),
        Some(program.callables().get(run).unwrap().result())
    );
}

#[test]
fn invalid_callable_names_fail_before_requesting_any_definition_or_type() {
    let symbols = SymbolTable::from_spellings(["f", "i32"]);
    let foreign = SymbolTable::from_spellings(["a", "b", "c"]);
    let mut reservations = Reservations::new(symbols.clone());
    let first = reservations.reserve(symbols.get("f").unwrap());
    let duplicate = reservations.reserve(symbols.get("f").unwrap());
    let builtin = reservations.reserve(symbols.get("i32").unwrap());
    let unknown = reservations.reserve(foreign.get("c").unwrap());
    assert_eq!(
        reservations.finish().unwrap_err(),
        [
            DeclarationViolation {
                declaration: DeclarationId::Callable(duplicate),
                position: HeaderPosition::Name,
                rule: DeclarationRule::DuplicateCallable { first },
            },
            DeclarationViolation {
                declaration: DeclarationId::Callable(builtin),
                position: HeaderPosition::Name,
                rule: DeclarationRule::ReservedBuiltinName,
            },
            DeclarationViolation {
                declaration: DeclarationId::Callable(unknown),
                position: HeaderPosition::Name,
                rule: DeclarationRule::UnknownSymbol,
            },
        ]
    );
}

#[test]
fn repeated_parameters_retain_the_first_input_even_when_it_shadows_a_declaration() {
    let symbols = SymbolTable::from_spellings(["Item", "run"]);
    let mut reservations = Reservations::new(symbols.clone());
    let item_name = symbols.get("Item").unwrap();
    let run_name = symbols.get("run").unwrap();
    let item = reservations.reserve_record(item_name);
    let run = reservations.reserve(run_name);
    let mut builder = reservations.finish().unwrap();
    builder.define_record(item, false, []).unwrap();
    builder
        .define(
            run,
            CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [item_name, item_name, run_name, run_name].map(|name| ParameterDefinition {
                name,
                data: BuiltinType::I32.into(),
                operation: ValueInputOperation::Read,
            }),
            BuiltinType::Void,
        )
        .unwrap();
    let rules: Vec<_> = builder
        .finish()
        .unwrap_err()
        .into_iter()
        .map(|violation| (violation.position, violation.rule))
        .collect();
    assert_eq!(
        rules,
        [
            (
                HeaderPosition::Parameter(0),
                DeclarationRule::ParameterShadowsRecord { declaration: item }
            ),
            (
                HeaderPosition::Parameter(1),
                DeclarationRule::DuplicateParameter { first: 0 }
            ),
            (
                HeaderPosition::Parameter(2),
                DeclarationRule::ParameterShadowsCallable { declaration: run }
            ),
            (
                HeaderPosition::Parameter(3),
                DeclarationRule::DuplicateParameter { first: 2 }
            ),
        ]
    );
}

fn build(reverse: bool) -> Program {
    let symbols = SymbolTable::from_spellings(["alpha", "beta", "input"]);
    let mut reservations = Reservations::new(symbols.clone());
    let a = reservations.reserve(symbols.get("alpha").unwrap());
    let b = reservations.reserve(symbols.get("beta").unwrap());
    let mut builder = reservations.finish().unwrap();
    for id in if reverse { [b, a] } else { [a, b] } {
        builder
            .define(
                id,
                crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
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
        let mut reservations = Reservations::new(symbols.clone());
        let id = reservations.reserve(symbols.get("f").unwrap());
        let mut builder = reservations.finish().unwrap();
        builder
            .define(
                id,
                crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
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
    let mut reservations = Reservations::new(symbols.clone());
    let id = reservations.reserve(symbols.get("f").unwrap());
    let mut builder = reservations.finish().unwrap();
    builder
        .define(
            id,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    assert!(matches!(
        builder.define(
            id,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::I32
        ),
        Err(DefinitionError::AlreadyDefined)
    ));
    let program = builder.finish().unwrap();
    assert_eq!(
        program.data_kind(program.callables().get(id).unwrap().result()),
        Some(BuiltinType::Void)
    );
    let mut reservations = Reservations::new(symbols.clone());
    reservations.reserve(symbols.get("f").unwrap());
    let builder = reservations.finish().unwrap();
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
    let mut reservations = Reservations::new(symbols.clone());
    let f = reservations.reserve(symbols.get("f").unwrap());
    let mut builder = reservations.finish().unwrap();
    let parameter = ParameterDefinition {
        name: symbols.get("x").unwrap(),
        data: BuiltinType::I32.into(),
        operation: ValueInputOperation::Read,
    };
    builder
        .define(
            f,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [parameter.clone(), parameter],
            BuiltinType::F64,
        )
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

#[test]
fn callable_constraints_keep_the_selected_witness_and_report_exact_mismatch_domains() {
    let program = build(false);
    let first = program
        .resolve(program.symbols().get("alpha").unwrap())
        .unwrap();
    let other = program
        .resolve(program.symbols().get("beta").unwrap())
        .unwrap();
    let data = program.data_type(BuiltinType::Error).unwrap();
    let mut expected = FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([FunctionInput {
            data,
            operation: ValueInputOperation::Edit,
        }]),
        result: data,
    };
    let source = program
        .function_data(FunctionWitness {
            origin: nocter_model::ValueFunctionOrigin::Declaration(first),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    let alternate = program
        .function_data(FunctionWitness {
            origin: nocter_model::ValueFunctionOrigin::Declaration(other),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    assert_ne!(source, alternate);
    assert_eq!(program.constrain_function(source, &expected), Ok(source));
    expected.inputs[0].operation = ValueInputOperation::Take;
    assert!(matches!(
        program.constrain_function(source, &expected),
        Err(FunctionMismatch::Input { position: 0, .. })
    ));
    expected.inputs[0].operation = ValueInputOperation::Edit;
    expected.result = program.data_type(BuiltinType::Bool).unwrap();
    assert!(matches!(
        program.constrain_function(source, &expected),
        Err(FunctionMismatch::Result { .. })
    ));
    expected.result = data;
    expected.inputs = Box::new([]);
    assert!(matches!(
        program.constrain_function(source, &expected),
        Err(FunctionMismatch::InputCount {
            actual: 1,
            expected: 0
        })
    ));
    assert!(!program.function_adapter(source, alternate));
}

#[test]
fn named_function_views_have_exact_copy_payloads_without_upgrading_restricted_views() {
    let program = build(false);
    let declaration = program
        .resolve(program.symbols().get("alpha").unwrap())
        .unwrap();
    let views = [
        ValueInvocation::Read,
        ValueInvocation::Edit,
        ValueInvocation::Take,
    ]
    .map(|invocation| {
        program
            .function_data(FunctionWitness {
                origin: nocter_model::ValueFunctionOrigin::Declaration(declaration),
                invocation,
            })
            .unwrap()
    });
    for data in views {
        assert!(matches!(
            program.data_schema(data),
            Some(PayloadSchema::Function(_))
        ));
        assert_eq!(program.data_copyable(data), Some(true));
        assert_eq!(program.data_kind(data), None);
    }
    assert!(
        program.function_adapter(views[0], views[1])
            && program.function_adapter(views[1], views[2])
    );
    assert!(
        !program.function_adapter(views[2], views[0])
            && !program.function_adapter(views[1], views[0])
    );
}

#[test]
fn callable_result_headers_publish_signatures_without_inventing_producers_or_copy() {
    let symbols = SymbolTable::from_spellings(["make"]);
    let mut reservations = Reservations::new(symbols.clone());
    let id = reservations.reserve(symbols.get("make").unwrap());
    let mut builder = reservations.finish().unwrap();
    builder
        .define(
            id,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            ValueDefinition::Function(FunctionDefinition {
                invocation: ValueInvocation::Read,
                inputs: Box::new([(ValueInputOperation::Edit, BuiltinType::I32.into())]),
                result: BuiltinType::Void.into(),
            }),
        )
        .unwrap();
    let program = builder.finish().unwrap();
    let callable = program.callables().get(id).unwrap();
    let Some(PayloadSchema::Function(witness)) = program.data_schema(callable.result()) else {
        panic!("result schema");
    };
    assert_eq!(
        witness.origin,
        nocter_model::ValueFunctionOrigin::Result(id)
    );
    assert_eq!(program.data_copyable(callable.result()), Some(false));
    assert_eq!(
        callable.result_contract().unwrap().inputs[0].operation,
        ValueInputOperation::Edit
    );
    for invocation in [
        ValueInvocation::Read,
        ValueInvocation::Edit,
        ValueInvocation::Take,
    ] {
        assert!(
            program
                .function_data(FunctionWitness {
                    origin: witness.origin,
                    invocation
                })
                .is_some()
        );
    }
}

#[test]
fn application_suffixes_replay_by_body_owner_without_changing_the_header() {
    use nocter_model::{AccessExpressionId, ArenaBuilder, ValueFunctionOrigin as Origin};
    let symbols = SymbolTable::from_spellings(["first", "second", "make"]);
    let mut reservations = Reservations::new(symbols.clone());
    let first = reservations.reserve(symbols.get("first").unwrap());
    let second = reservations.reserve(symbols.get("second").unwrap());
    let make = reservations.reserve(symbols.get("make").unwrap());
    let mut builder = reservations.finish().unwrap();
    builder
        .define(
            first,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    builder
        .define(
            second,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            ValueDefinition::Function(FunctionDefinition {
                invocation: ValueInvocation::Read,
                inputs: Box::new([]),
                result: BuiltinType::Void.into(),
            }),
        )
        .unwrap();
    builder
        .define(
            make,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            ValueDefinition::Function(FunctionDefinition {
                invocation: ValueInvocation::Read,
                inputs: Box::new([]),
                result: BuiltinType::Void.into(),
            }),
        )
        .unwrap();
    let header = builder.finish().unwrap();
    let call = ArenaBuilder::<AccessExpressionId, ()>::new().insert(());
    let mut a = header.body_types(first).unwrap();
    let da = a.application(call, make).unwrap();
    assert_eq!(a.application(call, make).unwrap(), da);
    assert!(a.application(call, second).is_err());
    let a = a.finish();
    let mut b = header.body_types(second).unwrap();
    let db = b.application(call, make).unwrap();
    let b = b.finish();
    assert_eq!(da, db, "branch-local suffix positions may coincide");
    let Some(PayloadSchema::Function(wa)) = a.data_schema(da) else {
        panic!("application")
    };
    assert!(matches!(wa.origin, Origin::Application(_)));
    assert!(header.function_data(wa).is_none());
    assert_eq!(a.data_copyable(da), Some(false));
    assert_eq!(a.types.store().is_concrete(da), Some(false));
    assert!(a.types.store().may_carry_storage(da));
    let canonical = header
        .canonical_body_types(std::collections::BTreeMap::from([
            (second, &b),
            (first, &a),
        ]))
        .unwrap();
    let ca = canonical.project(first, da).unwrap();
    let cb = canonical.project(second, db).unwrap();
    assert_ne!(ca, cb);
    let canonical = canonical.finish();
    assert_eq!(
        canonical.data_schema(ca).unwrap(),
        a.data_schema(da).unwrap()
    );
    assert_eq!(
        canonical.data_schema(cb).unwrap(),
        b.data_schema(db).unwrap()
    );
    assert!(canonical.body_types(first).is_err());
    assert!(
        header
            .body_types(first)
            .unwrap()
            .application(call, first)
            .is_err()
    );
    assert!(
        header
            .canonical_body_types(std::collections::BTreeMap::from([(first, &b)]))
            .is_err()
    );
}

#[test]
fn owned_closure_schemas_replay_with_their_own_body_and_preserve_capture_copy_evidence() {
    use nocter_model::{ValueClosure, ValueFunctionOrigin as Origin};
    let symbols = SymbolTable::from_spellings(["first", "second"]);
    let mut reservations = Reservations::new(symbols.clone());
    let first = reservations.reserve(symbols.get("first").unwrap());
    let second = reservations.reserve(symbols.get("second").unwrap());
    let mut builder = reservations.finish().unwrap();
    builder
        .define(
            first,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    builder
        .define(
            second,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    let header = builder.finish().unwrap();
    let make = |owner, environment| {
        let mut branch = header.body_types(owner).unwrap();
        let key = ValueClosure { owner, slot: 0 };
        let contract = FunctionConstraint {
            invocation: ValueInvocation::Read,
            inputs: Box::new([]),
            result: header.data_type(BuiltinType::Void).unwrap(),
        };
        let data = branch
            .closure(
                key,
                header.data_type(environment).unwrap(),
                contract.clone(),
            )
            .unwrap();
        assert!(
            branch
                .closure(key, header.data_type(environment).unwrap(), contract)
                .is_err()
        );
        (branch.finish(), data, key)
    };
    let (a, da, ka) = make(first, BuiltinType::Error);
    let (b, db, kb) = make(second, BuiltinType::I32);
    assert_eq!(da, db);
    assert!(header.closure(ka).is_none());
    let canonical = header
        .canonical_body_types(std::collections::BTreeMap::from([
            (first, &a),
            (second, &b),
        ]))
        .unwrap();
    let ca = canonical.project(first, da).unwrap();
    let cb = canonical.project(second, db).unwrap();
    let program = canonical.finish();
    assert_ne!(ca, cb);
    assert_eq!(program.data_copyable(ca), Some(false));
    assert_eq!(program.data_copyable(cb), Some(true));
    assert_eq!(program.closure(ka), a.closure(ka));
    assert_eq!(program.closure(kb), b.closure(kb));
    assert_eq!(
        program.function_data(FunctionWitness {
            origin: Origin::Closure(ka),
            invocation: ValueInvocation::Read
        }),
        Some(ca)
    );
    assert!(
        header
            .canonical_body_types(std::collections::BTreeMap::from([(first, &b)]))
            .is_err()
    );
}
