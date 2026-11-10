//! One unmarked snapshot decision, closed before initialization; explicit transfers are frozen earlier.
use super::{
    Failure, Query, Rule,
    ir::{ArgumentAction, BoundaryAction, Expression, ExpressionKind, Referent},
};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::Program;
use nocter_model::{AccessExpressionId, TypeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PlaceSupply {
    Copy,
    Pending,
    MissingCopy,
}

/// A structural result is pending, never provisional Copy. All consumers use this same choice.
pub(super) fn select(program: &Program, data: TypeId) -> Result<PlaceSupply, Failure> {
    let copyable = program
        .data_copyable(data)
        .ok_or(Failure::InconsistentSyntax)?;
    Ok(if super::results::is_result(program, data) {
        PlaceSupply::Pending
    } else if copyable {
        PlaceSupply::Copy
    } else {
        PlaceSupply::MissingCopy
    })
}

/// Resolve this expression's pending plans during the sole identity-projection traversal.
/// Known generic formal transfers remain source-selected transfers.
pub(super) fn resolve(
    context: &DeclarationContext<'_>,
    query: &Query,
    id: AccessExpressionId,
    expression: &mut Expression,
) -> Result<(), Failure> {
    match &mut expression.kind {
        ExpressionKind::Boundary {
            source,
            action: action @ BoundaryAction::PendingValue,
        } => {
            let place = query
                .body
                .expressions
                .get(*source)
                .and_then(|expression| expression.referent(*source))
                .ok_or(Failure::InconsistentSyntax)?;
            if unproduced(query, place)? {
                *action = BoundaryAction::Unproduced;
                return Ok(());
            }
            match resolved(context, query, id, place)? {
                PlaceSupply::Copy => *action = BoundaryAction::CopyPayload,
                PlaceSupply::Pending | PlaceSupply::MissingCopy => unreachable!("resolved above"),
            }
        }
        ExpressionKind::Call { arguments, .. } => {
            for argument in arguments {
                if let ArgumentAction::PendingValue(place) = argument.action {
                    if unproduced(query, place)? {
                        argument.action = ArgumentAction::Unproduced;
                        continue;
                    }
                    argument.action = match resolved(context, query, argument.value, place)? {
                        PlaceSupply::Copy => ArgumentAction::CopyPlace(place),
                        PlaceSupply::Pending | PlaceSupply::MissingCopy => {
                            unreachable!("resolved above")
                        }
                    };
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn unproduced(query: &Query, place: Referent) -> Result<bool, Failure> {
    let data = query
        .body
        .referent_data(place)
        .ok_or(Failure::InconsistentSyntax)?;
    Ok(query
        .result_evidence
        .as_ref()
        .is_some_and(|e| e.unproduced(query.program(), data)))
}

fn resolved(
    context: &DeclarationContext<'_>,
    query: &Query,
    origin: AccessExpressionId,
    place: Referent,
) -> Result<PlaceSupply, Failure> {
    let data = query
        .body
        .referent_data(place)
        .ok_or(Failure::InconsistentSyntax)?;
    match select(query.program(), data)? {
        PlaceSupply::Pending => Err(Failure::InconsistentSyntax),
        PlaceSupply::MissingCopy => Err(super::diagnostics::query_rejection(
            context,
            query,
            Some(origin),
            Rule::MissingTransfer,
            "this unmarked value has no Copy evidence; transfer from an eligible owner requires postfix ^",
        )),
        PlaceSupply::Copy => Ok(PlaceSupply::Copy),
    }
}
