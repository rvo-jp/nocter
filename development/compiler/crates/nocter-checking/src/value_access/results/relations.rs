//! Finite positive witness equations; call applications consume actual code return summaries.
use super::producers::{Inputs, Producers};
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::{ValueAppliedResult, ValueCode, ValueFunctionOrigin as Origin, ValueInvocation};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Term {
    /// One callable leaf of the original named or anonymous body's result descriptor.
    Result { code: ValueCode, ordinal: usize },
    /// The corresponding result leaf of one whole call, not an additional invocation.
    Application(ValueAppliedResult),
}
/// One reached source call's original callee and formal-to-producer alternatives.
/// These sets select neither a correlated joint frame nor a concrete runtime value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvocationRecipe {
    pub callees: Producers,
    pub inputs: Inputs,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Relation {
    Union(Producers),
    Apply(Arc<InvocationRecipe>),
}
pub(super) type Relations = BTreeMap<Term, Relation>;
pub(super) type Solutions = BTreeMap<Term, Producers>;

/// An empty positive solution is not proof of emptiness when an opaque target could return code.
pub(super) fn closed_empty(root: Term, relations: &Relations, known: &Solutions) -> bool {
    let mut pending = vec![root];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(term) = pending.pop() {
        if !seen.insert(term) {
            continue;
        }
        if !known.get(&term).is_some_and(Producers::is_empty) {
            return false;
        }
        match relations.get(&term) {
            Some(Relation::Union(inputs)) => {
                for witness in inputs {
                    let term = match witness.origin {
                        Origin::Result(result) => Term::Result {
                            code: result.owner.into(),
                            ordinal: result.ordinal,
                        },
                        Origin::Application(result) => Term::Application(result),
                        _ => return false,
                    };
                    pending.push(term);
                }
            }
            Some(Relation::Apply(invocation)) => {
                let Term::Application(application) = term else {
                    return false;
                };
                if invocation.callees.is_empty() {
                    return false;
                }
                for callee in &invocation.callees {
                    let Ok(selected) = values(*callee, known) else {
                        return false;
                    };
                    if selected.is_empty() {
                        return false;
                    }
                    for selected in selected {
                        let Some(code) = selected.origin.code() else {
                            return false;
                        };
                        pending.push(Term::Result {
                            code,
                            ordinal: application.ordinal,
                        });
                    }
                }
            }
            None => return false,
        }
    }
    true
}

fn values(term: FunctionWitness, known: &Solutions) -> Result<Producers, ()> {
    let selected = match term.origin {
        Origin::Declaration(_) | Origin::Closure(_) | Origin::Parameter(_) => {
            Producers::from([term])
        }
        Origin::Result(result) => known
            .get(&Term::Result {
                code: ValueCode::Declaration(result.owner),
                ordinal: result.ordinal,
            })
            .ok_or(())?
            .clone(),
        Origin::Application(result) => known.get(&Term::Application(result)).ok_or(())?.clone(),
    };
    selected
        .into_iter()
        .map(|source| adapt(source, term.invocation))
        .collect()
}
pub(super) fn adapt(
    source: FunctionWitness,
    invocation: ValueInvocation,
) -> Result<FunctionWitness, ()> {
    if !source.invocation.can_supply(invocation) {
        return Err(());
    }
    Ok(FunctionWitness {
        origin: source.origin,
        invocation,
    })
}
pub(super) fn solve(relations: &Relations) -> Result<Solutions, ()> {
    let mut known = relations
        .keys()
        .map(|&term| (term, Producers::new()))
        .collect::<Solutions>();
    loop {
        let mut changed = false;
        for (&term, relation) in relations {
            let mut next = known[&term].clone();
            match relation {
                Relation::Union(terms) => {
                    for &term in terms {
                        next.extend(values(term, &known)?);
                    }
                }
                Relation::Apply(invocation) => {
                    let Term::Application(application) = term else {
                        return Err(());
                    };
                    let InvocationRecipe { callees, inputs } = invocation.as_ref();
                    for callee in callees {
                        for selected in values(*callee, &known)? {
                            let Some(code) = selected.origin.code() else {
                                continue;
                            };
                            for &result in known
                                .get(&Term::Result {
                                    code,
                                    ordinal: application.ordinal,
                                })
                                .ok_or(())?
                            {
                                if let Origin::Parameter(parameter) = result.origin {
                                    if let Some(actuals) = inputs.get(&parameter) {
                                        for &actual in actuals {
                                            for witness in values(actual, &known)? {
                                                next.insert(adapt(witness, result.invocation)?);
                                            }
                                        }
                                    }
                                } else {
                                    next.insert(result);
                                }
                            }
                        }
                    }
                }
            }
            if next != known[&term] {
                known.insert(term, next);
                changed = true;
            }
        }
        if !changed {
            return Ok(known);
        }
    }
}

#[cfg(test)]
mod tests;
