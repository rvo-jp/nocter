//! One Copy-or-transfer decision for a selected place, closed before initialization.
use super::{
    Failure, Query, Rule,
    ir::{ArgumentAction, BoundaryAction, Expression, ExpressionKind},
};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::Program;
use nocter_model::{AccessExpressionId, AccessPlaceId, TypeId, ValueAccessMode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PlaceSupply {
    Copy,
    Transfer,
    Pending,
    MissingOwner,
}

/// A structural result is pending, never provisional Copy. All consumers use this same choice.
pub(super) fn select(
    program: &Program,
    data: TypeId,
    source: ValueAccessMode,
) -> Result<PlaceSupply, Failure> {
    let copyable = program
        .data_copyable(data)
        .ok_or(Failure::InconsistentSyntax)?;
    Ok(if super::results::is_result(program, data) {
        PlaceSupply::Pending
    } else if copyable {
        PlaceSupply::Copy
    } else if source == ValueAccessMode::Own {
        PlaceSupply::Transfer
    } else {
        PlaceSupply::MissingOwner
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
            let ExpressionKind::Place(place) = query
                .body
                .expressions
                .get(*source)
                .ok_or(Failure::InconsistentSyntax)?
                .kind
            else {
                return Err(Failure::InconsistentSyntax);
            };
            if unproduced(query, place)? {
                *action = BoundaryAction::Unproduced;
                return Ok(());
            }
            match resolved(context, query, id, place)? {
                PlaceSupply::Copy => *action = BoundaryAction::CopyPayload,
                PlaceSupply::Transfer => expression.kind = ExpressionKind::Move(place),
                PlaceSupply::Pending | PlaceSupply::MissingOwner => unreachable!("resolved above"),
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
                        PlaceSupply::Transfer => ArgumentAction::ReserveTransfer(place),
                        PlaceSupply::Pending | PlaceSupply::MissingOwner => {
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

fn unproduced(query: &Query, place: AccessPlaceId) -> Result<bool, Failure> {
    let data = query
        .body
        .places
        .get(place)
        .ok_or(Failure::InconsistentSyntax)?
        .data;
    Ok(query
        .result_evidence
        .as_ref()
        .is_some_and(|e| e.unproduced(query.program(), data)))
}

fn resolved(
    context: &DeclarationContext<'_>,
    query: &Query,
    origin: AccessExpressionId,
    place: AccessPlaceId,
) -> Result<PlaceSupply, Failure> {
    let place = query
        .body
        .places
        .get(place)
        .ok_or(Failure::InconsistentSyntax)?;
    let local = query
        .body
        .locals
        .get(place.local)
        .ok_or(Failure::InconsistentSyntax)?;
    match select(query.program(), place.data, local.access().mode())? {
        PlaceSupply::Pending => Err(Failure::InconsistentSyntax),
        PlaceSupply::MissingOwner => Err(super::diagnostics::query_rejection(
            context,
            query,
            Some(origin),
            Rule::MissingTransfer,
            "this value has no Copy evidence and the source does not own it",
        )),
        selected => Ok(selected),
    }
}
