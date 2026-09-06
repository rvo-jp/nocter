use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::BuiltinType;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    CheckedComparisonPlan, CheckedControl, CheckedOperation, ComparisonImplementation,
    ComparisonOperation, ConstantValue, LogicalOperation, PrimitiveBinary, PrimitiveOperation,
    PrimitiveUnary, ReadonlyOperandPreparation, StaticDispatch, prepare_program_checking,
};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

fn structural_comparison_evidence(
    output: &crate::CheckedProgramOutput,
) -> nocter_model::CapabilityEvidenceId {
    output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .find_map(|(_, node)| match node.operation() {
            CheckedOperation::Comparison(comparison) => {
                comparison
                    .plan()
                    .steps()
                    .find_map(|step| match step.implementation() {
                        ComparisonImplementation::Selected(selection) => match selection.dispatch()
                        {
                            StaticDispatch::StructuralRequirement { evidence } => Some(evidence),
                            _ => None,
                        },
                        ComparisonImplementation::Primitive => None,
                    })
            }
            _ => None,
        })
        .unwrap()
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
fn floating_literals_retain_target_bits_after_contextual_typing() {
    let output = check(
        "func default_value(): f64 { 0.1 }\n\
         func narrow_value(): f32 { 0.1 }\n\
         func suffixed_value(): f32 { 0.1f32 }\n",
    )
    .unwrap();
    let constants = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Constant(ConstantValue::Float32(bits)) => {
                Some((32, u64::from(*bits)))
            }
            CheckedOperation::Constant(ConstantValue::Float64(bits)) => Some((64, *bits)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        constants,
        [
            (64, 0x3fb9_9999_9999_999a),
            (32, 0x3dcc_cccd),
            (32, 0x3dcc_cccd),
        ]
    );
}

#[test]
fn floating_suffix_conflicts_and_nonfinite_rounding_are_source_errors() {
    let mismatch = check("func invalid(): f64 { 0.1f32 }\n").unwrap_err();
    assert_eq!(mismatch.source_diagnostic().unwrap().code(), "E0370");

    let overflow = check("func invalid(): f64 { 1.7976931348623159e308 }\n").unwrap_err();
    assert_eq!(overflow.source_diagnostic().unwrap().code(), "E0414");

    let underflow = check("func invalid(): f32 { 1e-50 }\n").unwrap_err();
    assert_eq!(underflow.source_diagnostic().unwrap().code(), "E0414");
}

#[test]
fn floating_arithmetic_negation_and_comparison_keep_one_exact_operand_type() {
    let output = check(
        "func calculate(left: f32): f32 { -(left + 0.5) * 2.0 }\n\
         func ordered(left: f64): bool { left < 1.0 }\n",
    )
    .unwrap();
    let f32 = output.program().types().builtin(BuiltinType::F32);
    let f64 = output.program().types().builtin(BuiltinType::F64);

    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.nodes().iter().any(|(_, node)| {
            node.ty() == f32
                && matches!(
                    node.operation(),
                    CheckedOperation::Primitive(PrimitiveOperation::Unary {
                        operation: PrimitiveUnary::Negate,
                        ..
                    })
                )
        })
    }));
    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.nodes().iter().any(|(_, node)| {
            node.ty() == f64
                && matches!(
                    node.operation(),
                    CheckedOperation::Constant(ConstantValue::Float64(0x3ff0_0000_0000_0000))
                )
        })
    }));

    let mismatch = check("func invalid(left: f32): f32 { left + 1.0f64 }\n").unwrap_err();
    assert_eq!(mismatch.source_diagnostic().unwrap().code(), "E0370");
    let integer = check("func invalid(left: f64): f64 { left + 1 }\n").unwrap_err();
    assert_eq!(integer.source_diagnostic().unwrap().code(), "E0370");
}

#[test]
fn invalid_negative_literals_report_the_numeric_boundary() {
    let too_small = check("func invalid(): i8 {\n    -129\n}\n").unwrap_err();
    assert_eq!(too_small.source_diagnostic().unwrap().code(), "E0375");

    let unsigned = check("func invalid(): u8 {\n    -1\n}\n").unwrap_err();
    assert_eq!(unsigned.source_diagnostic().unwrap().code(), "E0370");
}

#[test]
fn comparison_context_reaches_a_negative_integer_literal() {
    let output =
        check("func is_minimum(value: i64): bool {\n    value == -9_223_372_036_854_775_808\n}\n")
            .unwrap();

    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.nodes().iter().any(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Constant(ConstantValue::Integer(value))
                    if *value == i128::from(i64::MIN)
            ) && node.ty() == output.program().types().builtin(BuiltinType::I64)
        })
    }));
}

#[test]
fn comparison_context_reaches_a_builtin_integer_expression() {
    check(
        "func fits(size: usize, overhead: usize): bool {\n    size <= 18_446_744_073_709_551_615 - overhead\n}\n",
    )
    .unwrap();
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
    let plans = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Comparison(comparison) => Some(comparison.plan()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(plans.len(), 6);
    let direct = |plan: &&CheckedComparisonPlan| match plan {
        CheckedComparisonPlan::Direct { step, negate }
            if step.implementation() == &ComparisonImplementation::Primitive =>
        {
            Some((step.operation(), step.reverse(), *negate))
        }
        _ => None,
    };
    assert_eq!(
        direct(&plans[0]),
        Some((ComparisonOperation::Equal, false, false))
    );
    assert_eq!(
        direct(&plans[1]),
        Some((ComparisonOperation::Equal, false, true))
    );
    assert_eq!(
        direct(&plans[2]),
        Some((ComparisonOperation::Less, false, false))
    );
    assert!(matches!(
        plans[3],
        CheckedComparisonPlan::Inclusive { strict, equal }
            if strict.operation() == ComparisonOperation::Less
                && !strict.reverse()
                && equal.operation() == ComparisonOperation::Equal
                && !equal.reverse()
    ));
    assert_eq!(
        direct(&plans[4]),
        Some((ComparisonOperation::Less, true, false))
    );
    assert!(matches!(
        plans[5],
        CheckedComparisonPlan::Inclusive { strict, equal }
            if strict.operation() == ComparisonOperation::Less
                && strict.reverse()
                && equal.operation() == ComparisonOperation::Equal
                && !equal.reverse()
    ));
}

#[test]
fn inclusive_comparison_requires_strict_and_equality_operations() {
    let error = check(
        "struct Rank { value: i32 }\ninstance Rank {\n    operator (&self < other: &Self): bool {\n        return self.value < other.value\n    }\n}\nfunc invalid(left: Rank, right: Rank): bool {\n    left <= right\n}\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0389");
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
            CheckedOperation::Comparison(comparison)
                if comparison.plan().steps().any(|step| {
                    matches!(step.implementation(), ComparisonImplementation::Selected(_))
                }) =>
            {
                Some(comparison)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(comparisons.len(), 2);
    assert!(matches!(
        comparisons[0].plan(),
        CheckedComparisonPlan::Direct { step, negate: false }
            if step.operation() == ComparisonOperation::Equal
                && !step.reverse()
                && matches!(step.implementation(), ComparisonImplementation::Selected(selection)
                    if matches!(selection.dispatch(), StaticDispatch::Direct(_)))
    ));
    assert!(matches!(
        comparisons[1].plan(),
        CheckedComparisonPlan::Inclusive { strict, equal }
            if strict.operation() == ComparisonOperation::Less
                && strict.reverse()
                && equal.operation() == ComparisonOperation::Equal
                && matches!(strict.implementation(), ComparisonImplementation::Selected(selection)
                    if matches!(selection.dispatch(), StaticDispatch::Direct(_)))
                && matches!(equal.implementation(), ComparisonImplementation::Selected(selection)
                    if matches!(selection.dispatch(), StaticDispatch::Direct(_)))
    ));
    assert_eq!(
        (
            comparisons[0].left().preparation(),
            comparisons[0].right().preparation(),
        ),
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
        .flat_map(|(_, node)| match node.operation() {
            CheckedOperation::Comparison(comparison) => comparison
                .plan()
                .steps()
                .filter_map(|step| {
                    matches!(step.implementation(), ComparisonImplementation::Selected(_))
                        .then_some((
                            step.operation(),
                            step.reverse(),
                            step.left_coercion().is_some(),
                            step.right_coercion().is_some(),
                        ))
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
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
                if comparison.plan().steps().any(|step| {
                    matches!(step.implementation(), ComparisonImplementation::Selected(_))
                }) =>
            {
                Some(comparison)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(comparisons.len(), 1);
    let step = comparisons[0].plan().steps().next().unwrap();
    assert!(step.left_coercion().is_none());
    assert!(step.right_coercion().is_none());
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
        .flat_map(|(_, node)| match node.operation() {
            CheckedOperation::Comparison(comparison) => comparison
                .plan()
                .steps()
                .filter_map(|step| match step.implementation() {
                    ComparisonImplementation::Selected(selection) => Some((
                        selection.dispatch(),
                        comparison.left().preparation(),
                        comparison.right().preparation(),
                    )),
                    ComparisonImplementation::Primitive => None,
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();

    assert_eq!(selections.len(), 3);
    assert!(
        selections.iter().all(|(dispatch, _, _)| matches!(
            dispatch,
            StaticDispatch::StructuralRequirement { .. }
        ))
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
                    if comparison.plan().steps().any(|step| matches!(
                        step.implementation(), ComparisonImplementation::Selected(_)
                    ))
            )
        })
        .count();
    assert_eq!(selected, 4);
}

#[test]
fn interface_structural_prerequisite_exposes_equality_to_generic_bodies() {
    let output = check(
        "pub interface Equatable where (&Self == &Self): bool {}\n\
         func equal<T>(left: &T, right: &T): bool where T impl Equatable { left == right }\n",
    )
    .unwrap();

    assert!(
        output
            .program()
            .bodies()
            .iter()
            .flat_map(|(_, body)| body.nodes().iter())
            .any(|(_, node)| matches!(
                node.operation(),
                CheckedOperation::Comparison(comparison)
                    if comparison.plan().steps().any(|step| matches!(
                        step.implementation(),
                        ComparisonImplementation::Selected(selection)
                            if matches!(selection.dispatch(), StaticDispatch::StructuralRequirement { .. })
                    ))
            ))
    );
}

#[test]
fn duplicate_prerequisite_facts_retain_every_authored_derivation() {
    let output = check(
        "pub interface Left where (&Self == &Self): bool {}\n\
         pub interface Right where (&Self == &Self): bool {}\n\
         func equal<T>(left: &T, right: &T): bool where T impl Left, T impl Right { left == right }\n",
    )
    .unwrap();

    let evidence = structural_comparison_evidence(&output);
    let capability = output.program().capability_evidence(evidence).unwrap();

    assert_eq!(capability.derivations().len(), 2);
    assert_eq!(
        output
            .source_index()
            .bindings_for(nocter_source_index::SemanticEntity::CapabilityEvidence(
                evidence,
            ))
            .iter()
            .filter(|binding| { binding.role() == nocter_source_index::SourceRole::Declaration })
            .count(),
        2
    );
}

#[test]
fn converging_prerequisite_routes_do_not_duplicate_one_source_origin() {
    let output = check(
        "pub interface Base where (&Self == &Self): bool {}\n\
         pub interface Left where Self impl Base {}\n\
         pub interface Right where Self impl Base {}\n\
         func equal<T>(left: &T, right: &T): bool where T impl Left, T impl Right { left == right }\n",
    )
    .unwrap();
    let evidence = structural_comparison_evidence(&output);

    assert_eq!(
        output
            .program()
            .capability_evidence(evidence)
            .unwrap()
            .derivations()
            .len(),
        2
    );
    assert_eq!(
        output
            .source_index()
            .bindings_for(nocter_source_index::SemanticEntity::CapabilityEvidence(
                evidence,
            ))
            .iter()
            .filter(|binding| { binding.role() == nocter_source_index::SourceRole::Declaration })
            .count(),
        1
    );
}

#[test]
fn ambiguous_comparison_coercion_targets_are_rejected() {
    let error = check(
        "struct First { value: i32 }\nstruct Second { value: i32 }\nstruct Source {\n    first: First\n    second: Second\n}\ninstance First {\n    pub operator (&self == other: &Self): bool {\n        return self.value == other.value\n    }\n}\ninstance Second {\n    pub operator (&self == other: &Self): bool {\n        return self.value == other.value\n    }\n}\ninstance Source {\n    pub coerce &self as &First {\n        return &self.first\n    }\n    pub coerce &self as &Second {\n        return &self.second\n    }\n}\nfunc invalid(left: Source, right: Source): bool {\n    left == right\n}\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0389");
}
