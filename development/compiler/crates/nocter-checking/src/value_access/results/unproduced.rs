//! Signature-only data must never become a reached payload, prepared input or invocation.
use crate::value_access::{ExpressionKind, Query, flow};
use nocter_model::AccessExpressionId;

pub(in crate::value_access) fn validate(
    query: &Query,
    graph: &flow::Graph,
) -> Result<(), flow::IntegrityError> {
    let Some(evidence) = &query.result_evidence else {
        return Ok(());
    };
    let invalid = flow::IntegrityError::InconsistentTypedBody;
    let check = |id: AccessExpressionId| {
        let expression = query.body.expressions.get(id).ok_or(invalid)?;
        if expression
            .ty
            .access()
            .is_some_and(|access| evidence.unproduced(query.program(), access.data()))
        {
            return Err(invalid);
        }
        Ok(())
    };
    for point in graph.reachable()? {
        let block = graph.points().get(point).ok_or(invalid)?;
        for action in &block.actions {
            match action.operation {
                flow::Operation::Evaluate(id)
                | flow::Operation::Condition(id)
                | flow::Operation::Discard(id)
                | flow::Operation::StageReturn(id) => check(id)?,
                flow::Operation::CommitCall(id) => {
                    check(id)?;
                    let ExpressionKind::Call { target, .. } =
                        query.body.expressions.get(id).ok_or(invalid)?.kind
                    else {
                        return Err(invalid);
                    };
                    if evidence.has_no_callable_value(target) {
                        return Err(invalid);
                    }
                }
                flow::Operation::PrepareArgument { call, argument } => {
                    let ExpressionKind::Call { arguments, .. } =
                        &query.body.expressions.get(call).ok_or(invalid)?.kind
                    else {
                        return Err(invalid);
                    };
                    check(arguments.get(argument).ok_or(invalid)?.value)?;
                }
                flow::Operation::Forward { source, result, .. }
                | flow::Operation::BooleanAnswer { source, result } => {
                    check(source)?;
                    check(result)?;
                }
                flow::Operation::PrepareOperand { value, .. }
                | flow::Operation::Bind { value, .. }
                | flow::Operation::Assign { value, .. }
                | flow::Operation::CompoundAssign { value, .. } => check(value)?,
                // Beginning an interrupted evaluation does not produce its result. Scope exit
                // cleans initialized holders only; a dead local's known type is no owner.
                flow::Operation::IndexFailure(_)
                | flow::Operation::BeginEvaluation(_)
                | flow::Operation::EndEvaluation(_)
                | flow::Operation::CancelCall { .. }
                | flow::Operation::BooleanConstant { .. }
                | flow::Operation::Drop(_)
                | flow::Operation::ExitScope { .. } => {}
            }
        }
    }
    Ok(())
}
