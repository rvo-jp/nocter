use nocter_declaration_lowering::lower_compile_unit_declarations;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    CheckedControl, CheckedOperation, CleanupCondition, CleanupTarget, CleanupTiming, PlaceAccess,
    PlaceProjection, PrimitiveBinary, PrimitiveOperation, prepare_program_checking,
};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

fn assignment_node(body: &crate::CheckedBody) -> nocter_model::BodyNodeId {
    body.nodes()
        .iter()
        .find_map(|(node, checked)| {
            matches!(
                checked.operation(),
                CheckedOperation::Control(CheckedControl::Assign { .. })
            )
            .then_some(node)
        })
        .unwrap()
}

const OWNED: &str = "struct Owned {\n    value: i32\n}\n";

#[test]
fn initialized_var_assignment_drops_the_old_value_before_replacement() {
    let output = check(&format!(
        "{OWNED}func replace(first: Owned, second: Owned): void {{\n    var value = move first\n    value = move second\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let assign = assignment_node(body);
    let [action] = body
        .cleanups()
        .actions(assign, CleanupTiming::BeforeStore)
        .unwrap()
    else {
        panic!("replacement must clean exactly one initialized value");
    };

    assert_eq!(
        body.cleanups()
            .schedule(assign, CleanupTiming::BeforeStore)
            .unwrap()
            .timing(),
        CleanupTiming::BeforeStore
    );
    assert_eq!(action.condition(), CleanupCondition::Always);
    assert!(matches!(action.target(), CleanupTarget::Path(path) if path.fields().is_empty()));
}

#[test]
fn moved_or_dropped_var_can_be_reinitialized_without_old_value_cleanup() {
    for consume in ["let _ = move value", "drop value"] {
        let output = check(&format!(
            "{OWNED}func restore(first: Owned, second: Owned): void {{\n    var value = move first\n    {consume}\n    value = move second\n    let _ = move value\n    return\n}}\n"
        ))
        .unwrap();
        let (_, body) = output.program().bodies().iter().next().unwrap();

        assert!(
            body.cleanups()
                .actions(assignment_node(body), CleanupTiming::BeforeStore)
                .is_none()
        );
    }
}

#[test]
fn assignment_restores_a_moved_named_field_and_complete_parent() {
    let output = check(&format!(
        "{OWNED}struct Pair {{\n    first: Owned\n    second: Owned\n}}\nfunc restore(pair_input: Pair, replacement: Owned): void {{\n    var pair = move pair_input\n    let _ = move pair.first\n    pair.first = move replacement\n    let _ = move pair\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();

    assert!(
        body.cleanups()
            .actions(assignment_node(body), CleanupTiming::BeforeStore)
            .is_none()
    );
}

#[test]
fn maybe_initialized_var_uses_conditional_replacement_cleanup() {
    let output = check(&format!(
        "{OWNED}func restore(condition: bool, first: Owned, second: Owned): void {{\n    var value = move first\n    if condition {{\n        let _ = move value\n    }}\n    value = move second\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let [action] = body
        .cleanups()
        .actions(assignment_node(body), CleanupTiming::BeforeStore)
        .unwrap()
    else {
        panic!("maybe initialized replacement needs one conditional cleanup");
    };

    assert_eq!(action.condition(), CleanupCondition::IfInitialized);
}

#[test]
fn rhs_move_is_observed_before_replacement_cleanup_is_planned() {
    let output = check(&format!(
        "{OWNED}func preserve(input: Owned): void {{\n    var value = move input\n    value = move value\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();

    assert!(
        body.cleanups()
            .actions(assignment_node(body), CleanupTiming::BeforeStore)
            .is_none()
    );
}

#[test]
fn whole_assignment_over_a_partial_parent_cleans_only_remaining_fields() {
    let output = check(&format!(
        "{OWNED}struct Pair {{\n    first: Owned\n    second: Owned\n}}\nfunc replace(input: Pair, replacement: Pair): void {{\n    var pair = move input\n    let _ = move pair.first\n    pair = move replacement\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let [action] = body
        .cleanups()
        .actions(assignment_node(body), CleanupTiming::BeforeStore)
        .unwrap()
    else {
        panic!("only the remaining initialized field should be cleaned");
    };
    let CleanupTarget::Path(path) = action.target() else {
        panic!("partial owned replacement must clean an owned field path");
    };

    assert_eq!(path.fields().len(), 1);
    assert_eq!(action.condition(), CleanupCondition::Always);
}

#[test]
fn maybe_initialized_field_uses_conditional_replacement_cleanup() {
    let output = check(&format!(
        "{OWNED}struct Pair {{\n    first: Owned\n}}\nfunc restore(condition: bool, input: Pair, replacement: Owned): void {{\n    var pair = move input\n    if condition {{\n        let _ = move pair.first\n    }}\n    pair.first = move replacement\n    let _ = move pair\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let [action] = body
        .cleanups()
        .actions(assignment_node(body), CleanupTiming::BeforeStore)
        .unwrap()
    else {
        panic!("maybe initialized field needs one conditional cleanup");
    };

    assert_eq!(action.condition(), CleanupCondition::IfInitialized);
}

#[test]
fn readwrite_borrowed_field_uses_an_exact_place_cleanup() {
    let output = check(&format!(
        "{OWNED}struct Pair {{\n    field: Owned\n}}\nfunc replace(pair: &+Pair, replacement: Owned): void {{\n    pair.field = move replacement\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let assign = assignment_node(body);
    let CheckedOperation::Control(CheckedControl::Assign { target, .. }) =
        body.nodes().get(assign).unwrap().operation()
    else {
        unreachable!();
    };
    let [action] = body
        .cleanups()
        .actions(assign, CleanupTiming::BeforeStore)
        .unwrap()
    else {
        panic!("borrowed replacement needs one place cleanup");
    };

    assert_eq!(
        body.places().get(*target).unwrap().access(),
        PlaceAccess::Borrowed(nocter_model::BorrowCapability::ReadWrite)
    );
    assert!(matches!(action.target(), CleanupTarget::Place { place, .. } if place == target));
}

#[test]
fn immutable_and_non_place_targets_are_rejected_structurally() {
    for source in [
        "func invalid(): void {\n    let value = 1\n    value = 2\n    return\n}\n",
        "func invalid(value: i32): void {\n    value = 2\n    return\n}\n",
        "struct Pair { field: i32 }\nfunc invalid(value: &Pair): void {\n    value.field = 2\n    return\n}\n",
        "func make(): i32 { 1 }\nfunc invalid(): void {\n    make() = 2\n    return\n}\n",
        "func invalid(): void {\n    return\n    let value = 1\n    value = 2\n}\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0384");
    }
}

#[test]
fn field_assignment_cannot_recreate_a_whole_moved_parent() {
    let error = check(&format!(
        "{OWNED}struct Pair {{\n    first: Owned\n}}\nfunc invalid(input: Pair, replacement: Owned): void {{\n    var pair = move input\n    let _ = move pair\n    pair.first = move replacement\n    return\n}}\n"
    ))
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0385");
}

#[test]
fn assignment_provides_its_target_type_as_the_rhs_expectation() {
    let error =
        check("func invalid(): void {\n    var value = 1\n    value = true\n    return\n}\n")
            .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0370");
}

#[test]
fn continuation_newline_does_not_obscure_the_assignment_operator() {
    let output =
        check("func update(): i32 {\n    var value = 1\n    value\n        = 2\n    value\n}\n")
            .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();

    assert!(matches!(
        body.nodes().get(assignment_node(body)).unwrap().operation(),
        CheckedOperation::Control(CheckedControl::Assign { .. })
    ));
}

#[test]
fn compound_assignment_selects_each_numeric_operation_without_desugaring() {
    let output = check(
        "func calculate(input: i32): i32 {\n    var value = input\n    value += 1\n    value -= 1\n    value *= 2\n    value /= 2\n    value %= 2\n    value\n}\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let operations = body
        .nodes()
        .iter()
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Control(CheckedControl::CompoundAssign { operation, .. }) => {
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
    assert!(!body.nodes().iter().any(|(_, node)| matches!(
        node.operation(),
        CheckedOperation::Primitive(PrimitiveOperation::Binary { .. })
    )));
}

#[test]
fn compound_assignment_uses_the_target_type_for_rhs_literals() {
    let output = check(
        "func calculate(input: u64): u64 {\n    var value = input\n    value += 1\n    value\n}\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let (_, value) = body
        .nodes()
        .iter()
        .find_map(|(node, checked)| match checked.operation() {
            CheckedOperation::Control(CheckedControl::CompoundAssign { value, .. }) => {
                Some((node, *value))
            }
            _ => None,
        })
        .unwrap();

    assert_eq!(
        body.nodes().get(value).unwrap().ty(),
        output
            .program()
            .types()
            .builtin(nocter_model::BuiltinType::U64)
    );
}

#[test]
fn compound_assignment_has_one_dedicated_structural_diagnostic() {
    for source in [
        "func invalid(): void {\n    let value = 1\n    value += 1\n    return\n}\n",
        "func invalid(): void {\n    var value = true\n    value += true\n    return\n}\n",
        "func invalid(left: u16, right: u32): void {\n    var value = left\n    value += right\n    return\n}\n",
        "func make(): i32 { 1 }\nfunc invalid(): void {\n    make() += 1\n    return\n}\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0386");
        assert_eq!(
            error.rule(),
            Some(crate::BodyRule::InvalidCompoundAssignment)
        );
    }
}

#[test]
fn compound_assignment_requires_the_complete_target_to_be_initialized() {
    let error = check(&format!(
        "{OWNED}struct Pair {{\n    number: i32\n    owned: Owned\n}}\nfunc invalid(input: Pair): void {{\n    var pair = move input\n    let _ = move pair\n    pair.number += 1\n    return\n}}\n"
    ))
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn compound_assignment_writes_a_readwrite_borrowed_integer_field() {
    let output = check(
        "struct Counter { value: i32 }\nfunc increment(counter: &+Counter): void {\n    counter.value += 1\n    return\n}\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let (_, target) = body
        .nodes()
        .iter()
        .find_map(|(node, checked)| match checked.operation() {
            CheckedOperation::Control(CheckedControl::CompoundAssign { target, .. }) => {
                Some((node, *target))
            }
            _ => None,
        })
        .unwrap();

    assert_eq!(
        body.places().get(target).unwrap().access(),
        PlaceAccess::Borrowed(nocter_model::BorrowCapability::ReadWrite)
    );
}

#[test]
fn compound_assignment_operator_may_follow_its_continuation_newline() {
    let output = check(
        "func increment(input: i32): i32 {\n    var value = input\n    value\n        += 1\n    value\n}\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();

    assert!(body.nodes().iter().any(|(_, node)| matches!(
        node.operation(),
        CheckedOperation::Control(CheckedControl::CompoundAssign { .. })
    )));
}

#[test]
fn fixed_array_assignment_builds_one_index_without_requiring_index_reads() {
    let output = check(
        "func replace(input: [i32; 4], index: usize, replacement: i32): void {\n    var values = input\n    values[index] = replacement\n    return\n}\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let assign = assignment_node(body);
    let CheckedOperation::Control(CheckedControl::Assign { target, .. }) =
        body.nodes().get(assign).unwrap().operation()
    else {
        unreachable!();
    };
    let place = body.places().get(*target).unwrap();

    assert_eq!(place.projections().len(), 1);
    assert!(matches!(
        place.projections()[0],
        PlaceProjection::BuiltinIndex { .. }
    ));
    assert!(
        body.cleanups()
            .actions(assign, CleanupTiming::BeforeStore)
            .is_none()
    );
}

#[test]
fn indexed_move_only_replacement_uses_a_pre_store_place_cleanup() {
    let output = check(&format!(
        "{OWNED}func replace(input: [Owned; 2], index: usize, replacement: Owned): void {{\n    var values = move input\n    values[index] = move replacement\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let assign = assignment_node(body);
    let [action] = body
        .cleanups()
        .actions(assign, CleanupTiming::BeforeStore)
        .unwrap()
    else {
        panic!("indexed replacement must destroy its old move-only element");
    };

    assert_eq!(
        body.cleanups()
            .schedule(assign, CleanupTiming::BeforeStore)
            .unwrap()
            .timing(),
        CleanupTiming::BeforeStore
    );
    assert!(matches!(action.target(), CleanupTarget::Place { .. }));
}

#[test]
fn readwrite_slice_supports_simple_and_compound_index_assignment() {
    let output = check(
        "func update(values: &+[i32], index: usize): void {\n    values[index] = 1\n    values[index] += 2\n    return\n}\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let targets = body
        .nodes()
        .iter()
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Control(
                CheckedControl::Assign { target, .. }
                | CheckedControl::CompoundAssign { target, .. },
            ) => Some(*target),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(targets.len(), 2);
    assert!(targets.iter().all(|target| {
        body.places().get(*target).unwrap().access()
            == PlaceAccess::Borrowed(nocter_model::BorrowCapability::ReadWrite)
    }));
}

#[test]
fn readonly_slice_and_wrong_index_type_are_rejected() {
    let readonly = check(
        "func invalid(values: &[i32], index: usize): void {\n    values[index] = 1\n    return\n}\n",
    )
    .unwrap_err();
    assert_eq!(readonly.source_diagnostic().unwrap().code(), "E0384");

    let wrong_index = check(
        "func invalid(input: [i32; 2], index: i32): void {\n    var values = input\n    values[index] = 1\n    return\n}\n",
    )
    .unwrap_err();
    assert_eq!(wrong_index.source_diagnostic().unwrap().code(), "E0370");
}

#[test]
fn rhs_partial_move_does_not_invalidate_the_disjoint_indexed_base() {
    let output = check(&format!(
        "{OWNED}struct Holder {{\n    values: [Owned; 1]\n    replacement: Owned\n}}\nfunc replace(input: Holder): void {{\n    var holder = move input\n    holder.values[0] = move holder.replacement\n    return\n}}\n"
    ))
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();

    assert!(
        body.cleanups()
            .schedule(assignment_node(body), CleanupTiming::BeforeStore)
            .is_some()
    );
}

#[test]
fn source_defined_readwrite_index_supports_assignment() {
    let output = check(
        "struct Owned { value: i32 }\nstruct Buffer { values: [Owned; 2] }\ninstance Buffer {\n    pub operator (&+self[index: usize]): &+Owned {\n        return &+self.values[index]\n    }\n}\nfunc update(buffer: &+Buffer, replacement: Owned): void {\n    buffer[0] = move replacement\n    return\n}\n",
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
    let assign = assignment_node(body);
    let CheckedOperation::Control(CheckedControl::Assign { target, .. }) =
        body.nodes().get(assign).unwrap().operation()
    else {
        unreachable!();
    };
    let place = body.places().get(*target).unwrap();

    assert!(matches!(
        place.projections().last(),
        Some(PlaceProjection::SelectedIndex { .. })
    ));
    assert!(place.is_writable());
    assert_eq!(
        body.cleanups()
            .schedule(assign, CleanupTiming::BeforeStore)
            .unwrap()
            .timing(),
        CleanupTiming::BeforeStore
    );
}

#[test]
fn readwrite_coercion_can_produce_an_indexed_assignment_place() {
    let output = check(
        "struct Owned { value: i32 }\nstruct Wrapper { values: [Owned; 1] }\ninstance Wrapper {\n    pub coerce &+self as &+[Owned; 1] {\n        return &+self.values\n    }\n}\nfunc update(wrapper: &+Wrapper, replacement: Owned): void {\n    wrapper[0] = move replacement\n    return\n}\n",
    )
    .unwrap();

    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.places().iter().any(|(_, place)| {
            place.is_writable()
                && matches!(
                    place.projections().last(),
                    Some(PlaceProjection::CoercedBuiltinIndex { .. })
                )
        })
    }));
}

#[test]
fn readwrite_index_selection_does_not_make_an_immutable_receiver_writable() {
    let error = check(
        "struct Wrapper { values: [i32; 1] }\ninstance Wrapper {\n    pub coerce &+self as &+[i32; 1] {\n        return &+self.values\n    }\n}\nfunc invalid(wrapper: Wrapper): void {\n    wrapper[0] = 1\n    return\n}\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0384");
}
