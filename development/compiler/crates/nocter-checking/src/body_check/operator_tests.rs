use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::BuiltinType;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    CheckedControl, CheckedOperation, ConstantValue, LogicalOperation, PrimitiveBinary,
    PrimitiveComparison, PrimitiveOperation, PrimitiveUnary, prepare_program_checking,
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
fn logical_not_and_runtime_negation_have_closed_unary_operations() {
    let output = check(
        "func invert(value: bool): bool {\n    !value\n}\nfunc negate(value: i64): i64 {\n    -value\n}\n",
    )
    .unwrap();
    let operations = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Primitive(PrimitiveOperation::Unary { operation, .. }) => {
                Some(*operation)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        operations,
        vec![PrimitiveUnary::LogicalNot, PrimitiveUnary::Negate]
    );
}

#[test]
fn negative_literals_include_the_exact_signed_minimum_in_one_constant() {
    let output = check(
        "func minimum_i8(): i8 {\n    -128\n}\nfunc minimum_i64(): i64 {\n    -9_223_372_036_854_775_808\n}\n",
    )
    .unwrap();
    let constants = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Constant(ConstantValue::Integer(value)) if *value < 0 => {
                Some((node.ty(), *value))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(constants.len(), 2);
    assert_eq!(constants[0].1, -128);
    assert_eq!(constants[1].1, i128::from(i64::MIN));
    assert_eq!(
        constants[0].0,
        output.program().types().builtin(BuiltinType::I8)
    );
    assert_eq!(
        constants[1].0,
        output.program().types().builtin(BuiltinType::I64)
    );
    assert!(!output.program().bodies().iter().any(|(_, body)| {
        body.nodes().iter().any(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Primitive(PrimitiveOperation::Unary {
                    operation: PrimitiveUnary::Negate,
                    ..
                })
            )
        })
    }));
}

#[test]
fn invalid_negative_literals_report_the_numeric_boundary() {
    let too_small = check("func invalid(): i8 {\n    -129\n}\n").unwrap_err();
    assert_eq!(too_small.source_diagnostic().unwrap().code(), "E0375");

    let unsigned = check("func invalid(): u8 {\n    -1\n}\n").unwrap_err();
    assert_eq!(unsigned.source_diagnostic().unwrap().code(), "E0370");
}

#[test]
fn shifts_distinguish_left_signed_right_and_unsigned_right() {
    let output = check(
        "func left(value: u32, count: u32): u32 {\n    value << count\n}\nfunc signed(value: i32, count: i32): i32 {\n    value >> count\n}\nfunc unsigned(value: u32, count: u32): u32 {\n    value >> count\n}\n",
    )
    .unwrap();
    let operations = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Primitive(PrimitiveOperation::Binary { operation, .. }) => {
                Some(*operation)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        operations,
        vec![
            PrimitiveBinary::ShiftLeft,
            PrimitiveBinary::ShiftRightSigned,
            PrimitiveBinary::ShiftRightUnsigned,
        ]
    );

    let mismatch =
        check("func invalid(value: u32, count: u64): u32 {\n    value << count\n}\n").unwrap_err();
    assert_eq!(mismatch.source_diagnostic().unwrap().code(), "E0370");
}

#[test]
fn primitive_comparisons_retain_the_strict_derivation() {
    let output = check(
        "enum Flag {\n    on\n    off\n}\nfunc same_flag(left: Flag, right: Flag): bool {\n    left == right\n}\nfunc different(left: bool, right: bool): bool {\n    left != right\n}\nfunc less(left: i32, right: i32): bool {\n    left < right\n}\nfunc at_most(left: i32, right: i32): bool {\n    left <= right\n}\nfunc greater(left: i32, right: i32): bool {\n    left > right\n}\nfunc at_least(left: i32, right: i32): bool {\n    left >= right\n}\n",
    )
    .unwrap();
    let operations = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Primitive(PrimitiveOperation::Comparison {
                operation,
                reverse,
                negate,
                ..
            }) => Some((*operation, *reverse, *negate)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        operations,
        vec![
            (PrimitiveComparison::Equal, false, false),
            (PrimitiveComparison::Equal, false, true),
            (PrimitiveComparison::Less, false, false),
            (PrimitiveComparison::Less, true, true),
            (PrimitiveComparison::Less, true, false),
            (PrimitiveComparison::Less, false, true),
        ]
    );
}

#[test]
fn logical_operations_are_short_circuit_control_nodes() {
    let output = check(
        "func both(left: bool, right: bool): bool {\n    left && right\n}\nfunc either(left: bool, right: bool): bool {\n    left || right\n}\n",
    )
    .unwrap();
    let operations = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Control(CheckedControl::Logical { operation, .. }) => {
                Some(*operation)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        operations,
        vec![LogicalOperation::And, LogicalOperation::Or]
    );
    assert!(!output.program().bodies().iter().any(|(_, body)| {
        body.nodes().iter().any(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Primitive(PrimitiveOperation::Binary { .. })
            )
        })
    }));
}

#[test]
fn short_circuit_rhs_ownership_is_joined_with_the_bypass_path() {
    let error = check(
        "struct Owned { value: i32 }\nfunc invalid(condition: bool, input: Owned): Owned {\n    let _ = condition && if condition {\n        let _ = move input\n        true\n    } else {\n        true\n    }\n    move input\n}\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}
