use super::*;
use nocter_model::ValueInvocation;

#[test]
fn wrong_slot_counts_leave_both_definition_kinds_retryable_without_resizing() {
    let symbols = SymbolTable::from_spellings(["Box", "run", "value"]);
    let mut reservations = Reservations::new(symbols.clone());
    let record = reservations.reserve_record(symbols.get("Box").unwrap(), 1);
    let run = reservations.reserve(symbols.get("run").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    let fields = builder.field_ids(record).unwrap().to_vec();
    let inputs = builder.parameter_ids(run).unwrap().to_vec();
    let kind = CodeKind::Function(CompileTimeGuarantee::RuntimeOnly);
    let parameter = ParameterDefinition {
        binding: crate::value_access::ParameterBinding::Name(symbols.get("value").unwrap()),
        data: BuiltinType::I32.into(),
        operation: ValueInputOperation::Read,
    };
    let field = FieldDefinition {
        name: symbols.get("value").unwrap(),
        data: BuiltinType::Bool.into(),
    };
    for actual in [0, 2] {
        assert_eq!(
            builder.define(
                run,
                kind,
                vec![parameter.clone(); actual],
                BuiltinType::Void
            ),
            Err(DefinitionError::WrongArity {
                expected: 1,
                actual
            })
        );
        assert_eq!(
            builder.define_record(record, true, vec![field.clone(); actual]),
            Err(DefinitionError::WrongArity {
                expected: 1,
                actual
            })
        );
        assert_eq!(builder.parameter_ids(run), Some(inputs.as_slice()));
        assert_eq!(builder.field_ids(record), Some(fields.as_slice()));
    }
    builder
        .define(run, kind, [parameter], BuiltinType::Void)
        .unwrap();
    builder.define_record(record, true, [field]).unwrap();
    // Completed definitions keep precedence over an arity error on a later attempted replacement.
    assert_eq!(
        builder.define(run, kind, [], BuiltinType::Void),
        Err(DefinitionError::AlreadyDefined)
    );
    assert_eq!(
        builder.define_record(record, false, []),
        Err(DefinitionError::AlreadyDefined)
    );
    let program = builder.finish().unwrap();
    assert_eq!(program.codes().get(run).unwrap().parameters(), inputs);
    assert_eq!(program.record(record).unwrap().fields(), fields);
    assert_eq!(
        program.data_kind(program.parameters().get(inputs[0]).unwrap().data()),
        Some(BuiltinType::I32)
    );
    assert_eq!(
        program.data_kind(program.fields().get(fields[0]).unwrap().data()),
        Some(BuiltinType::Bool)
    );
}

#[test]
fn untyped_slot_reservations_do_not_publish_incomplete_declarations() {
    let symbols = SymbolTable::from_spellings(["Box", "run"]);
    let mut reservations = Reservations::new(symbols.clone());
    let record = reservations.reserve_record(symbols.get("Box").unwrap(), 3);
    let run = reservations.reserve(symbols.get("run").unwrap(), 2);
    let builder = reservations.finish().unwrap();
    assert_eq!(builder.field_ids(record).unwrap().len(), 3);
    assert_eq!(builder.parameter_ids(run).unwrap().len(), 2);
    let mut headers = builder.into_publication();
    for id in [DeclarationId::Code(run), DeclarationId::Nominal(record)] {
        assert_eq!(headers.is_defined(id), Some(false));
        assert_eq!(
            headers.publish(id),
            Err(HeaderFailure::Rejected(vec![DeclarationViolation {
                declaration: id,
                position: HeaderPosition::Name,
                rule: DeclarationRule::MissingDefinition,
            }]))
        );
        assert!(!headers.snapshot().header_ready(id));
    }
    assert!(headers.snapshot().fields().is_empty());
    assert!(headers.snapshot().parameters().is_empty());
    assert!(headers.finish().is_err());
}

#[test]
fn publication_keeps_reserved_slots_and_formal_witnesses_after_reversed_definitions() {
    let symbols = SymbolTable::from_spellings([
        "Box", "alpha", "beta", "action", "value", "item", "first", "second",
    ]);
    let symbol = |name| symbols.get(name).unwrap();
    let mut reservations = Reservations::new(symbols.clone());
    let record = reservations.reserve_record(symbol("Box"), 2);
    let alpha = reservations.reserve(symbol("alpha"), 2);
    let beta = reservations.reserve(symbol("beta"), 1);
    let mut builder = reservations.finish().unwrap();
    let fields = builder.field_ids(record).unwrap().to_vec();
    let inputs = builder.parameter_ids(alpha).unwrap().to_vec();
    let beta_input = builder.parameter_ids(beta).unwrap()[0];
    let kind = CodeKind::Function(CompileTimeGuarantee::RuntimeOnly);
    // Definitions deliberately run in the reverse callable order. Formal witness keys must use
    // the earlier input reservations rather than an allocation position in this traversal.
    builder
        .define(
            beta,
            kind,
            [ParameterDefinition {
                binding: crate::value_access::ParameterBinding::Name(symbol("item")),
                data: DataReference::Nominal(record).into(),
                operation: ValueInputOperation::Edit,
            }],
            BuiltinType::Void,
        )
        .unwrap();
    builder
        .define(
            alpha,
            kind,
            [
                ParameterDefinition {
                    binding: crate::value_access::ParameterBinding::Name(symbol("action")),
                    operation: ValueInputOperation::Read,
                    data: ValueDefinition::Function(FunctionDefinition {
                        invocation: ValueInvocation::Read,
                        inputs: Box::new([]),
                        result: BuiltinType::I32.into(),
                    }),
                },
                ParameterDefinition {
                    binding: crate::value_access::ParameterBinding::Name(symbol("value")),
                    data: BuiltinType::Usize.into(),
                    operation: ValueInputOperation::Take,
                },
            ],
            BuiltinType::Usize,
        )
        .unwrap();
    builder
        .define_record(
            record,
            true,
            [
                FieldDefinition {
                    name: symbol("first"),
                    data: BuiltinType::I32.into(),
                },
                FieldDefinition {
                    name: symbol("second"),
                    data: BuiltinType::Bool.into(),
                },
            ],
        )
        .unwrap();
    let program = builder.finish().unwrap();
    assert_eq!(program.codes().get(alpha).unwrap().parameters(), inputs);
    assert_eq!(
        program.codes().get(beta).unwrap().parameters(),
        &[beta_input]
    );
    assert_eq!(program.record(record).unwrap().fields(), fields);
    for (position, &id) in inputs.iter().enumerate() {
        let parameter = program.parameters().get(id).unwrap();
        assert_eq!((parameter.owner(), parameter.position()), (alpha, position));
    }
    for (position, &id) in fields.iter().enumerate() {
        let field = program.fields().get(id).unwrap();
        assert_eq!((field.owner(), field.position()), (record, position));
        assert_eq!(
            program.record(record).unwrap().resolve_field(field.name()),
            Some(id)
        );
    }
    let parameter = program.parameters().get(inputs[0]).unwrap();
    let Some(PayloadSchema::Function(witness)) = program.data_schema(parameter.data()) else {
        panic!("formal witness");
    };
    assert_eq!(
        witness.origin,
        nocter_model::ValueFunctionOrigin::Parameter(inputs[0].into())
    );
}

#[test]
fn one_closed_namespace_preserves_declaration_roles_before_and_after_type_publication() {
    let symbols = SymbolTable::from_spellings(["Item", "run"]);
    let mut reservations = Reservations::new(symbols.clone());
    let item_name = symbols.get("Item").unwrap();
    let run_name = symbols.get("run").unwrap();
    let item = reservations.reserve_record(item_name, 0);
    let run = reservations.reserve(run_name, 0);
    let mut builder = reservations.finish().unwrap();
    // A forward record reference needs no invented TypeId or complete record definition.
    assert_eq!(
        builder.resolve_data(item_name),
        Ok(DataReference::Nominal(item))
    );
    assert_eq!(
        builder.resolve_data(run_name),
        Err(DeclarationRule::UnknownDataName { name: run_name })
    );
    builder.define_record(item, false, []).unwrap();
    builder
        .define(
            run,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            DataReference::Nominal(item),
        )
        .unwrap();
    let program = builder.finish().unwrap();
    assert_eq!(program.resolve_nominal(item_name), Some(item));
    assert_eq!(program.resolve(run_name), Some(run));
    assert_eq!(program.resolve_nominal(run_name), None);
    assert_eq!(program.resolve_code(item_name), None);
    assert_eq!(program.type_named("run"), None);
    assert_eq!(
        program.type_named("Item"),
        Some(program.codes().get(run).unwrap().result())
    );
}

#[test]
fn invalid_callable_names_fail_before_requesting_any_definition_or_type() {
    let symbols = SymbolTable::from_spellings(["f", "i32"]);
    let foreign = SymbolTable::from_spellings(["a", "b", "c"]);
    let mut reservations = Reservations::new(symbols.clone());
    let first = reservations.reserve(symbols.get("f").unwrap(), 0);
    let duplicate = reservations.reserve(symbols.get("f").unwrap(), 0);
    let builtin = reservations.reserve(symbols.get("i32").unwrap(), 0);
    let unknown = reservations.reserve(foreign.get("c").unwrap(), 0);
    assert_eq!(
        reservations.finish().unwrap_err(),
        [
            DeclarationViolation {
                declaration: DeclarationId::Code(duplicate),
                position: HeaderPosition::Name,
                rule: DeclarationRule::DuplicateValue { first },
            },
            DeclarationViolation {
                declaration: DeclarationId::Code(builtin),
                position: HeaderPosition::Name,
                rule: DeclarationRule::ReservedBuiltinName,
            },
            DeclarationViolation {
                declaration: DeclarationId::Code(unknown),
                position: HeaderPosition::Name,
                rule: DeclarationRule::UnknownSymbol,
            },
        ]
    );
}

#[test]
fn equal_type_and_value_spellings_retain_distinct_unready_identities() {
    let symbols = SymbolTable::from_spellings(["Shared"]);
    let name = symbols.get("Shared").unwrap();
    let mut reservations = Reservations::new(symbols);
    let record = reservations.reserve_record(name, 0);
    let code = reservations.reserve(name, 0);
    let builder = reservations.finish().unwrap();
    assert_eq!(
        builder.resolve_data(name),
        Ok(DataReference::Nominal(record))
    );
    let headers = builder.into_publication();
    let program = headers.snapshot();
    assert_eq!(
        program.declaration(Namespace::Type, name),
        Some(record.into())
    );
    assert_eq!(
        program.declaration(Namespace::Value, name),
        Some(code.into())
    );
    assert!(!program.header_ready(record));
    assert!(!program.header_ready(code));
    assert_eq!(program.resolve_nominal(name), Some(record));
    assert_eq!(program.resolve_code(name), Some(code));
    assert_eq!(program.resolve(name), None);
    assert_eq!(program.type_named("Shared"), None);
}

#[test]
fn repeated_parameters_retain_the_first_input_even_when_it_shadows_a_declaration() {
    let symbols = SymbolTable::from_spellings(["Item", "run"]);
    let mut reservations = Reservations::new(symbols.clone());
    let item_name = symbols.get("Item").unwrap();
    let run_name = symbols.get("run").unwrap();
    let item = reservations.reserve_record(item_name, 0);
    let run = reservations.reserve(run_name, 4);
    let mut builder = reservations.finish().unwrap();
    builder.define_record(item, false, []).unwrap();
    builder
        .define(
            run,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [item_name, item_name, run_name, run_name].map(|name| ParameterDefinition {
                binding: crate::value_access::ParameterBinding::Name(name),
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
                HeaderPosition::ParameterName { input: 1, leaf: 0 },
                DeclarationRule::DuplicateParameter {
                    first: HeaderPosition::ParameterName { input: 0, leaf: 0 }
                }
            ),
            (
                HeaderPosition::ParameterName { input: 2, leaf: 0 },
                DeclarationRule::ParameterShadowsValue { declaration: run }
            ),
            (
                HeaderPosition::ParameterName { input: 3, leaf: 0 },
                DeclarationRule::DuplicateParameter {
                    first: HeaderPosition::ParameterName { input: 2, leaf: 0 }
                }
            ),
        ]
    );
}

fn build(reverse: bool) -> Program {
    let symbols = SymbolTable::from_spellings(["alpha", "beta", "input"]);
    let mut reservations = Reservations::new(symbols.clone());
    let a = reservations.reserve(symbols.get("alpha").unwrap(), 1);
    let b = reservations.reserve(symbols.get("beta").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    for id in if reverse { [b, a] } else { [a, b] } {
        builder
            .define(
                id,
                crate::value_access::CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
                [ParameterDefinition {
                    binding: crate::value_access::ParameterBinding::Name(
                        symbols.get("input").unwrap(),
                    ),
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
    assert_eq!(a.codes, b.codes);
    assert_eq!(a.parameters, b.parameters);
    assert_eq!(a.namespace, b.namespace);
    for (id, callable) in a.codes.iter() {
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
        let id = reservations.reserve(symbols.get("f").unwrap(), 3);
        let mut builder = reservations.finish().unwrap();
        builder
            .define(
                id,
                crate::value_access::CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
                [
                    ParameterDefinition {
                        binding: crate::value_access::ParameterBinding::Name(
                            symbols.get("read").unwrap(),
                        ),
                        data: data.into(),
                        operation: ValueInputOperation::Read,
                    },
                    ParameterDefinition {
                        binding: crate::value_access::ParameterBinding::Name(
                            symbols.get("edit").unwrap(),
                        ),
                        data: data.into(),
                        operation: ValueInputOperation::Edit,
                    },
                    ParameterDefinition {
                        binding: crate::value_access::ParameterBinding::Name(
                            symbols.get("take").unwrap(),
                        ),
                        data: data.into(),
                        operation: ValueInputOperation::Take,
                    },
                ],
                data,
            )
            .unwrap();
        let program = builder.finish().unwrap();
        let callable = program.codes().get(id).unwrap();
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
    let id = reservations.reserve(symbols.get("f").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    builder
        .define(
            id,
            crate::value_access::CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    assert!(matches!(
        builder.define(
            id,
            crate::value_access::CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::I32
        ),
        Err(DefinitionError::AlreadyDefined)
    ));
    let program = builder.finish().unwrap();
    assert_eq!(
        program.data_kind(program.codes().get(id).unwrap().result()),
        Some(BuiltinType::Void)
    );
    let mut reservations = Reservations::new(symbols.clone());
    reservations.reserve(symbols.get("f").unwrap(), 0);
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
    let f = reservations.reserve(symbols.get("f").unwrap(), 2);
    let mut builder = reservations.finish().unwrap();
    let parameter = ParameterDefinition {
        binding: crate::value_access::ParameterBinding::Name(symbols.get("x").unwrap()),
        data: BuiltinType::I32.into(),
        operation: ValueInputOperation::Read,
    };
    builder
        .define(
            f,
            crate::value_access::CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [parameter.clone(), parameter],
            BuiltinType::F64,
        )
        .unwrap();
    let violations = builder.finish().unwrap_err();
    assert!(violations.iter().any(|v| matches!(
        v.rule,
        DeclarationRule::DuplicateParameter {
            first: HeaderPosition::ParameterName { input: 0, leaf: 0 }
        }
    )));
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
    let id = reservations.reserve(symbols.get("make").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    builder
        .define(
            id,
            crate::value_access::CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            ValueDefinition::Function(FunctionDefinition {
                invocation: ValueInvocation::Read,
                inputs: Box::new([(ValueInputOperation::Edit, BuiltinType::I32.into())]),
                result: BuiltinType::Void.into(),
            }),
        )
        .unwrap();
    let program = builder.finish().unwrap();
    let callable = program.codes().get(id).unwrap();
    let Some(PayloadSchema::Function(witness)) = program.data_schema(callable.result()) else {
        panic!("result schema");
    };
    assert_eq!(
        witness.origin,
        nocter_model::ValueFunctionOrigin::Result(id.into())
    );
    assert_eq!(program.data_copyable(callable.result()), Some(false));
    assert_eq!(
        callable.result_contracts()[0].inputs[0].operation,
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
fn anonymous_type_expression_code_has_no_name_or_function_witness() {
    let symbols = SymbolTable::from_spellings(["Buffer", "open", "value"]);
    let mut reservations = Reservations::new(symbols.clone());
    let record = reservations.reserve_record(symbols.get("Buffer").unwrap(), 1);
    let function = reservations.reserve(symbols.get("open").unwrap(), 1);
    let sites = [
        TypeExpressionSource {
            declaration: record.into(),
            position: HeaderPosition::Field(0),
        },
        TypeExpressionSource {
            declaration: function.into(),
            position: HeaderPosition::Parameter(0),
        },
        TypeExpressionSource {
            declaration: function.into(),
            position: HeaderPosition::Result,
        },
    ];
    let expressions = sites.map(|site| reservations.reserve_type_expression(site).unwrap());
    let mut builder = reservations.finish().unwrap();
    for &expression in &expressions {
        assert_eq!(
            builder.define(
                expression,
                CodeKind::ConstantInitializer,
                [],
                BuiltinType::I32
            ),
            Err(DefinitionError::AlreadyDefined)
        );
    }
    builder
        .define_record(
            record,
            false,
            [FieldDefinition {
                name: symbols.get("value").unwrap(),
                data: BuiltinType::I32.into(),
            }],
        )
        .unwrap();
    builder
        .define(
            function,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [ParameterDefinition {
                binding: crate::value_access::ParameterBinding::Name(symbols.get("value").unwrap()),
                data: BuiltinType::I32.into(),
                operation: ValueInputOperation::Read,
            }],
            BuiltinType::Void,
        )
        .unwrap();
    let mut headers = builder.into_publication();
    assert_eq!(headers.declaration_ids().count(), 2);
    assert_eq!(headers.code_ids().count(), 4);
    headers.publish(expressions[0]).unwrap();
    let early = headers.snapshot();
    assert!(!early.header_ready(record));
    assert!(!early.headers_complete());
    assert_eq!(
        early.code_label(expressions[0]).as_deref(),
        Some("type expression in Buffer")
    );
    let program = headers.finish().unwrap();
    assert!(program.headers_complete());
    assert!(program.preserves_headers(&early));
    assert_eq!(
        program.resolve_code(symbols.get("open").unwrap()),
        Some(function)
    );
    assert_eq!(program.resolve_code(symbols.get("Buffer").unwrap()), None);
    for (expression, site) in expressions.into_iter().zip(sites) {
        let code = program.codes().get(expression).unwrap();
        assert_eq!(code.name(), None);
        assert_eq!(code.kind(), CodeKind::TypeExpression(site));
        assert!(code.kind().is_initializer());
        assert_eq!(code.compile_time(), CompileTimeGuarantee::Evaluatable);
        assert!(code.parameters().is_empty());
        assert_eq!(program.data_kind(code.result()), Some(BuiltinType::Usize));
        let witness = FunctionWitness {
            origin: nocter_model::ValueFunctionOrigin::Declaration(expression),
            invocation: ValueInvocation::Read,
        };
        assert_eq!(program.function_contract(witness), None);
        assert_eq!(program.function_data(witness), None);
    }
}

#[test]
fn invalid_type_expression_sites_do_not_consume_code_slots() {
    let symbols = SymbolTable::from_spellings(["Record", "run", "after"]);
    let mut reservations = Reservations::new(symbols.clone());
    let record = reservations.reserve_record(symbols.get("Record").unwrap(), 1);
    let function = reservations.reserve(symbols.get("run").unwrap(), 1);
    let mut outside_codes = ArenaBuilder::<AccessCodeId, ()>::new();
    outside_codes.insert(());
    let unknown_code = outside_codes.insert(());
    let mut outside_records = ArenaBuilder::<NominalTypeId, ()>::new();
    outside_records.insert(());
    let unknown_record = outside_records.insert(());
    for (declaration, position) in [
        (unknown_code.into(), HeaderPosition::Result),
        (unknown_record.into(), HeaderPosition::Field(0)),
        (record.into(), HeaderPosition::Name),
        (record.into(), HeaderPosition::Field(1)),
        (record.into(), HeaderPosition::Result),
        (record.into(), HeaderPosition::Parameter(0)),
        (function.into(), HeaderPosition::Name),
        (function.into(), HeaderPosition::Field(0)),
        (function.into(), HeaderPosition::Parameter(1)),
    ] {
        assert_eq!(
            reservations.reserve_type_expression(TypeExpressionSource {
                declaration,
                position
            }),
            Err(DefinitionError::UnknownId)
        );
    }
    let after = reservations.reserve(symbols.get("after").unwrap(), 0);
    // Failed reservations cannot leave anonymous slots in the eventual complete-code domain.
    let expression = reservations
        .reserve_type_expression(TypeExpressionSource {
            declaration: function.into(),
            position: HeaderPosition::Result,
        })
        .unwrap();
    assert_eq!(
        reservations.reserve_type_expression(TypeExpressionSource {
            declaration: expression.into(),
            position: HeaderPosition::Result
        }),
        Err(DefinitionError::UnknownId)
    );
    let headers = reservations.finish().unwrap().into_publication();
    assert_eq!(
        headers.code_ids().collect::<Vec<_>>(),
        [function, after, expression]
    );
}

#[test]
fn named_code_cannot_masquerade_as_an_anonymous_type_expression() {
    let symbols = SymbolTable::from_spellings(["run"]);
    let mut reservations = Reservations::new(symbols.clone());
    let function = reservations.reserve(symbols.get("run").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    builder
        .define(
            function,
            CodeKind::TypeExpression(TypeExpressionSource {
                declaration: function.into(),
                position: HeaderPosition::Result,
            }),
            [],
            BuiltinType::Usize,
        )
        .unwrap();
    let failure = builder.finish().unwrap_err();
    assert_eq!(
        failure,
        [DeclarationViolation {
            declaration: function.into(),
            position: HeaderPosition::Result,
            rule: DeclarationRule::InvalidInitializerHeader
        }]
    );
}

mod lexical_scopes;
mod scopes;
