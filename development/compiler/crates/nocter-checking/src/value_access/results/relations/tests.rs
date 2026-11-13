use super::*;
use nocter_model::{AccessCodeId, AccessExpressionId, ArenaBuilder, ValueClosure, ValueParameter};

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
            Term::Result(ValueCode::Declaration(maker)),
            Relation::Union(Producers::from([captured])),
        ),
        (
            Term::Result(ValueCode::Closure(closure)),
            Relation::Union(Producers::new()),
        ),
        (
            Term::Application(make),
            Relation::Apply(Arc::new(InvocationRecipe {
                callees: Producers::from([maker.into()]),
                inputs: Inputs::new(),
            })),
        ),
    ]);
    for (application, actual) in [(first, identity), (second, other)] {
        equations.insert(
            Term::Application(application),
            Relation::Apply(Arc::new(InvocationRecipe {
                callees: Producers::from([FunctionWitness {
                    origin: Origin::Application(make),
                    invocation: ValueInvocation::Read,
                }]),
                inputs: Inputs::from([(parameter, Producers::from([actual.into()]))]),
            })),
        );
    }
    let empty = solve(&equations).unwrap();
    assert!(empty[&Term::Application(first)].is_empty());
    assert!(empty[&Term::Application(second)].is_empty());
    equations.insert(
        Term::Result(ValueCode::Closure(closure)),
        Relation::Union(Producers::from([formal])),
    );
    let known = solve(&equations).unwrap();
    for (application, code) in [(first, identity), (second, other)] {
        assert_eq!(
            known[&Term::Application(application)],
            Producers::from([FunctionWitness {
                origin: Origin::Declaration(code),
                invocation: ValueInvocation::Take,
            }])
        );
    }
}
