use nocter_declaration_lowering::lower_compile_unit_declarations;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    CheckedControl, CheckedOperation, CleanupCondition, CleanupTarget, CleanupTiming, PlaceRoot,
    prepare_program_checking,
};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

#[test]
fn borrowed_drop_receiver_is_initialized_but_not_owned_by_the_body() {
    let output = check(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();

    assert!(
        body.nodes()
            .iter()
            .all(|(node, _)| body.cleanups().schedules(node).unwrap().is_empty())
    );
}

#[test]
fn explicit_drop_uses_one_path_cleanup_and_consumes_the_binding() {
    let output = check(
        "struct Owned {\n    value: i32\n}\n\
         func destroy(value: Owned): void {\n    drop value\n    return\n}\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let (drop_, _) = body
        .nodes()
        .iter()
        .find(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Control(CheckedControl::Drop(_))
            )
        })
        .unwrap();
    let [action] = body
        .cleanups()
        .actions(drop_, CleanupTiming::BeforeTransfer)
        .unwrap()
    else {
        panic!("explicit drop must own exactly one cleanup action");
    };

    assert_eq!(action.condition(), CleanupCondition::Always);
    assert!(matches!(
        action.target(),
        CleanupTarget::Path(path) if matches!(path.root(), PlaceRoot::Parameter(_)) && path.fields().is_empty()
    ));
    assert!(
        body.cleanups()
            .actions(body.root(), CleanupTiming::BeforeTransfer)
            .is_none()
    );
}

#[test]
fn later_use_and_second_drop_observe_the_uninitialized_state() {
    for tail in ["let _ = move value", "drop value"] {
        let error = check(&format!(
            "struct Owned {{\n    value: i32\n}}\n\
             func invalid(value: Owned): void {{\n    drop value\n    {tail}\n    return\n}}\n"
        ))
        .unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
    }
}

#[test]
fn maybe_initialized_binding_cannot_be_explicitly_dropped() {
    let error = check(
        "struct Owned {\n    value: i32\n}\n\
         func invalid(condition: bool, value: Owned): void {\n    if condition {\n        let _ = move value\n    }\n    drop value\n    return\n}\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn copy_and_borrow_bindings_are_structurally_invalid_drop_targets() {
    for source in [
        "func invalid(value: i32): void {\n    drop value\n    return\n}\n",
        "func invalid(value: &i32): void {\n    drop value\n    return\n}\n",
        "func invalid(value: &+i32): void {\n    drop value\n    return\n}\n",
        "func invalid(value: i32): void {\n    return\n    drop value\n}\n",
    ] {
        let error = check(source).unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0383");
    }
}

#[test]
fn unreachable_valid_drop_has_no_executable_cleanup_edge() {
    let output = check(
        "struct Owned {\n    value: i32\n}\n\
         func finish(value: Owned): void {\n    return\n    drop value\n}\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let (drop_, _) = body
        .nodes()
        .iter()
        .find(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Control(CheckedControl::Drop(_))
            )
        })
        .unwrap();

    assert!(
        body.cleanups()
            .actions(drop_, CleanupTiming::BeforeTransfer)
            .is_none()
    );
}
