use nocter_declaration_lowering::lower_compile_unit_declarations;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    CheckedControl, CheckedOperation, CleanupCondition, CleanupTarget, PlaceProjection, PlaceRoot,
    prepare_program_checking,
};

fn check(source: &str) -> crate::CheckedProgramOutput {
    let fixture = Fixture::new(source);
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    check_prepared_program(&input, prepared).unwrap()
}

#[test]
fn return_cleanup_reverses_locals_then_owned_parameters() {
    let output = check(
        "struct Owned {\n    value: i32\n}\n\
         func cleanup(first: Owned, second: Owned): void {\n    let local = move first\n    return\n}\n",
    );
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let (return_, _) = body
        .nodes()
        .iter()
        .find(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Control(CheckedControl::Return(_))
            )
        })
        .unwrap();
    let roots = body
        .cleanups()
        .actions(return_)
        .unwrap()
        .iter()
        .map(|action| match action.target() {
            CleanupTarget::Path(path) => path.root(),
            CleanupTarget::Place { .. } | CleanupTarget::Value { .. } => {
                panic!("scope cleanup must target an owned path")
            }
        })
        .collect::<Vec<_>>();

    assert!(matches!(roots[0], PlaceRoot::Local(_)));
    assert!(matches!(roots[1], PlaceRoot::Parameter(_)));
    assert_eq!(roots.len(), 2);
}

#[test]
fn returned_move_is_not_cleaned_in_the_callee() {
    let output = check(
        "struct Owned {\n    value: i32\n}\n\
         func pass(value: Owned): Owned {\n    return move value\n}\n",
    );
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let (return_, _) = body
        .nodes()
        .iter()
        .find(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Control(CheckedControl::Return(_))
            )
        })
        .unwrap();

    assert!(body.cleanups().actions(return_).unwrap().is_empty());
}

#[test]
fn normal_callable_fallthrough_cleans_owned_parameters() {
    let output = check(
        "struct Owned {\n    value: i32\n}\n\
         func finish(value: Owned): void {}\n",
    );
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let actions = body.cleanups().actions(body.root()).unwrap();

    assert_eq!(actions.len(), 1);
    assert!(matches!(
        actions[0].target(),
        CleanupTarget::Path(path) if matches!(path.root(), PlaceRoot::Parameter(_))
    ));
}

#[test]
fn partial_move_cleans_the_value_then_only_the_remaining_field() {
    let output = check(
        "struct Owned {\n    value: i32\n}\n\
         struct Pair {\n    first: Owned\n    second: Owned\n}\n\
         func partial(pair: Pair): void {\n    let _ = move pair.first\n    return\n}\n",
    );
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let (discard, moved_field) = body
        .nodes()
        .iter()
        .find_map(|(node, checked)| {
            let CheckedOperation::Control(CheckedControl::Discard(value)) = checked.operation()
            else {
                return None;
            };
            let CheckedOperation::Move(place) = body.nodes().get(*value)?.operation() else {
                return None;
            };
            let PlaceProjection::Field(field) = body.places().get(*place)?.projections()[0] else {
                return None;
            };
            Some((node, field))
        })
        .unwrap();
    assert!(matches!(
        body.cleanups().actions(discard).unwrap()[0].target(),
        CleanupTarget::Value { .. }
    ));

    let (return_, _) = body
        .nodes()
        .iter()
        .find(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Control(CheckedControl::Return(_))
            )
        })
        .unwrap();
    let [remaining] = body.cleanups().actions(return_).unwrap() else {
        panic!("expected exactly one remaining field cleanup");
    };
    let CleanupTarget::Path(path) = remaining.target() else {
        panic!("remaining field cleanup must target storage");
    };

    assert_eq!(path.fields().len(), 1);
    assert_ne!(path.fields()[0], moved_field);
    assert_eq!(remaining.condition(), CleanupCondition::Always);
}

#[test]
fn branch_move_produces_conditional_return_cleanup() {
    let output = check(
        "struct Owned {\n    value: i32\n}\n\
         func maybe(condition: bool, value: Owned): void {\n    if condition {\n        let _ = move value\n    }\n    return\n}\n",
    );
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let (return_, _) = body
        .nodes()
        .iter()
        .find(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Control(CheckedControl::Return(_))
            )
        })
        .unwrap();
    let actions = body.cleanups().actions(return_).unwrap();

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].condition(), CleanupCondition::IfInitialized);
}

#[test]
fn break_cleans_loop_scopes_before_joining_the_exit() {
    let output = check(
        "struct Owned {\n    value: i32\n}\n\
         func exit(condition: bool, value: Owned): void {\n    while condition {\n        let local = move value\n        break\n    }\n    return\n}\n",
    );
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let (break_, _) = body
        .nodes()
        .iter()
        .find(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Control(CheckedControl::Break(_))
            )
        })
        .unwrap();
    let [action] = body.cleanups().actions(break_).unwrap() else {
        panic!("break must clean its live loop local");
    };

    assert!(matches!(
        action.target(),
        CleanupTarget::Path(path) if matches!(path.root(), PlaceRoot::Local(_))
    ));
}

#[test]
fn copy_and_borrow_bindings_create_no_cleanup_action() {
    let output = check("func finish(number: i32, output: &+i32): void {\n    return\n}\n");
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let (return_, _) = body
        .nodes()
        .iter()
        .find(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Control(CheckedControl::Return(_))
            )
        })
        .unwrap();

    assert!(body.cleanups().actions(return_).unwrap().is_empty());
}
