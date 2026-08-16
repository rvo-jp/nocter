use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::TypeKind;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    CallTarget, CheckedOperation, CheckedOutcome, StaticDispatch, prepare_program_checking,
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
fn direct_static_call_freezes_dispatch_and_argument_order() {
    let output = check(
        "func add_one(value: i32): i32 {\n    value + 1\n}\nfunc apply(input: i32): i32 {\n    add_one(input)\n}\n",
    )
    .unwrap();
    let calls = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Call(call) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls.len(), 1);
    let CallTarget::Static(selection) = calls[0].target() else {
        panic!("direct function must have static dispatch")
    };
    assert!(matches!(selection.dispatch(), StaticDispatch::Direct(_)));
    assert!(selection.generic_arguments().as_slice().is_empty());
    assert_eq!(calls[0].arguments().len(), 1);
}

#[test]
fn generic_calls_infer_arguments_and_rank_result_contexts() {
    let output = check(
        "func identity<T>(value: T): T {\n    move value\n}\nfunc unreachable_value<T>(): T {\n    loop {}\n}\nfunc from_argument(): i32 {\n    identity(42)\n}\nfunc from_result(): i64 {\n    unreachable_value()\n}\nfunc exact_optional_result(): i32? {\n    unreachable_value()\n}\n",
    )
    .unwrap();
    let generic_arguments = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Call(call) => match call.target() {
                CallTarget::Static(selection) => {
                    Some(selection.generic_arguments().as_slice()[0].ty())
                }
                CallTarget::CallableValue { .. } => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(generic_arguments.len(), 3);
    assert!(matches!(
        output.program().types().get(generic_arguments[2]),
        Some(TypeKind::Optional(_))
    ));
}

#[test]
fn concrete_parameter_types_contextualize_literals_before_inference() {
    check(
        "func wide(value: u64): u64 {\n    value\n}\nfunc maximum(): u64 {\n    wide(18_446_744_073_709_551_615)\n}\n",
    )
    .unwrap();
}

#[test]
fn call_arguments_support_deferred_absence_and_result_injection() {
    let output = check(
        "func choose<T>(fallback: T, value: T?): T {\n    move fallback\n}\nfunc number(): i32 {\n    1\n}\nfunc choose_default(): i32 {\n    choose(1, none)\n}\nfunc maybe_number(): i32? {\n    number()\n}\n",
    )
    .unwrap();

    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.nodes().iter().any(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Outcome(CheckedOutcome::Inject { .. })
            )
        })
    }));
}

#[test]
fn static_call_requirements_use_the_common_recursive_proof_authority() {
    check(
        "func same<T>(left: &T, right: &T): bool where (&T == &T): bool {\n    left == right\n}\nfunc compare(left: i32, right: i32): bool {\n    same(&left, &right)\n}\n",
    )
    .unwrap();

    let error = check(
        "struct Value { field: i32 }\nfunc same<T>(left: &T, right: &T): bool where (&T == &T): bool {\n    left == right\n}\nfunc invalid(left: Value, right: Value): bool {\n    same(&left, &right)\n}\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0390");
}

#[test]
fn call_ownership_requires_explicit_moves_and_visits_arguments_left_to_right() {
    let implicit = check(
        "struct Owned { value: i32 }\nfunc consume(value: Owned): i32 {\n    value.value\n}\nfunc invalid(input: Owned): i32 {\n    consume(input)\n}\n",
    )
    .unwrap_err();
    assert_eq!(implicit.source_diagnostic().unwrap().code(), "E0371");

    let repeated = check(
        "struct Owned { value: i32 }\nfunc consume(first: Owned, second: Owned): void {\n    return\n}\nfunc invalid(input: Owned): void {\n    consume(move input, move input)\n    return\n}\n",
    )
    .unwrap_err();
    assert_eq!(repeated.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn static_call_arity_is_rejected_at_the_call_boundary() {
    let error =
        check("func one(value: i32): i32 {\n    value\n}\nfunc invalid(): i32 {\n    one()\n}\n")
            .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0390");
}
