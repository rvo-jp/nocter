use super::*;
use nocter_model::{AccessCodeId, AccessParameterId, ArenaBuilder, ValueInvocation};

#[test]
fn substitution_keeps_the_requested_view_without_upgrading_a_restricted_binding() {
    use ValueInvocation::{Edit, Read, Take};
    let parameter = ArenaBuilder::<AccessParameterId, ()>::new().insert(());
    let code = ArenaBuilder::<AccessCodeId, ()>::new().insert(());
    for (actual, requested, allowed) in [
        (Read, Read, true),
        (Read, Edit, true),
        (Read, Take, true),
        (Edit, Read, false),
        (Edit, Edit, true),
        (Edit, Take, true),
        (Take, Read, false),
        (Take, Edit, false),
        (Take, Take, true),
    ] {
        let mut looked_up = 0;
        let selected = FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(parameter.into()),
            invocation: requested,
        };
        let result = selected.substitute(|id| {
            assert_eq!(id, parameter.into());
            looked_up += 1;
            Some(FunctionWitness {
                origin: ValueFunctionOrigin::Declaration(code),
                invocation: actual,
            })
        });
        assert_eq!(looked_up, 1);
        if allowed {
            assert_eq!(
                result,
                Ok(Some(FunctionWitness {
                    origin: ValueFunctionOrigin::Declaration(code),
                    invocation: requested
                }))
            );
        } else {
            assert_eq!(
                result,
                Err(FunctionMismatch::Invocation {
                    actual,
                    expected: requested
                })
            );
        }
    }
}

#[test]
fn substitution_never_searches_for_unbound_or_unclosed_terms_or_recurses_through_formals() {
    let mut parameters = ArenaBuilder::<AccessParameterId, ()>::new();
    let first = parameters.insert(());
    let second = parameters.insert(());
    let code = ArenaBuilder::<AccessCodeId, ()>::new().insert(());
    let named = FunctionWitness {
        origin: ValueFunctionOrigin::Declaration(code),
        invocation: ValueInvocation::Read,
    };
    assert_eq!(
        named.substitute(|_| panic!("a direct code view needs no formal lookup")),
        Ok(Some(named))
    );
    let formal = FunctionWitness {
        origin: ValueFunctionOrigin::Parameter(first.into()),
        ..named
    };
    assert_eq!(formal.substitute(|_| None), Ok(None));
    let symbolic = FunctionWitness {
        origin: ValueFunctionOrigin::Parameter(second.into()),
        ..named
    };
    assert_eq!(
        formal.substitute(|id| {
            assert_eq!(id, first.into());
            Some(symbolic)
        }),
        Ok(Some(symbolic))
    );
    assert_eq!(symbolic.origin.code(), None);
    let result = FunctionWitness {
        origin: ValueFunctionOrigin::Result(code),
        ..named
    };
    assert_eq!(
        result.substitute(|_| panic!("an unresolved result is not a code candidate")),
        Ok(None)
    );
}
