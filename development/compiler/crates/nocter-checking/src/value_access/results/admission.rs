//! Report known, unrealized representation changes before ownership consumes the typed graph.
use super::Evidence;
use crate::value_access::{BoundaryAction, ExpressionKind, Failure, Query, Rule, flow};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_model::{AccessExpressionId, TypeId};

pub(super) fn validate(
    context: &DeclarationContext<'_>,
    control: &flow::Control,
    evidence: &Evidence,
) -> Result<(), Failure> {
    let query = &control.typed;
    let program = query.program();
    for point in control
        .graph
        .reachable()
        .map_err(|_| Failure::InconsistentSyntax)?
    {
        let block = control
            .graph
            .points()
            .get(point)
            .ok_or(Failure::InconsistentSyntax)?;
        for action in &block.actions {
            let Some((source, actual, target)) = boundary(query, &action.operation)? else {
                continue;
            };
            if !evidence.matches_payload_metadata(program, actual, target)
                && program.matches_guarded_transport_metadata(actual, target, |witness| {
                    evidence.uninhabited_origin(witness.origin)
                })
            {
                return Err(crate::value_access::diagnostics::query_rejection(
                    context,
                    query,
                    Some(source),
                    Rule::UnsupportedData,
                    "this outcome needs guarded representation transport, which is not implemented at this boundary",
                ));
            }
        }
    }
    Ok(())
}

fn boundary(
    query: &Query,
    operation: &flow::Operation,
) -> Result<Option<(AccessExpressionId, TypeId, TypeId)>, Failure> {
    let data = |id| {
        Ok::<_, Failure>(
            query
                .body()
                .expressions()
                .get(id)
                .ok_or(Failure::InconsistentSyntax)?
                .ty
                .access()
                .map(nocter_model::ValueAccess::data),
        )
    };
    let (source, actual, target) = match *operation {
        flow::Operation::Evaluate(result) => {
            let ExpressionKind::Boundary {
                source,
                action: BoundaryAction::TakeValue,
            } = query
                .body()
                .expressions()
                .get(result)
                .ok_or(Failure::InconsistentSyntax)?
                .kind
            else {
                return Ok(None);
            };
            (source, data(source)?, data(result)?)
        }
        flow::Operation::Forward { source, result, .. } => (source, data(source)?, data(result)?),
        flow::Operation::StageReturn(source) => (source, data(source)?, Some(query.result_data())),
        _ => return Ok(None),
    };
    Ok(actual
        .zip(target)
        .map(|(actual, target)| (source, actual, target)))
}
