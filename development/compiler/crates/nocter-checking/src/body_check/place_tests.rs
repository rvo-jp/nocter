use nocter_declaration_lowering::lower_compile_unit_declarations;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    CheckedOperation, PlaceAccess, PlaceProjection, PrimitiveOperation, prepare_program_checking,
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
fn readonly_borrow_uses_the_same_resolved_parameter_place() {
    let fixture =
        Fixture::new("func observe(value: i32): void {\n    let view = &value\n    return\n}\n");
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();

    assert!(
        body.nodes()
            .iter()
            .any(|(_, node)| matches!(node.operation(), CheckedOperation::Borrow { .. }))
    );
    assert_eq!(body.places().len(), 1);
}

#[test]
fn fixed_array_and_slice_reads_are_copy_places_with_one_index_evaluation() {
    for source in [
        "func read(values: [i32; 2], index: usize): i32 {\n    values[index]\n}\n",
        "func read(values: &[i32], index: usize): i32 {\n    values[index]\n}\n",
        "func read(values: &+[i32], index: usize): i32 {\n    values[index]\n}\n",
    ] {
        let output = check(source).unwrap();
        let (_, body) = output.program().bodies().iter().next().unwrap();
        let place = body
            .nodes()
            .iter()
            .find_map(|(_, node)| match node.operation() {
                CheckedOperation::Copy(place)
                    if body.places().get(*place).is_some_and(|place| {
                        matches!(
                            place.projections().last(),
                            Some(PlaceProjection::BuiltinIndex { .. })
                        )
                    }) =>
                {
                    Some(*place)
                }
                _ => None,
            })
            .unwrap();

        assert_eq!(
            body.places().get(place).unwrap().evaluation_nodes().count(),
            1
        );
    }
}

#[test]
fn nested_index_evaluations_are_retained_once_in_source_order() {
    let output = check("func read(values: [[i32; 2]; 2]): i32 {\n    values[0][1]\n}\n").unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let place = body
        .places()
        .iter()
        .find(|(_, place)| {
            place
                .projections()
                .iter()
                .filter(|projection| matches!(projection, PlaceProjection::BuiltinIndex { .. }))
                .count()
                == 2
        })
        .unwrap()
        .1;
    let values = place
        .evaluation_nodes()
        .map(|node| match body.nodes().get(node).unwrap().operation() {
            CheckedOperation::Constant(crate::ConstantValue::Integer(value)) => *value,
            operation => panic!("unexpected index operation: {operation:?}"),
        })
        .collect::<Vec<_>>();

    assert_eq!(values, vec![0, 1]);
}

#[test]
fn move_only_index_read_is_rejected_as_an_implicit_move() {
    let error = check(
        "struct Owned { value: i32 }\nfunc invalid(values: [Owned; 1]): Owned {\n    values[0]\n}\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0371");
}

#[test]
fn indexed_places_support_readonly_and_readwrite_borrows() {
    let output = check(
        "func borrow(values: &+[i32], index: usize): void {\n    let readonly = &values[index]\n    let writable = &+values[index]\n    return\n}\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let capabilities = body
        .nodes()
        .iter()
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Borrow { capability, .. } => Some(*capability),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        capabilities,
        vec![
            nocter_model::BorrowCapability::Readonly,
            nocter_model::BorrowCapability::ReadWrite,
        ]
    );
}

#[test]
fn readwrite_borrow_rejects_readonly_or_immutable_index_storage() {
    for source in [
        "func invalid(values: &[i32], index: usize): void {\n    let value = &+values[index]\n    return\n}\n",
        "func invalid(values: [i32; 1]): void {\n    let value = &+values[0]\n    return\n}\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0387");
    }
}

#[test]
fn named_field_reads_use_the_same_postfix_place_builder() {
    let output =
        check("struct Pair { value: i32 }\nfunc read(pair: Pair): i32 {\n    pair.value\n}\n")
            .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();

    assert!(body.nodes().iter().any(|(_, node)| matches!(
        node.operation(),
        CheckedOperation::Copy(place)
            if matches!(body.places().get(*place).unwrap().projections(), [PlaceProjection::Field(_)])
    )));
    assert!(!body.nodes().iter().any(|(_, node)| matches!(
        node.operation(),
        CheckedOperation::Primitive(PrimitiveOperation::Binary { .. })
    )));
}

#[test]
fn readwrite_slice_index_retains_borrowed_storage_authority() {
    let output = check("func read(values: &+[i32]): i32 {\n    values[0]\n}\n").unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let place = body
        .places()
        .iter()
        .find(|(_, place)| {
            matches!(
                place.projections().last(),
                Some(PlaceProjection::BuiltinIndex { .. })
            )
        })
        .unwrap()
        .1;

    assert_eq!(
        place.access(),
        PlaceAccess::Borrowed(nocter_model::BorrowCapability::ReadWrite)
    );
}

#[test]
fn borrow_dereference_preserves_the_initialized_owned_field_prefix() {
    let output = check(
        "struct Owned { value: i32 }\nstruct Pair { value: i32 }\nstruct Holder { pair: &Pair\n    owned: Owned\n}\nfunc read(holder: Holder): i32 {\n    let _ = move holder.owned\n    holder.pair.value\n}\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let place = body
        .places()
        .iter()
        .find(|(_, place)| {
            matches!(
                place.projections(),
                [
                    PlaceProjection::Field(_),
                    PlaceProjection::BorrowDeref { .. },
                    PlaceProjection::Field(_)
                ]
            )
        })
        .unwrap()
        .1;

    assert_eq!(
        place.access(),
        PlaceAccess::Borrowed(nocter_model::BorrowCapability::Readonly)
    );
}
