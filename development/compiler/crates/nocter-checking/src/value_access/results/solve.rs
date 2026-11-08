//! Monotone producer sets close recursive result equations without choosing a signature candidate.
use super::{Evidence, reject};
use crate::value_access::{accepted, flow};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::{AccessCallableId, AccessExpressionId, ValueFunctionOrigin};
use std::collections::{BTreeMap, BTreeSet};

fn collect(
    context: &DeclarationContext<'_>,
    controls: &BTreeMap<AccessCallableId, flow::Control>,
) -> Result<(Equations, Evidence), accepted::Failure> {
    let mut equations = Equations::new();
    let mut evidence = Evidence::default();
    for (&id, control) in controls {
        if context
            .program()
            .callables()
            .get(id)
            .is_none_or(|c| c.result_contract().is_none())
        {
            continue;
        }
        let terms = super::producers::returns(control).map_err(accepted::Failure::Integrity)?;
        if let Some(&(_, value)) = terms
            .iter()
            .find(|(witness, _)| matches!(witness.origin, ValueFunctionOrigin::Parameter(_)))
        {
            return Err(reject(
                context,
                &control.typed,
                Some(value),
                "an input-dependent callable result needs a witness relation outside this bounded fragment",
            ));
        }
        evidence.bodies.insert(id, control.typed.body_bytes.clone());
        equations.insert(id, terms);
    }
    Ok((equations, evidence))
}

type Equations = BTreeMap<AccessCallableId, Vec<(FunctionWitness, AccessExpressionId)>>;

pub(super) fn solve(
    context: &DeclarationContext<'_>,
    controls: &BTreeMap<AccessCallableId, flow::Control>,
) -> Result<Evidence, accepted::Failure> {
    let (equations, mut evidence) = collect(context, controls)?;
    propagate(context, controls, &equations, &mut evidence)?;
    prove_equalities(context, controls, &evidence)?;
    Ok(evidence)
}
fn propagate(
    context: &DeclarationContext<'_>,
    controls: &BTreeMap<AccessCallableId, flow::Control>,
    equations: &Equations,
    evidence: &mut Evidence,
) -> Result<(), accepted::Failure> {
    let mut possibilities = equations
        .keys()
        .map(|&id| (id, BTreeSet::<FunctionWitness>::new()))
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for (&id, terms) in equations {
            let mut next = possibilities[&id].clone();
            for &(term, _) in terms {
                match term.origin {
                    ValueFunctionOrigin::Declaration(_) => {
                        next.insert(term);
                    }
                    ValueFunctionOrigin::Result(source) => {
                        let incoming =
                            possibilities
                                .get(&source)
                                .ok_or(accepted::Failure::Integrity(
                                    flow::IntegrityError::InconsistentTypedBody,
                                ))?;
                        for producer in incoming {
                            if !producer.invocation.can_supply(term.invocation) {
                                return Err(reject(
                                    context,
                                    &controls[&id].typed,
                                    None,
                                    "the returned witness cannot supply the selected invocation view",
                                ));
                            }
                            next.insert(FunctionWitness {
                                origin: producer.origin,
                                invocation: term.invocation,
                            });
                        }
                    }
                    ValueFunctionOrigin::Parameter(_) => unreachable!("rejected above"),
                }
            }
            if next.len() > 1 {
                return Err(reject(
                    context,
                    &controls[&id].typed,
                    None,
                    "callable returns establish different named witnesses; equal signatures do not erase their identities",
                ));
            }
            if next != possibilities[&id] {
                possibilities.insert(id, next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for (id, producers) in possibilities {
        let Some(producer) = producers.into_iter().next() else {
            return Err(reject(
                context,
                &controls[&id].typed,
                None,
                "callable result has no established named producer; an unanchored recursive result cannot infer one from its signature",
            ));
        };
        evidence.producers.insert(id, producer);
    }
    Ok(())
}
fn prove_equalities(
    context: &DeclarationContext<'_>,
    controls: &BTreeMap<AccessCallableId, flow::Control>,
    evidence: &Evidence,
) -> Result<(), accepted::Failure> {
    for control in controls.values() {
        for equality in &control.typed.equalities {
            let left = evidence
                .data(context.program(), equality.left)
                .map_err(|failure| accepted::Failure::Body {
                    callable: control.typed.callable,
                    failure,
                })?;
            let right = evidence
                .data(context.program(), equality.right)
                .map_err(|failure| accepted::Failure::Body {
                    callable: control.typed.callable,
                    failure,
                })?;
            if left != right {
                return Err(reject(
                    context,
                    &control.typed,
                    Some(equality.origin),
                    "these values establish different callable witnesses after result closure",
                ));
            }
        }
    }
    Ok(())
}
