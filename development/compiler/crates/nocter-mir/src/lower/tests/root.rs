use nocter_model::{BuiltinType, TypeId};
use nocter_target_program::ProcessSuccessType;
use nocter_test_support::CompilerFixture;

use super::{executable_fixture, test_executable_fixture};
use crate::{
    MirCallTarget, MirConstant, MirOperationKind, MirRoot, MirTerminator, MirValueDefinition,
    lower_executable,
};

#[test]
fn materializes_all_six_process_result_contracts() {
    let cases = [
        (
            "func main(): void { return }\n",
            ProcessSuccessType::Void,
            false,
        ),
        (
            "func main(): i32 { return 7 }\n",
            ProcessSuccessType::I32,
            false,
        ),
        (
            "func main(): usize { return 7 }\n",
            ProcessSuccessType::Usize,
            false,
        ),
        (
            "func main(): void! { return }\n",
            ProcessSuccessType::Void,
            true,
        ),
        (
            "func main(): i32! { return 7 }\n",
            ProcessSuccessType::I32,
            true,
        ),
        (
            "func main(): usize! { return 7 }\n",
            ProcessSuccessType::Usize,
            true,
        ),
    ];

    for (source, success, fallible) in cases {
        let program = lower_executable(executable_fixture(&CompilerFixture::with_app(source)))
            .unwrap_or_else(|error| panic!("failed to lower {source:?}: {error:?}"));
        let MirRoot::Process(root) = program.root() else {
            panic!("process fixture produced test roots")
        };
        assert_eq!(root.result().success(), success);
        assert_eq!(root.result().is_fallible(), fallible);
        assert_eq!(direct_calls(root.body()), [root.entry()]);

        let reports = root
            .body()
            .operations()
            .iter()
            .filter(|(_, operation)| {
                matches!(operation.kind(), MirOperationKind::ReportError { .. })
            })
            .count();
        assert_eq!(reports, usize::from(fallible));
        assert!(
            root.body()
                .blocks()
                .iter()
                .all(|(_, block)| { !matches!(block.terminator(), MirTerminator::Return(_)) })
        );

        let exits = root
            .body()
            .blocks()
            .iter()
            .filter_map(|(_, block)| match block.terminator() {
                MirTerminator::Exit(status) => Some(*status),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(exits.len(), if fallible { 2 } else { 1 });
        assert!(
            exits.iter().any(|status| {
                status.is_some_and(|status| integer_constant(root.body(), status) == Some(1))
            }) == fallible
        );
        let success_status_type = exits.iter().find_map(|status| {
            let status = (*status)?;
            (integer_constant(root.body(), status) != Some(1))
                .then(|| root.body().values().get(status).unwrap().ty())
        });
        assert_eq!(success_status_type, expected_status_type(&program, success));
    }
}

#[test]
fn materializes_one_isolated_root_per_test_in_declaration_order() {
    let fixture = CompilerFixture::with_tests(
        "test first { return }\n\
         test second { return }\n",
    );
    let program = lower_executable(test_executable_fixture(&fixture)).unwrap();
    let MirRoot::Tests { cases, .. } = program.root() else {
        panic!("test fixture produced a process root")
    };
    assert_eq!(
        cases.iter().map(|case| case.name()).collect::<Vec<_>>(),
        ["first", "second"]
    );
    for case in cases {
        assert_eq!(direct_calls(case.body()), [case.item()]);
        assert_eq!(
            case.body()
                .operations()
                .iter()
                .filter(|(_, operation)| matches!(
                    operation.kind(),
                    MirOperationKind::ReportError { .. }
                ))
                .count(),
            1
        );
    }
}

#[test]
fn preserves_an_empty_test_target_without_inventing_a_runner() {
    let fixture = CompilerFixture::with_tests("func helper(): void { return }\n");
    let program = lower_executable(test_executable_fixture(&fixture)).unwrap();
    let MirRoot::Tests { cases, .. } = program.root() else {
        panic!("test fixture produced a process root")
    };

    assert!(cases.is_empty());
    assert!(program.functions().is_empty());
}

fn direct_calls(body: &crate::MirBody) -> Vec<nocter_model::ExecutableItemId> {
    body.operations()
        .iter()
        .filter_map(|(_, operation)| match operation.kind() {
            MirOperationKind::Call(call) => match call.target() {
                MirCallTarget::Direct(item) => Some(*item),
                MirCallTarget::StandardPrimitive { .. } | MirCallTarget::Structural(_) => None,
            },
            _ => None,
        })
        .collect()
}

fn integer_constant(body: &crate::MirBody, value: nocter_model::MirValueId) -> Option<i128> {
    let MirValueDefinition::Operation(operation) = body.values().get(value)?.definition() else {
        return None;
    };
    match body.operations().get(operation)?.kind() {
        MirOperationKind::Constant(MirConstant::Integer(value)) => Some(*value),
        _ => None,
    }
}

fn expected_status_type(
    program: &crate::MirProgram,
    success: ProcessSuccessType,
) -> Option<TypeId> {
    let builtin = match success {
        ProcessSuccessType::Void => return None,
        ProcessSuccessType::I32 => BuiltinType::I32,
        ProcessSuccessType::Usize => BuiltinType::Usize,
    };
    Some(program.types().builtin(builtin))
}
