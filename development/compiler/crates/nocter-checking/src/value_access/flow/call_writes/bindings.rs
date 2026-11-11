//! Project explicit callee write roots through the already selected source input plan.
use super::{INVALID, IntegrityError, TypedQuery};
use crate::value_access::{ArgumentAction, BoundaryAction, ExpressionKind, Referent, Statement};
use nocter_model::{
    AccessExpressionId, AccessLocalId, ValueAccessMode, ValueBindingKind, ValueInputOperation,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) enum Root {
    Owned,
    Input(usize),
}
/// Derived lexical binding index, never an address, ownership or lifetime registry.
pub(super) struct Roots {
    aliases: BTreeMap<AccessLocalId, Referent>,
}
impl Roots {
    pub(super) fn prepare(typed: &TypedQuery) -> Result<Self, IntegrityError> {
        let mut aliases = BTreeMap::new();
        for (_, statement) in typed.body().statements().iter() {
            let Statement::Bind { local, value } = *statement else {
                continue;
            };
            if typed.body().locals().get(local).ok_or(INVALID)?.kind != ValueBindingKind::EditAlias
            {
                continue;
            }
            let ExpressionKind::Boundary {
                source,
                action: BoundaryAction::DelegateUpdate,
            } = typed.body().expressions().get(value).ok_or(INVALID)?.kind
            else {
                continue;
            };
            if let Some(referent) = typed
                .body()
                .expressions()
                .get(source)
                .ok_or(INVALID)?
                .referent(source)
                && aliases.insert(local, referent).is_some()
            {
                return Err(INVALID);
            }
        }
        Ok(Self { aliases })
    }
    /// Resolve only the owning/input root. Dynamic selections supply no exact numeric path.
    pub(super) fn classify(&self, typed: &TypedQuery, mut source: Referent) -> Option<Root> {
        let mut visited = BTreeSet::new();
        loop {
            let place = source.static_base(|id| typed.body().expressions().get(id))?;
            let local = typed.body().places().get(place)?.local;
            if !visited.insert(local) {
                return None;
            }
            if typed.body().locals().get(local)?.access().mode() == ValueAccessMode::Own {
                return Some(Root::Owned);
            }
            if let Some(input) = typed.body().inputs().explicit().iter().position(|input| {
                input.local == local && input.operation == ValueInputOperation::Edit
            }) {
                return Some(Root::Input(input));
            }
            source = *self.aliases.get(&local)?;
        }
    }
}

pub(super) fn actual_writes(
    typed: &TypedQuery,
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
            ArgumentAction::ReserveUpdate(source) => writes.push(*source),
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
