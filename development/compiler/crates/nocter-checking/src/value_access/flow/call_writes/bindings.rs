//! Project explicit callee write roots through the already selected source input plan.
use super::super::aliases::Aliases;
use super::{INVALID, IntegrityError, TypedQuery};
use crate::value_access::{ArgumentAction, ExpressionKind, Referent};
use nocter_model::{AccessExpressionId, ValueAccessMode, ValueInputOperation};
use std::collections::BTreeSet;

pub(super) enum Root {
    Owned,
    Input(Input),
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Input {
    Environment,
    Explicit(usize),
}
/// Resolve only the owning/input root. Dynamic selections supply no exact numeric path.
pub(super) fn classify(typed: &TypedQuery, roots: &Aliases, source: Referent) -> Option<Root> {
    let place = roots.root(typed, source)?;
    let local = typed.body().places().get(place)?.local;
    if typed.body().locals().get(local)?.access().mode() == ValueAccessMode::Own {
        return Some(Root::Owned);
    }
    if typed
        .body()
        .inputs()
        .environment()
        .is_some_and(|input| input.local == local && input.operation == ValueInputOperation::Edit)
    {
        return Some(Root::Input(Input::Environment));
    }
    typed
        .body()
        .inputs()
        .explicit()
        .iter()
        .position(|input| input.local == local && input.operation == ValueInputOperation::Edit)
        .map(|position| Root::Input(Input::Explicit(position)))
}

pub(super) fn actual_writes(
    typed: &TypedQuery,
    roots: &Aliases,
    call: AccessExpressionId,
    inputs: &BTreeSet<Input>,
) -> Result<Option<Vec<Referent>>, IntegrityError> {
    let ExpressionKind::Call {
        kind, arguments, ..
    } = &typed.body().expressions().get(call).ok_or(INVALID)?.kind
    else {
        return Err(INVALID);
    };
    let environment = kind.environment_argument(arguments);
    let arguments = kind.explicit_arguments(arguments).ok_or(INVALID)?;
    let mut writes = Vec::new();
    for &input in inputs {
        let argument = match input {
            Input::Environment => environment,
            Input::Explicit(position) => arguments.get(position),
        }
        .ok_or(INVALID)?;
        let received_environment =
            input == Input::Environment && argument.operation == ValueInputOperation::Take;
        if !received_environment && argument.operation != ValueInputOperation::Edit {
            return Err(INVALID);
        }
        match &argument.action {
            ArgumentAction::ReserveUpdate(source) if !received_environment => {
                let Some(footprint) = roots.footprint(typed, *source) else {
                    return Ok(None);
                };
                writes.push(footprint);
            }
            // The transfer already retired the old source. The callee edits its ordinary
            // owned temporary, not the caller's remaining storage.
            ArgumentAction::ReserveTransfer { .. }
            | ArgumentAction::AccessOwnedValue
            | ArgumentAction::TakeValue
            | ArgumentAction::CopyPlace(_) => {}
            ArgumentAction::ReserveRead(_)
            | ArgumentAction::ReserveUpdate(_)
            | ArgumentAction::ReuseRead { .. }
            | ArgumentAction::PendingValue(_)
            | ArgumentAction::Unproduced
            | ArgumentAction::Interrupted => return Err(INVALID),
        }
    }
    Ok(Some(writes))
}
