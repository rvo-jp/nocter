//! One finite proof-history cell per concrete containing array, irrespective of array count.
//! These cells describe committed transfers; the resource ledger alone owns actual holes/backing.
use super::{Cells, Equalities, INVALID};
use crate::value_access::{
    ArgumentAction, ExpressionKind, Query as TypedQuery, Referent,
    flow::{
        Graph, IntegrityError, Operation,
        resources::{Path, RetainedIndex},
    },
};
use nocter_model::{AccessExpressionId, AccessLocalId, FieldPath, ValueAccessMode};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Phase {
    Empty,
    Prefix,
    Unproved,
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Array {
    local: AccessLocalId,
    fields: FieldPath,
}
#[derive(Clone, Debug)]
struct History {
    array: Array,
    cell: usize,
}
#[derive(Clone, Copy, Debug)]
struct Selection {
    history: usize,
    index: AccessExpressionId,
}
#[derive(Clone, Debug, Default)]
pub(super) struct Histories {
    arrays: Vec<History>,
    selections: BTreeMap<AccessExpressionId, Selection>,
}
fn selected(typed: &TypedQuery, source: Referent) -> Option<(Array, RetainedIndex)> {
    let Referent::Evaluated(id) = source else {
        return None;
    };
    let index = RetainedIndex::for_selection(typed, id)?;
    let ExpressionKind::Index { base, .. } = typed.body().expressions().get(id)?.kind else {
        return None;
    };
    let base = typed.body().expressions().get(base)?.referent(base)?;
    let (local, fields) = Path::for_referent(typed, base)?;
    if typed.body().locals().get(local)?.access().mode() != ValueAccessMode::Own {
        return None;
    }
    Some((
        Array {
            local,
            fields: fields.concrete()?,
        },
        index,
    ))
}
/// Execution-order transfers only. Merely prepared/cancelled arguments produce no event.
pub(super) fn transfers(
    typed: &TypedQuery,
    operation: &Operation,
) -> Result<Vec<Referent>, IntegrityError> {
    Ok(match *operation {
        Operation::Evaluate(id) => match typed.body().expressions().get(id).ok_or(INVALID)?.kind {
            ExpressionKind::Move(source) => vec![source],
            _ => Vec::new(),
        },
        Operation::CommitCall(id) => {
            let ExpressionKind::Call { ref arguments, .. } =
                typed.body().expressions().get(id).ok_or(INVALID)?.kind
            else {
                return Err(INVALID);
            };
            arguments
                .iter()
                .filter_map(|argument| match argument.action {
                    ArgumentAction::ReserveTransfer { place, .. } => Some(place),
                    _ => None,
                })
                .collect()
        }
        _ => Vec::new(),
    })
}
impl Histories {
    pub(super) fn len(&self) -> usize {
        self.arrays.len()
    }
    pub(super) fn prepare(
        typed: &TypedQuery,
        graph: &Graph,
        count: &mut usize,
    ) -> Result<Self, IntegrityError> {
        let mut result = Self::default();
        let mut arrays = BTreeMap::new();
        for (_, point) in graph.points().iter() {
            for action in &point.actions {
                for source in transfers(typed, &action.operation)? {
                    let Some((array, index)) = selected(typed, source) else {
                        continue;
                    };
                    let history = if let Some(&id) = arrays.get(&array) {
                        id
                    } else {
                        let id = result.arrays.len();
                        *count = count.checked_add(1).ok_or(INVALID)?;
                        result.arrays.push(History {
                            array: array.clone(),
                            cell: *count,
                        });
                        arrays.insert(array, id);
                        id
                    };
                    result.selections.insert(
                        index.selection(),
                        Selection {
                            history,
                            index: index.operand(),
                        },
                    );
                }
            }
        }
        Ok(result)
    }
    fn local_epoch(&self, phases: &mut [Phase], state: &mut Equalities, local: AccessLocalId) {
        for (id, history) in self.arrays.iter().enumerate() {
            if history.array.local == local {
                phases[id] = Phase::Empty;
                state.forget(history.cell);
            }
        }
    }
    fn changed(
        &self,
        typed: &TypedQuery,
        phases: &mut [Phase],
        state: &mut Equalities,
        source: Referent,
        except: Option<usize>,
    ) {
        let target = Path::for_referent(typed, source).filter(|(local, _)| {
            typed
                .body()
                .locals()
                .get(*local)
                .is_some_and(|local| local.access().mode() == ValueAccessMode::Own)
        });
        for (id, history) in self.arrays.iter().enumerate() {
            if Some(id) == except {
                continue;
            }
            let array = Path::from_fields(&history.array.fields);
            if let Some((local, path)) = &target {
                if *local != history.array.local || !path.overlaps(&array) {
                    continue;
                }
                phases[id] = if path.is_prefix_of(&array) {
                    Phase::Empty
                } else {
                    Phase::Unproved
                };
            } else {
                phases[id] = Phase::Unproved;
            }
            state.forget(history.cell);
        }
    }
}
pub(super) fn action(
    typed: &TypedQuery,
    cells: &Cells,
    phases: &mut [Phase],
    state: &mut Equalities,
    operation: &Operation,
) -> Result<BTreeSet<AccessExpressionId>, IntegrityError> {
    let histories = &cells.histories;
    let mut proven = BTreeSet::new();
    for source in transfers(typed, operation)? {
        let selected = match source {
            Referent::Evaluated(id) => histories
                .selections
                .get(&id)
                .copied()
                .map(|selection| (id, selection)),
            Referent::Place(_) => None,
        };
        if let Some((id, selection)) = selected {
            let history = histories.arrays.get(selection.history).ok_or(INVALID)?;
            let index = cells.value(selection.index);
            let extends = index.is_some_and(|index| match phases[selection.history] {
                Phase::Empty => state.constant(index) == Some(0),
                Phase::Prefix => state.difference(index, history.cell) == Some(1),
                Phase::Unproved => false,
            });
            if extends {
                proven.insert(id);
            }
            phases[selection.history] = if extends {
                Phase::Prefix
            } else {
                Phase::Unproved
            };
            if let Some(index) = index {
                state.assign(history.cell, index, 0);
            } else {
                state.forget(history.cell);
            }
            histories.changed(typed, phases, state, source, Some(selection.history));
        } else {
            histories.changed(typed, phases, state, source, None);
        }
    }
    match *operation {
        Operation::CommitCall(_) => {
            // An invoked callback/capture can reach an array beyond explicit edit operands.
            // Until closed write summaries are consumed, no prefix history crosses invocation.
            // The transfer proofs above still describe the pre-invocation atomic bundle.
            phases.fill(Phase::Unproved);
            for history in &histories.arrays {
                state.forget(history.cell);
            }
        }
        Operation::Bind { local, .. } | Operation::Drop(local) => {
            histories.local_epoch(phases, state, local);
        }
        Operation::ExitScope { ref locals, .. } => {
            for &local in locals {
                histories.local_epoch(phases, state, local);
            }
        }
        Operation::Assign { target, .. } | Operation::CompoundAssign { target, .. } => {
            histories.changed(typed, phases, state, target, None);
        }
        _ => {}
    }
    Ok(proven)
}
