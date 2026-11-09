//! Reachable result summaries close first; per-call type applications then substitute those summaries.
use super::{
    ApplicationEvidence, Evidence,
    producers::Producers,
    reject,
    relations::{self, Relation, Relations},
};
use crate::value_access::{ExpressionKind, accepted, flow};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::{FunctionWitness, PayloadSchema};
use nocter_model::{AccessCallableId, ValueFunctionOrigin as Origin};
use std::collections::{BTreeMap, BTreeSet};

const INVALID: accepted::Failure =
    accepted::Failure::Integrity(flow::IntegrityError::InconsistentTypedBody);

pub(super) fn solve(
    context: &DeclarationContext<'_>,
    controls: &BTreeMap<AccessCallableId, flow::Control>,
) -> Result<Evidence, accepted::Failure> {
    let mut evidence = Evidence::default();
    let mut equations = Relations::new();
    let mut committed = BTreeSet::new();
    for (&id, control) in controls {
        let report = super::producers::analyze(control).map_err(accepted::Failure::Integrity)?;
        if control
            .typed
            .program()
            .callables()
            .get(id)
            .ok_or(INVALID)?
            .result_contract()
            .is_some()
        {
            evidence.bodies.insert(id, control.typed.body_bytes.clone());
            equations.insert(
                Origin::Result(id),
                Relation::Union(report.returned.into_iter().map(|(w, _)| w).collect()),
            );
        }
        for (application, inputs) in report.applications {
            committed.insert(application);
            equations.insert(
                Origin::Application(application),
                Relation::Apply {
                    callee: application.callee,
                    inputs,
                },
            );
        }
    }
    let solved = relations::solve(&equations).map_err(|()| INVALID)?;
    for (&origin, producers) in &solved {
        let Origin::Result(id) = origin else { continue };
        let query = &controls.get(&id).ok_or(INVALID)?.typed;
        let Some(witness) = single(producers) else {
            let message = if producers.is_empty() {
                "callable result has no established code or input producer; recursive forwarding needs a reachable return anchor".to_owned()
            } else {
                let alternatives = producers
                    .iter()
                    .take(2)
                    .map(|&witness| describe_witness(query.program(), witness))
                    .collect::<Result<Vec<_>, _>>()?;
                format!(
                    "callable returns establish different witnesses: {} versus {}; a single witnessed result cannot merge them",
                    alternatives[0], alternatives[1]
                )
            };
            return Err(reject(context, query, None, &message));
        };
        if let Origin::Parameter(parameter) = witness.origin
            && query
                .program()
                .parameters()
                .get(parameter)
                .is_none_or(|p| p.owner() != id)
        {
            return Err(INVALID);
        }
        evidence.producers.insert(id, witness);
    }
    close_applications(context, controls, &committed, &mut evidence)?;
    prove_equalities(context, controls, &evidence)?;
    Ok(evidence)
}
fn single(producers: &Producers) -> Option<FunctionWitness> {
    (producers.len() == 1).then(|| *producers.first().expect("one witness"))
}
fn close_applications(
    context: &DeclarationContext<'_>,
    controls: &BTreeMap<AccessCallableId, flow::Control>,
    committed: &BTreeSet<nocter_model::ValueApplication>,
    evidence: &mut Evidence,
) -> Result<(), accepted::Failure> {
    let mut equations = evidence
        .producers
        .iter()
        .map(|(&id, &witness)| {
            (
                Origin::Result(id),
                Relation::Union(Producers::from([witness])),
            )
        })
        .collect::<Relations>();
    for control in controls.values() {
        let query = &control.typed;
        for (id, expression) in query.body.expressions.iter() {
            let ExpressionKind::Call {
                target, arguments, ..
            } = &expression.kind
            else {
                continue;
            };
            let Some(PayloadSchema::Function(witness)) = expression
                .ty
                .access()
                .and_then(|a| query.program().data_schema(a.data()))
            else {
                continue;
            };
            let Origin::Application(application) = witness.origin else {
                continue;
            };
            if application.owner != query.callable()
                || application.call != id
                || *target != Origin::Declaration(application.callee)
            {
                return Err(INVALID);
            }
            let inputs = arguments
                .iter()
                .filter_map(|argument| argument.binding.as_ref())
                .map(|binding| (binding.parameter, Producers::from([binding.witness])))
                .collect();
            equations.insert(
                Origin::Application(application),
                Relation::Apply {
                    callee: application.callee,
                    inputs,
                },
            );
        }
    }
    let solved = relations::solve(&equations).map_err(|()| INVALID)?;
    for (origin, producers) in solved {
        let Origin::Application(application) = origin else {
            continue;
        };
        let query = &controls.get(&application.owner).ok_or(INVALID)?.typed;
        if producers.is_empty() && !committed.contains(&application) {
            evidence
                .applications
                .insert(application, ApplicationEvidence::Unproduced);
            continue;
        }
        let witness = single(&producers).ok_or_else(|| reject(context, query, Some(application.call),
            "this callable result application has no single established witness; an unknown-shaped input cannot supply a dependent result witness"))?;
        if let Origin::Parameter(parameter) = witness.origin
            && query
                .program()
                .parameters()
                .get(parameter)
                .is_none_or(|p| p.owner() != application.owner)
        {
            return Err(INVALID);
        }
        evidence
            .applications
            .insert(application, ApplicationEvidence::Witness(witness));
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
            let project = |data| {
                evidence
                    .data(control.typed.program(), data)
                    .map_err(|failure| accepted::Failure::Body {
                        callable: control.typed.callable,
                        failure,
                    })
            };
            let left = project(equality.left)?;
            let right = project(equality.right)?;
            if evidence.unproduced(control.typed.program(), left)
                || evidence.unproduced(control.typed.program(), right)
            {
                // Structural compatibility was checked upstream. No identity equality can be
                // demanded of a value that is never produced. Reachable uses reject separately.
                continue;
            }
            if left != right {
                return Err(reject(
                    context,
                    &control.typed,
                    Some(equality.origin),
                    &format!(
                        "these values establish different callable witnesses: {} versus {}",
                        describe(control.typed.program(), left)?,
                        describe(control.typed.program(), right)?
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn describe(
    program: &nocter_declarations::value_access::Program,
    data: nocter_model::TypeId,
) -> Result<String, accepted::Failure> {
    let Some(PayloadSchema::Function(witness)) = program.data_schema(data) else {
        return Err(INVALID);
    };
    describe_witness(program, witness)
}
fn describe_witness(
    program: &nocter_declarations::value_access::Program,
    witness: FunctionWitness,
) -> Result<String, accepted::Failure> {
    let (kind, name) = match witness.origin {
        Origin::Closure(id) => {
            return Ok(format!(
                "closure {} in `{}`",
                id.slot,
                program
                    .symbols()
                    .spelling(program.callables().get(id.owner).ok_or(INVALID)?.name())
                    .ok_or(INVALID)?
            ));
        }
        Origin::Declaration(id) => ("code", program.callables().get(id).ok_or(INVALID)?.name()),
        Origin::Parameter(id) => ("input", program.parameters().get(id).ok_or(INVALID)?.name()),
        _ => return Err(INVALID),
    };
    Ok(format!(
        "{kind} `{}`",
        program.symbols().spelling(name).ok_or(INVALID)?
    ))
}
