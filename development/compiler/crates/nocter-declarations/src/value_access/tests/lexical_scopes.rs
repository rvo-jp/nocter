use super::*;
use nocter_model::{ModuleAudience, ModuleIdentity, PackageIdentity};
use std::sync::Arc;

fn module(path: &[&str]) -> ModuleIdentity {
    ModuleIdentity::new(PackageIdentity::new("workspace:app"), path.iter().copied())
}

#[test]
fn lexical_ancestry_keeps_siblings_independent_and_preserves_nearest_typed_identity() {
    let symbols = SymbolTable::from_spellings(["item", "local", "api", "Data", "run"]);
    let name = |text| symbols.get(text).unwrap();
    let mut reservations = Reservations::new(symbols.clone());
    assert_eq!(
        reservations.new_lexical_scope(ScopeId::new(99)),
        Err(ScopeBindingError::UnknownScope)
    );
    let child = reservations.new_lexical_scope(ScopeId::ROOT).unwrap();
    assert_eq!(child, ScopeId::new(1));
    let nested = reservations.new_lexical_scope(child).unwrap();
    let sibling = reservations.new_lexical_scope(ScopeId::ROOT).unwrap();
    let independent = reservations.new_scope();
    let exports = reservations.new_scope();
    let run = reservations.reserve_in(nested, name("run"), 1).unwrap();
    let root = reservations.reserve(name("item"), 0);
    let local = reservations.reserve_in(child, name("local"), 0).unwrap();
    let nearest = reservations.reserve_in(child, name("item"), 0).unwrap();
    let data = reservations.reserve_record(name("item"), 0);
    let exported = reservations
        .reserve_record_in(exports, name("Data"), 0)
        .unwrap();
    reservations
        .bind_namespace(ScopeId::ROOT, name("api"), exports, None)
        .unwrap();
    let program = reservations.finish().unwrap().into_publication().snapshot();
    assert_eq!(program.resolve_code_in(nested, name("item")), Some(nearest));
    assert_eq!(program.resolve_code_in(sibling, name("item")), Some(root));
    assert_eq!(program.resolve_record_in(nested, name("item")), Some(data));
    assert_eq!(program.resolve_code_in(nested, name("local")), Some(local));
    for scope in [sibling, independent, ScopeId::ROOT] {
        assert_eq!(program.resolve_code_in(scope, name("local")), None);
    }
    assert_eq!(program.resolve_code_in(independent, name("item")), None);
    assert!(!program.has_namespace_in(independent, name("api")));
    assert!(program.has_namespace_in(nested, name("api")));
    assert_eq!(
        program.declaration_path_in(
            nested,
            Namespace::Type,
            &[Some(name("api")), Some(name("Data"))]
        ),
        Ok(exported.into())
    );
    for (parameter, rule) in [
        (
            name("api"),
            DeclarationRule::ParameterShadowsNamespace { name: name("api") },
        ),
        (
            name("local"),
            DeclarationRule::ParameterShadowsValue { declaration: local },
        ),
    ] {
        assert_eq!(
            program
                .parameter_name_violations_in(nested, run, [parameter])
                .unwrap()[0]
                .rule,
            rule
        );
    }
    assert!(!program.header_ready(exported));
    assert!(!program.header_ready(nearest));
}

#[test]
fn lexical_caller_is_inherited_but_namespace_members_never_search_lexical_ancestors() {
    let symbols = SymbolTable::from_spellings(["api", "public", "restricted", "private"]);
    let name = |text| symbols.get(text).unwrap();
    let mut reservations = Reservations::new(symbols.clone());
    let privileged = reservations.new_scope();
    let exports = reservations.new_lexical_scope(privileged).unwrap();
    let client = reservations.new_lexical_scope(ScopeId::ROOT).unwrap();
    let friend = reservations.new_lexical_scope(privileged).unwrap();
    reservations
        .set_scope_module(ScopeId::ROOT, module(&["client"]))
        .unwrap();
    reservations
        .set_scope_module(privileged, module(&["secret"]))
        .unwrap();
    assert_eq!(
        reservations.set_scope_module(client, module(&["secret"])),
        Err(ScopeBindingError::ConflictingModule)
    );
    reservations
        .set_scope_module(client, module(&["client"]))
        .unwrap();
    let public = reservations.reserve_in(exports, name("public"), 0).unwrap();
    let restricted = reservations
        .reserve_in(privileged, name("restricted"), 0)
        .unwrap();
    reservations
        .reserve_in(privileged, name("private"), 0)
        .unwrap();
    reservations
        .bind_export(
            exports,
            name("restricted"),
            restricted,
            Arc::new(ModuleAudience::Descendants(module(&["secret"]))),
        )
        .unwrap();
    for scope in [ScopeId::ROOT, privileged] {
        reservations
            .bind_namespace(scope, name("api"), exports, None)
            .unwrap();
    }
    let program = reservations.finish().unwrap().into_publication().snapshot();
    let path = |leaf| [Some(name("api")), Some(name(leaf))];
    assert_eq!(
        program.declaration_path_in(client, Namespace::Value, &path("public")),
        Ok(public.into())
    );
    assert_eq!(
        program.declaration_path_in(client, Namespace::Value, &path("restricted")),
        Err(QualifiedNameError::Inaccessible { position: 1 })
    );
    assert_eq!(
        program.declaration_path_in(friend, Namespace::Value, &path("restricted")),
        Ok(restricted.into())
    );
    assert_eq!(
        program.declaration_path_in(friend, Namespace::Value, &path("private")),
        Err(QualifiedNameError::UnknownDeclaration { position: 1 })
    );
    assert_eq!(
        program.declaration_visible_from(
            exports,
            Namespace::Value,
            name("restricted"),
            &module(&["client"])
        ),
        None
    );
}

#[test]
fn deep_lexical_ancestry_is_iterative_and_can_only_be_extended_by_new_children() {
    let symbols = SymbolTable::from_spellings(["item"]);
    let item = symbols.get("item").unwrap();
    let mut reservations = Reservations::new(symbols);
    let value = reservations.reserve(item, 0);
    let mut deepest = ScopeId::ROOT;
    for _ in 0..10_000 {
        deepest = reservations.new_lexical_scope(deepest).unwrap();
    }
    let program = reservations.finish().unwrap().into_publication().snapshot();
    assert_eq!(program.resolve_code_in(deepest, item), Some(value));
    assert_eq!(
        program.declaration_prefix_in(deepest, Namespace::Value, &[Some(item), None]),
        Ok((value.into(), 1))
    );
}
