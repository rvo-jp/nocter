use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::BuiltinType;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    BorrowConversionImplementation, BorrowConversionPreparation, CheckedOperation,
    PrimitiveOperation, StaticDispatch, prepare_program_checking,
};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

#[test]
fn readwrite_borrows_weaken_at_the_common_expected_type_boundary() {
    let output = check(
        "func accept(value: &i32): &i32 {\n    value\n}\nfunc weaken(value: &+i32): &i32 {\n    accept(value)\n}\n",
    )
    .unwrap();
    let conversions = conversions(&output);

    assert_eq!(conversions.len(), 1);
    assert_eq!(
        conversions[0].preparation(),
        BorrowConversionPreparation::WeakenReadwrite
    );
    assert!(matches!(
        conversions[0].implementation(),
        BorrowConversionImplementation::CapabilityWeakening
    ));
}

#[test]
fn generic_call_inference_accepts_readwrite_to_readonly_weakening() {
    let output = check(
        "func borrow<T>(value: &T): &T {\n    value\n}\nfunc weaken(value: &+i32): &i32 {\n    borrow(value)\n}\n",
    )
    .unwrap();
    let conversions = conversions(&output);

    assert_eq!(conversions.len(), 1);
    assert_eq!(
        conversions[0].preparation(),
        BorrowConversionPreparation::WeakenReadwrite
    );
}

#[test]
fn generic_call_result_inference_accepts_readwrite_to_readonly_weakening() {
    let output = check(
        "func unreachable_borrow<T>(): &+T {\n    loop {}\n}\nfunc weaken(): &i32 {\n    unreachable_borrow()\n}\n",
    )
    .unwrap();
    let conversions = conversions(&output);

    assert_eq!(conversions.len(), 1);
    assert_eq!(
        conversions[0].preparation(),
        BorrowConversionPreparation::WeakenReadwrite
    );
}

#[test]
fn source_coercions_are_selected_for_arguments_and_results() {
    let output = check(
        "struct Text { value: i32 }\nstruct Wrapper { text: Text }\ninstance Wrapper {\n    pub coerce &self as &Text {\n        return &self.text\n    }\n}\nfunc accept(value: &Text): i32 {\n    value.value\n}\nfunc call(wrapper: &Wrapper): i32 {\n    accept(wrapper)\n}\nfunc view(wrapper: &Wrapper): &Text {\n    wrapper\n}\n",
    )
    .unwrap();
    let conversions = conversions(&output);

    assert_eq!(conversions.len(), 2);
    assert!(conversions.iter().all(|conversion| {
        conversion.preparation() == BorrowConversionPreparation::PreserveReadonly
            && matches!(
                conversion.implementation(),
                BorrowConversionImplementation::Selected(selection)
                    if matches!(selection.dispatch(), StaticDispatch::Direct(_))
            )
    }));
}

#[test]
fn static_call_results_use_conversion_after_inference() {
    let output = check(
        "struct Text { value: i32 }\nstruct Wrapper { text: Text }\ninstance Wrapper {\n    pub coerce &self as &Text {\n        return &self.text\n    }\n}\nfunc identity(wrapper: &Wrapper): &Wrapper {\n    wrapper\n}\nfunc view(wrapper: &Wrapper): &Text {\n    identity(wrapper)\n}\n",
    )
    .unwrap();
    let conversions = conversions(&output);

    assert_eq!(conversions.len(), 1);
    assert!(matches!(
        conversions[0].implementation(),
        BorrowConversionImplementation::Selected(_)
    ));
}

#[test]
fn generic_bodies_dispatch_coercion_through_the_lexical_requirement() {
    let output = check(
        "struct Text { value: i32 }\nfunc view<T>(value: &T): &Text where &T as &Text {\n    value\n}\n",
    )
    .unwrap();
    let conversions = conversions(&output);

    assert_eq!(conversions.len(), 1);
    assert!(matches!(
        conversions[0].implementation(),
        BorrowConversionImplementation::Selected(selection)
            if matches!(selection.dispatch(), StaticDispatch::StructuralRequirement(_))
    ));
}

#[test]
fn readwrite_receiver_coercion_is_a_readonly_fallback_not_an_authority_upgrade() {
    let output = check(
        "struct Text { value: i32 }\nstruct Wrapper { text: Text }\ninstance Wrapper {\n    pub coerce &+self as &Text {\n        return &self.text\n    }\n}\nfunc view(wrapper: &+Wrapper): &Text {\n    wrapper\n}\n",
    )
    .unwrap();
    let conversions = conversions(&output);

    assert_eq!(conversions.len(), 1);
    assert_eq!(
        conversions[0].preparation(),
        BorrowConversionPreparation::PreserveReadwrite
    );
    assert!(matches!(
        conversions[0].implementation(),
        BorrowConversionImplementation::Selected(_)
    ));
}

#[test]
fn readonly_receiver_coercion_has_minimum_authority_priority() {
    let output = check(
        "struct Text { value: i32 }\nstruct Wrapper { text: Text }\ninstance Wrapper {\n    pub coerce &self as &Text {\n        return &self.text\n    }\n    pub coerce &+self as &Text {\n        return &self.text\n    }\n}\nfunc view(wrapper: &+Wrapper): &Text {\n    wrapper\n}\n",
    )
    .unwrap();
    let conversions = conversions(&output);

    assert_eq!(conversions.len(), 1);
    assert_eq!(
        conversions[0].preparation(),
        BorrowConversionPreparation::WeakenReadwrite
    );
}

#[test]
fn expected_type_conversion_never_chains_source_coercions() {
    let error = check(
        "struct Final { value: i32 }\nstruct Middle { final_value: Final }\nstruct Source { middle: Middle }\ninstance Source {\n    pub coerce &self as &Middle {\n        return &self.middle\n    }\n}\ninstance Middle {\n    pub coerce &self as &Final {\n        return &self.final_value\n    }\n}\nfunc invalid(source: &Source): &Final {\n    source\n}\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0370");
}

#[test]
fn explicit_integer_conversion_records_one_lossless_checked_operation() {
    let output = check("func widen(value: u32): i64 {\n    value as i64\n}\n").unwrap();
    let conversions = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Primitive(PrimitiveOperation::IntegerConversion {
                operand,
                target,
            }) => Some((*operand, *target, node.ty())),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(conversions.len(), 1);
    assert_eq!(
        conversions[0].1,
        output.program().types().builtin(BuiltinType::I64)
    );
    assert_eq!(conversions[0].1, conversions[0].2);
}

#[test]
fn explicit_integer_conversion_rejects_narrowing_and_signed_to_unsigned_ranges() {
    for source in [
        "func narrow(value: u64): u8 {\n    value as u8\n}\n",
        "func change_sign(value: i32): u64 {\n    value as u64\n}\n",
        "func too_wide(value: u64): i64 {\n    value as i64\n}\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0370");
    }
}

#[test]
fn explicit_borrow_conversion_selects_one_exact_source_entry() {
    let output = check(
        "struct Text { value: i32 }\nstruct Wrapper { text: Text }\ninstance Wrapper {\n    pub coerce &self as &Text {\n        return &self.text\n    }\n}\nfunc view(wrapper: &Wrapper): &Text {\n    wrapper as &Text\n}\n",
    )
    .unwrap();
    let conversions = conversions(&output);

    assert_eq!(conversions.len(), 1);
    assert!(matches!(
        conversions[0].implementation(),
        BorrowConversionImplementation::Selected(_)
    ));
}

fn conversions(output: &crate::CheckedProgramOutput) -> Vec<&crate::CheckedBorrowConversion> {
    output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::BorrowConversion(conversion) => Some(conversion),
            _ => None,
        })
        .collect()
}
