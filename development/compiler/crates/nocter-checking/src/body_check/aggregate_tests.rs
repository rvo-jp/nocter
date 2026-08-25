use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::{BuiltinType, TypeKind};
use nocter_source_index::{SemanticEntity, SourceRole};

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{AggregateConstruction, CheckedOperation, prepare_program_checking};

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
fn struct_literals_infer_owner_arguments_and_retain_source_field_order() {
    let output = check(
        "struct Pair<T> {\n    left: T\n    right: T\n}\n\
         func make(): Pair<i32> {\n\
             Pair { right: 2, left: 1 }\n\
         }\n",
    )
    .unwrap();
    let (definition, fields, ty) = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .find_map(|(_, node)| match node.operation() {
            CheckedOperation::Aggregate(AggregateConstruction::Struct { definition, fields }) => {
                Some((*definition, fields, node.ty()))
            }
            _ => None,
        })
        .expect("struct aggregate");

    let field_names = fields
        .iter()
        .map(|(field, _)| {
            let declaration = output
                .program()
                .graph()
                .declarations()
                .fields()
                .get(*field)
                .unwrap();
            output
                .program()
                .graph()
                .symbols()
                .spelling(declaration.name())
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(field_names, vec!["right", "left"]);
    assert!(matches!(
        output.program().types().get(ty),
        Some(TypeKind::Nominal {
            definition: actual,
            arguments,
        }) if *actual == definition
            && output.program().types().get(arguments[0])
                == Some(&TypeKind::Builtin(BuiltinType::I32))
    ));
    for (field, _) in fields {
        assert!(
            output
                .source_index()
                .bindings_for(SemanticEntity::Field(*field))
                .iter()
                .any(|binding| binding.role() == SourceRole::Reference)
        );
    }
}

#[test]
fn struct_literals_reject_incomplete_duplicate_and_unknown_fields() {
    for source in [
        "struct Pair {\n    left: i32\n    right: i32\n}\nfunc invalid(): Pair { Pair { left: 1 } }\n",
        "struct Pair {\n    left: i32\n    right: i32\n}\nfunc invalid(): Pair { Pair { left: 1, left: 2 } }\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0391");
    }

    let error = check(
        "struct Pair {\n    left: i32\n    right: i32\n}\nfunc invalid(): Pair { Pair { left: 1, other: 2 } }\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0379");
}

#[test]
fn construction_surface_does_not_restrict_raw_external_initialization() {
    let fixture = Fixture::with_child(
        "use ./child\nfunc invalid(): child.Value { child.Value { value: 1 } }\n",
        "pub struct Value { pub value: i32 }\n\
         construct Value {\n\
             pub func create(value: i32): Self { Value { value: value } }\n\
         }\n",
    );
    for reverse in [false, true] {
        check_fixture(&fixture, reverse).unwrap();
    }
}

#[test]
fn public_structural_entry_is_stable_under_compile_unit_order() {
    let fixture = Fixture::with_child(
        "use ./child\nfunc make(): child.Value { child.Value { value: 1 } }\n",
        "pub struct Value { pub value: i32 }\n",
    );
    for reverse in [false, true] {
        check_fixture(&fixture, reverse).unwrap();
    }
}

#[test]
fn fixed_arrays_use_context_or_infer_one_canonical_element_type() {
    let output = check(
        "func bytes(): [u8; 3] { [1, 2, 255] }\n\
         func first(): i32 {\n\
             let values = [1, 2]\n\
             values[0]\n\
         }\n",
    )
    .unwrap();
    let arrays = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Aggregate(AggregateConstruction::FixedArray(values)) => {
                Some((node.ty(), values.len()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(arrays.len(), 2);
    assert!(arrays.iter().any(|(ty, length)| {
        *length == 3
            && matches!(
                output.program().types().get(*ty),
                Some(TypeKind::FixedArray { element, length: 3 })
                    if output.program().types().get(*element)
                        == Some(&TypeKind::Builtin(BuiltinType::U8))
            )
    }));
    assert!(arrays.iter().any(|(ty, length)| {
        *length == 2
            && matches!(
                output.program().types().get(*ty),
                Some(TypeKind::FixedArray { element, length: 2 })
                    if output.program().types().get(*element)
                        == Some(&TypeKind::Builtin(BuiltinType::I32))
            )
    }));
}

#[test]
fn fixed_array_length_mismatch_is_a_construction_error() {
    let error = check("func invalid(): [i32; 2] { [1, 2, 3] }\n").unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0391");
}

#[test]
fn empty_fixed_array_requires_and_consumes_element_context() {
    let output = check("func empty(): [bool; 0]? { [] }\n").unwrap();
    assert!(
        output
            .program()
            .bodies()
            .iter()
            .flat_map(|(_, body)| body.nodes().iter())
            .any(|(_, node)| matches!(
                node.operation(),
                CheckedOperation::Aggregate(AggregateConstruction::FixedArray(values))
                    if values.is_empty()
                        && matches!(
                            output.program().types().get(node.ty()),
                            Some(TypeKind::FixedArray { element, length: 0 })
                                if output.program().types().get(*element)
                                    == Some(&TypeKind::Builtin(BuiltinType::Bool))
                        )
            ))
    );

    let error = check("func invalid(): void { let value = []\nreturn }\n").unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0391");
}

#[test]
fn aggregate_ownership_visits_initializers_in_source_order() {
    let error = check(
        "struct Owned { value: i32 }\n\
         func invalid(value: Owned): void {\n\
             let _ = [move value, move value]\n\
             return\n\
         }\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn enum_variants_share_nominal_owner_inference_and_surface_identity() {
    let output = check(
        "enum Maybe<T> {\n    empty\n    value(item: T)\n}\n\
         func present(): Maybe<i32> { Maybe.value(1) }\n\
         func absent(): Maybe<i32> { Maybe.empty }\n\
         func explicit(): Maybe<bool> { Maybe<bool>.value(true) }\n",
    )
    .unwrap();
    let variants = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Aggregate(AggregateConstruction::Enum { variant, payload }) => {
                Some((*variant, payload.len(), node.ty()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(variants.len(), 3);
    assert!(variants.iter().any(|(_, payload, ty)| {
        *payload == 0
            && matches!(
                output.program().types().get(*ty),
                Some(TypeKind::Nominal { arguments, .. })
                    if output.program().types().get(arguments[0])
                        == Some(&TypeKind::Builtin(BuiltinType::I32))
            )
    }));
    for (variant, _, _) in variants {
        assert!(
            output
                .source_index()
                .bindings_for(SemanticEntity::Variant(variant))
                .iter()
                .any(|binding| binding.role() == SourceRole::Reference)
        );
    }
}

#[test]
fn enum_variant_arity_and_payload_ownership_are_checked_at_construction() {
    let arity = check(
        "enum Maybe<T> {\n    value(item: T)\n}\nfunc invalid(): Maybe<i32> { Maybe.value() }\n",
    )
    .unwrap_err();
    assert_eq!(arity.source_diagnostic().unwrap().code(), "E0391");

    let payloadless_call =
        check("enum State { ready }\nfunc invalid(): State { State.ready() }\n").unwrap_err();
    assert_eq!(
        payloadless_call.source_diagnostic().unwrap().code(),
        "E0391"
    );

    let ownership = check(
        "struct Owned { value: i32 }\n\
         enum Pair { both(first: Owned, second: Owned) }\n\
         func invalid(value: Owned): Pair { Pair.both(move value, move value) }\n",
    )
    .unwrap_err();
    assert_eq!(ownership.source_diagnostic().unwrap().code(), "E0378");
}
