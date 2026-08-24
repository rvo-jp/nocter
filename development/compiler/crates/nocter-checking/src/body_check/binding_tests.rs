use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::{BuiltinType, TypeKind};

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    BodyRule, CheckedControl, CheckedOperation, ConstantExpressionRule, TypeValidityRule,
    prepare_program_checking,
};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    check_fixture(&Fixture::new(source), false)
}

fn check_fixture(
    fixture: &Fixture,
    reverse: bool,
) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let input = fixture.input(reverse);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

#[test]
fn annotation_contextualizes_initializer_and_defines_the_declared_local_type() {
    let output = check("func value(): u8 {\n    let number: u8 = 42\n    number\n}\n").unwrap();
    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| !body.locals().is_empty())
        .unwrap();
    let (_, local) = body.locals().iter().next().unwrap();

    assert_eq!(
        local.ty(),
        output.program().types().builtin(BuiltinType::U8)
    );
}

#[test]
fn annotations_supply_context_for_absence_and_empty_aggregate_literals() {
    let output = check(
        "func values(): [i32; 0] {\n    let maybe: i32? = none\n    let empty: [i32; 0] = []\n    let _ = maybe\n    empty\n}\n",
    )
    .unwrap();
    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| !body.locals().is_empty())
        .unwrap();
    let local_types = body
        .locals()
        .iter()
        .map(|(_, local)| local.ty())
        .collect::<Vec<_>>();

    assert!(local_types.iter().any(|ty| matches!(
        output.program().types().get(*ty),
        Some(TypeKind::Optional(payload))
            if *payload == output.program().types().builtin(BuiltinType::I32)
    )));
    assert!(local_types.iter().any(|ty| matches!(
        output.program().types().get(*ty),
        Some(TypeKind::FixedArray { element, length: 0 })
            if *element == output.program().types().builtin(BuiltinType::I32)
    )));
}

#[test]
fn constants_share_values_between_expressions_and_body_array_annotations() {
    let output = check(
        "const width: usize = 2\n\
         const answer: i32 = 40 + 2\n\
         func value(): i32 {\n\
             let values: [i32; width * 2] = [answer, answer, answer, answer]\n\
             answer\n\
         }\n",
    )
    .unwrap();

    assert!(
        output
            .program()
            .types()
            .iter()
            .any(|(_, ty)| { matches!(ty, TypeKind::FixedArray { length: 4, .. }) })
    );
    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.nodes().iter().any(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Constant(nocter_model::ConstantValue::Integer(42))
            )
        })
    }));
}

#[test]
fn body_array_types_use_the_same_block_import_scope_as_value_expressions() {
    for body in [
        "use ./child.{Value, width}\n\n    let values: [Value; width] = []",
        "use ./child\n\n    let values: [child.Value; child.width] = []",
    ] {
        let source = format!("func value(): void {{\n    {body}\n    return\n}}\n");
        let fixture =
            Fixture::with_child(&source, "pub const width: usize = 0\npub struct Value {}\n");
        for reverse in [false, true] {
            let output = check_fixture(&fixture, reverse).unwrap();
            assert!(
                output
                    .program()
                    .types()
                    .iter()
                    .any(|(_, ty)| { matches!(ty, TypeKind::FixedArray { length: 0, .. }) })
            );
        }
    }
}

#[test]
fn body_array_lengths_reject_lexical_runtime_values() {
    let error = check(
        "func value(): void {\n\
             let width: usize = 0\n\
             let values: [i32; width] = []\n\
             return\n\
         }\n",
    )
    .unwrap_err();

    assert_eq!(
        error.constant_expression_rule(),
        Some(ConstantExpressionRule::NonConstantExpression)
    );
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0322");
}

#[test]
fn body_array_lengths_report_shared_constant_expression_rules() {
    for (expression, expected) in [
        ("true", ConstantExpressionRule::TypeMismatch),
        ("1 / 0", ConstantExpressionRule::ArithmeticFailure),
    ] {
        let source = format!(
            "func value(): void {{\n    let values: [i32; {expression}] = []\n    return\n}}\n"
        );
        let error = check(&source).unwrap_err();
        assert_eq!(error.constant_expression_rule(), Some(expected));
        assert_eq!(error.source_diagnostic().unwrap().code(), expected.code());
    }
}

#[test]
fn body_array_length_conversions_share_normal_type_resolution() {
    let output =
        check("func value(): void {\n    let values: [i32; 0 as usize] = []\n    return\n}\n")
            .unwrap();

    assert!(
        output
            .program()
            .types()
            .iter()
            .any(|(_, ty)| { matches!(ty, TypeKind::FixedArray { length: 0, .. }) })
    );
}

#[test]
fn constants_remain_values_in_place_only_contexts() {
    let cases = [
        (
            "const answer: i32 = 42\nfunc invalid(): void {\n    let value = &answer\n    return\n}\n",
            BodyRule::InvalidBorrowSource,
        ),
        (
            "const answer: i32 = 42\nfunc invalid(): void {\n    let value = move answer\n    return\n}\n",
            BodyRule::InvalidMoveSource,
        ),
        (
            "const answer: i32 = 42\nfunc invalid(): void {\n    answer = 1\n    return\n}\n",
            BodyRule::InvalidAssignmentTarget,
        ),
    ];
    for (source, rule) in cases {
        assert_eq!(check(source).unwrap_err().rule(), Some(rule));
    }
}

#[test]
fn annotation_uses_the_common_one_step_expected_conversion_boundary() {
    let output = check(
        "struct Box<T> { value: T }\n\
         instance Box<T> {\n\
             pub coerce &self as &T { &self.value }\n\
         }\n\
         func view(source: &Box<i32>): &i32 {\n\
             let result: &i32 = source\n\
             result\n\
         }\n",
    )
    .unwrap();
    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| !body.locals().is_empty())
        .unwrap();

    assert!(body.nodes().iter().any(|(_, node)| matches!(
        node.operation(),
        CheckedOperation::Control(CheckedControl::Bind { .. })
    )));
}

#[test]
fn generic_and_imported_annotation_names_use_semantic_identity() {
    let generic = check(
        "func retain<T>(value: &T): &T from value {\n    let result: &T = value\n    result\n}\n",
    )
    .unwrap();
    let (_, body) = generic.program().bodies().iter().next().unwrap();
    assert!(matches!(
        generic.program().types().get(body.locals().iter().next().unwrap().1.ty()),
        Some(TypeKind::Borrow { referent, .. })
            if matches!(generic.program().types().get(*referent), Some(TypeKind::GenericParameter(_)))
    ));

    let fixture = Fixture::with_child(
        "pub use ./child.Item\nfunc make(): Item {\n    let item: Item = Item { value: 1 }\n    move item\n}\n",
        "pub struct Item { pub value: i32 }\n",
    );
    let forward = check_fixture(&fixture, false).unwrap();
    let reverse = check_fixture(&fixture, true).unwrap();
    let forward_ty = forward
        .program()
        .bodies()
        .iter()
        .next()
        .unwrap()
        .1
        .locals()
        .iter()
        .next()
        .unwrap()
        .1
        .ty();
    let reverse_ty = reverse
        .program()
        .bodies()
        .iter()
        .next()
        .unwrap()
        .1
        .locals()
        .iter()
        .next()
        .unwrap()
        .1
        .ty();
    assert_eq!(
        forward.program().types().get(forward_ty),
        reverse.program().types().get(reverse_ty)
    );
}

#[test]
fn incompatible_initializer_remains_the_common_expected_type_error() {
    let error =
        check("func invalid(): void {\n    let value: u8 = true\n    return\n}\n").unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::TypeMismatch));
}

#[test]
fn unresolved_annotation_has_its_own_body_type_use_rule() {
    let error = check("func invalid(): void {\n    let value: Missing<i32> = 1\n    return\n}\n")
        .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::InvalidBodyTypeUse));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0406");
}

#[test]
fn annotation_reuses_normalized_data_position_validity() {
    for (annotation, rule) in [
        ("str", TypeValidityRule::UnsizedData),
        ("[i32]", TypeValidityRule::UnsizedData),
        ("void", TypeValidityRule::VoidData),
        ("never", TypeValidityRule::NeverData),
    ] {
        let error = check(&format!(
            "func invalid(): void {{\n    let value: {annotation} = 1\n    return\n}}\n"
        ))
        .unwrap_err();
        assert_eq!(error.type_validity_rule(), Some(rule));
        assert_eq!(error.source_diagnostic().unwrap().code(), rule.code());
    }
}

#[test]
fn discard_binding_rejects_mutability_and_annotations_before_value_checking() {
    for source in [
        "func invalid(): void {\n    var _ = 1\n    return\n}\n",
        "func invalid(): void {\n    let _: i32 = 1\n    return\n}\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.rule(), Some(BodyRule::InvalidDiscardBinding));
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0407");
    }
}
