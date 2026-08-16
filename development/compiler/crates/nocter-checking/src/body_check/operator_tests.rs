use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::BuiltinType;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    CheckedControl, CheckedOperation, ComparisonImplementation, ComparisonOperation, ConstantValue,
    LogicalOperation, PrimitiveBinary, PrimitiveOperation, PrimitiveUnary,
    ReadonlyOperandPreparation, StaticDispatch, prepare_program_checking,
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
            CheckedOperation::Comparison(comparison)
                if comparison.implementation() == &ComparisonImplementation::Primitive =>
            {
                Some((
                    comparison.operation(),
                    comparison.reverse(),
                    comparison.negate(),
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        operations,
        vec![
            (ComparisonOperation::Equal, false, false),
            (ComparisonOperation::Equal, false, true),
            (ComparisonOperation::Less, false, false),
            (ComparisonOperation::Less, true, true),
            (ComparisonOperation::Less, true, false),
            (ComparisonOperation::Less, false, true),
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

#[test]
fn direct_comparisons_borrow_move_only_places_and_retain_static_dispatch() {
    let output = check(
        "struct Rank { value: i32 }\ninstance Rank {\n    pub operator (&self == other: &Self): bool {\n        return self.value == other.value\n    }\n    pub operator (&self < other: &Self): bool {\n        return self.value < other.value\n    }\n}\nfunc compare(left: Rank, right: Rank): i32 {\n    let _ = left == right\n    let _ = left >= right\n    left.value + right.value\n}\n",
    )
    .unwrap();
    let comparisons = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Comparison(comparison) => {
                let ComparisonImplementation::Selected(selection) = comparison.implementation()
                else {
                    return None;
                };
                Some((
                    comparison.operation(),
                    comparison.reverse(),
                    comparison.negate(),
                    selection.dispatch(),
                    comparison.left().preparation(),
                    comparison.right().preparation(),
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(comparisons.len(), 2);
    assert_eq!(comparisons[0].0, ComparisonOperation::Equal);
    assert_eq!(comparisons[1].0, ComparisonOperation::Less);
    assert_eq!((comparisons[1].1, comparisons[1].2), (false, true));
    assert!(matches!(comparisons[0].3, StaticDispatch::Direct(_)));
    assert!(matches!(comparisons[1].3, StaticDispatch::Direct(_)));
    assert_eq!(
        (comparisons[0].4, comparisons[0].5),
        (
            ReadonlyOperandPreparation::BorrowPlace,
            ReadonlyOperandPreparation::BorrowPlace,
        )
    );
}

#[test]
fn comparison_coercions_are_attached_to_source_operands_after_semantic_reversal() {
    let output = check(
        "struct Text { value: i32 }\nstruct Wrapper { value: Text }\ninstance Text {\n    pub operator (&self == other: &Self): bool {\n        return self.value == other.value\n    }\n    pub operator (&self < other: &Self): bool {\n        return self.value < other.value\n    }\n}\ninstance Wrapper {\n    pub coerce &self as &Text {\n        return &self.value\n    }\n}\nfunc equal(left: Text, right: Wrapper): bool {\n    left == right\n}\nfunc greater(left: Wrapper, right: Text): bool {\n    left > right\n}\n",
    )
    .unwrap();
    let comparisons = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Comparison(comparison)
                if matches!(
                    comparison.implementation(),
                    ComparisonImplementation::Selected(_)
                ) =>
            {
                Some((
                    comparison.operation(),
                    comparison.reverse(),
                    comparison.left().coercion().is_some(),
                    comparison.right().coercion().is_some(),
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        comparisons,
        vec![
            (ComparisonOperation::Equal, false, false, true),
            (ComparisonOperation::Less, true, true, false),
        ]
    );
}

#[test]
fn exact_left_comparison_declaration_outranks_coercion_routes() {
    let output = check(
        "struct View { value: i32 }\nstruct Source { view: View }\ninstance View {\n    pub operator (&self == other: &Self): bool {\n        return self.value == other.value\n    }\n}\ninstance Source {\n    pub operator (&self == other: &Self): bool {\n        return true\n    }\n    pub coerce &self as &View {\n        return &self.view\n    }\n}\nfunc same(left: Source, right: Source): bool {\n    left == right\n}\n",
    )
    .unwrap();
    let comparisons = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Comparison(comparison)
                if matches!(
                    comparison.implementation(),
                    ComparisonImplementation::Selected(_)
                ) =>
            {
                Some(comparison)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(comparisons.len(), 1);
    assert!(comparisons[0].left().coercion().is_none());
    assert!(comparisons[0].right().coercion().is_none());
}

#[test]
fn generic_comparisons_dispatch_through_the_lexical_requirement() {
    let output = check(
        "func same<T>(left: &T, right: &T): bool where (&T == &T): bool {\n    left == right\n}\nfunc same_mut<T>(left: &+T, right: &T): bool where (&T == &T): bool {\n    left == right\n}\nfunc earlier<T>(left: &T, right: &T): bool where (&T < &T): bool {\n    left < right\n}\n",
    )
    .unwrap();
    let selections = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Comparison(comparison) => match comparison.implementation() {
                ComparisonImplementation::Selected(selection) => Some((
                    selection.dispatch(),
                    comparison.left().preparation(),
                    comparison.right().preparation(),
                )),
                ComparisonImplementation::Primitive | ComparisonImplementation::Unreachable => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(selections.len(), 3);
    assert!(
        selections
            .iter()
            .all(|(dispatch, _, _)| matches!(dispatch, StaticDispatch::StructuralRequirement(_)))
    );
    assert_eq!(
        (selections[1].1, selections[1].2),
        (
            ReadonlyOperandPreparation::WeakenReadwriteBorrow,
            ReadonlyOperandPreparation::UseReadonlyBorrow,
        )
    );
}

#[test]
fn conditional_comparison_instances_use_recursive_operation_proof() {
    let output = check(
        "struct Box<T> { value: T }\ninstance Box<T> where (&T == &T): bool, (&T < &T): bool {\n    pub operator (&self == other: &Self): bool {\n        return self.value == other.value\n    }\n    pub operator (&self < other: &Self): bool {\n        return self.value < other.value\n    }\n}\nfunc compare(left: Box<i32>, right: Box<i32>): bool {\n    (left == right) || (left < right)\n}\n",
    )
    .unwrap();

    let selected = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Comparison(comparison)
                    if matches!(comparison.implementation(), ComparisonImplementation::Selected(_))
            )
        })
        .count();
    assert_eq!(selected, 4);
}

#[test]
fn ambiguous_comparison_coercion_targets_are_rejected() {
    let error = check(
        "struct First { value: i32 }\nstruct Second { value: i32 }\nstruct Source {\n    first: First\n    second: Second\n}\ninstance First {\n    pub operator (&self == other: &Self): bool {\n        return self.value == other.value\n    }\n}\ninstance Second {\n    pub operator (&self == other: &Self): bool {\n        return self.value == other.value\n    }\n}\ninstance Source {\n    pub coerce &self as &First {\n        return &self.first\n    }\n    pub coerce &self as &Second {\n        return &self.second\n    }\n}\nfunc invalid(left: Source, right: Source): bool {\n    left == right\n}\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0389");
}
