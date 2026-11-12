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
