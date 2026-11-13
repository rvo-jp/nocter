//! Definition-template and exact-instance admission over the one ordinary checked-body product.
use super::{Checked, DeclarationContext, Failure, Program, flow};
use crate::value_access::{constant, instances, results, source_origin};
use std::collections::BTreeMap;

/// Close the direct callable-formal conditions over exact checked instance metadata before
/// executing any plan. This consumes the ordinary bodies and never manufactures named const.
pub(in crate::value_access) fn check_required_const(
    context: &DeclarationContext<'_>,
    checked: &Checked,
    root: nocter_model::ValueCodeRoot,
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
    let mut parents = BTreeMap::from([(root.clone(), None)]);
    let mut pending = vec![root];
    while let Some(key) = pending.pop() {
        let query = checked.code(key.code()).ok_or(Failure::Integrity(
            flow::IntegrityError::InconsistentTypedBody,
        ))?;
        let callable = key.code().source_owner();
        let bound = query
            .bind_verified(&verified)
            .map_err(|failure| Failure::Body { callable, failure })?;
        if let Some(violation) = constant::check(
            query,
            &|code| availability.permitted_template(code),
            Some(&key),
        )
        .map_err(Failure::Integrity)?
        {
            let mut diagnostic = violation
                .project(&bound)
                .map_err(|failure| Failure::Body { callable, failure })?;
            availability
                .annotate(
                    diagnostic.rule(),
                    &mut diagnostic,
                    &checked.closures,
                    &verified,
                )
                .map_err(|failure| Failure::Body { callable, failure })?;
            let mut path = Vec::new();
            let mut current = &key;
            while let Some(Some((parent, origin))) = parents.get(current) {
                path.push(*origin);
                current = parent;
            }
            for origin in path.into_iter().rev() {
                diagnostic.note_call(origin);
            }
            return Err(Failure::Const {
                callable,
                diagnostic: Box::new(diagnostic),
            });
        }
        for (id, selected) in
            instances::calls(program, &key, query.typed()).map_err(Failure::Integrity)?
        {
            if parents.contains_key(&selected) {
                continue;
            }
            let origin = bound
                .typed()
                .expression_origin(id)
                .ok_or(Failure::Integrity(
                    flow::IntegrityError::InconsistentTypedBody,
                ))?;
            let origin = source_origin(bound.typed().syntax(), origin)
                .map_err(|failure| Failure::Body { callable, failure })?;
            parents.insert(selected.clone(), Some((key.clone(), origin)));
            pending.push(selected);
        }
    }
    Ok(())
}
pub(super) fn check_const_bodies(
    program: &Program,
    bodies: &BTreeMap<nocter_model::ValueCodeRoot, flow::Query>,
    closures: &BTreeMap<nocter_model::ValueClosure, flow::Query>,
    verified: &results::Verified<'_, '_>,
) -> Result<constant::Availability, Failure> {
    let availability = constant::Availability::prepare(closures).map_err(Failure::Integrity)?;
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
            && let Some(violation) =
                constant::check(query, &|code| availability.permitted_template(code), None)
                    .map_err(Failure::Integrity)?
        {
            let bound = query
                .bind_verified(verified)
                .map_err(|failure| Failure::Body { callable, failure })?;
            let mut diagnostic = violation
                .project(&bound)
                .map_err(|failure| Failure::Body { callable, failure })?;
            availability
                .annotate(diagnostic.rule(), &mut diagnostic, closures, verified)
                .map_err(|failure| Failure::Body { callable, failure })?;
            return Err(Failure::Const {
                callable,
                diagnostic: Box::new(diagnostic),
            });
        }
    }
    Ok(availability)
}
