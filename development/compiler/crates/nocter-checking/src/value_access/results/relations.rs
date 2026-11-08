//! Finite positive witness equations; application substitutes selected formal identities, never names.
use super::producers::{Inputs, Producers};
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::{ValueFunctionOrigin as Origin, ValueInvocation};
use std::collections::BTreeMap;

pub(super) enum Relation {
    Union(Producers),
    Apply {
        callee: nocter_model::AccessCallableId,
        inputs: Inputs,
    },
}
pub(super) type Relations = BTreeMap<Origin, Relation>;
pub(super) type Solutions = BTreeMap<Origin, Producers>;

fn values(term: FunctionWitness, known: &Solutions) -> Result<Producers, ()> {
    let selected = match term.origin {
        Origin::Declaration(_) | Origin::Parameter(_) => Producers::from([term]),
        _ => known.get(&term.origin).ok_or(())?.clone(),
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
        .map(|&origin| (origin, Producers::new()))
        .collect::<Solutions>();
    loop {
        let mut changed = false;
        for (&origin, relation) in relations {
            let mut next = known[&origin].clone();
            match relation {
                Relation::Union(terms) => {
                    for &term in terms {
                        next.extend(values(term, &known)?);
                    }
                }
                Relation::Apply { callee, inputs } => {
                    for &result in known.get(&Origin::Result(*callee)).ok_or(())? {
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
            if next != known[&origin] {
                known.insert(origin, next);
                changed = true;
            }
        }
        if !changed {
            return Ok(known);
        }
    }
}
