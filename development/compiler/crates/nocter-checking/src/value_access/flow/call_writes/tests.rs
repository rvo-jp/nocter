use super::*;
use crate::value_access::accepted::SourceUnit;
use nocter_test_support::ValueSourceFixture;

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
        assert!(catalog.no_writes.contains(&code(&controls, name)), "{name}");
    }
    for name in ["writes", "proxy", "recursive"] {
        assert!(
            !catalog.no_writes.contains(&code(&controls, name)),
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
            !catalog.no_writes.contains(&code(&controls, name)),
            "{name}"
        );
    }
    assert!(catalog.no_writes.contains(&code(&controls, "leaf")));
}

#[test]
fn summaries_require_the_exact_typed_body_owner_and_the_available_closed_callees() {
    let controls = controls("func leaf(): usize { 7 }\nfunc caller(): usize { leaf() }");
    let caller = controls
        .iter()
        .find(|control| control.typed.code() == ValueCode::Declaration(code(&controls, "caller")))
        .unwrap();
    let catalog = Catalog::prepare(&controls).unwrap();
    assert_eq!(catalog.calls(&caller.typed).unwrap().len(), 1);
    // An equal clone is a different source/typed owner, even with the same program and code key.
    assert!(catalog.calls(&Arc::new((*caller.typed).clone())).is_err());
    let partial = Catalog::prepare([caller]).unwrap();
    assert!(partial.calls(&caller.typed).unwrap().is_empty());
    let foreign = self::controls("func leaf(): usize { 9 }\nfunc caller(): usize { leaf() }");
    assert!(Catalog::prepare([caller, &foreign[0]]).is_err());
}
