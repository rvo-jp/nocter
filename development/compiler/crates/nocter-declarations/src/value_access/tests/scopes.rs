use super::*;

#[test]
fn scope_local_names_and_shared_bindings_preserve_one_identity_and_type_authority() {
    let symbols = SymbolTable::from_spellings(["Data", "value", "renamed", "other", "i32"]);
    let data = symbols.get("Data").unwrap();
    let value = symbols.get("value").unwrap();
    let alias = symbols.get("renamed").unwrap();
    let other_name = symbols.get("other").unwrap();
    let mut names = Reservations::new(symbols.clone());
    let other = names.new_scope();
    let empty = names.new_scope();
    let a = names.reserve(value, 0);
    let b = names.reserve_in(other, value, 0).unwrap();
    let a_data = names.reserve_record(data, 0);
    let b_data = names.reserve_record_in(other, data, 0).unwrap();
    names.bind(ScopeId::ROOT, alias, b).unwrap();
    names.bind(ScopeId::ROOT, other_name, b_data).unwrap();
    names.bind(empty, alias, b).unwrap();
    names.bind(empty, alias, b).unwrap();
    let mut definitions = names.finish().unwrap();
    assert_eq!(
        definitions.resolve_data_in(ScopeId::ROOT, data),
        Ok(DataReference::Record(a_data))
    );
    assert_eq!(
        definitions.resolve_data_in(other, data),
        Ok(DataReference::Record(b_data))
    );
    assert_eq!(
        definitions.resolve_data_in(ScopeId::ROOT, other_name),
        Ok(DataReference::Record(b_data))
    );
    assert_eq!(
        definitions.resolve_data_in(empty, data),
        Err(DeclarationRule::UnknownDataName { name: data })
    );
    for (code, data) in [(a, a_data), (b, b_data)] {
        definitions
            .define(
                code,
                CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
                Vec::new(),
                DataReference::Record(data),
            )
            .unwrap();
        definitions.define_record(data, true, Vec::new()).unwrap();
    }
    let headers = definitions.into_publication();
    let ids = headers.declaration_ids().collect::<Vec<_>>();
    assert_eq!(ids.len(), 4);
    assert_eq!(
        ids.iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    let early = headers.snapshot();
    assert_eq!(
        early.declaration_in(ScopeId::ROOT, Namespace::Value, value),
        Some(a.into())
    );
    assert_eq!(
        early.declaration_in(other, Namespace::Value, value),
        Some(b.into())
    );
    assert_eq!(early.declaration_in(empty, Namespace::Value, value), None);
    assert_eq!(
        early.declaration_in(empty, Namespace::Value, alias),
        Some(b.into())
    );
    assert_eq!(early.declaration_label(b.into()).as_deref(), Some("value"));
    assert!(!early.header_ready(b));
    let complete = headers.finish().unwrap();
    assert!(complete.preserves_headers(&early));
    let first = complete.type_named_in(ScopeId::ROOT, "Data").unwrap();
    let second = complete.type_named_in(other, "Data").unwrap();
    assert_ne!(first, second);
    assert_eq!(complete.type_named_in(ScopeId::ROOT, "other"), Some(second));
    assert_eq!(complete.type_named_in(empty, "Data"), None);
    assert_eq!(complete.codes().get(a).unwrap().result(), first);
    assert_eq!(complete.codes().get(b).unwrap().result(), second);
    assert_eq!(complete.record_identity(second), Some(b_data));
}

#[test]
fn bindings_into_one_scope_reject_distinct_identities_before_missing_type_definitions() {
    for record in [false, true] {
        let symbols = SymbolTable::from_spellings(["item"]);
        let name = symbols.get("item").unwrap();
        let mut declarations = Reservations::new(symbols);
        let other = declarations.new_scope();
        let (first, second): (DeclarationId, DeclarationId) = if record {
            (
                declarations.reserve_record(name, 0).into(),
                declarations
                    .reserve_record_in(other, name, 0)
                    .unwrap()
                    .into(),
            )
        } else {
            (
                declarations.reserve(name, 0).into(),
                declarations.reserve_in(other, name, 0).unwrap().into(),
            )
        };
        declarations.bind(ScopeId::ROOT, name, second).unwrap();
        let errors = declarations.finish().unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].declaration, second);
        assert_eq!(errors[0].position, HeaderPosition::Name);
        assert_eq!(
            errors[0].rule,
            match first {
                DeclarationId::Code(first) => DeclarationRule::DuplicateValue { first },
                DeclarationId::Record(first) => DeclarationRule::DuplicateRecord { first },
            }
        );
    }
}

#[test]
fn parameter_collision_checks_use_the_declarations_own_scope() {
    for exposed in [false, true] {
        let symbols = SymbolTable::from_spellings(["input", "run"]);
        let input = symbols.get("input").unwrap();
        let run_name = symbols.get("run").unwrap();
        let mut declarations = Reservations::new(symbols);
        let other = declarations.new_scope();
        let outside = declarations.reserve(input, 0);
        let run = declarations.reserve_in(other, run_name, 1).unwrap();
        if exposed {
            declarations.bind(other, input, outside).unwrap();
        }
        let mut headers = declarations.finish().unwrap().into_publication();
        headers
            .define(
                outside,
                CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
                Vec::new(),
                ValueDefinition::Data(BuiltinType::I32.into()),
            )
            .unwrap();
        headers
            .define(
                run,
                CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
                vec![ParameterDefinition {
                    name: input,
                    data: BuiltinType::I32.into(),
                    operation: ValueInputOperation::Read,
                }],
                ValueDefinition::Data(BuiltinType::I32.into()),
            )
            .unwrap();
        let result = headers.publish(run);
        if exposed {
            assert!(
                matches!(result, Err(HeaderFailure::Rejected(errors)) if errors.iter().any(|error| error.rule == DeclarationRule::ParameterShadowsValue { declaration: outside }))
            );
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn unknown_scopes_reject_reservation_and_builtin_lookup_without_mutating_slots() {
    let symbols = SymbolTable::from_spellings(["run", "Data", "i32"]);
    let name = symbols.get("run").unwrap();
    let data = symbols.get("Data").unwrap();
    let builtin = symbols.get("i32").unwrap();
    let mut declarations = Reservations::new(symbols);
    let invalid = ScopeId::new(1);
    assert_eq!(
        declarations.reserve_in(invalid, name, 3),
        Err(DefinitionError::UnknownId)
    );
    assert_eq!(
        declarations.reserve_record_in(invalid, data, 3),
        Err(DefinitionError::UnknownId)
    );
    let code = declarations.reserve(name, 0);
    assert_eq!(
        declarations.bind(invalid, name, code),
        Err(DefinitionError::UnknownId)
    );
    let headers = declarations.finish().unwrap().into_publication();
    assert_eq!(headers.declaration_ids().count(), 1);
    assert_eq!(headers.code_ids().count(), 1);
    let early = headers.snapshot();
    assert!(!early.has_scope(invalid));
    assert_eq!(early.type_identity_named_in(invalid, "i32"), None);
    assert_eq!(
        headers.resolve_data_in(invalid, builtin),
        Err(DeclarationRule::UnknownDataName { name: builtin })
    );
    assert_eq!(
        headers.resolve_data_in(ScopeId::ROOT, builtin),
        Ok(DataReference::Builtin(BuiltinType::I32))
    );
}

#[test]
fn repeated_type_slots_can_reserve_distinct_physical_expression_scopes() {
    let symbols = SymbolTable::from_spellings(["run"]);
    let name = symbols.get("run").unwrap();
    let mut declarations = Reservations::new(symbols);
    let other = declarations.new_scope();
    let code = declarations.reserve(name, 0);
    let slot = TypeExpressionSource {
        declaration: code.into(),
        position: HeaderPosition::Result,
    };
    let root_count = declarations.reserve_type_expression(slot).unwrap();
    let other_count = declarations
        .reserve_type_expression_in(other, slot)
        .unwrap();
    assert_ne!(root_count, other_count);
    assert_eq!(
        declarations.codes.get(root_count).unwrap().scope,
        ScopeId::ROOT
    );
    assert_eq!(declarations.codes.get(other_count).unwrap().scope, other);
    for (scope, site) in [
        (ScopeId::new(2), slot),
        (
            other,
            TypeExpressionSource {
                declaration: root_count.into(),
                position: HeaderPosition::Result,
            },
        ),
        (
            other,
            TypeExpressionSource {
                declaration: code.into(),
                position: HeaderPosition::Parameter(0),
            },
        ),
    ] {
        assert_eq!(
            declarations.reserve_type_expression_in(scope, site),
            Err(DefinitionError::UnknownId)
        );
    }
    let headers = declarations.finish().unwrap().into_publication();
    assert_eq!(headers.declaration_ids().count(), 1);
    assert_eq!(
        headers.code_ids().collect::<Vec<_>>(),
        [code, root_count, other_count]
    );
    assert!(!headers.snapshot().headers_complete());
}

#[test]
fn scoped_code_classification_never_falls_back_to_root_or_grants_unready_callability() {
    for kind in [
        CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
        CodeKind::ConstantInitializer,
        CodeKind::StaticInitializer,
    ] {
        let symbols = SymbolTable::from_spellings(["item"]);
        let name = symbols.get("item").unwrap();
        let mut reservations = Reservations::new(symbols);
        let other = reservations.new_scope();
        let first = reservations.reserve(name, 0);
        let second = reservations.reserve_in(other, name, 0).unwrap();
        let mut headers = reservations.finish().unwrap().into_publication();
        let early = headers.snapshot();
        assert_eq!(early.resolve_code_in(other, name), Some(second));
        assert_eq!(early.resolve_in(other, name), None);
        assert_eq!(early.resolve_constant_in(other, name), None);
        assert_eq!(early.resolve_static_in(other, name), None);
        for code in [first, second] {
            headers.define(code, kind, [], BuiltinType::I32).unwrap();
            headers.publish(code).unwrap();
        }
        let program = headers.snapshot();
        for (scope, code) in [(ScopeId::ROOT, first), (other, second)] {
            assert_eq!(program.resolve_code_in(scope, name), Some(code));
            assert_eq!(
                program.resolve_in(scope, name),
                kind.is_callable().then_some(code)
            );
            assert_eq!(
                program.resolve_constant_in(scope, name),
                (kind == CodeKind::ConstantInitializer).then_some(code)
            );
            assert_eq!(
                program
                    .resolve_static_in(scope, name)
                    .map(|s| s.initializer),
                (kind == CodeKind::StaticInitializer).then_some(code)
            );
        }
        let unknown = ScopeId::new(2);
        assert_eq!(program.resolve_code_in(unknown, name), None);
        assert_eq!(program.resolve_in(unknown, name), None);
        assert_eq!(program.resolve_constant_in(unknown, name), None);
        assert_eq!(program.resolve_static_in(unknown, name), None);
    }
}

struct NamespacePathFixture {
    headers: HeaderPublication,
    symbols: SymbolTable,
    privileged: ScopeId,
    unknown_caller: ScopeId,
    data: NominalTypeId,
    hidden: NominalTypeId,
}

fn namespace_path_fixture() -> NamespacePathFixture {
    use nocter_model::{ModuleAudience, ModuleIdentity, PackageIdentity};
    use std::sync::Arc;
    let symbols = SymbolTable::from_spellings([
        "facade", "inner", "private", "Data", "Hidden", "item", "i32",
    ]);
    let name = |text| symbols.get(text).unwrap();
    let mut reservations = Reservations::new(symbols.clone());
    let privileged = reservations.new_scope();
    let facade = reservations.new_scope();
    let exports = reservations.new_scope();
    let unknown_caller = reservations.new_scope();
    let module =
        |path: &[&str]| ModuleIdentity::new(PackageIdentity::new("app"), path.iter().copied());
    let restricted = Arc::new(ModuleAudience::Descendants(module(&["secret"])));
    reservations
        .set_scope_module(ScopeId::ROOT, module(&["client"]))
        .unwrap();
    reservations
        .set_scope_module(privileged, module(&["secret"]))
        .unwrap();
    reservations
        .set_scope_module(facade, module(&["secret"]))
        .unwrap();
    reservations
        .set_scope_module(exports, module(&["secret"]))
        .unwrap();
    assert_eq!(
        reservations.set_scope_module(ScopeId::ROOT, module(&["secret"])),
        Err(ScopeBindingError::ConflictingModule)
    );
    let data = reservations
        .reserve_record_in(privileged, name("Data"), 0)
        .unwrap();
    let hidden = reservations
        .reserve_record_in(privileged, name("Hidden"), 0)
        .unwrap();
    reservations
        .bind_export(
            exports,
            name("Data"),
            data,
            Arc::new(ModuleAudience::Public),
        )
        .unwrap();
    reservations
        .bind_export(exports, name("Hidden"), hidden, Arc::clone(&restricted))
        .unwrap();
    for scope in [ScopeId::ROOT, privileged, unknown_caller] {
        reservations
            .bind_namespace(scope, name("facade"), facade, None)
            .unwrap();
    }
    reservations
        .bind_namespace(facade, name("inner"), exports, None)
        .unwrap();
    reservations
        .bind_namespace(
            facade,
            name("private"),
            exports,
            Some(Arc::clone(&restricted)),
        )
        .unwrap();
    assert_eq!(
        reservations.bind_namespace(ScopeId::ROOT, name("facade"), exports, None),
        Err(ScopeBindingError::ConflictingBinding)
    );
    assert_eq!(
        reservations.bind_namespace(ScopeId::new(99), name("facade"), exports, None),
        Err(ScopeBindingError::UnknownScope)
    );
    assert_eq!(
        reservations.bind_namespace(ScopeId::ROOT, name("i32"), exports, None),
        Err(ScopeBindingError::ReservedBuiltinName)
    );
    reservations
        .bind_namespace(
            ScopeId::ROOT,
            name("facade"),
            facade,
            Some(Arc::new(ModuleAudience::Public)),
        )
        .unwrap();
    assert_eq!(
        reservations.bind_namespace(
            ScopeId::ROOT,
            name("facade"),
            facade,
            Some(Arc::clone(&restricted))
        ),
        Err(ScopeBindingError::ConflictingBinding)
    );
    NamespacePathFixture {
        headers: reservations.finish().unwrap().into_publication(),
        symbols,
        privileged,
        unknown_caller,
        data,
        hidden,
    }
}

#[test]
fn namespace_paths_preserve_the_original_caller_and_never_create_a_value_or_type() {
    let NamespacePathFixture {
        headers,
        symbols,
        privileged,
        unknown_caller,
        data,
        hidden,
    } = namespace_path_fixture();
    let name = |text| symbols.get(text).unwrap();
    assert_eq!(headers.declaration_ids().count(), 2);
    assert_eq!(headers.code_ids().count(), 0);
    let program = headers.snapshot();
    assert!(program.has_namespace_in(ScopeId::ROOT, name("facade")));
    for namespace in [Namespace::Type, Namespace::Value] {
        assert_eq!(
            program.declaration_in(ScopeId::ROOT, namespace, name("facade")),
            None
        );
    }
    let path = |leaf| [Some(name("facade")), Some(name("inner")), Some(name(leaf))];
    assert_eq!(
        program.declaration_path_in(ScopeId::ROOT, Namespace::Type, &path("Data")),
        Ok(data.into())
    );
    assert_eq!(
        program.declaration_path_in(ScopeId::ROOT, Namespace::Type, &path("Hidden")),
        Err(QualifiedNameError::Inaccessible { position: 2 })
    );
    assert_eq!(
        program.declaration_path_in(privileged, Namespace::Type, &path("Hidden")),
        Ok(hidden.into())
    );
    assert_eq!(
        program.declaration_path_in(unknown_caller, Namespace::Type, &path("Hidden")),
        Err(QualifiedNameError::Inaccessible { position: 2 })
    );
    assert_eq!(
        program.declaration_path_in(
            ScopeId::ROOT,
            Namespace::Type,
            &[
                Some(name("facade")),
                Some(name("private")),
                Some(name("Data"))
            ]
        ),
        Err(QualifiedNameError::Inaccessible { position: 1 })
    );
    assert_eq!(
        program.declaration_path_in(ScopeId::ROOT, Namespace::Type, &[]),
        Err(QualifiedNameError::EmptyPath)
    );
    assert_eq!(
        program.declaration_path_in(ScopeId::new(99), Namespace::Type, &path("Data")),
        Err(QualifiedNameError::UnknownScope)
    );
    assert_eq!(
        program.declaration_path_in(ScopeId::ROOT, Namespace::Value, &path("Data")),
        Err(QualifiedNameError::UnknownDeclaration { position: 2 })
    );
    assert_eq!(
        program.declaration_path_in(
            ScopeId::ROOT,
            Namespace::Type,
            &[Some(name("item")), Some(name("Data"))]
        ),
        Err(QualifiedNameError::UnknownNamespace { position: 0 })
    );
    assert!(!program.header_ready(data));
    assert_eq!(program.records().len(), 0);
}

#[test]
fn qualified_prefix_selection_leaves_uninterned_members_to_ordinary_typing() {
    let NamespacePathFixture {
        headers,
        symbols,
        data,
        ..
    } = namespace_path_fixture();
    let program = headers.snapshot();
    let name = |text| symbols.get(text).unwrap();
    assert_eq!(
        program.declaration_prefix_in(
            ScopeId::ROOT,
            Namespace::Type,
            &[
                Some(name("facade")),
                Some(name("inner")),
                Some(name("Data")),
                None
            ]
        ),
        Ok((data.into(), 3))
    );
    assert_eq!(
        program.declaration_path_in(
            ScopeId::ROOT,
            Namespace::Type,
            &[
                Some(name("facade")),
                Some(name("inner")),
                Some(name("Data")),
                None
            ]
        ),
        Err(QualifiedNameError::UnknownNamespace { position: 2 })
    );
    assert_eq!(
        program.declaration_prefix_in(
            ScopeId::ROOT,
            Namespace::Value,
            &[Some(name("facade")), None]
        ),
        Err(QualifiedNameError::UnknownDeclaration { position: 1 })
    );
    assert!(!program.header_ready(data));
}

#[test]
fn namespace_bindings_reject_typed_name_collisions_and_conflicting_export_audiences() {
    use nocter_model::ModuleAudience;
    use std::sync::Arc;
    for record in [false, true] {
        let symbols = SymbolTable::from_spellings(["item"]);
        let name = symbols.get("item").unwrap();
        let mut reservations = Reservations::new(symbols);
        let exported = reservations.new_scope();
        let id: DeclarationId = if record {
            reservations.reserve_record(name, 0).into()
        } else {
            reservations.reserve(name, 0).into()
        };
        reservations
            .bind_namespace(ScopeId::ROOT, name, exported, None)
            .unwrap();
        let errors = reservations.finish().unwrap_err();
        assert!(errors.iter().any(|error| error.declaration == id
            && error.rule == DeclarationRule::NameConflictsWithNamespace { name }));
    }
    let symbols = SymbolTable::from_spellings(["item"]);
    let name = symbols.get("item").unwrap();
    let mut reservations = Reservations::new(symbols);
    let exports = reservations.new_scope();
    let item = reservations.reserve(name, 0);
    reservations
        .bind_export(
            exports,
            name,
            item,
            Arc::new(ModuleAudience::Package(nocter_model::PackageIdentity::new(
                "app",
            ))),
        )
        .unwrap();
    reservations.bind(exports, name, item).unwrap();
    assert!(
        reservations
            .finish()
            .unwrap_err()
            .iter()
            .any(|error| error.rule == DeclarationRule::InconsistentBindingAudience { name })
    );
}

#[test]
fn namespace_parameter_collisions_and_finite_cyclic_paths_share_the_closed_name_authority() {
    let symbols = SymbolTable::from_spellings(["run", "module", "Data"]);
    let name = |text| symbols.get(text).unwrap();
    let mut reservations = Reservations::new(symbols.clone());
    let linked = reservations.new_scope();
    let run = reservations.reserve(name("run"), 1);
    let data = reservations
        .reserve_record_in(linked, name("Data"), 0)
        .unwrap();
    reservations
        .bind_namespace(ScopeId::ROOT, name("module"), linked, None)
        .unwrap();
    reservations
        .bind_namespace(linked, name("module"), linked, None)
        .unwrap();
    let mut headers = reservations.finish().unwrap().into_publication();
    assert_eq!(
        headers
            .snapshot()
            .parameter_name_violations_in(ScopeId::ROOT, run, [name("module")])
            .unwrap()[0]
            .rule,
        DeclarationRule::ParameterShadowsNamespace {
            name: name("module")
        }
    );
    headers
        .define(
            run,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [ParameterDefinition {
                name: name("module"),
                data: BuiltinType::I32.into(),
                operation: ValueInputOperation::Read,
            }],
            BuiltinType::I32,
        )
        .unwrap();
    let super::super::HeaderFailure::Rejected(errors) = headers.publish(run).unwrap_err() else {
        panic!("parameter collision");
    };
    assert!(errors.iter().any(|error| error.rule
        == DeclarationRule::ParameterShadowsNamespace {
            name: name("module")
        }));
    let program = headers.snapshot();
    let mut path = vec![Some(name("module")); 10_000];
    path.push(Some(name("Data")));
    assert_eq!(
        program.declaration_path_in(ScopeId::ROOT, Namespace::Type, &path),
        Ok(data.into())
    );
    assert!(!program.header_ready(data));
}
