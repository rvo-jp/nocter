use super::super::{Failure, ModuleSourceUnit, SourceFailure};
use nocter_compile_input::{
    ModuleIdentity,
    ModuleSourceKind::{Implementation, Root},
    PrimitiveRoleLocator, ToolchainInput,
};
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
            assert_eq!(result.unwrap().unit().destructor_frames().count(), 1);
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
            assert_eq!(result.unwrap().unit().destructor_frames().count(), 1);
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
