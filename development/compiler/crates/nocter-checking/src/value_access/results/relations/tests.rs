use super::*;
use nocter_model::{
    AccessCodeId, AccessExpressionId, ArenaBuilder, ValueApplication, ValueCallableResult,
    ValueClosure, ValueParameter,
};

#[test]
fn selected_and_unproduced_evidence_belongs_to_the_exact_result_leaf() {
    use super::super::{ApplicationEvidence, Evidence};
    let mut codes = ArenaBuilder::<AccessCodeId, ()>::new();
    let maker = codes.insert(());
    let witnesses = [codes.insert(()), codes.insert(())].map(FunctionWitness::from);
    let application = ValueApplication {
        owner: maker.into(),
        call: ArenaBuilder::<AccessExpressionId, ()>::new().insert(()),
    };
    let mut evidence = Evidence::default();
    for (ordinal, witness) in witnesses.into_iter().enumerate() {
        evidence.producers.insert(
            ValueCallableResult {
                owner: maker,
                ordinal,
            },
            witness,
        );
        evidence.applications.insert(
            ValueAppliedResult {
                application,
                ordinal,
            },
            ApplicationEvidence::Witness(witness),
        );
    }
    let absent = ValueAppliedResult {
        application,
        ordinal: 2,
    };
    assert!(evidence.selected(Origin::Application(absent)).is_none());
    assert!(!evidence.has_no_callable_value(Origin::Application(absent)));
    evidence
        .applications
        .insert(absent, ApplicationEvidence::Uncommitted);
    assert!(evidence.has_no_callable_value(Origin::Application(absent)));
    assert!(!evidence.uninhabited_origin(Origin::Application(absent)));
    evidence
        .applications
        .insert(absent, ApplicationEvidence::Uninhabited);
    assert!(evidence.has_no_callable_value(Origin::Application(absent)));
    assert!(evidence.uninhabited_origin(Origin::Application(absent)));
    assert!(!evidence.uncommitted_application(application));
    assert!(evidence.selected(Origin::Application(absent)).is_none());
    assert!(
        evidence
            .selected(Origin::Result(ValueCallableResult {
                owner: maker,
                ordinal: 2
            }))
            .is_none()
    );
    let mut expressions = ArenaBuilder::<AccessExpressionId, ()>::new();
    expressions.insert(());
    let other_call = ValueApplication {
        owner: maker.into(),
        call: expressions.insert(()),
    };
    evidence
        .applications
        .insert(other_call.into(), ApplicationEvidence::Uncommitted);
    assert!(evidence.uncommitted_application(other_call));
    evidence
        .applications
        .insert(other_call.into(), ApplicationEvidence::Uninhabited);
    assert!(!evidence.uncommitted_application(other_call));
    for (ordinal, witness) in witnesses.into_iter().enumerate() {
        let result = Origin::Result(ValueCallableResult {
            owner: maker,
            ordinal,
        });
        let applied = Origin::Application(ValueAppliedResult {
            application,
            ordinal,
        });
        assert_eq!(evidence.selected(result), Some(witness));
        assert_eq!(evidence.selected(applied), Some(witness));
        assert!(!evidence.has_no_callable_value(applied));
    }
}

#[test]
fn result_leaves_of_one_call_use_independent_equations_and_never_fall_back_to_the_root() {
    let mut codes = ArenaBuilder::<AccessCodeId, ()>::new();
    let maker = codes.insert(());
    let first = FunctionWitness::from(codes.insert(()));
    let second = FunctionWitness::from(codes.insert(()));
    let application = ValueApplication {
        owner: maker.into(),
        call: ArenaBuilder::<AccessExpressionId, ()>::new().insert(()),
    };
    let invocation = Arc::new(InvocationRecipe {
        callees: Producers::from([maker.into()]),
        inputs: Inputs::new(),
    });
    let mut equations = Relations::new();
    for ordinal in 0..3 {
        equations.insert(
            Term::Result {
                code: maker.into(),
                ordinal,
            },
            Relation::Union(Producers::new()),
        );
        equations.insert(
            Term::Application(ValueAppliedResult {
                application,
                ordinal,
            }),
            Relation::Apply(invocation.clone()),
        );
    }
    let forwarding = FunctionWitness {
        origin: Origin::Result(ValueCallableResult {
            owner: maker,
            ordinal: 1,
        }),
        invocation: ValueInvocation::Read,
    };
    equations.insert(
        Term::Result {
            code: maker.into(),
            ordinal: 0,
        },
        Relation::Union(Producers::from([first])),
    );
    equations.insert(
        Term::Result {
            code: maker.into(),
            ordinal: 1,
        },
        Relation::Union(Producers::from([second, forwarding])),
    );
    let known = solve(&equations).unwrap();
    for (ordinal, expected) in [
        Producers::from([first]),
        Producers::from([second]),
        Producers::new(),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            known[&Term::Application(ValueAppliedResult {
                application,
                ordinal
            })],
            expected
        );
    }
    equations.remove(&Term::Result {
        code: maker.into(),
        ordinal: 1,
    });
    assert!(solve(&equations).is_err());
}

#[test]
fn an_application_relation_cannot_be_attached_to_a_body_result_term() {
    let code = ArenaBuilder::<AccessCodeId, ()>::new().insert(()).into();
    assert!(
        solve(&Relations::from([(
            Term::Result { code, ordinal: 0 },
            Relation::Apply(Arc::new(InvocationRecipe {
                callees: Producers::new(),
                inputs: Inputs::new(),
            })),
        )]))
        .is_err()
    );
}

#[test]
fn empty_input_substitution_needs_actual_producers_and_original_result_equations() {
    let mut codes = ArenaBuilder::<AccessCodeId, ()>::new();
    let absent = codes.insert(());
    let forward = codes.insert(());
    let parameter = ValueParameter::Closure {
        owner: ValueClosure {
            owner: forward.into(),
            slot: 0,
        },
        position: 0,
    };
    let supplied = FunctionWitness {
        origin: Origin::Result(ValueCallableResult {
            owner: absent,
            ordinal: 0,
        }),
        invocation: ValueInvocation::Read,
    };
    let absent_result = Term::Result {
        code: absent.into(),
        ordinal: 0,
    };
    let formal_result = Term::Result {
        code: forward.into(),
        ordinal: 0,
    };
    let application = Term::Application(
        ValueApplication {
            owner: forward.into(),
            call: ArenaBuilder::<AccessExpressionId, ()>::new().insert(()),
        }
        .into(),
    );
    let originals = Relations::from([
        (absent_result, Relation::Union(Producers::new())),
        (
            formal_result,
            Relation::Union(Producers::from([FunctionWitness {
                origin: Origin::Parameter(parameter),
                invocation: ValueInvocation::Read,
            }])),
        ),
    ]);
    for inputs in [
        Inputs::from([(parameter, Producers::from([supplied]).into())]),
        Inputs::from([(parameter, Producers::new().into())]),
        Inputs::new(),
    ] {
        let has_actual = inputs
            .get(&parameter)
            .is_some_and(|values| !values.producers().is_empty());
        let mut applications = originals.clone();
        applications.insert(
            application,
            Relation::Apply(Arc::new(InvocationRecipe {
                callees: Producers::from([forward.into()]),
                inputs,
            })),
        );
        let known = solve(&applications).unwrap();
        assert!(known[&application].is_empty());
        assert!(!known[&formal_result].is_empty());
        assert_eq!(
            closed_empty_application(application, &originals, &applications, &known),
            has_actual
        );
        let mut missing = originals.clone();
        missing.remove(&absent_result);
        assert!(!closed_empty_application(
            application,
            &missing,
            &applications,
            &known
        ));
    }
}

#[test]
fn closed_empty_proof_requires_known_call_targets_and_complete_equations() {
    let mut codes = ArenaBuilder::<AccessCodeId, ()>::new();
    let maker = codes.insert(());
    let owner = codes.insert(());
    let application = ValueApplication {
        owner: owner.into(),
        call: ArenaBuilder::<AccessExpressionId, ()>::new().insert(()),
    };
    let result = Term::Result {
        code: maker.into(),
        ordinal: 0,
    };
    let applied = Term::Application(application.into());
    let mut equations = Relations::from([
        (result, Relation::Union(Producers::new())),
        (
            applied,
            Relation::Apply(Arc::new(InvocationRecipe {
                callees: Producers::from([maker.into()]),
                inputs: Inputs::new(),
            })),
        ),
    ]);
    let known = solve(&equations).unwrap();
    assert!(closed_empty(result, &equations, &known));
    assert!(closed_empty(applied, &equations, &known));
    equations.remove(&result);
    assert!(!closed_empty(applied, &equations, &known));
    let parameter = ValueParameter::Closure {
        owner: ValueClosure {
            owner: owner.into(),
            slot: 0,
        },
        position: 0,
    };
    equations.insert(
        applied,
        Relation::Apply(Arc::new(InvocationRecipe {
            callees: Producers::from([FunctionWitness {
                origin: Origin::Parameter(parameter),
                invocation: ValueInvocation::Read,
            }]),
            inputs: Inputs::new(),
        })),
    );
    let known = solve(&equations).unwrap();
    assert!(known[&applied].is_empty());
    assert!(!closed_empty(applied, &equations, &known));
}

#[test]
fn returned_callees_and_input_results_use_real_return_summaries_without_signature_anchors() {
    let mut codes = ArenaBuilder::<AccessCodeId, ()>::new();
    let maker = codes.insert(());
    let identity = codes.insert(());
    let other = codes.insert(());
    let closure = ValueClosure {
        owner: maker.into(),
        slot: 0,
    };
    let parameter = ValueParameter::Closure {
        owner: closure,
        position: 0,
    };
    let mut calls = ArenaBuilder::<AccessExpressionId, ()>::new();
    let make = ValueApplication {
        owner: ValueCode::Declaration(maker),
        call: calls.insert(()),
    };
    let first = ValueApplication {
        owner: make.owner,
        call: calls.insert(()),
    };
    let second = ValueApplication {
        owner: make.owner,
        call: calls.insert(()),
    };
    let captured = FunctionWitness {
        origin: Origin::Closure(closure),
        invocation: ValueInvocation::Read,
    };
    let formal = FunctionWitness {
        origin: Origin::Parameter(parameter),
        invocation: ValueInvocation::Take,
    };
    let mut equations = Relations::from([
        (
            Term::Result {
                code: ValueCode::Declaration(maker),
                ordinal: 0,
            },
            Relation::Union(Producers::from([captured])),
        ),
        (
            Term::Result {
                code: ValueCode::Closure(closure),
                ordinal: 0,
            },
            Relation::Union(Producers::new()),
        ),
        (
            Term::Application(make.into()),
            Relation::Apply(Arc::new(InvocationRecipe {
                callees: Producers::from([maker.into()]),
                inputs: Inputs::new(),
            })),
        ),
    ]);
    for (application, actual) in [(first, identity), (second, other)] {
        equations.insert(
            Term::Application(application.into()),
            Relation::Apply(Arc::new(InvocationRecipe {
                callees: Producers::from([FunctionWitness {
                    origin: Origin::Application(make.into()),
                    invocation: ValueInvocation::Read,
                }]),
                inputs: Inputs::from([(parameter, Producers::from([actual.into()]).into())]),
            })),
        );
    }
    let empty = solve(&equations).unwrap();
    assert!(empty[&Term::Application(first.into())].is_empty());
    assert!(empty[&Term::Application(second.into())].is_empty());
    equations.insert(
        Term::Result {
            code: ValueCode::Closure(closure),
            ordinal: 0,
        },
        Relation::Union(Producers::from([formal])),
    );
    let known = solve(&equations).unwrap();
    for (application, code) in [(first, identity), (second, other)] {
        assert_eq!(
            known[&Term::Application(application.into())],
            Producers::from([FunctionWitness {
                origin: Origin::Declaration(code),
                invocation: ValueInvocation::Take,
            }])
        );
    }
}
