use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::{BuiltinType, TypeKind};

use super::{check_prepared_program, check_prepared_program_recovering};
use crate::test_support::Fixture;
use crate::{
    CheckedOperation, CheckedOutcome, CleanupTarget, CleanupTiming, OutcomeLayer,
    prepare_program_checking,
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

#[test]
fn propagation_and_force_select_the_immediate_operand_layer() {
    let output = check(
        "func optional(input: i32?): i32? { input? }\n\
         func fallible(input: i32!): i32! { move input? }\n\
         func force_optional(input: i32?): i32 { input! }\n\
         func force_fallible(input: i32!): i32 { move input! }\n",
    )
    .unwrap();
    let operations = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Outcome(CheckedOutcome::Propagate { layer, .. }) => {
                Some((false, *layer, node.ty()))
            }
            CheckedOperation::Outcome(CheckedOutcome::Force { layer, .. }) => {
                Some((true, *layer, node.ty()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(operations.len(), 4);
    assert!(
        operations
            .iter()
            .any(|(force, layer, _)| { !force && *layer == OutcomeLayer::Optional })
    );
    assert!(
        operations
            .iter()
            .any(|(force, layer, _)| { !force && *layer == OutcomeLayer::Fallible })
    );
    assert_eq!(operations.iter().filter(|(force, _, _)| *force).count(), 2);
    assert!(
        operations
            .iter()
            .all(|(_, _, ty)| { *ty == output.program().types().builtin(BuiltinType::I32) })
    );
}

#[test]
fn propagation_retains_outer_result_layers_without_reordering() {
    let output = check(
        "func absent(input: i32?): (i32?)! { input? }\n\
         func failed(input: i32!): (i32!)? { move input? }\n",
    )
    .unwrap();
    let outer = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Outcome(CheckedOutcome::Propagate { layer, outer, .. }) => {
                Some((*layer, outer.as_ref()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(outer.contains(&(OutcomeLayer::Optional, &[OutcomeLayer::Fallible][..])));
    assert!(outer.contains(&(OutcomeLayer::Fallible, &[OutcomeLayer::Optional][..])));
}

#[test]
fn propagation_carries_its_payload_context_into_generic_call_inference() {
    check(
        "func produce<T>(): T! { loop {} }\n\
         func forward<T>(): T! { produce()? }\n",
    )
    .unwrap();
}

#[test]
fn propagation_with_both_result_layers_infers_a_statically_shaped_generic_operand() {
    check(
        "func produce<T>(): T? { loop {} }\n\
         func forward(): i32?! { produce()? }\n",
    )
    .unwrap();
}

#[test]
fn move_only_outcome_places_require_move_before_elimination() {
    let implicit =
        check("struct Owned { value: i32 }\nfunc invalid(input: Owned?): Owned? { input? }\n")
            .unwrap_err();
    assert_eq!(implicit.source_diagnostic().unwrap().code(), "E0371");

    let repeated = check(
        "struct Owned { value: i32 }\n\
         func invalid(input: Owned?): Owned? {\n\
             let value = move input?\n\
             let _ = input\n\
             move value\n\
         }\n",
    )
    .unwrap_err();
    assert_eq!(repeated.source_diagnostic().unwrap().code(), "E0378");

    check("struct Owned { value: i32 }\nfunc valid(input: Owned?): Owned? { move input? }\n")
        .unwrap();
}

#[test]
fn catch_and_otherwise_bind_only_the_matching_branch() {
    let output = check(
        "func recover_error(input: i32!): i32 { move input catch failure { 0 } }\n\
         func discard_error(input: i32!): i32 { move input catch _ { 0 } }\n\
         func recover_absence(input: i32?): i32 { input otherwise { 0 } }\n\
         func recover_both(input: i32?!): i32 {\n\
             move input catch _ { none } otherwise { 0 }\n\
         }\n",
    )
    .unwrap();
    let recoveries = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Outcome(CheckedOutcome::Recover { layer, binding, .. }) => {
                Some((*layer, binding.is_some()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(recoveries.len(), 5);
    assert!(recoveries.contains(&(OutcomeLayer::Fallible, true)));
    assert!(recoveries.contains(&(OutcomeLayer::Fallible, false)));
    assert!(recoveries.contains(&(OutcomeLayer::Optional, false)));
}

#[test]
fn mismatched_or_unreturnable_outcome_operations_have_one_rule() {
    for source in [
        "func invalid(input: i32?): i32 { input? }\n",
        "func invalid(input: i32?): i32 { input catch _ { 0 } }\n",
        "func invalid(input: i32!): i32 { input otherwise { 0 } }\n",
        "func invalid(input: i32): i32 { input! }\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0392");
    }
}

#[test]
fn propagation_failure_retains_the_typed_callable_contract_repair() {
    for (source, expected_layer) in [
        (
            "func invalid(input: i32?): i32 { input? }\n",
            OutcomeLayer::Optional,
        ),
        (
            "func invalid(input: i32!): i32 { move input? }\n",
            OutcomeLayer::Fallible,
        ),
    ] {
        let fixture = Fixture::new(source);
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
        let failure = check_prepared_program_recovering(&input, prepared).unwrap_err();
        let recovery = failure.recovery().expect("body recovery");
        let interruption = recovery.interruptions().next().expect("typed interruption");
        let crate::TypedBodyInterruptionKind::OutcomeContract {
            layer,
            proposed_result: _,
        } = interruption.kind()
        else {
            panic!("unexpected interruption: {:?}", interruption.kind());
        };
        assert_eq!(*layer, expected_layer);
        let projection = recovery
            .interrupted_outcome_type(interruption)
            .expect("outcome projection")
            .unwrap();
        match (expected_layer, projection.types().get(projection.root())) {
            (OutcomeLayer::Optional, Some(TypeKind::Optional(payload)))
            | (OutcomeLayer::Fallible, Some(TypeKind::Fallible(payload))) => {
                assert_eq!(*payload, projection.types().builtin(BuiltinType::I32));
            }
            (_, actual) => panic!("unexpected proposed result: {actual:?}"),
        }
    }
}

#[test]
fn propagation_contract_recovery_selects_a_missing_inner_optional_layer() {
    let source = "func invalid(input: i32?): i32! { input? }\n";
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let failure = check_prepared_program_recovering(&input, prepared).unwrap_err();
    let diagnostic = failure.error().source_diagnostic().unwrap();
    assert_eq!(diagnostic.code(), "E0392");
    let recovery = failure.recovery().expect("body recovery");
    let interruption = recovery.interruptions().next().expect("typed interruption");
    let crate::TypedBodyInterruptionKind::OutcomeContract {
        layer,
        proposed_result: _,
    } = interruption.kind()
    else {
        panic!("unexpected interruption: {:?}", interruption.kind());
    };
    assert_eq!(*layer, OutcomeLayer::Optional);
    let projection = recovery
        .interrupted_outcome_type(interruption)
        .expect("outcome projection")
        .unwrap();
    let Some(TypeKind::Fallible(optional)) = projection.types().get(projection.root()) else {
        panic!("expected canonical fallible optional result")
    };
    assert!(matches!(
        projection.types().get(*optional),
        Some(TypeKind::Optional(payload))
            if *payload == projection.types().builtin(BuiltinType::I32)
    ));
}

#[test]
fn propagation_contract_recovery_types_a_generic_operand_by_its_payload() {
    let source = concat!(
        "func produce<T>(): T? { loop {} }\n",
        "func invalid(): i32! { produce()? }\n",
    );
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let failure = check_prepared_program_recovering(&input, prepared).unwrap_err();
    assert_eq!(failure.error().source_diagnostic().unwrap().code(), "E0392");
    let interruption = failure
        .recovery()
        .and_then(|recovery| recovery.interruptions().next())
        .expect("typed outcome interruption");
    assert!(matches!(
        interruption.kind(),
        crate::TypedBodyInterruptionKind::OutcomeContract {
            layer: OutcomeLayer::Optional,
            ..
        }
    ));
}

#[test]
fn recovery_collects_body_interruptions_independently_of_declaration_order() {
    fn layers(source: &str) -> Vec<OutcomeLayer> {
        let fixture = Fixture::new(source);
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
        let failure = check_prepared_program_recovering(&input, prepared).unwrap_err();
        let recovery = failure.recovery().expect("body recovery");
        let interruptions = recovery.interruptions().collect::<Vec<_>>();
        assert_eq!(interruptions.len(), 2);
        assert_eq!(recovery.rejection_diagnostics().count(), 2);
        assert!(
            recovery
                .body_evidence_iter()
                .all(|(_, evidence)| matches!(evidence, crate::BodyEvidence::Rejected(_)))
        );
        for interruption in &interruptions {
            let origin = interruption.origin();
            assert_eq!(
                recovery.interruption_at(origin.source(), origin.span().range().start()),
                Some(*interruption)
            );
        }
        interruptions
            .into_iter()
            .filter_map(|interruption| match interruption.kind() {
                crate::TypedBodyInterruptionKind::OutcomeContract { layer, .. } => Some(*layer),
                _ => None,
            })
            .collect()
    }

    let optional_first = concat!(
        "func optional(input: i32?): i32 { input? }\n",
        "func fallible(input: i32!): i32 { move input? }\n",
    );
    let fallible_first = concat!(
        "func fallible(input: i32!): i32 { move input? }\n",
        "func optional(input: i32?): i32 { input? }\n",
    );

    assert_eq!(
        layers(optional_first),
        vec![OutcomeLayer::Optional, OutcomeLayer::Fallible]
    );
    assert_eq!(
        layers(fallible_first),
        vec![OutcomeLayer::Fallible, OutcomeLayer::Optional]
    );
}

#[test]
fn recovery_retains_independently_successful_typed_bodies() {
    for source in [
        concat!(
            "func invalid(input: i32?): i32 { input? }\n",
            "func valid(): i32 { let retained = 1\nretained }\n",
        ),
        concat!(
            "func valid(): i32 { let retained = 1\nretained }\n",
            "func invalid(input: i32?): i32 { input? }\n",
        ),
    ] {
        let fixture = Fixture::new(source);
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
        let failure = check_prepared_program_recovering(&input, prepared).unwrap_err();
        let recovery = failure.recovery().expect("body recovery");
        let recovered = recovery
            .prepared()
            .graph()
            .declarations()
            .bodies()
            .iter()
            .filter_map(|(body, _)| recovery.typed_body(body))
            .collect::<Vec<_>>();

        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].locals().len(), 1);
        assert_eq!(
            recovered[0].locals().iter().next().unwrap().1.ty(),
            recovery.prepared().types().builtin(BuiltinType::I32)
        );
    }
}

#[test]
fn recovery_joins_fallback_ownership_with_the_success_path() {
    let error = check(
        "struct Owned { value: i32 }\n\
         func invalid(input: i32!, owned: Owned): i32 {\n\
             let value = move input catch _ {\n\
                 let _ = move owned\n\
                 0\n\
             }\n\
             let _ = move owned\n\
             value\n\
         }\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn propagation_edge_cleans_scopes_and_prior_sequence_temporaries() {
    let output = check(
        "struct Owned { value: i32 }\n\
         func make(input: Owned?): [Owned; 2]? {\n\
             let local = Owned { value: 1 }\n\
             [Owned { value: 2 }, move input?]\n\
         }\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let propagation = body
        .nodes()
        .iter()
        .find_map(|(node, checked)| {
            matches!(
                checked.operation(),
                CheckedOperation::Outcome(CheckedOutcome::Propagate { .. })
            )
            .then_some(node)
        })
        .expect("propagation node");
    let schedule = body
        .cleanups()
        .schedule(propagation, CleanupTiming::OnOutcomePropagation)
        .expect("failure edge cleanup");

    assert_eq!(schedule.timing(), CleanupTiming::OnOutcomePropagation);
    assert!(
        schedule
            .actions()
            .iter()
            .any(|action| { matches!(action.target(), CleanupTarget::Value { .. }) })
    );
    assert!(
        schedule
            .actions()
            .iter()
            .any(|action| { matches!(action.target(), CleanupTarget::Path(_)) })
    );
}

#[test]
fn call_argument_propagation_cleans_prior_argument_temporary() {
    let output = check(
        "struct Owned { value: i32 }\n\
         func pair(first: Owned, second: Owned): [Owned; 2] {\n\
             [move first, move second]\n\
         }\n\
         func make(input: Owned?): [Owned; 2]? {\n\
             pair(Owned { value: 1 }, move input?)\n\
         }\n",
    )
    .unwrap();
    let propagation = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| {
            body.nodes().iter().filter_map(move |(node, checked)| {
                matches!(
                    checked.operation(),
                    CheckedOperation::Outcome(CheckedOutcome::Propagate { .. })
                )
                .then_some((body, node))
            })
        })
        .next()
        .expect("propagation node");
    let schedule = propagation
        .0
        .cleanups()
        .schedule(propagation.1, CleanupTiming::OnOutcomePropagation)
        .expect("argument failure cleanup");

    assert_eq!(schedule.timing(), CleanupTiming::OnOutcomePropagation);
    assert!(
        schedule
            .actions()
            .iter()
            .any(|action| { matches!(action.target(), CleanupTarget::Value { .. }) })
    );
}

#[test]
fn propagation_before_later_local_declarations_skips_uninitialized_storage() {
    check(
        "struct Owned { value: i32 }\n\
         func make(input: i32?): i32? {\n\
             let value = input?\n\
             let later = Owned { value: 1 }\n\
             value\
         }\n",
    )
    .unwrap();
}

#[test]
fn owned_callable_is_a_staged_temporary_until_its_arguments_succeed() {
    let output = check(
        "struct Owned { value: i32 }\n\
         func invoke<F>(callback: F, input: Owned?): Owned? where F: func(value: Owned): Owned {\n\
             callback(move input?)\n\
         }\n",
    )
    .unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();
    let propagation = body
        .nodes()
        .iter()
        .find_map(|(node, checked)| {
            matches!(
                checked.operation(),
                CheckedOperation::Outcome(CheckedOutcome::Propagate { .. })
            )
            .then_some(node)
        })
        .expect("propagation node");
    let actions = body
        .cleanups()
        .actions(propagation, CleanupTiming::OnOutcomePropagation)
        .expect("owned callable staging cleanup");

    assert!(actions.iter().any(|action| {
        matches!(action.target(), CleanupTarget::Value { node, .. }
        if matches!(
            body.nodes().get(*node).map(crate::CheckedNode::operation),
            Some(CheckedOperation::Place(_))
        ))
    }));
}

#[test]
fn terminating_recovery_fallback_does_not_invent_unwinding() {
    let output = check(
        "struct Owned { value: i32 }\n\
         func stop(): never { loop {} }\n\
         func recover(input: i32!): i32 {\n\
             move input catch _ {\n\
                 let local = Owned { value: 1 }\n\
                 stop()\n\
             }\n\
         }\n",
    )
    .unwrap();
    let recovery = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| {
            body.nodes().iter().filter_map(move |(_, checked)| {
                let CheckedOperation::Outcome(CheckedOutcome::Recover { fallback, .. }) =
                    checked.operation()
                else {
                    return None;
                };
                Some((body, *fallback))
            })
        })
        .next()
        .expect("recovery node");

    assert!(
        recovery
            .0
            .cleanups()
            .schedule(recovery.1, CleanupTiming::BeforeTransfer)
            .is_none()
    );
}

#[test]
fn force_moves_the_complete_move_only_outcome_before_trapping_or_unwrapping() {
    check("struct Owned { value: i32 }\nfunc take(input: Owned?): Owned { move input! }\n")
        .unwrap();
    let error = check("struct Owned { value: i32 }\nfunc take(input: Owned?): Owned { input! }\n")
        .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0371");
}
