use std::collections::HashSet;

use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::{BuiltinType, TypeKind};

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{CheckedControl, CheckedOperation, LoopKind, prepare_program_checking};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

#[test]
fn while_and_infinite_loop_construct_reserved_loop_identities() {
    let output = check(
        "func nested(condition: bool): void {\n    while condition {}\n    loop {\n        loop {\n            break\n        }\n        break\n    }\n    return\n}\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let break_targets = body
        .nodes()
        .iter()
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Control(CheckedControl::Break(loop_)) => Some(*loop_),
            _ => None,
        })
        .collect::<HashSet<_>>();

    assert_eq!(body.loops().len(), 3);
    assert_eq!(break_targets.len(), 2);
}

#[test]
fn nonbreaking_infinite_loop_is_a_never_terminator() {
    check("func spin(): never {\n    loop {}\n}\n").unwrap();
    check("func spin(): never {\n    loop {\n        continue\n        break\n    }\n}\n").unwrap();
}

#[test]
fn break_and_continue_require_a_loop_in_the_current_body() {
    for control in ["break", "continue"] {
        let error = check(&format!("func invalid(): void {{\n    {control}\n}}\n")).unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0382");
    }
}

#[test]
fn loop_body_and_while_condition_keep_their_value_contracts() {
    for source in [
        "func invalid(): void {\n    while 1 {}\n    return\n}\n",
        "func invalid(): void {\n    loop {\n        1\n    }\n}\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0370");
    }
}

#[test]
fn repeated_loop_move_is_rejected_at_the_fixed_point() {
    for body in ["let _ = move value", "let _ = move value\n        continue"] {
        let error = check(&format!(
            "struct Owned {{\n    value: i32\n}}\n\
             func invalid(condition: bool, value: Owned): void {{\n    while condition {{\n        {body}\n    }}\n    return\n}}\n"
        ))
        .unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
    }
}

#[test]
fn loop_exit_joins_zero_iteration_and_break_states() {
    check(
        "struct Owned {\n    value: i32\n}\n\
         func maybe_consume(condition: bool, value: Owned): void {\n    while condition {\n        let _ = move value\n        break\n    }\n    return\n}\n",
    )
    .unwrap();

    let error = check(
        "struct Owned {\n    value: i32\n}\n\
         func invalid(value: Owned): Owned {\n    loop {\n        let _ = move value\n        break\n    }\n    move value\n}\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn integer_range_constructs_one_typed_loop_binding() {
    let output = check(
        "func visit(limit: i32): void {\n    for index in 0..<limit {\n        let _ = index\n    }\n    return\n}\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let (_, loop_) = body.loops().iter().next().unwrap();
    let LoopKind::Range { binding, .. } = loop_.kind() else {
        panic!("expected a checked integer range");
    };

    assert_eq!(
        output
            .program()
            .types()
            .get(body.locals().get(*binding).unwrap().ty()),
        Some(&TypeKind::Builtin(BuiltinType::I32))
    );
}

#[test]
fn integer_range_requires_matching_integer_endpoints() {
    for source in [
        "func invalid(): void {\n    for index in true..<1 {}\n    return\n}\n",
        "func invalid(): void {\n    for index in 0..<true {}\n    return\n}\n",
    ] {
        let error = check(source).unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0370");
    }
}

#[test]
fn range_backedge_rejects_a_repeated_move() {
    let error = check(
        "struct Owned {\n    value: i32\n}\n\
         func invalid(limit: i32, value: Owned): void {\n    for index in 0..<limit {\n        let _ = index\n        let _ = move value\n    }\n    return\n}\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn range_exit_includes_zero_iterations_and_breaks() {
    let error = check(
        "struct Owned {\n    value: i32\n}\n\
         func invalid(limit: i32, value: Owned): Owned {\n    for index in 0..<limit {\n        let _ = index\n        let _ = move value\n        break\n    }\n    move value\n}\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}
