use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::{BuiltinType, TypeKind};

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{CheckedOperation, PrimitiveBinary, PrimitiveOperation, prepare_program_checking};

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
fn every_integer_arithmetic_token_selects_one_closed_primitive() {
    let output = check(
        "func calculate(left: i32, right: i32): i32 {\n    let add = left + right\n    let subtract = add - right\n    let multiply = subtract * right\n    let divide = multiply / right\n    divide % right\n}\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let operations = body
        .nodes()
        .iter()
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
            PrimitiveBinary::Add,
            PrimitiveBinary::Subtract,
            PrimitiveBinary::Multiply,
            PrimitiveBinary::Divide,
            PrimitiveBinary::Remainder,
        ]
    );
}

#[test]
fn destination_type_contextualizes_both_integer_literal_operands() {
    let output = check("func calculate(): u64 {\n    1 + 2\n}\n").unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let (_, arithmetic) = body
        .nodes()
        .iter()
        .find(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Primitive(PrimitiveOperation::Binary { .. })
            )
        })
        .unwrap();

    assert_eq!(
        output.program().types().get(arithmetic.ty()),
        Some(&TypeKind::Builtin(BuiltinType::U64))
    );
}

#[test]
fn left_operand_type_contextualizes_the_rhs_literal_without_a_destination() {
    let output =
        check("func calculate(left: u16): u16 {\n    let value = left + 1\n    value\n}\n")
            .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let (_, arithmetic) = body
        .nodes()
        .iter()
        .find(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Primitive(PrimitiveOperation::Binary { .. })
            )
        })
        .unwrap();

    assert_eq!(
        output.program().types().get(arithmetic.ty()),
        Some(&TypeKind::Builtin(BuiltinType::U16))
    );
}

#[test]
fn arithmetic_rejects_non_integer_and_mismatched_operands() {
    for source in [
        "func invalid(left: bool, right: bool): bool {\n    left + right\n}\n",
        "func invalid(left: u16, right: u32): u16 {\n    left + right\n}\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0370");
    }
}
