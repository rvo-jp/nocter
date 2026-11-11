//! One finite proof-history cell per concrete containing array, irrespective of array count.
//! These cells describe committed transfers; the resource ledger alone owns actual holes/backing.
use super::{Cells, Equalities, INVALID};
use crate::value_access::{
    ArgumentAction, ExpressionKind, Query as TypedQuery, Referent,
    flow::{
        Graph, IntegrityError, Operation,
        resources::{ArrayFrontier, Holder, Path, Place, RetainedIndex},
    },
};
use nocter_model::{AccessExpressionId, AccessLocalId, ValueAccessMode};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Phase {
    Empty,
    Prefix,
    Unproved,
}
#[derive(Clone, Copy)]
enum Change {
    Definite {
        source: Referent,
        except: Option<usize>,
    },
    MayWrite(Referent),
}
#[derive(Clone, Debug)]
struct History {
    array: Place,
    cell: usize,
    frontier: ArrayFrontier,
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
fn selected(typed: &TypedQuery, source: Referent) -> Option<(Place, RetainedIndex)> {
    let Referent::Evaluated(id) = source else {
        return None;
    };
    let index = RetainedIndex::for_selection(typed, id)?;
    Some((index.array(typed)?, index))
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
    pub(super) fn frontier(&self, selection: AccessExpressionId) -> Option<ArrayFrontier> {
        self.arrays
            .get(self.selections.get(&selection)?.history)
            .map(|history| history.frontier)
    }
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
                            frontier: ArrayFrontier::new(index),
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
        // Observing/preparing another element does not commit a transfer. It can still ask
        // whether its actual evaluated index lies beyond this array's committed prefix.
        for (id, _) in typed.body().expressions().iter() {
            let Some((array, index)) = selected(typed, Referent::Evaluated(id)) else {
                continue;
            };
            if let Some(&history) = arrays.get(&array) {
                result.selections.insert(
                    id,
                    Selection {
                        history,
                        index: index.operand(),
                    },
                );
            }
        }
        Ok(result)
    }
    pub(super) fn available(
        &self,
        cells: &Cells,
        phases: &[Phase],
        state: &Equalities,
        selection: AccessExpressionId,
    ) -> Option<Place> {
        let selection = self.selections.get(&selection)?;
        let history = self.arrays.get(selection.history)?;
        let available = match phases.get(selection.history)? {
            Phase::Empty => true,
            Phase::Prefix => state
                .difference(cells.value(selection.index)?, history.cell)
                .is_some_and(|difference| difference > 0),
            Phase::Unproved => false,
        };
        available.then(|| history.array.clone())
    }
    fn forget_seen(&self, seen: &mut BTreeSet<AccessExpressionId>, history: usize) {
        seen.retain(|selection| {
            self.selections
                .get(selection)
                .is_some_and(|selection| selection.history != history)
        });
    }
    fn local_epoch(
        &self,
        phases: &mut [Phase],
        state: &mut Equalities,
        seen: &mut BTreeSet<AccessExpressionId>,
        local: AccessLocalId,
    ) {
        for (id, history) in self.arrays.iter().enumerate() {
            if history.array.holder() == Holder::Local(local) {
                phases[id] = Phase::Empty;
                state.forget(history.cell);
                self.forget_seen(seen, id);
            }
        }
    }
    fn call_writes(
        &self,
        typed: &TypedQuery,
        targets: Option<&[Referent]>,
        phases: &mut [Phase],
        state: &mut Equalities,
        seen: &mut BTreeSet<AccessExpressionId>,
    ) {
        if let Some(targets) = targets {
            for &target in targets {
                self.changed(typed, phases, state, seen, Change::MayWrite(target));
            }
        } else {
            // Unknown invocation effects can reach an array beyond explicit edit operands.
            phases.fill(Phase::Unproved);
            seen.clear();
            for history in &self.arrays {
                state.forget(history.cell);
            }
        }
    }
    fn changed(
        &self,
        typed: &TypedQuery,
        phases: &mut [Phase],
        state: &mut Equalities,
        seen: &mut BTreeSet<AccessExpressionId>,
        change: Change,
    ) {
        let (source, except, definite) = match change {
            Change::Definite { source, except } => (source, except, true),
            Change::MayWrite(source) => (source, None, false),
        };
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
            let array = history.array.fields();
            if let Some((local, path)) = &target {
                if Holder::Local(*local) != history.array.holder() || !path.overlaps(array) {
                    continue;
                }
                // An initialized edit preserves an already empty extraction history. It
                // cannot turn a prefix/unproved history into a fresh epoch by may-write alone.
                phases[id] = if (definite && path.is_prefix_of(array))
                    || (!definite && phases[id] == Phase::Empty)
                {
                    Phase::Empty
                } else {
                    Phase::Unproved
                };
            } else {
                phases[id] = Phase::Unproved;
            }
            state.forget(history.cell);
            self.forget_seen(seen, id);
        }
    }
}
pub(super) fn action(
    typed: &TypedQuery,
    cells: &Cells,
    phases: &mut [Phase],
    state: &mut Equalities,
    seen: &mut BTreeSet<AccessExpressionId>,
    operation: &Operation,
) -> Result<super::Transfers, IntegrityError> {
    let histories = &cells.histories;
    let mut proven = super::Transfers::default();
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
                proven.proven.insert(id);
                proven.peers.insert(
                    id,
                    seen.iter()
                        .copied()
                        .filter(|site| {
                            histories
                                .selections
                                .get(site)
                                .is_some_and(|other| other.history == selection.history)
                        })
                        .chain(std::iter::once(id))
                        .collect(),
                );
                if seen.contains(&id) {
                    proven.repeated.insert(id);
                }
            }
            seen.insert(id);
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
            histories.changed(
                typed,
                phases,
                state,
                seen,
                Change::Definite {
                    source,
                    except: Some(selection.history),
                },
            );
        } else {
            histories.changed(
                typed,
                phases,
                state,
                seen,
                Change::Definite {
                    source,
                    except: None,
                },
            );
        }
    }
    match *operation {
        Operation::CommitCall(call) => {
            histories.call_writes(typed, cells.call_writes.at(call), phases, state, seen);
        }
        Operation::Bind { local, .. } | Operation::Drop(local) => {
            histories.local_epoch(phases, state, seen, local);
        }
        Operation::ExitScope { ref locals, .. } => {
            for &local in locals {
                histories.local_epoch(phases, state, seen, local);
            }
        }
        Operation::Assign { target, .. } | Operation::CompoundAssign { target, .. } => {
            histories.changed(
                typed,
                phases,
                state,
                seen,
                Change::Definite {
                    source: target,
                    except: None,
                },
            );
        }
        _ => {}
    }
    Ok(proven)
}
