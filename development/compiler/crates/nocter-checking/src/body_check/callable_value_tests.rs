use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::CallableCapability;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{CallTarget, CheckedOperation, StaticDispatch, prepare_program_checking};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

#[test]
fn readonly_callable_requirement_freezes_dispatch_and_permits_repeated_calls() {
    let output = check(
        "func inspect<F>(callback: F, value: i32): bool where F: &func(value: i32): bool {\n    let _ = callback(value)\n    callback(value)\n}\n",
    )
    .unwrap();
    let calls = callable_calls(&output);

    assert_eq!(calls.len(), 2);
    assert!(calls.iter().all(|(_, capability, dispatch)| {
        *capability == CallableCapability::Readonly
            && matches!(
                dispatch.dispatch(),
                StaticDispatch::StructuralRequirement(_)
            )
    }));
}

#[test]
fn readwrite_callable_requires_writable_storage_and_remains_reusable() {
    check(
        "func transform<F>(callback: F, value: i32): i32 where F: &+func(value: i32): i32 {\n    var callable = move callback\n    let first = callable(value)\n    callable(first)\n}\n",
    )
    .unwrap();

    let error = check(
        "func invalid<F>(callback: F, value: i32): i32 where F: &+func(value: i32): i32 {\n    callback(value)\n}\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0390");
}

#[test]
fn owned_callable_call_consumes_its_place_once() {
    let error = check(
        "func finish<F>(callback: F, value: i32): i32 where F: func(value: i32): i32 {\n    let _ = callback(value)\n    callback(value)\n}\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn callable_arguments_use_the_common_expected_conversion_boundary() {
    let output = check(
        "struct Text { value: i32 }\nstruct Wrapper { text: Text }\ninstance Wrapper {\n    pub coerce &self as &Text {\n        return &self.text\n    }\n}\nfunc apply<F>(callback: F, wrapper: &Wrapper): i32 where F: &func(value: &Text): i32 {\n    callback(wrapper)\n}\n",
    )
    .unwrap();

    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.nodes()
            .iter()
            .any(|(_, node)| matches!(node.operation(), CheckedOperation::BorrowConversion(_)))
    }));
}

#[test]
fn callable_value_arity_and_missing_contract_use_the_call_diagnostic() {
    let arity = check(
        "func invalid<F>(callback: F): bool where F: &func(value: i32): bool {\n    callback()\n}\n",
    )
    .unwrap_err();
    assert_eq!(arity.source_diagnostic().unwrap().code(), "E0390");

    let missing =
        check("func invalid<T>(value: T): void {\n    value()\n    return\n}\n").unwrap_err();
    assert_eq!(missing.source_diagnostic().unwrap().code(), "E0390");
}

fn callable_calls(
    output: &crate::CheckedProgramOutput,
) -> Vec<(
    nocter_model::BodyNodeId,
    CallableCapability,
    &crate::StaticSelection,
)> {
    output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Call(call) => match call.target() {
                CallTarget::CallableValue {
                    value,
                    capability,
                    dispatch,
                } => Some((*value, *capability, dispatch)),
                CallTarget::Static(_) | CallTarget::ClosureValue { .. } => None,
            },
            _ => None,
        })
        .collect()
}
