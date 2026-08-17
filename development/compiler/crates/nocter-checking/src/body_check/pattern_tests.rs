use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::{BorrowCapability, BuiltinType, TypeKind};
use nocter_source_index::{SemanticEntity, SourceRole};

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    CheckedControl, CheckedOperation, CheckedOutcome, CleanupCondition, CleanupTarget,
    CleanupTiming, PatternSubjectPreparation, prepare_program_checking,
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
fn match_and_if_is_share_one_checked_enum_pattern_plan() {
    let output = check(
        "enum Maybe<T> {\n    empty\n    value(item: T)\n}\n\
         func read(value: Maybe<i32>): i32 {\n\
             let first = match value {\n\
                 Maybe.empty { 0 }\n\
                 Maybe.value(item) { item }\n\
             }\n\
             if value is Maybe.value(item) {\n\
                 item\n\
             } else {\n\
                 first\n\
             }\n\
         }\n",
    )
    .unwrap();
    let patterns = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Control(CheckedControl::Pattern { subject, arms, .. }) => {
                Some((subject.preparation(), arms.len()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        patterns,
        vec![
            (PatternSubjectPreparation::RetainedPlace, 2),
            (PatternSubjectPreparation::RetainedPlace, 1),
        ]
    );
}

#[test]
fn borrowed_patterns_bind_every_named_payload_as_a_borrow() {
    let output = check(
        "struct Owned { value: i32 }\n\
         enum Maybe<T> {\n    empty\n    value(item: T)\n}\n\
         func inspect(value: &Maybe<Owned>): i32 {\n\
             match value {\n\
                 Maybe.empty { 0 }\n\
                 Maybe.value(item) { item.value }\n\
             }\n\
         }\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let payload = body
        .locals()
        .iter()
        .find(|(_, local)| {
            matches!(
                output.program().types().get(local.ty()),
                Some(TypeKind::Borrow {
                    capability: BorrowCapability::Readonly,
                    ..
                })
            )
        })
        .unwrap()
        .1;
    assert!(matches!(
        output.program().types().get(payload.ty()),
        Some(TypeKind::Borrow {
            capability: BorrowCapability::Readonly,
            referent,
        }) if matches!(output.program().types().get(*referent), Some(TypeKind::Nominal { .. }))
    ));
}

#[test]
fn retained_places_reject_only_named_move_only_payloads() {
    let error = check(
        "struct Owned { value: i32 }\n\
         enum Maybe {\n    empty\n    value(item: Owned)\n}\n\
         func invalid(value: Maybe): void {\n\
             match value {\n                 Maybe.empty {}\n                 Maybe.value(item) { let _ = move item }\n             }\n\
             return\n\
         }\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0393");

    check(
        "struct Owned { value: i32 }\n\
         enum Maybe {\n    empty\n    value(item: Owned)\n}\n\
         func inspect(value: Maybe): void {\n\
             match value {\n                 Maybe.empty {}\n                 Maybe.value(_) {}\n             }\n\
             return\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn moved_pattern_subject_is_consumed_and_cannot_be_used_again() {
    let error = check(
        "struct Owned { value: i32 }\n\
         enum Maybe {\n    empty\n    value(item: Owned)\n}\n\
         func invalid(value: Maybe): void {\n\
             match move value {\n                 Maybe.empty {}\n                 Maybe.value(item) { let _ = move item }\n             }\n\
             let _ = move value\n\
             return\n\
         }\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn owned_pattern_residual_records_only_unnamed_move_only_payloads() {
    let output = check(
        "struct Owned { value: i32 }\n\
         enum Pair { values(first: Owned, second: Owned) }\n\
         func consume(first: Owned, second: Owned): void {\n\
             let _ = match Pair.values(move first, move second) {\n\
                 Pair.values(item, _) { move item }\n\
             }\n\
             return\n\
         }\n",
    )
    .unwrap();
    let residuals = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| {
            body.nodes()
                .iter()
                .flat_map(|(node, _)| body.cleanups().schedules(node).into_iter().flatten())
        })
        .flat_map(crate::CleanupSchedule::actions)
        .filter_map(|action| match action.target() {
            CleanupTarget::EnumResidual { payload, .. } => Some(payload.len()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(residuals, vec![1]);
}

#[test]
fn pattern_coverage_qualifier_variant_and_arity_have_closed_rules() {
    for source in [
        "enum State {\n    ready\n    waiting\n}\nfunc invalid(value: State): i32 { match value { State.ready { 1 } } }\n",
        "enum State {\n    ready\n    waiting\n}\nfunc invalid(value: State): i32 {\n    match value {\n        State.ready { 1 }\n        State.ready { 2 }\n        _ { 3 }\n    }\n}\n",
        "enum State {\n    ready\n    waiting\n}\nfunc invalid(value: State): i32 {\n    match value {\n        _ { 1 }\n        State.ready { 2 }\n    }\n}\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0394");
    }

    for source in [
        "enum State { value(item: i32) }\nfunc invalid(value: State): i32 { match value { State.value { 1 } } }\n",
        "enum State { ready }\nenum Other { ready }\nfunc invalid(value: State): i32 { match value { Other.ready { 1 } } }\n",
        "enum State { ready }\nfunc invalid(value: State): i32 { match value { State.missing { 1 } } }\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0393");
    }
}

#[test]
fn exhaustive_fallback_is_checked_but_does_not_create_a_runtime_join() {
    check(
        "struct Owned { value: i32 }\n\
         enum State { ready }\n\
         func retain(value: Owned, state: State): Owned {\n\
             match state {\n\
                 State.ready {}\n\
                 _ { let _ = move value }\n\
             }\n\
             move value\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn pattern_owner_and_variant_tokens_project_to_semantic_declarations() {
    let output = check(
        "enum State { ready }\nfunc inspect(value: State): void {\n    match value {\n        State.ready {}\n    }\n    return\n}\n",
    )
    .unwrap();
    assert!(
        output.program().types().builtin(BuiltinType::Void)
            != output.program().types().builtin(BuiltinType::Never)
    );
    let variants = output.program().graph().declarations().variants();
    let (variant, _) = variants.iter().next().unwrap();
    assert!(
        output
            .source_index()
            .bindings_for(SemanticEntity::Variant(variant))
            .iter()
            .any(|binding| binding.role() == SourceRole::Reference)
    );
}

#[test]
fn readwrite_borrowed_patterns_bind_readwrite_payload_borrows() {
    let output = check(
        "struct Owned { value: i32 }\n\
         enum Maybe {\n    empty\n    value(item: Owned)\n}\n\
         func mutate(value: &+Maybe): void {\n\
             match value {\n\
                 Maybe.empty {}\n\
                 Maybe.value(item) { item.value = 1 }\n\
             }\n\
             return\n\
         }\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    assert!(body.nodes().iter().any(|(_, checked)| matches!(
        checked.operation(),
        CheckedOperation::Control(CheckedControl::Pattern { subject, .. })
            if subject.preparation()
                == PatternSubjectPreparation::Borrowed(BorrowCapability::ReadWrite)
    )));
    assert!(body.locals().iter().any(|(_, local)| matches!(
        output.program().types().get(local.ty()),
        Some(TypeKind::Borrow {
            capability: BorrowCapability::ReadWrite,
            ..
        })
    )));
}

#[test]
fn owned_if_is_without_else_retains_distinct_match_and_nonmatch_residuals() {
    let output = check(
        "struct Owned { value: i32 }\n\
         enum Pair { values(first: Owned, second: Owned) }\n\
         func inspect(first: Owned, second: Owned): void {\n\
             if Pair.values(move first, move second) is Pair.values(item, _) {\n\
                 let _ = move item\n\
             }\n\
             return\n\
         }\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    assert!(body.nodes().iter().any(|(_, checked)| matches!(
        checked.operation(),
        CheckedOperation::Control(CheckedControl::Pattern {
            unmatched: true,
            ..
        })
    )));
    let actions = body
        .nodes()
        .iter()
        .find_map(|(node, _)| {
            body.cleanups()
                .actions(node, CleanupTiming::AtStatementEnd)
                .filter(|actions| actions.len() == 2)
        })
        .unwrap();
    assert_eq!(actions.len(), 2);
    assert!(
        actions
            .iter()
            .all(|action| action.condition() == CleanupCondition::IfInitialized)
    );
    assert!(
        actions
            .iter()
            .any(|action| matches!(action.target(), CleanupTarget::Value { .. }))
    );
    assert!(actions.iter().any(|action| matches!(action.target(), CleanupTarget::EnumResidual { payload, .. } if payload.len() == 1)));
}

#[test]
fn early_return_and_propagation_cleanup_owned_pattern_residuals() {
    let returned = check(
        "struct Owned { value: i32 }\n\
         enum Pair { values(first: Owned, second: Owned) }\n\
         func select(first: Owned, second: Owned): Owned {\n\
             match Pair.values(move first, move second) {\n\
                 Pair.values(item, _) { return move item }\n\
             }\n\
         }\n",
    )
    .unwrap();
    let (_, return_body) = returned.program().bodies().iter().next().unwrap();
    let return_node = return_body
        .nodes()
        .iter()
        .find_map(|(node, checked)| {
            matches!(
                checked.operation(),
                CheckedOperation::Control(CheckedControl::Return(_))
            )
            .then_some(node)
        })
        .unwrap();
    assert!(
        return_body
            .cleanups()
            .actions(return_node, CleanupTiming::BeforeTransfer)
            .unwrap()
            .iter()
            .any(|action| matches!(action.target(), CleanupTarget::EnumResidual { .. }))
    );

    let propagated = check(
        "struct Owned { value: i32 }\n\
         enum Pair { values(first: Owned, second: Owned) }\n\
         func select(first: Owned, second: Owned, input: i32?): i32? {\n\
             match Pair.values(move first, move second) {\n\
                 Pair.values(item, _) {\n\
                     let value = input?\n\
                     let _ = move item\n\
                     value\n\
                 }\n\
             }\n\
         }\n",
    )
    .unwrap();
    let (_, propagation_body) = propagated.program().bodies().iter().next().unwrap();
    let propagation = propagation_body
        .nodes()
        .iter()
        .find_map(|(node, checked)| {
            matches!(
                checked.operation(),
                CheckedOperation::Outcome(CheckedOutcome::Propagate { .. })
            )
            .then_some(node)
        })
        .unwrap();
    assert!(
        propagation_body
            .cleanups()
            .actions(propagation, CleanupTiming::OnOutcomePropagation)
            .unwrap()
            .iter()
            .any(|action| matches!(action.target(), CleanupTarget::EnumResidual { .. }))
    );
}

#[test]
fn owned_fallback_retains_the_complete_active_enum_value() {
    let output = check(
        "struct Owned { value: i32 }\n\
         enum Maybe {\n    empty\n    value(item: Owned)\n}\n\
         func discard(value: Owned): void {\n\
             match Maybe.value(move value) {\n\
                 _ {}\n\
             }\n\
             return\n\
         }\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    assert!(
        body.nodes()
            .iter()
            .flat_map(|(node, _)| body.cleanups().schedules(node).into_iter().flatten())
            .flat_map(crate::CleanupSchedule::actions)
            .any(|action| matches!(action.target(), CleanupTarget::Value { .. }))
    );
}

#[test]
fn pattern_targets_and_branch_results_fail_through_existing_type_rules() {
    let target = check(
        "enum State { ready }\nfunc invalid(value: State?): void {\n    if value is State.ready {}\n    return\n}\n",
    )
    .unwrap_err();
    assert_eq!(target.source_diagnostic().unwrap().code(), "E0393");

    let result = check(
        "enum State {\n    ready\n    waiting\n}\nfunc invalid(value: State): i32 {\n    match value {\n        State.ready { 1 }\n        State.waiting { true }\n    }\n}\n",
    )
    .unwrap_err();
    assert_eq!(result.source_diagnostic().unwrap().code(), "E0370");
}

#[test]
fn type_owned_drop_is_frozen_before_move_only_payload_transfer() {
    let moved = check(
        "struct Owned { value: i32 }\n\
         enum Resource { active(item: Owned) }\n\
         drop Resource(&+self) { return }\n\
         func consume(value: Owned): void {\n\
             let _ = match Resource.active(move value) {\n\
                 Resource.active(item) { move item }\n\
             }\n\
             return\n\
         }\n",
    )
    .unwrap();
    let (_, moved_body) = moved
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| {
            body.nodes().iter().any(|(_, checked)| {
                matches!(
                    checked.operation(),
                    CheckedOperation::Control(CheckedControl::Pattern { .. })
                )
            })
        })
        .unwrap();
    let pattern = moved_body
        .nodes()
        .iter()
        .find_map(|(_, checked)| match checked.operation() {
            CheckedOperation::Control(CheckedControl::Pattern { arms, .. }) => {
                Some(arms[0].pattern())
            }
            _ => None,
        })
        .unwrap();
    assert!(pattern.before_transfer_drop().is_some());
    assert!(
        !moved_body
            .nodes()
            .iter()
            .flat_map(|(node, _)| moved_body.cleanups().schedules(node).into_iter().flatten())
            .flat_map(crate::CleanupSchedule::actions)
            .any(|action| matches!(action.target(), CleanupTarget::EnumResidual { .. }))
    );

    let copied = check(
        "enum Tagged { active(code: i32) }\n\
         drop Tagged(&+self) { return }\n\
         func inspect(): void {\n\
             match Tagged.active(1) {\n\
                 Tagged.active(code) { let _ = code }\n\
             }\n\
             return\n\
         }\n",
    )
    .unwrap();
    let (_, copied_body) = copied
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| {
            body.nodes().iter().any(|(_, checked)| {
                matches!(
                    checked.operation(),
                    CheckedOperation::Control(CheckedControl::Pattern { .. })
                )
            })
        })
        .unwrap();
    let copied_pattern = copied_body
        .nodes()
        .iter()
        .find_map(|(_, checked)| match checked.operation() {
            CheckedOperation::Control(CheckedControl::Pattern { arms, .. }) => {
                Some(arms[0].pattern())
            }
            _ => None,
        })
        .unwrap();
    assert!(copied_pattern.before_transfer_drop().is_none());
    assert!(
        copied_body
            .nodes()
            .iter()
            .flat_map(|(node, _)| copied_body.cleanups().schedules(node).into_iter().flatten())
            .flat_map(crate::CleanupSchedule::actions)
            .any(|action| matches!(action.target(), CleanupTarget::Value { .. }))
    );
}

#[test]
fn pattern_drop_preserves_the_concrete_drop_substitution() {
    let output = check(
        "struct Owned { value: i32 }\n\
         enum Resource<T> { active(item: T) }\n\
         drop Resource<T>(&+self) { return }\n\
         func consume(value: Owned): void {\n\
             let _ = match Resource.active(move value) {\n\
                 Resource.active(item) { move item }\n\
             }\n\
             return\n\
         }\n",
    )
    .unwrap();
    let pattern = output
        .program()
        .bodies()
        .iter()
        .find_map(|(_, body)| {
            body.nodes()
                .iter()
                .find_map(|(_, checked)| match checked.operation() {
                    CheckedOperation::Control(CheckedControl::Pattern { arms, .. }) => {
                        Some(arms[0].pattern())
                    }
                    _ => None,
                })
        })
        .unwrap();
    let selection = pattern.before_transfer_drop().unwrap();
    let drop = output
        .program()
        .graph()
        .declarations()
        .drops()
        .get(selection.declaration())
        .unwrap();
    let parameter = drop.generic_parameters()[0];
    let argument = selection.generic_arguments().get(parameter).unwrap();
    let Some(TypeKind::Nominal { definition, .. }) = output.program().types().get(argument) else {
        panic!("drop argument must retain the concrete nominal type")
    };
    let nominal = output
        .program()
        .graph()
        .declarations()
        .nominal_types()
        .get(*definition)
        .unwrap();

    assert_eq!(
        output.program().graph().symbols().spelling(nominal.name()),
        Some("Owned")
    );
}
