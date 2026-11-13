//! Definition-template and exact-instance admission over the one ordinary checked-body product.
use super::{Checked, DeclarationContext, Failure, Program, flow};
use crate::value_access::{constant, evaluation, results, source_origin};
use std::{collections::BTreeMap, sync::Arc};

mod provenance;
use provenance::Parent;

/// Close the direct callable-formal conditions over exact checked instance metadata before
/// executing any plan. This consumes the ordinary bodies and never manufactures named const.
pub(in crate::value_access) fn check_required_const(
    context: &DeclarationContext<'_>,
    checked: &Checked,
    root: nocter_model::ValueCodeRoot,
    work: &mut nocter_constant_evaluation::state::WorkBudget,
) -> Result<(), Failure> {
    let program = &checked.program;
    let verified = checked
        .result_evidence
        .verify(context)
        .map_err(|_| Failure::Integrity(flow::IntegrityError::InconsistentTypedBody))?;
    let availability = &checked.const_templates;
    let root = program
        .bind_code(root.code(), [])
        .map_err(|_| Failure::Integrity(flow::IntegrityError::InconsistentTypedBody))?;
    let mut traversal = availability.traversal();
    let mut parents: BTreeMap<_, Option<Parent>> = BTreeMap::from([(root.clone(), None)]);
    let mut pending = vec![root];
    while let Some(key) = pending.pop() {
        if !traversal.visit(program, &key).map_err(Failure::Integrity)? {
            continue;
        }
        let requirements = availability.get(key.code()).ok_or(Failure::Integrity(
            flow::IntegrityError::InconsistentTypedBody,
        ))?;
        let query = requirements.body();
        let callable = key.code().source_owner();
        let bound = query
            .bind_verified(&verified)
            .map_err(|failure| Failure::Body { callable, failure })?;
        if let Some(violation) = requirements
            .check(&|code| availability.permitted_template(code), Some(&key))
            .map_err(Failure::Integrity)?
        {
            let mut diagnostic = violation
                .project(&bound)
                .map_err(|failure| Failure::Body { callable, failure })?;
            availability
                .annotate(diagnostic.rule(), &mut diagnostic, &verified)
                .map_err(|failure| Failure::Body { callable, failure })?;
            provenance::annotate(
                availability,
                &verified,
                &parents,
                &key,
                violation,
                &mut diagnostic,
            )?;
            return Err(Failure::Const {
                callable,
                diagnostic: Box::new(diagnostic),
            });
        }
        for call in requirements.completed_calls() {
            let id = call.expression();
            let origin = bound
                .typed()
                .expression_origin(id)
                .ok_or(Failure::Integrity(
                    flow::IntegrityError::InconsistentTypedBody,
                ))?;
            let origin = source_origin(bound.typed().syntax(), origin)
                .map_err(|failure| Failure::Body { callable, failure })?;
            evaluation::condition_step(work, callable, origin)?;
            let selected = call.close(&key).map_err(Failure::Integrity)?;
            if parents.contains_key(&selected) {
                continue;
            }
            parents.insert(
                selected.clone(),
                Some(Parent {
                    key: key.clone(),
                    call: id,
                    origin,
                }),
            );
            pending.push(selected);
        }
    }
    Ok(())
}
pub(super) fn check_const_bodies(
    program: &Program,
    bodies: &BTreeMap<nocter_model::ValueCodeRoot, Arc<flow::Query>>,
    closures: &BTreeMap<nocter_model::ValueClosure, Arc<flow::Query>>,
    verified: &results::Verified<'_, '_>,
) -> Result<Arc<constant::Catalog>, Failure> {
    let availability =
        constant::Catalog::prepare(bodies.values().chain(closures.values()).cloned())
            .map_err(Failure::Integrity)?;
    for (&root, query) in bodies {
        let callable = root.source_owner();
        let evaluatable = match root {
            nocter_model::ValueCodeRoot::Declaration(id) => {
                program
                    .codes()
                    .get(id)
                    .ok_or(Failure::Integrity(
                        flow::IntegrityError::InconsistentTypedBody,
                    ))?
                    .compile_time()
                    == nocter_model::CompileTimeGuarantee::Evaluatable
            }
            nocter_model::ValueCodeRoot::Expression(_) => true,
        };
        if evaluatable
            && let Some(violation) = availability
                .get(root.code())
                .ok_or(Failure::Integrity(
                    flow::IntegrityError::InconsistentTypedBody,
                ))?
                .check(&|code| availability.permitted_template(code), None)
                .map_err(Failure::Integrity)?
        {
            let bound = query
                .bind_verified(verified)
                .map_err(|failure| Failure::Body { callable, failure })?;
            let mut diagnostic = violation
                .project(&bound)
                .map_err(|failure| Failure::Body { callable, failure })?;
            availability
                .annotate(diagnostic.rule(), &mut diagnostic, verified)
                .map_err(|failure| Failure::Body { callable, failure })?;
            return Err(Failure::Const {
                callable,
                diagnostic: Box::new(diagnostic),
            });
        }
    }
    Ok(Arc::new(availability))
}
