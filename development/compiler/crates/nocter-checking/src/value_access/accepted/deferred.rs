//! Non-waiting drives over the original checked bodies and cleanup.
use super::{BodyFailure, Failure, flow};
use crate::value_access::{ExpressionKind, Rule, diagnostics, results};
use flow::{
    Location, Operation,
    resources::{Disposal, Step},
};
use nocter_declaration_lowering::access_declarations::{CodeSource, Context};
use nocter_declarations::{
    CallableExecution,
    value_access::{PayloadSchema, Program},
};
use nocter_model::{AccessCodeId, AccessExpressionId, TypeId, ValueCodeRoot, ValueFunctionOrigin};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
mod elision;
pub(super) use elision::Elisions;

pub(super) fn require_local_activation(
    context: &Context<'_>,
    root: ValueCodeRoot,
) -> Result<(), Failure> {
    let ValueCodeRoot::Declaration(callable) = root else {
        return Ok(());
    };
    let program = context.program();
    let Some(CallableExecution::Deferred { output }) = program.code_execution(callable) else {
        return Ok(());
    };
    let code = program.codes().get(callable).ok_or(INVALID)?;
    // Process driving uses an empty activation and consumes the output immediately.
    // Future construction and general cancellation still need their own checked transitions.
    if !code.parameters().is_empty() || !local_backing(program, output, &mut BTreeSet::new()) {
        return Err(body_failure(callable, modifier(context, callable)));
    }
    Ok(())
}

pub(super) fn qualify(
    program: &Program,
    bodies: &BTreeMap<ValueCodeRoot, Arc<flow::Query>>,
    verified: &results::Verified<'_, '_>,
    elisions: &Elisions,
) -> Result<BTreeSet<AccessCodeId>, Failure> {
    let mut drives = BTreeSet::new();
    let mut checked = BTreeSet::new();
    for (&root, body) in bodies {
        let ValueCodeRoot::Declaration(callable) = root else {
            continue;
        };
        if matches!(
            program.code_execution(callable),
            Some(CallableExecution::Deferred { .. })
        ) {
            drive(program, bodies, verified, elisions, body, &mut checked)?;
            drives.insert(callable);
        }
    }
    Ok(drives)
}

const INVALID: Failure = Failure::Integrity(flow::IntegrityError::InconsistentTypedBody);

fn body_failure(callable: AccessCodeId, failure: BodyFailure) -> Failure {
    Failure::Body { callable, failure }
}

fn modifier(context: &Context<'_>, callable: AccessCodeId) -> BodyFailure {
    let project = || {
        let selected = context.for_code(callable)?;
        let node = selected
            .code_source(callable)
            .and_then(CodeSource::deferred_modifier)
            .ok_or(BodyFailure::InconsistentSyntax)?;
        let rule = Rule::UnqualifiedFuture;
        Ok::<_, BodyFailure>(diagnostics::authored_rejection(
            selected.syntax(),
            nocter_syntax::SyntaxOrigin::Node(node),
            rule,
            rule.diagnostic().1,
            Vec::new(),
        ))
    };
    project().unwrap_or_else(|failure| failure)
}

fn drive(
    program: &Program,
    bodies: &BTreeMap<ValueCodeRoot, Arc<flow::Query>>,
    verified: &results::Verified<'_, '_>,
    elisions: &Elisions,
    body: &flow::Query,
    checked: &mut BTreeSet<AccessCodeId>,
) -> Result<(), Failure> {
    let callable = body.typed().source_owner();
    let mut examined = BTreeSet::new();
    let mut pending = vec![(callable, body)];
    let mut local_data = BTreeSet::new();
    while let Some((owner, body)) = pending.pop() {
        if checked.contains(&owner) || !examined.insert(owner) {
            continue;
        }
        let query = body.typed();
        for (point, block) in body.graph().points().iter() {
            if !body.initialization().reachable(point) {
                continue;
            }
            for (action, step) in block.actions.iter().enumerate() {
                let location = Location { point, action };
                let mut transitions = body.access().resources().at(location).peekable();
                if transitions.peek().is_none() {
                    return Err(INVALID);
                }
                for transition in transitions {
                    for resource in transition.steps() {
                        if let Some(data) = backing_data(resource)
                            && !elisions.contains(query.code(), data)
                            && !local_backing(program, data, &mut local_data)
                        {
                            return Err(reject(
                                verified,
                                body,
                                location,
                                "this activation or cleanup requires non-waiting backing evidence",
                            ));
                        }
                    }
                }
                if let Some(id) = selected(program, elisions, query, &step.operation)
                    .transpose()
                    .map_err(|message| reject(verified, body, location, message))?
                {
                    let target = bodies.get(&id.into()).ok_or_else(|| {
                        reject(
                            verified,
                            body,
                            location,
                            "the selected operation has no checked body for non-waiting drive",
                        )
                    })?;
                    pending.push((id, target.as_ref()));
                }
            }
        }
    }
    // A cycle of local operations, with no source of external progress, can diverge. It cannot
    // wait for external progress. Cache only after its entire reachable graph is examined.
    checked.extend(examined);
    Ok(())
}

// A future operation must opt in after its suspension, activation and effects are qualified.
fn selected(
    program: &Program,
    elisions: &Elisions,
    query: &crate::value_access::Query,
    operation: &Operation,
) -> Option<Result<AccessCodeId, &'static str>> {
    match *operation {
        Operation::CommitCall(call) => Some(
            immediate_body(program, query, call)
                .ok_or("the selected invocation has no verified non-waiting drive evidence"),
        ),
        Operation::DriveFuture { result, .. } => Some(
            elisions
                .producer(query.code(), result)
                .ok_or("await requires qualified future driving and cancellation"),
        ),
        Operation::IndexFailure(_)
        | Operation::BeginEvaluation(_)
        | Operation::EndEvaluation(_)
        | Operation::Evaluate(_)
        | Operation::PrepareOperand { .. }
        | Operation::Condition(_)
        | Operation::BooleanConstant { .. }
        | Operation::Forward { .. }
        | Operation::BooleanAnswer { .. }
        | Operation::PrepareArgument { .. }
        | Operation::ConstructFuture(_)
        | Operation::CancelCall { .. }
        | Operation::Bind { .. }
        | Operation::Assign { .. }
        | Operation::CompoundAssign { .. }
        | Operation::Discard(_)
        | Operation::Drop(_)
        | Operation::StageReturn(_)
        | Operation::ExitScope { .. } => None,
    }
}

fn immediate_body(
    program: &Program,
    query: &crate::value_access::Query,
    call: AccessExpressionId,
) -> Option<AccessCodeId> {
    let ExpressionKind::Call { target, .. } = query.body().expressions().get(call)?.kind else {
        return None;
    };
    match (target, program.function_execution(target)) {
        (ValueFunctionOrigin::Declaration(id), Some(CallableExecution::Immediate)) => Some(id),
        _ => None,
    }
}

fn backing_data(step: &Step) -> Option<TypeId> {
    match step {
        Step::Construct { payload, .. }
        | Step::Capture { payload, .. }
        | Step::Dispose { payload, .. }
        | Step::DisposeArrayRemainder { payload, .. }
        | Step::DisposeArrayElements { payload, .. } => Some(payload.data()),
        Step::ReplaceReferent { retired, .. } => Some(retired.data()),
        Step::AdaptFunction { .. }
        | Step::Transport { .. }
        | Step::Assemble { .. }
        | Step::DriveFuture { .. }
        | Step::Receive { .. }
        | Step::Move { .. }
        | Step::MoveArrayElement { .. }
        | Step::ConsolidateArrayPrefix { .. }
        | Step::ReleaseContainer { .. }
        | Step::Invoke { .. } => None,
    }
}

fn local_backing(program: &Program, data: TypeId, known: &mut BTreeSet<TypeId>) -> bool {
    let mut pending = vec![data];
    let mut found = BTreeSet::new();
    while let Some(data) = pending.pop() {
        if known.contains(&data) || !found.insert(data) {
            continue;
        }
        match program.data_schema(data) {
            Some(PayloadSchema::Optional(payload)) => pending.push(payload),
            Some(PayloadSchema::Aggregate(aggregate)) => {
                pending.extend(aggregate.components().map(|field| field.data));
            }
            Some(PayloadSchema::Array(array)) => {
                if !array.is_empty() {
                    pending.push(array.element());
                }
            }
            Some(PayloadSchema::Builtin(_) | PayloadSchema::Function(_))
                if flow::resources::payload(program, data)
                    .is_ok_and(|payload| payload.disposal() == Disposal::Trivial) => {}
            _ => return false,
        }
    }
    known.extend(found);
    true
}

fn reject(
    verified: &results::Verified<'_, '_>,
    body: &flow::Query,
    location: Location,
    message: &str,
) -> Failure {
    let project = || {
        let binding = body.bind_verified(verified)?;
        let origin = binding.origin(location)?;
        Ok::<_, BodyFailure>(diagnostics::authored_rejection(
            binding.typed().syntax(),
            origin,
            Rule::UnqualifiedFuture,
            message,
            Vec::new(),
        ))
    };
    body_failure(
        body.typed().source_owner(),
        project().unwrap_or_else(|failure| failure),
    )
}
