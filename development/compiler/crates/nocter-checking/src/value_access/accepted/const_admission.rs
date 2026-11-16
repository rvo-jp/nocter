//! Definition-template and exact-instance admission over the one ordinary checked-body product.
use super::{Checked, DeclarationContext, Failure, Program, flow};
use crate::value_access::{constant, evaluation, results, source_origin};
use std::{collections::BTreeMap, sync::Arc};

mod provenance;
const INVALID: Failure = Failure::Integrity(flow::IntegrityError::InconsistentTypedBody);
use nocter_declarations::value_access::CodeInstanceKey;
use provenance::Parent;

pub(in crate::value_access) type ConstConditions = BTreeMap<
    nocter_declarations::value_access::CodeInstanceKey,
    Box<[constant::InvocationCondition]>,
>;
/// A successful required-root proof tied to the exact immutable checked product.
/// Construction remains with this admission routine; metadata or a body cannot substitute.
pub(in crate::value_access) struct ConstAdmission<'a> {
    checked: &'a Checked,
    root: nocter_model::ValueCodeRoot,
    conditions: ConstConditions,
}
impl<'a> ConstAdmission<'a> {
    pub(in crate::value_access) fn into_parts(
        self,
    ) -> (&'a Checked, nocter_model::ValueCodeRoot, ConstConditions) {
        let Self {
            checked,
            root,
            conditions,
        } = self;
        (checked, root, conditions)
    }
}

/// Close the direct callable-formal conditions over exact checked instance metadata before
/// executing any plan. This consumes the ordinary bodies and never manufactures named const.
pub(in crate::value_access) fn check_required_const<'a>(
    context: &DeclarationContext<'_>,
    checked: &'a Checked,
    root: nocter_model::ValueCodeRoot,
    work: &mut nocter_constant_evaluation::state::WorkBudget,
) -> Result<ConstAdmission<'a>, Failure> {
    let mut admitted = ConstAdmission {
        checked,
        root,
        conditions: BTreeMap::new(),
    };
    let program = &checked.program;
    let verified = checked
        .result_evidence
        .verify(context)
        .map_err(|_| INVALID)?;
    let availability = &checked.const_templates;
    let root = program.bind_code(root.code(), []).map_err(|_| INVALID)?;
    if availability.prove_closed_root(root.code(), &verified, work)? {
        return Ok(admitted);
    }
    admitted.conditions = check_instances(program, availability, &verified, root, work)?;
    Ok(admitted)
}

/// Shared actual-source admission; declaration checks do not manufacture a Checked product.
fn check_instances(
    program: &Program,
    availability: &constant::Catalog,
    verified: &results::Verified<'_, '_>,
    root: CodeInstanceKey,
    work: &mut nocter_constant_evaluation::state::WorkBudget,
) -> Result<ConstConditions, Failure> {
    let mut conditions_by_frame = BTreeMap::new();
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
            .bind_verified(verified)
            .map_err(|failure| Failure::Body { callable, failure })?;
        let (calls, control, conditions) = match requirements
            .check_bound(&|code| availability.permitted_template(code), &key, work)
            .map_err(|error| bound_failure(error, &bound, callable))?
        {
            constant::BoundCheck::Calls {
                calls,
                control,
                conditions,
            } => (calls, control, conditions),
            constant::BoundCheck::Violation(violation) => {
                let diagnostic =
                    bound_violation(availability, verified, &parents, &key, &bound, violation)?;
                return Err(Failure::Const {
                    callable,
                    diagnostic: Box::new(diagnostic),
                });
            }
        };
        for call in requirements.completed_calls() {
            let id = call.expression();
            if conditions.contains_key(&id) {
                continue;
            }
            let sources = verified
                .evidence
                .value_sources(key.code())
                .ok_or(Failure::Integrity(
                    flow::IntegrityError::InconsistentTypedBody,
                ))?;
            if !control
                .call_reachable(sources, id)
                .map_err(Failure::Integrity)?
            {
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
            let selected = calls.get(&id).ok_or(Failure::Integrity(
                flow::IntegrityError::InconsistentTypedBody,
            ))?;
            for selected in selected {
                if parents.contains_key(selected) {
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
                pending.push(selected.clone());
            }
        }
        if !conditions.is_empty() {
            conditions_by_frame.insert(key, conditions.into_values().collect());
        }
    }
    Ok(conditions_by_frame)
}
fn bound_violation(
    availability: &constant::Catalog,
    verified: &results::Verified<'_, '_>,
    parents: &BTreeMap<CodeInstanceKey, Option<Parent>>,
    key: &CodeInstanceKey,
    bound: &flow::Context<'_, '_>,
    violation: constant::Violation,
) -> Result<constant::Diagnostic, Failure> {
    let mut diagnostic = violation.project(bound).map_err(|failure| Failure::Body {
        callable: key.code().source_owner(),
        failure,
    })?;
    availability
        .annotate(diagnostic.rule(), &mut diagnostic, verified)
        .map_err(|failure| Failure::Body {
            callable: key.code().source_owner(),
            failure,
        })?;
    provenance::annotate(
        availability,
        verified,
        parents,
        key,
        violation,
        &mut diagnostic,
    )?;
    Ok(diagnostic)
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
    let mut const_roots = Vec::new();
    let mut definition_work = nocter_constant_evaluation::state::WorkBudget::new(
        nocter_constant_evaluation::state::Limits::default().steps(),
    );
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
        if evaluatable {
            const_roots.push(root);
        }
        if evaluatable
            && let Some(violation) = availability
                .get(root.code())
                .ok_or(Failure::Integrity(
                    flow::IntegrityError::InconsistentTypedBody,
                ))?
                .check(&|code| availability.permitted_template(code))
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
    for root in const_roots {
        check_definition_context(program, &availability, verified, root, &mut definition_work)?;
        if let Some((callable, diagnostic)) =
            availability.closed_violation(root.code(), verified)?
        {
            return Err(Failure::Const {
                callable,
                diagnostic: Box::new(diagnostic),
            });
        }
    }
    Ok(Arc::new(availability))
}

fn check_definition_context(
    program: &Program,
    availability: &constant::Catalog,
    verified: &results::Verified<'_, '_>,
    root: nocter_model::ValueCodeRoot,
    definition_work: &mut nocter_constant_evaluation::state::WorkBudget,
) -> Result<(), Failure> {
    let item = availability.get(root.code()).ok_or(INVALID)?;
    let contextual = availability.contextual_call(root.code());
    let declared_function = matches!(root, nocter_model::ValueCodeRoot::Declaration(id)
        if program.codes().get(id).is_some_and(|code| !code.kind().is_initializer()));
    if declared_function && (item.needs_value_context() || contextual.is_some()) {
        let frame = program.bind_code(root.code(), []).ok().filter(|frame| {
            root.code().origin().is_none_or(|origin| {
                program
                    .compound_input_contracts(nocter_declarations::value_access::FunctionWitness {
                        origin,
                        invocation: nocter_model::ValueInvocation::Read,
                    })
                    .is_ok_and(|inputs| {
                        inputs
                            .keys()
                            .all(|&index| frame.input_data(index).is_some())
                    })
            })
        });
        match frame {
            Some(frame) => {
                check_instances(program, availability, verified, frame, definition_work)?;
            }
            None => {
                if let Some(violation) = contextual {
                    // General symbolic input/environment contexts remain outside this source scope.
                    let callable = root.source_owner();
                    let bound = item
                        .body()
                        .bind_verified(verified)
                        .map_err(|failure| Failure::Body { callable, failure })?;
                    let diagnostic = violation
                        .project(&bound)
                        .map_err(|failure| Failure::Body { callable, failure })?;
                    return Err(Failure::Const {
                        callable,
                        diagnostic: Box::new(diagnostic),
                    });
                }
            }
        }
    }
    Ok(())
}

fn bound_failure(
    error: constant::BoundCheckFailure,
    bound: &flow::Context<'_, '_>,
    callable: nocter_model::AccessCodeId,
) -> Failure {
    let (origin, failure) = match error {
        constant::BoundCheckFailure::Integrity(error) => return Failure::Integrity(error),
        constant::BoundCheckFailure::Control(failure) => match bound.origin(flow::Location {
            point: bound.query().graph().entry(),
            action: 0,
        }) {
            Ok(origin) => (origin, failure),
            Err(failure) => return Failure::Body { callable, failure },
        },
        constant::BoundCheckFailure::Invocation {
            expression,
            failure,
        } => {
            let Some(origin) = bound.typed().expression_origin(expression) else {
                return INVALID;
            };
            (origin, failure)
        }
        constant::BoundCheckFailure::Data { at, failure } => match bound.origin(at) {
            Ok(origin) => (origin, failure),
            Err(failure) => return Failure::Body { callable, failure },
        },
    };
    match source_origin(bound.typed().syntax(), origin) {
        Ok(origin) => evaluation::condition_failure(failure, callable, origin),
        Err(failure) => Failure::Body { callable, failure },
    }
}
