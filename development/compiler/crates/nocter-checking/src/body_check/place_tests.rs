use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::BuiltinType;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    CheckedOperation, PlaceAccess, PlaceProjection, PrimitiveOperation, prepare_program_checking,
};

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
fn readonly_borrow_uses_the_same_resolved_parameter_place() {
    let fixture =
        Fixture::new("func observe(value: i32): void {\n    let view = &value\n    return\n}\n");
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
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
    assert_eq!(place.projection_types().len(), place.projections().len());
    assert_eq!(
        place.projection_types().last().copied(),
        Some(output.program().types().builtin(BuiltinType::I32))
    );
}

#[test]
fn source_defined_readonly_index_is_selected_once_as_a_place_projection() {
    let output = check(
        "struct Buffer { values: [i32; 2] }\ninstance Buffer {\n    pub operator (&self[index: usize]): &i32 {\n        return &self.values[index]\n    }\n}\nfunc read(buffer: &Buffer): i32 {\n    buffer[0]\n}\n",
    )
    .unwrap();
    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(body, _)| {
            matches!(
                output.program().graph().declarations().bodies().get(*body).unwrap().owner(),
                nocter_declarations::BodyOwner::Callable(callable)
                    if output.program().graph().declarations().callables().get(callable).unwrap().kind()
                        == nocter_declarations::CallableKind::Function
            )
        })
        .unwrap();
    let selected = body
        .places()
        .iter()
        .find_map(|(_, place)| match place.projections().last() {
            Some(PlaceProjection::SelectedIndex {
                operation,
                receiver_coercion,
                ..
            }) => Some((operation, receiver_coercion)),
            _ => None,
        })
        .unwrap();

    assert!(matches!(
        selected.0.dispatch(),
        crate::StaticDispatch::Direct(_)
    ));
    assert!(selected.0.generic_arguments().as_slice().is_empty());
    assert!(selected.1.is_none());
}

#[test]
fn generic_index_selection_retains_the_complete_instance_substitution() {
    let output = check(
        "struct Buffer<T> { values: [T; 1] }\ninstance Buffer<T> {\n    pub operator (&self[index: usize]): &T {\n        return &self.values[index]\n    }\n}\nfunc read(buffer: &Buffer<i32>): i32 {\n    buffer[0]\n}\n",
    )
    .unwrap();
    let generic_arguments = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.places().iter())
        .find_map(|(_, place)| match place.projections().last() {
            Some(PlaceProjection::SelectedIndex { operation, .. })
                if !operation.generic_arguments().as_slice().is_empty() =>
            {
                Some(operation.generic_arguments())
            }
            _ => None,
        })
        .unwrap();

    assert_eq!(generic_arguments.as_slice().len(), 1);
    assert_eq!(
        generic_arguments.as_slice()[0].ty(),
        output
            .program()
            .types()
            .builtin(nocter_model::BuiltinType::I32)
    );
}

#[test]
fn one_receiver_coercion_can_reach_a_builtin_index_projection() {
    let output = check(
        "struct Wrapper { values: [i32; 1] }\ninstance Wrapper {\n    pub coerce &self as &[i32; 1] {\n        return &self.values\n    }\n}\nfunc read(wrapper: &Wrapper): i32 {\n    wrapper[0]\n}\n",
    )
    .unwrap();

    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.places().iter().any(|(_, place)| {
            matches!(
                place.projections().last(),
                Some(PlaceProjection::CoercedBuiltinIndex { .. })
            )
        })
    }));
}

#[test]
fn lexical_index_requirement_dispatches_a_generic_place() {
    let output = check(
        "func read<C, V>(source: &C, index: usize): &V where (&C[usize]): &V {\n    return &source[index]\n}\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let selection = body
        .places()
        .iter()
        .find_map(|(_, place)| match place.projections().last() {
            Some(PlaceProjection::SelectedIndex { operation, .. }) => Some(operation),
            _ => None,
        })
        .unwrap();

    assert!(matches!(
        selection.dispatch(),
        crate::StaticDispatch::StructuralRequirement(_)
    ));
    assert!(selection.generic_arguments().as_slice().is_empty());
}

#[test]
fn generic_receiver_selects_the_matching_generic_instance_operation() {
    let output = check(
        "struct Buffer<T> { values: [T; 1] }\ninstance Buffer<T> {\n    pub operator (&self[index: usize]): &T {\n        return &self.values[index]\n    }\n}\nfunc invalid<T>(source: &Buffer<T>): &T {\n    return &source[0]\n}\n",
    )
    .unwrap();
    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.places().iter().any(|(_, place)| {
            matches!(
                place.projections().last(),
                Some(PlaceProjection::SelectedIndex { operation, .. })
                    if matches!(operation.dispatch(), crate::StaticDispatch::Direct(_))
                        && operation.generic_arguments().as_slice().len() == 1
            )
        })
    }));
}

#[test]
fn instance_requirements_control_concrete_index_applicability() {
    let available = check(
        "struct Buffer<T> { values: [T; 1] }\ninstance Buffer<T> where copy T {\n    pub operator (&self[index: usize]): &T {\n        return &self.values[index]\n    }\n}\nfunc read(buffer: &Buffer<i32>): i32 {\n    buffer[0]\n}\n",
    )
    .unwrap();
    assert!(available.program().bodies().iter().any(|(_, body)| {
        body.places().iter().any(|(_, place)| {
            matches!(
                place.projections().last(),
                Some(PlaceProjection::SelectedIndex { .. })
            )
        })
    }));

    let unavailable = check(
        "struct Owned { value: i32 }\nstruct Buffer<T> { values: [T; 1] }\ninstance Buffer<T> where copy T {\n    pub operator (&self[index: usize]): &T {\n        return &self.values[index]\n    }\n}\nfunc invalid(buffer: &Buffer<Owned>): void {\n    let _ = &buffer[0]\n    return\n}\n",
    )
    .unwrap_err();
    assert_eq!(unavailable.source_diagnostic().unwrap().code(), "E0388");
}

#[test]
fn concrete_index_requirement_reuses_the_operation_selector() {
    let available = check(
        "struct Inner { values: [i32; 1] }\ninstance Inner {\n    pub operator (&self[index: usize]): &i32 {\n        return &self.values[index]\n    }\n}\nstruct Wrapper<T> { inner: T }\ninstance Wrapper<T> where (&T[usize]): &i32 {\n    pub operator (&self[index: usize]): &i32 {\n        return &self.inner[index]\n    }\n}\nfunc read(wrapper: &Wrapper<Inner>): i32 {\n    wrapper[0]\n}\n",
    )
    .unwrap();
    assert!(available.program().bodies().iter().any(|(_, body)| {
        body.places().iter().any(|(_, place)| {
            matches!(
                place.projections().last(),
                Some(PlaceProjection::SelectedIndex { .. })
            )
        })
    }));

    let unavailable = check(
        "struct Missing {}\nstruct Wrapper<T> { value: i32 }\ninstance Wrapper<T> where (&T[usize]): &i32 {\n    pub operator (&self[index: usize]): &i32 {\n        return &self.value\n    }\n}\nfunc invalid(wrapper: &Wrapper<Missing>): void {\n    let _ = &wrapper[0]\n    return\n}\n",
    )
    .unwrap_err();
    assert_eq!(unavailable.source_diagnostic().unwrap().code(), "E0388");
}

#[test]
fn concrete_coercion_requirement_reuses_the_operation_selector() {
    let available = check(
        "struct Source { text: &str }\ninstance Source {\n    pub coerce &self as &str {\n        return self.text\n    }\n}\nstruct Wrapper<T> { value: i32 }\ninstance Wrapper<T> where &T as &str {\n    pub operator (&self[index: usize]): &i32 {\n        return &self.value\n    }\n}\nfunc read(wrapper: &Wrapper<Source>): i32 {\n    wrapper[0]\n}\n",
    )
    .unwrap();
    assert!(available.program().bodies().iter().any(|(_, body)| {
        body.places().iter().any(|(_, place)| {
            matches!(
                place.projections().last(),
                Some(PlaceProjection::SelectedIndex { .. })
            )
        })
    }));

    let unavailable = check(
        "struct Missing {}\nstruct Wrapper<T> { value: i32 }\ninstance Wrapper<T> where &T as &str {\n    pub operator (&self[index: usize]): &i32 {\n        return &self.value\n    }\n}\nfunc invalid(wrapper: &Wrapper<Missing>): void {\n    let _ = &wrapper[0]\n    return\n}\n",
    )
    .unwrap_err();
    assert_eq!(unavailable.source_diagnostic().unwrap().code(), "E0388");
}

#[test]
fn direct_index_operation_has_priority_over_receiver_coercion() {
    let output = check(
        "struct Wrapper { values: [i32; 1] }\ninstance Wrapper {\n    pub operator (&self[index: usize]): &i32 {\n        return &self.values[index]\n    }\n    pub coerce &self as &[i32; 1] {\n        return &self.values\n    }\n}\nfunc read(wrapper: &Wrapper): i32 {\n    wrapper[0]\n}\n",
    )
    .unwrap();

    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.places().iter().any(|(_, place)| {
            matches!(
                place.projections().last(),
                Some(PlaceProjection::SelectedIndex {
                    receiver_coercion: None,
                    ..
                })
            )
        })
    }));
}

#[test]
fn lower_ranked_coercion_cannot_change_direct_index_context() {
    let output = check(
        "struct Alternate { values: [i32; 1] }\ninstance Alternate {\n    pub operator (&self[index: i32]): &i32 {\n        return &self.values[0]\n    }\n}\nstruct Wrapper { values: [i32; 1]\n    alternate: Alternate\n}\ninstance Wrapper {\n    pub operator (&self[index: usize]): &i32 {\n        return &self.values[index]\n    }\n    pub coerce &self as &Alternate {\n        return &self.alternate\n    }\n}\nfunc read(wrapper: &Wrapper): i32 {\n    wrapper[0]\n}\n",
    )
    .unwrap();
    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(body, _)| {
            matches!(
                output.program().graph().declarations().bodies().get(*body).unwrap().owner(),
                nocter_declarations::BodyOwner::Callable(callable)
                    if output.program().graph().declarations().callables().get(callable).unwrap().kind()
                        == nocter_declarations::CallableKind::Function
            )
        })
        .unwrap();
    let (index, receiver_coercion) = body
        .places()
        .iter()
        .find_map(|(_, place)| match place.projections().last() {
            Some(PlaceProjection::SelectedIndex {
                index,
                receiver_coercion,
                ..
            }) => Some((*index, receiver_coercion)),
            _ => None,
        })
        .unwrap();

    assert!(receiver_coercion.is_none());
    assert_eq!(
        body.nodes().get(index).unwrap().ty(),
        output
            .program()
            .types()
            .builtin(nocter_model::BuiltinType::Usize)
    );
}

#[test]
fn equally_ranked_coercion_paths_are_rejected_as_ambiguous() {
    let error = check(
        "struct Left { values: [i32; 1] }\ninstance Left {\n    pub operator (&self[index: usize]): &i32 {\n        return &self.values[index]\n    }\n}\nstruct Right { values: [i32; 1] }\ninstance Right {\n    pub operator (&self[index: usize]): &i32 {\n        return &self.values[index]\n    }\n}\nstruct Wrapper { left: Left\n    right: Right\n}\ninstance Wrapper {\n    pub coerce &self as &Left {\n        return &self.left\n    }\n    pub coerce &self as &Right {\n        return &self.right\n    }\n}\nfunc invalid(wrapper: &Wrapper): i32 {\n    wrapper[0]\n}\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0388");
}
