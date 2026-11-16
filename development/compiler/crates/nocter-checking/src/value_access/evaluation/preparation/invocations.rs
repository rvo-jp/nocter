//! Original invocation discovery advances body dependencies before consuming their results.
use super::{CallOrigin, Demand, Failure, Needed, Preparation};
use crate::value_access::evaluation::{condition_failure, discovery};
use crate::value_access::{InstantiationFailure, accepted, instances, source_origin};
use nocter_declarations::value_access::{CodeInstanceKey, Program};
use nocter_model::{ValueCode, ValueCodeRoot};
use std::collections::BTreeMap;

type BodyDependency = (ValueCode, Demand, CallOrigin);

impl Preparation<'_, '_> {
    /// Discover only actual invocation dependencies after ordinary result closure. Metadata
    /// bindings do not demand the bodies of unused callback values or retype a source body.
    pub(super) fn bound_requirements(
        &mut self,
        resolved: &accepted::Resolved,
        root: ValueCodeRoot,
        selected_bodies: &BTreeMap<ValueCode, Needed>,
    ) -> Result<Vec<BodyDependency>, Failure> {
        let program = resolved.program();
        let scope = resolved.scope().map_err(Failure::Integrity)?;
        let controls = resolved.controls().map_err(Failure::Integrity)?;
        let mut discovery = discovery::Discovery::new(&scope);
        discovery
            .insert(
                program,
                program
                    .bind_code(root.code(), [])
                    .map_err(|_| super::INVALID)?,
            )
            .map_err(Failure::Integrity)?;
        let mut dependencies = Vec::new();
        let mut unresolved = None;
        while let Some(key) = discovery.next() {
            let Some(query) = scope.get(&key.code()) else {
                continue;
            };
            let bound = query.bind(&self.context).map_err(|failure| Failure::Body {
                callable: query.code().source_owner(),
                failure,
            })?;
            let sources = query
                .result_evidence
                .as_ref()
                .and_then(|evidence| evidence.value_sources(key.code()))
                .ok_or(super::INVALID)?;
            let mut control = None;
            for (id, expression) in query.body().expressions().iter() {
                if sources.call(id).is_none()
                    || !crate::value_access::instances::is_completed_call(&expression.kind)
                {
                    continue;
                }
                let site = bound.expression_origin(id).ok_or(super::INVALID)?;
                let primary =
                    source_origin(bound.syntax(), site).map_err(|failure| Failure::Body {
                        callable: key.code().source_owner(),
                        failure,
                    })?;
                if control.is_none() {
                    let original = controls.get(&key.code()).ok_or(super::INVALID)?;
                    control = Some(
                        crate::value_access::results::instantiate_graph_control(
                            program,
                            query.result_evidence.as_ref().ok_or(super::INVALID)?,
                            query,
                            &original.graph,
                            &key,
                            &mut self.condition_work,
                        )
                        .map_err(|failure| {
                            condition_failure(failure, key.code().source_owner(), primary)
                        })?,
                    );
                }
                if !control
                    .as_ref()
                    .ok_or(super::INVALID)?
                    .call_reachable(sources, id)
                    .map_err(Failure::Integrity)?
                {
                    continue;
                }
                let Some(selected) = discovery_call(
                    instances::condition_calls(
                        query,
                        &key,
                        id,
                        control.as_ref(),
                        &mut self.condition_work,
                    ),
                    key.code(),
                    primary,
                    &mut unresolved,
                )?
                else {
                    continue;
                };
                let expand = extend_dependencies(
                    program,
                    selected,
                    CallOrigin {
                        parent: key.code(),
                        site,
                    },
                    selected_bodies,
                    &mut discovery,
                    &mut dependencies,
                )?;
                if expand {
                    // Publish the entire original candidate batch before a later call can
                    // require these newly discovered bodies' returned value contexts.
                    return Ok(dependencies);
                }
            }
        }
        match unresolved {
            Some(failure) => Err(failure),
            None => Ok(dependencies),
        }
    }
}

fn discovery_call(
    selected: Result<Option<Box<[CodeInstanceKey]>>, InstantiationFailure>,
    caller: ValueCode,
    primary: nocter_source_index::SourceOrigin,
    unresolved: &mut Option<Failure>,
) -> Result<Option<Box<[CodeInstanceKey]>>, Failure> {
    match selected {
        Ok(Some(frames)) => Ok(Some(frames)),
        Ok(None) => Err(super::INVALID),
        Err(InstantiationFailure::Unresolved) => {
            unresolved.get_or_insert_with(|| {
                condition_failure(
                    InstantiationFailure::Unresolved,
                    caller.source_owner(),
                    primary,
                )
            });
            Ok(None)
        }
        Err(failure) => Err(condition_failure(failure, caller.source_owner(), primary)),
    }
}

fn extend_dependencies(
    program: &Program,
    selected: Box<[CodeInstanceKey]>,
    origin: CallOrigin,
    selected_bodies: &BTreeMap<ValueCode, Needed>,
    discovery: &mut discovery::Discovery,
    dependencies: &mut Vec<BodyDependency>,
) -> Result<bool, Failure> {
    let mut expand = false;
    for selected in selected {
        // Runtime-only values retain their original const violation without demanding a body.
        if let ValueCode::Declaration(code) = selected.code()
            && program
                .codes()
                .get(code)
                .ok_or(super::INVALID)?
                .compile_time()
                != nocter_model::CompileTimeGuarantee::Evaluatable
        {
            continue;
        }
        expand |= selected_bodies
            .get(&selected.code())
            .is_none_or(|needed| needed.demand < Demand::Invoke);
        dependencies.push((selected.code(), Demand::Invoke, origin));
        discovery
            .insert(program, selected)
            .map_err(Failure::Integrity)?;
    }
    Ok(expand)
}
