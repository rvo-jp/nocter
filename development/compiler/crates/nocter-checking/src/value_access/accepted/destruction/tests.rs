use super::super::{Failure, ModuleSourceUnit, SourceFailure};
use nocter_compile_input::{
    ModuleIdentity,
    ModuleSourceKind::{Implementation, Root},
    PrimitiveRoleLocator, ToolchainInput,
};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::CodeKind;
use nocter_model::PackageIdentity;
use nocter_runtime_contract::PrimitiveRole;
use nocter_test_support::ValueModuleFixture;

#[test]
fn destructor_checks_a_reached_callees_authorized_wait() {
    let package = PackageIdentity::new("workspace:app");
    let module = ModuleIdentity::new(package.clone(), [] as [&str; 0]);
    let toolchain = ToolchainInput::new(package, module.clone(), Vec::new(), Vec::new())
        .with_primitive_roles(vec![PrimitiveRoleLocator::new(
            PrimitiveRole::StandardOutputWrite,
            module,
            "output",
        )]);
    for (action, waits) in [("relay()", true), ("if false { relay() }", false)] {
        let body = format!(
            "see ./index.nct\n\
             struct Handle {{ value: i32 }}\n\
             impl Handle {{ drop {{ {action} }} }}\n\
             func relay(): void {{ let _ = output(\"from relay\") }}"
        );
        let fixture = ValueModuleFixture::new(
            &[
                (
                    "/app/index.nct",
                    Root,
                    "see ./body.nct\nstruct Handle\nprimitive func output(text: str): bool",
                ),
                ("/app/body.nct", Implementation, &body),
            ],
            &[(0, 0, 1), (1, 0, 0)],
        );
        let input = fixture.input().with_toolchain(toolchain.clone());
        let result = ModuleSourceUnit::prepare(&input);
        if !waits {
            assert_eq!(result.unwrap().unit().destructor_completions().len(), 1);
            continue;
        }
        let Err(SourceFailure::Checking(Failure::Flow { diagnostics, .. })) = result else {
            panic!("expected a source diagnostic for the reached destructor lifecycle: {result:?}");
        };
        let [error] = diagnostics.as_ref() else {
            panic!("one failed destructor obligation");
        };
        assert!(
            error.message().contains("may wait for external progress"),
            "{error:?}"
        );
        assert_eq!(error.primary().source(), fixture.tree(1).source());
        assert_eq!(
            error.primary().span().range().start().get() as usize,
            body.find("output(").unwrap()
        );
        assert!(
            error
                .notes()
                .iter()
                .any(|note| note.message().contains("this destructor"))
        );
    }
}

#[test]
fn destructor_effects_follow_the_receiver_callback_not_the_nominal_schema() {
    let package = PackageIdentity::new("workspace:app");
    let module = ModuleIdentity::new(package.clone(), [] as [&str; 0]);
    let toolchain = ToolchainInput::new(package, module.clone(), Vec::new(), Vec::new())
        .with_primitive_roles(vec![PrimitiveRoleLocator::new(
            PrimitiveRole::StandardOutputWrite,
            module,
            "output",
        )]);
    let body = "see ./index.nct\n\
                struct Handler { callback: func(): void }\n\
                impl Handler { drop { self.callback() } }\n\
                func safe(): void {}\n\
                func waiting(): void { let _ = output(\"from callback\") }";
    let fixture = ValueModuleFixture::new(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./body.nct\nstruct Handler\nprimitive func output(text: str): bool",
            ),
            ("/app/body.nct", Implementation, body),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let input = fixture.input().with_toolchain(toolchain);
    let prepared = ModuleSourceUnit::prepare(&input).unwrap();
    let unit = prepared.unit();
    let program = unit.program();
    assert!(unit.destructor_completions().is_empty());
    let owner = program
        .codes()
        .iter()
        .find_map(|(_, code)| match code.kind() {
            CodeKind::Destructor(owner) => Some(owner),
            _ => None,
        })
        .unwrap();
    let data = program.nominal_type_identity(owner).unwrap();
    let receiver = |name| {
        let symbol = program.symbols().get(name).unwrap();
        let (code, _) = program
            .codes()
            .iter()
            .find(|(_, code)| code.name() == Some(symbol))
            .unwrap();
        let witness = program.concrete_witness(code.into()).unwrap();
        program
            .instantiate_callable_domains(data, |_, _| Ok(Some(witness.clone())))
            .unwrap()
    };
    let mut work = WorkBudget::new(std::num::NonZeroU64::new(10_000).unwrap());
    let safe = receiver("safe");
    let frame = program.bind_destructor(&safe).unwrap();
    assert_eq!(
        unit.qualify_destructor(safe.clone(), &mut work).unwrap(),
        [(frame, safe)].into()
    );
    let error = unit
        .qualify_destructor(receiver("waiting"), &mut work)
        .unwrap_err();
    let unrelated = ModuleSourceUnit::prepare(&input).unwrap();
    assert!(matches!(
        error.bind(
            unrelated.unit(),
            &unrelated.declarations().current().unwrap()
        ),
        Failure::Integrity(_)
    ));
    let current = prepared.declarations().current().unwrap();
    let Failure::Flow { diagnostics, .. } = error.bind(unit, &current) else {
        panic!("contextual destruction rejection must bind to its checked source: {error:?}");
    };
    let [error] = diagnostics.as_ref() else {
        panic!("one failed destructor obligation");
    };
    assert!(
        error.message().contains("may wait for external progress"),
        "{error:?}"
    );
    assert_eq!(error.primary().source(), fixture.tree(1).source());
    assert_eq!(
        error.primary().span().range().start().get() as usize,
        body.find("output(").unwrap(),
    );
}

#[test]
fn a_destructor_input_preserves_its_selected_domain_even_when_the_declared_type_closes() {
    use nocter_declarations::value_access::PayloadSchema;
    use nocter_model::{BuiltinType, ValueField};

    let fixture = ValueModuleFixture::new(
        &[
            ("/app/index.nct", Root, "see ./body.nct\nstruct Slot"),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nstruct Slot { value: i32?\ncount: i32 }\nimpl Slot { drop { self.count += 1 } }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let unit = ModuleSourceUnit::prepare(&fixture.input())
        .unwrap()
        .into_unit();
    let program = unit.program();
    let data = program.type_named("Slot").unwrap();
    let closed = program.instantiate_closed_data(data).unwrap();
    let Some(PayloadSchema::Aggregate(schema)) = program.data_schema(data) else {
        panic!("the receiver has a record layout");
    };
    let mut components = schema.components();
    let field = components.next().unwrap();
    let count = components.next().unwrap();
    let integer = program.data_type(BuiltinType::I32).unwrap();
    let absent = program
        .instantiate_components(
            field.data,
            [(
                ValueField::Variant(true),
                program.instantiate_uninhabited_data(integer).unwrap(),
            )],
        )
        .unwrap();
    let receiver = program
        .instantiate_components(
            data,
            [
                (field.field, absent),
                (
                    count.field,
                    program.instantiate_closed_data(count.data).unwrap(),
                ),
            ],
        )
        .unwrap();
    assert_ne!(receiver, closed);

    let frame = program.bind_destructor(&receiver).unwrap();
    let local = unit
        .code_body(frame.code())
        .unwrap()
        .typed()
        .body()
        .inputs()
        .explicit()[0]
        .local;
    let mut work = WorkBudget::new(std::num::NonZeroU64::new(1_000).unwrap());
    assert_eq!(
        unit.instantiate_input_data(&frame, local, &mut work)
            .unwrap(),
        receiver
    );
    assert_eq!(
        unit.qualify_destructor(receiver.clone(), &mut work)
            .unwrap(),
        [(frame, receiver)].into()
    );
}

#[test]
fn destructor_receiver_cannot_be_replaced_through_a_delegated_edit() {
    for (action, accepted) in [
        ("self = Handle { value: 0 }", false),
        ("edit other = self\nrelay(other)", false),
        ("set_field(self)", true),
    ] {
        let body = format!(
            "see ./index.nct\n\
             struct Handle {{ value: i32 }}\n\
             impl Handle {{ drop {{ {action} }} }}\n\
             func relay(edit target: Handle): void {{ reset(target) }}\n\
             func reset(edit target: Handle): void {{ target = Handle {{ value: 0 }} }}\n\
             func set_field(edit target: Handle): void {{ target.value = 1 }}"
        );
        let fixture = ValueModuleFixture::new(
            &[
                ("/app/index.nct", Root, "see ./body.nct\nstruct Handle"),
                ("/app/body.nct", Implementation, &body),
            ],
            &[(0, 0, 1), (1, 0, 0)],
        );
        let input = fixture.input();
        let result = ModuleSourceUnit::prepare(&input);
        if accepted {
            assert_eq!(result.unwrap().unit().destructor_completions().len(), 1);
            continue;
        }
        let Err(SourceFailure::Checking(Failure::Flow { diagnostics, .. })) = result else {
            panic!("a whole-receiver replacement would destroy self recursively: {result:?}");
        };
        assert!(
            diagnostics
                .iter()
                .any(|error| error.message().contains("whose destructor is running"))
        );
    }
}
