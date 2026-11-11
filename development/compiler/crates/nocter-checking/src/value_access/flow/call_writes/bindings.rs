//! Project explicit callee write roots through the already selected source input plan.
use super::super::aliases::Aliases;
use super::{INVALID, IntegrityError, TypedQuery};
use crate::value_access::{ArgumentAction, ExpressionKind, Referent};
use nocter_model::{AccessExpressionId, ValueAccessMode, ValueInputOperation};
use std::collections::BTreeSet;

pub(super) enum Root {
    Owned,
    Input(usize),
}
/// Resolve only the owning/input root. Dynamic selections supply no exact numeric path.
pub(super) fn classify(typed: &TypedQuery, roots: &Aliases, source: Referent) -> Option<Root> {
    let place = roots.root(typed, source)?;
    let local = typed.body().places().get(place)?.local;
    if typed.body().locals().get(local)?.access().mode() == ValueAccessMode::Own {
        return Some(Root::Owned);
    }
    typed
        .body()
        .inputs()
        .explicit()
        .iter()
        .position(|input| input.local == local && input.operation == ValueInputOperation::Edit)
        .map(Root::Input)
}

pub(super) fn actual_writes(
    typed: &TypedQuery,
    roots: &Aliases,
    call: AccessExpressionId,
    inputs: &BTreeSet<usize>,
) -> Result<Option<Vec<Referent>>, IntegrityError> {
    let ExpressionKind::Call { ref arguments, .. } =
        typed.body().expressions().get(call).ok_or(INVALID)?.kind
    else {
        return Err(INVALID);
    };
    let mut writes = Vec::new();
    for &input in inputs {
        let argument = arguments.get(input).ok_or(INVALID)?;
        if argument.operation != ValueInputOperation::Edit {
            return Err(INVALID);
        }
        match &argument.action {
            ArgumentAction::ReserveUpdate(source) => {
                let Some(footprint) = roots.footprint(typed, *source) else {
                    return Ok(None);
                };
                writes.push(footprint);
            }
            ArgumentAction::ReserveTransfer { adapters, .. } if !adapters.is_empty() => {
                return Ok(None);
            }
            // The transfer already retired the old source. The callee edits its ordinary
            // owned temporary, not the caller's remaining storage.
            ArgumentAction::ReserveTransfer { .. }
            | ArgumentAction::AccessOwnedValue
            | ArgumentAction::TakeValue
            | ArgumentAction::CopyPlace(_) => {}
            ArgumentAction::ReserveRead(_)
            | ArgumentAction::PendingValue(_)
            | ArgumentAction::Unproduced
            | ArgumentAction::Interrupted => return Err(INVALID),
        }
    }
    Ok(Some(writes))
}
