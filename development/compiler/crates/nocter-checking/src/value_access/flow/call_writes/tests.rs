use super::*;
use crate::value_access::accepted::SourceUnit;
use nocter_model::AccessCodeId;
use nocter_test_support::ValueSourceFixture;
use std::collections::BTreeSet;
mod substitution;

#[test]
fn primitive_write_evidence_needs_exact_checked_authority_and_survives_transitive_closure() {
    use nocter_compile_input::{ModuleIdentity, PrimitiveRoleLocator, ToolchainInput};
    use nocter_declaration_lowering::access_declarations::PrimitiveAuthority;
    use nocter_model::PackageIdentity;
    use nocter_runtime_contract::PrimitiveRole;
    let package = PackageIdentity::new("effects:standard");
    let module = ModuleIdentity::new(package.clone(), ["error"]);
    let toolchain = ToolchainInput::new(
        package.clone(),
        ModuleIdentity::new(package, ["prelude"]),
        Vec::new(),
        Vec::new(),
    )
    .with_primitive_roles(vec![PrimitiveRoleLocator::new(
        PrimitiveRole::NewError,
        module.clone(),
        "create_failure",
    )]);
    let authority = PrimitiveAuthority::for_module(&toolchain, &module).unwrap();
    let fixture = ValueSourceFixture::new(
        "primitive func create_failure(code: str, message: str): error\nfunc create(): error { create_failure(\"code\", \"message\") }\nfunc run(): error { create() }",
    );
    let source =
        SourceUnit::prepare_with_primitive_authority(fixture.syntax(), &authority).unwrap();
    let context = source.declarations().bind(fixture.syntax()).unwrap();
    let controls: Vec<_> = source
        .unit()
        .bodies()
        .map(|(_, query)| Control::prepare(Arc::new(query.typed().clone())).unwrap())
        .collect();
    let ordinary = Catalog::prepare(&controls).unwrap();
    let trusted = Catalog::prepare_with_context(&controls, &context).unwrap();
    for name in ["create_failure", "create", "run"] {
        let request = Request::unbound(code(&controls, name).into());
        assert_eq!(ordinary.summaries[&request], Writes::Unknown, "{name}");
        assert_eq!(
            trusted.summaries[&request],
            Writes::Known(BTreeSet::new()),
            "{name}"
        );
    }
    let caller = controls
        .iter()
        .find(|control| control.typed.code() == ValueCode::Declaration(code(&controls, "create")))
        .unwrap();
    // A direct primitive dependency has no body guard to hide a missing role-authority check.
    let calls = trusted.calls(caller).unwrap();
    assert!(calls.verify(&context).is_ok());
    let other = SourceUnit::prepare_with_primitive_authority(fixture.syntax(), &authority).unwrap();
    let other = other.declarations().bind(fixture.syntax()).unwrap();
    assert!(matches!(
        Catalog::prepare_with_context(&controls, &other),
        Err(IntegrityError::InconsistentTypedBody)
    ));
    assert!(matches!(
        calls.verify(&other),
        Err(crate::value_access::Failure::ChangedAuthority)
    ));
}

fn controls(source: &str) -> Vec<Control> {
    let fixture = ValueSourceFixture::new(source);
    let unit = SourceUnit::prepare(fixture.syntax()).unwrap().into_unit();
    unit.bodies()
        .map(|(_, query)| Control::prepare(Arc::new(query.typed().clone())).unwrap())
        .collect()
}
fn code(controls: &[Control], name: &str) -> AccessCodeId {
    let program = controls[0].typed.program();
    program
        .resolve(program.symbols().get(name).unwrap())
        .unwrap()
}

#[test]
fn fallible_cleanup_closes_both_variants_without_certifying_an_unbound_environment() {
    let controls = controls(
        "func maybe(): i32! { 42 }\nfunc wrap(take callback: func(): void): i32! { 42 }\nfunc idle(): void {}\nfunc run(): void { let first = wrap(idle)\nlet second = maybe() }",
    );
    let catalog = Catalog::prepare(&controls).unwrap();
    for name in ["maybe", "run"] {
        assert_eq!(
            catalog.summaries[&Request::unbound(code(&controls, name).into())],
            Writes::Known(BTreeSet::new())
        );
    }
    let wrap = ValueCode::Declaration(code(&controls, "wrap"));
    assert_eq!(catalog.summaries[&Request::unbound(wrap)], Writes::Unknown);
    let bound: Vec<_> = catalog
        .summaries
        .iter()
        .filter(|(request, _)| request.code == wrap && !request.bindings.is_empty())
        .collect();
    assert_eq!(bound.len(), 1);
    assert_eq!(bound[0].1, &Writes::Known(BTreeSet::new()));
}

#[test]
fn captured_code_summaries_separate_retained_environments_and_explicit_inputs() {
    let controls = controls(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/closure-write-prefix.nct"
    )));
    let catalog = Catalog::prepare(&controls).unwrap();
    for (name, expected) in [
        ("mutating", BTreeSet::from([bindings::Input::Environment])),
        ("nested", BTreeSet::from([bindings::Input::Environment])),
        (
            "explicit_inputs",
            BTreeSet::from([bindings::Input::Explicit(1)]),
        ),
        (
            "copied_once",
            BTreeSet::from([bindings::Input::Environment]),
        ),
        ("consumed", BTreeSet::new()),
    ] {
        let root = code(&controls, name);
        let mut found = false;
        for (request, summary) in &catalog.summaries {
            if request.bindings.is_empty()
                && matches!(request.code, ValueCode::Closure(id) if id.owner.source_owner() == root)
            {
                found = true;
                assert_eq!(summary, &Writes::Known(expected.clone()), "{name}");
            }
        }
        assert!(found, "{name}");
    }
    for (name, changes_existing_storage) in [
        ("mutating", true),
        ("copied_once", false),
        ("consumed", false),
    ] {
        let control = controls
            .iter()
            .find(|control| control.typed.code() == ValueCode::Declaration(code(&controls, name)))
            .unwrap();
        let calls = catalog.calls(control).unwrap();
        assert_eq!(calls.writes.len(), 1, "{name}");
        assert_eq!(
            calls.writes.values().any(|writes| !writes.is_empty()),
            changes_existing_storage,
            "{name}"
        );
    }
}

#[test]
fn captured_calls_keep_transitive_body_and_value_supplier_reuse_guards() {
    let text = "const WRITE: bool = false\nfunc helper(edit value: usize): void { if WRITE { value = 0 } }\nfunc run(): usize { let unit = ()\nlet callback = (unit; edit value: usize): void { helper(value) }\nvar value: usize = 7\ncallback(value)\nvalue }\nfunc unrelated(): i32 { 1 }";
    let fixture = ValueSourceFixture::new(text);
    let source = SourceUnit::prepare(fixture.syntax()).unwrap();
    let root = source
        .unit()
        .program()
        .resolve(source.unit().program().symbols().get("run").unwrap())
        .unwrap();
    let query = source
        .unit()
        .bodies()
        .find(|(id, _)| *id == root)
        .unwrap()
        .1;
    for (before, after, valid) in [
        ("if WRITE { value = 0 }", "value = 0", false),
        ("bool = false", "bool = true", false),
        ("unrelated(): i32 { 1 }", "unrelated(): i32 { 2 }", true),
    ] {
        let changed = ValueSourceFixture::new(&text.replace(before, after));
        let context = source.declarations().bind(changed.syntax()).unwrap();
        assert!(query.typed().bind(&context).is_ok());
        assert_eq!(query.bind(&context).is_ok(), valid, "{before}");
    }
}

#[test]
fn closed_recursive_components_preserve_caller_storage_and_external_writes_propagate() {
    let controls = controls(
        "func first(value: usize): usize { if value == 0 { 0 } else { second(value - 1) } }\n\
         func second(value: usize): usize { first(value) }\n\
         func local(): usize { var value: usize = 1\nvalue += 1\nvalue }\n\
         func writes(edit value: usize): void { value += 1 }\n\
         func proxy(edit value: usize): void { writes(value) }\n\
         func recursive(edit value: usize, again: bool): void { if again { writes(value) } else { recursive(value, true) } }",
    );
    let catalog = Catalog::prepare(&controls).unwrap();
    for name in ["first", "second", "local"] {
        assert!(
            catalog
                .summaries
                .get(&Request::unbound(ValueCode::Declaration(code(
                    &controls, name
                ))))
                == Some(&Writes::Known(BTreeSet::new())),
            "{name}"
        );
    }
    for name in ["writes", "proxy", "recursive"] {
        assert!(
            catalog
                .summaries
                .get(&Request::unbound(ValueCode::Declaration(code(
                    &controls, name
                ))))
                == Some(&Writes::Known(BTreeSet::from([bindings::Input::Explicit(
                    0
                )]))),
            "{name}"
        );
    }
}

#[test]
fn formal_invocation_and_environment_cleanup_do_not_gain_closed_effects() {
    let controls = controls(
        "func invoke(action: func(): void): void { action() }\n\
         func forward(action: func(): void): void { invoke(action) }\n\
         func cleanup(take action: once(): void): void {}\n\
         func leaf(take values: [error; 2]): void {}",
    );
    let catalog = Catalog::prepare(&controls).unwrap();
    for name in ["invoke", "forward", "cleanup"] {
        assert!(
            catalog
                .summaries
                .get(&Request::unbound(ValueCode::Declaration(code(
                    &controls, name
                ))))
                == Some(&Writes::Unknown),
            "{name}"
        );
    }
    assert!(
        catalog
            .summaries
            .get(&Request::unbound(ValueCode::Declaration(code(
                &controls, "leaf"
            ))))
            == Some(&Writes::Known(BTreeSet::new()))
    );
}

#[test]
fn exact_owned_environment_cleanup_does_not_invoke_the_captured_body() {
    let controls = controls(
        "func discard(take input: error): void { let held = (input^;): error { input^ } }\n\
         func aggregate(take input: [error; 2]): void { let held = (input^;): void {} }\n\
         func dormant(): void { let value = 7\nlet action = (value;): void { loop {} } }",
    );
    let catalog = Catalog::prepare(&controls).unwrap();
    for name in ["discard", "aggregate", "dormant"] {
        assert_eq!(
            catalog
                .summaries
                .get(&Request::unbound(ValueCode::Declaration(code(
                    &controls, name
                )))),
            Some(&Writes::Known(BTreeSet::new())),
            "{name}"
        );
    }
}

#[test]
fn summaries_require_the_exact_typed_body_owner_and_the_available_closed_callees() {
    let controls = controls("func leaf(): usize { 7 }\nfunc caller(): usize { leaf() }");
    let caller = controls
        .iter()
        .find(|control| control.typed.code() == ValueCode::Declaration(code(&controls, "caller")))
        .unwrap();
    let catalog = Catalog::prepare(&controls).unwrap();
    assert_eq!(catalog.calls(caller).unwrap().writes.len(), 1);
    // An equal clone is a different source/typed owner, even with the same program and code key.
    let cloned = Control::prepare(Arc::new((*caller.typed).clone())).unwrap();
    assert!(catalog.calls(&cloned).is_err());
    let partial = Catalog::prepare([caller]).unwrap();
    assert!(partial.calls(caller).unwrap().writes.is_empty());
    let foreign = self::controls("func leaf(): usize { 9 }\nfunc caller(): usize { leaf() }");
    assert!(Catalog::prepare([caller, &foreign[0]]).is_err());
}

#[test]
fn recursive_input_permutations_and_lexical_aliases_keep_the_written_root() {
    let controls = controls(
        "func write(edit value: usize): void { value += 1 }\nfunc first(edit left: usize, edit right: usize, again: bool): void { if again { second(right, left, false) } else { write(right) } }\nfunc second(edit left: usize, edit right: usize, again: bool): void { if again { first(right, left, false) } else { write(left) } }\nfunc alias(edit value: usize): void { edit target = value\nwrite(target) }\nfunc reset(edit value: error, take next: error): void { value = next^ }",
    );
    let catalog = Catalog::prepare(&controls).unwrap();
    for (name, input) in [("first", 1), ("second", 0), ("reset", 0), ("alias", 0)] {
        assert_eq!(
            catalog
                .summaries
                .get(&Request::unbound(ValueCode::Declaration(code(
                    &controls, name
                )))),
            Some(&Writes::Known(BTreeSet::from([bindings::Input::Explicit(
                input
            )]))),
            "{name}"
        );
    }
}

#[test]
fn lexical_alias_chains_separate_owned_storage_from_explicit_input_positions() {
    let controls = controls(
        "func set(edit value: usize): void { value += 1 }\n\
         func local(): void { var value: usize = 0\nedit first = value\nedit second = first\nset(second) }\n\
         func external(left: usize, edit right: usize): void { edit first = right\nedit second = first\nset(second) }\n\
         func indexed(edit values: [usize; 2], index: usize): void { edit selected = values[index]\nselected += 1 }",
    );
    let catalog = Catalog::prepare(&controls).unwrap();
    for (name, writes) in [
        ("local", BTreeSet::new()),
        ("external", BTreeSet::from([bindings::Input::Explicit(1)])),
        ("indexed", BTreeSet::from([bindings::Input::Explicit(0)])),
    ] {
        assert_eq!(
            catalog
                .summaries
                .get(&Request::unbound(ValueCode::Declaration(code(
                    &controls, name
                )))),
            Some(&Writes::Known(writes)),
            "{name}"
        );
    }
}

#[test]
fn caller_alias_footprints_resolve_without_reselecting_an_evaluated_index() {
    let controls = controls(
        "func set(edit value: usize): void { value += 1 }\n\
         func caller(): void { var count: usize = 0\nedit first = count\nedit second = first\nset(second) }\n\
         func indexed(): void { var values: [usize; 2] = [0, 0]\nvar index: usize = 0\nedit selected = values[index]\nindex = 1\nset(selected) }",
    );
    let catalog = Catalog::prepare(&controls).unwrap();
    for name in ["caller", "indexed"] {
        let control = controls
            .iter()
            .find(|control| control.typed.code() == ValueCode::Declaration(code(&controls, name)))
            .unwrap();
        let calls = catalog.calls(control).unwrap();
        let target = calls.writes.values().next().unwrap();
        let [Referent::Place(place)] = target.as_ref() else {
            panic!("static conservative footprint")
        };
        let place = control.typed.body().places().get(*place).unwrap();
        let local = control.typed.body().locals().get(place.local).unwrap();
        assert_eq!(
            local.access().mode(),
            nocter_model::ValueAccessMode::Own,
            "{name}"
        );
        assert!(place.fields.fields().is_empty(), "{name}");
    }
}

#[test]
fn flow_binding_guards_transitive_effect_bodies_without_changing_typed_call_reuse() {
    let text = "func write(edit value: usize): void {}\nfunc proxy(edit value: usize): void { write(value) }\nfunc run(): usize { var value: usize = 0\nproxy(value)\nvalue }\nfunc other(): i32 { 7 }";
    let fixture = ValueSourceFixture::new(text);
    let source = SourceUnit::prepare(fixture.syntax()).unwrap();
    let code = source
        .unit()
        .program()
        .resolve(source.unit().program().symbols().get("run").unwrap())
        .unwrap();
    let query = source
        .unit()
        .bodies()
        .find(|(id, _)| *id == code)
        .unwrap()
        .1;
    for (before, after, valid) in [
        ("usize): void {}", "usize): void { value = 1 }", false),
        ("other(): i32 { 7 }", "other(): i32 { 8 }", true),
    ] {
        let changed = ValueSourceFixture::new(&text.replace(before, after));
        let context = source.declarations().bind(changed.syntax()).unwrap();
        assert!(query.typed().bind(&context).is_ok());
        assert_eq!(query.bind(&context).is_ok(), valid, "{before}");
    }
}

#[test]
fn flow_binding_guards_value_suppliers_that_change_a_callees_reached_writes() {
    let text = "const WRITE: bool = false\nfunc maybe(edit value: usize): void { if WRITE { value = 1 } }\nfunc run(): usize { var value: usize = 0\nmaybe(value)\nvalue }";
    let fixture = ValueSourceFixture::new(text);
    let source = SourceUnit::prepare(fixture.syntax()).unwrap();
    let program = source.unit().program();
    let code = program
        .resolve(program.symbols().get("run").unwrap())
        .unwrap();
    let query = source
        .unit()
        .bodies()
        .find(|(id, _)| *id == code)
        .unwrap()
        .1;
    let changed = ValueSourceFixture::new(&text.replace("bool = false", "bool = true"));
    let context = source.declarations().bind(changed.syntax()).unwrap();
    assert!(query.typed().bind(&context).is_ok());
    assert!(query.bind(&context).is_err());
}

#[test]
fn known_actual_callbacks_close_invocation_and_cleanup_but_not_unbound_templates() {
    let controls = controls(
        "func pure(): void {}\nfunc invoke(action: func(): void): void { action() }\nfunc discard(take action: once(): void): void {}\nfunc relay(action: func(): void, flag: bool): void { if flag { action() } else { relay(action, true) } }\nfunc run(): void { invoke(pure)\ndiscard(pure)\nrelay(pure, false) }",
    );
    let catalog = Catalog::prepare(&controls).unwrap();
    for name in ["invoke", "discard", "relay"] {
        let code = ValueCode::Declaration(code(&controls, name));
        assert_eq!(
            catalog.summaries.get(&Request::unbound(code)),
            Some(&Writes::Unknown)
        );
        let selected: Vec<_> = catalog
            .summaries
            .iter()
            .filter(|(key, _)| key.code == code && !key.bindings.is_empty())
            .collect();
        assert_eq!(selected.len(), 1, "{name}");
        assert_eq!(selected[0].1, &Writes::Known(BTreeSet::new()), "{name}");
    }
    let root = controls
        .iter()
        .find(|control| control.typed.code() == ValueCode::Declaration(code(&controls, "run")))
        .unwrap();
    let calls = catalog.calls(root).unwrap();
    assert_eq!(calls.writes.len(), 3);
    assert!(calls.writes.values().all(|writes| writes.is_empty()));
}

#[test]
fn actual_callback_edits_keep_their_explicit_caller_input_position() {
    let controls = controls(
        "func write(edit value: usize): void { value = 0 }\nfunc invoke(action: func(edit usize): void, edit value: usize): void { action(value) }\nfunc run(edit value: usize): void { invoke(write, value) }",
    );
    let catalog = Catalog::prepare(&controls).unwrap();
    let invoke = ValueCode::Declaration(code(&controls, "invoke"));
    let selected = catalog
        .summaries
        .iter()
        .find(|(key, _)| key.code == invoke && !key.bindings.is_empty())
        .unwrap();
    assert_eq!(
        selected.1,
        &Writes::Known(BTreeSet::from([bindings::Input::Explicit(1)]))
    );
    let root = controls
        .iter()
        .find(|control| control.typed.code() == ValueCode::Declaration(code(&controls, "run")))
        .unwrap();
    let calls = catalog.calls(root).unwrap();
    assert_eq!(calls.writes.len(), 1);
    assert_eq!(calls.writes.values().next().unwrap().len(), 1);
}

#[test]
fn bound_effect_source_guards_do_not_union_callbacks_selected_by_other_callers() {
    let text = "func first(): void {}\nfunc second(): void {}\nfunc invoke(action: func(): void): void { action() }\nfunc discard(take action: once(): void): void {}\nfunc use_first(): void { invoke(first) }\nfunc use_second(): void { invoke(second) }\nfunc discard_only(): void { discard(second) }";
    let fixture = ValueSourceFixture::new(text);
    let source = SourceUnit::prepare(fixture.syntax()).unwrap();
    let changed =
        ValueSourceFixture::new(&text.replace("second(): void {}", "second(): void { loop {} }"));
    let context = source.declarations().bind(changed.syntax()).unwrap();
    for (name, reusable) in [
        ("use_first", true),
        ("use_second", false),
        ("discard_only", true),
    ] {
        let id = source
            .unit()
            .program()
            .resolve(source.unit().program().symbols().get(name).unwrap())
            .unwrap();
        let query = source
            .unit()
            .bodies()
            .find(|(code, _)| *code == id)
            .unwrap()
            .1;
        assert!(query.typed().bind(&context).is_ok());
        assert_eq!(query.bind(&context).is_ok(), reusable, "{name}");
    }
}
