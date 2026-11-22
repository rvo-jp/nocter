//! Non-waiting drives over the original checked bodies and cleanup.
use super::{BodyFailure, Failure, flow};
use crate::value_access::{ExpressionKind, Rule, diagnostics, results};
use flow::{
    Location, Operation,
    resources::{Disposal, Step},
};
use nocter_declaration_lowering::access_declarations::{CodeSource, Context, PrimitiveBindings};
use nocter_declarations::{
    CallableExecution,
    value_access::{PayloadSchema, Program},
};
use nocter_model::{AccessCodeId, AccessExpressionId, TypeId, ValueCodeRoot, ValueFunctionOrigin};
use nocter_runtime_contract::RuntimeAbiIdentity;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
mod local_futures;
pub(super) use local_futures::LocalFutures;

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
    let mut known = BTreeSet::new();
    let cancellation =
        DataUse::backing_or_cleanup(context.target().and_then(RuntimeAbiIdentity::for_target));
    let inputs_are_represented = code.parameters().iter().all(|&id| {
        program.parameters().get(id).is_some_and(|parameter| {
            use nocter_model::ValueInputOperation;
            flow::resources::local_future_input(program, parameter.data()).is_some()
                && match parameter.operation() {
                    ValueInputOperation::Take => {
                        local_data(program, parameter.data(), cancellation, &mut known)
                    }
                    ValueInputOperation::Read => true,
                    ValueInputOperation::Edit => false,
                }
        })
    });
    if !inputs_are_represented || !local_data(program, output, DataUse::Output, &mut known) {
        return Err(body_failure(callable, modifier(context, callable)));
    }
    Ok(())
}

pub(super) fn qualify(
    program: &Program,
    source: &Context<'_>,
    bodies: &BTreeMap<ValueCodeRoot, Arc<flow::Query>>,
    verified: &results::Verified<'_, '_>,
    futures: &LocalFutures,
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
            drive(
                program,
                source,
                bodies,
                verified,
                futures,
                body,
                &mut checked,
            )?;
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
    source: &Context<'_>,
    bodies: &BTreeMap<ValueCodeRoot, Arc<flow::Query>>,
    verified: &results::Verified<'_, '_>,
    futures: &LocalFutures,
    body: &flow::Query,
    checked: &mut BTreeSet<AccessCodeId>,
) -> Result<(), Failure> {
    let callable = body.typed().source_owner();
    let mut examined = BTreeSet::new();
    let mut pending = vec![(callable, body)];
    let mut known_backing = BTreeSet::new();
    let abi = source.target().and_then(RuntimeAbiIdentity::for_target);
    let use_ = DataUse::backing_or_cleanup(abi);
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
                            && !futures.contains(query.code(), data)
                            && !local_data(program, data, use_, &mut known_backing)
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
                if let Some(selection) = selected(
                    program,
                    source.primitives(),
                    abi,
                    futures,
                    query,
                    &step.operation,
                )
                .transpose()
                .map_err(|message| reject(verified, body, location, message))?
                {
                    let (single, alternatives) = match selection {
                        Selection::Immediate(id) => (Some(id), None),
                        Selection::Deferred(producers) => (None, Some(producers)),
                    };
                    for id in single
                        .into_iter()
                        .chain(alternatives.into_iter().flatten().copied())
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
    }
    // A cycle of local operations, with no source of external progress, can diverge. It cannot
    // wait for external progress. Cache only after its entire reachable graph is examined.
    checked.extend(examined);
    Ok(())
}

// A future operation must opt in after its suspension, activation and effects are qualified.
enum Selection<'a> {
    Immediate(AccessCodeId),
    Deferred(&'a BTreeSet<AccessCodeId>),
}

fn selected<'a>(
    program: &Program,
    primitives: &PrimitiveBindings,
    abi: Option<RuntimeAbiIdentity>,
    futures: &'a LocalFutures,
    query: &crate::value_access::Query,
    operation: &Operation,
) -> Option<Result<Selection<'a>, &'static str>> {
    match *operation {
        Operation::CommitCall(call) => {
            let target = immediate_body(program, query, call);
            if target
                .and_then(|id| primitives.role(id))
                .is_some_and(|role| abi.is_some_and(|abi| role.value_drive_backing(abi).is_some()))
            {
                return None;
            }
            Some(
                target
                    .map(Selection::Immediate)
                    .ok_or("the selected invocation has no verified non-waiting drive evidence"),
            )
        }
        Operation::DriveFuture { result, .. } => Some(
            futures
                .producers(query.code(), result)
                .map(Selection::Deferred)
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
        | Step::ConstructActivation { payload, .. }
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

#[derive(Clone, Copy)]
enum DataUse {
    Output,
    BackingOrCleanup { error_release: bool },
}
impl DataUse {
    fn backing_or_cleanup(abi: Option<RuntimeAbiIdentity>) -> Self {
        Self::BackingOrCleanup {
            error_release: abi.is_some_and(RuntimeAbiIdentity::can_release_error_during_drive),
        }
    }

    const fn allows_error(self) -> bool {
        match self {
            Self::Output => true,
            Self::BackingOrCleanup { error_release } => error_release,
        }
    }
}

fn local_data(
    program: &Program,
    data: TypeId,
    use_: DataUse,
    known: &mut BTreeSet<TypeId>,
) -> bool {
    let mut pending = vec![data];
    let mut found = BTreeSet::new();
    while let Some(data) = pending.pop() {
        if known.contains(&data) || !found.insert(data) {
            continue;
        }
        match program.data_schema(data) {
            Some(PayloadSchema::Optional(payload)) => pending.push(payload),
            // Transport alone needs the handle, while reached cleanup also needs release evidence.
            Some(PayloadSchema::Fallible(payload)) if use_.allows_error() => {
                pending.push(payload);
            }
            Some(PayloadSchema::Builtin(nocter_model::BuiltinType::Error))
                if use_.allows_error() => {}
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
