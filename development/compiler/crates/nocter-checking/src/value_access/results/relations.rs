//! Finite positive witness equations; call applications consume actual code return summaries.
use super::producers::{Inputs, Producers};
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::{ValueApplication, ValueCode, ValueFunctionOrigin as Origin, ValueInvocation};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Term {
    Result(ValueCode),
    Application(ValueApplication),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Relation {
    Union(Producers),
    Apply {
        callee: FunctionWitness,
        inputs: Inputs,
    },
}
pub(super) type Relations = BTreeMap<Term, Relation>;
pub(super) type Solutions = BTreeMap<Term, Producers>;

fn values(term: FunctionWitness, known: &Solutions) -> Result<Producers, ()> {
    let selected = match term.origin {
        Origin::Declaration(_) | Origin::Closure(_) | Origin::Parameter(_) => {
            Producers::from([term])
        }
        Origin::Result(code) => known
            .get(&Term::Result(ValueCode::Declaration(code)))
            .ok_or(())?
            .clone(),
        Origin::Application(application) => known
            .get(&Term::Application(application))
            .ok_or(())?
            .clone(),
    };
    selected
        .into_iter()
        .map(|source| adapt(source, term.invocation))
        .collect()
}
fn adapt(source: FunctionWitness, invocation: ValueInvocation) -> Result<FunctionWitness, ()> {
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
                Relation::Apply { callee, inputs } => {
                    for selected in values(*callee, &known)? {
                        let Some(code) = selected.origin.code() else {
                            continue;
                        };
                        for &result in known.get(&Term::Result(code)).ok_or(())? {
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
