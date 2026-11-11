//! Normal checked usize relationships over the existing operation graph. These are numeric facts,
//! never permission to access a value, skip evaluation or remove its original bounds check.
mod equalities;
mod history;
mod progressions;
mod state;
#[cfg(test)]
mod tests;
mod transfer;

use super::owned_base;
use super::resources::ArrayFrontier;
use super::{Graph, IntegrityError, Location};
use crate::value_access::{ExpressionKind, Query as TypedQuery, Referent};
use equalities::Equalities;
use nocter_model::{
    AccessExpressionId, AccessFlowPointId, AccessLocalId, BuiltinType, ValueAccessMode,
};
use state::State;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;

#[derive(Default)]
struct Transfers {
    proven: BTreeSet<AccessExpressionId>,
    repeated: BTreeSet<AccessExpressionId>,
    // Transfer sites that can participate in the same proven array epoch. This is finite
    // source provenance, not equal index values or a resource ownership alternative.
    peers: BTreeMap<AccessExpressionId, BTreeSet<AccessExpressionId>>,
}

/// Fixed body-local cells. Reassigning a cell changes its current value; a syntax identity does
/// not denote the same number on every execution. Prepared operands have their own frozen cells.
#[derive(Clone, Debug, Default)]
struct Cells {
    count: usize,
    locals: BTreeMap<AccessLocalId, usize>,
    values: BTreeMap<AccessExpressionId, usize>,
    operands: BTreeMap<(AccessExpressionId, usize), usize>,
    histories: history::Histories,
    no_caller_writes: BTreeSet<AccessExpressionId>,
}
impl Cells {
    fn fresh(&mut self) -> Result<usize, IntegrityError> {
        self.count = self.count.checked_add(1).ok_or(INVALID)?;
        Ok(self.count)
    }
    fn prepare(typed: &TypedQuery, graph: &Graph) -> Result<Self, IntegrityError> {
        let mut cells = Self::default();
        for (id, local) in typed.body().locals().iter() {
            if local.access().mode() == ValueAccessMode::Own
                && typed.program().data_kind(local.data) == Some(BuiltinType::Usize)
            {
                let cell = cells.fresh()?;
                cells.locals.insert(id, cell);
            }
        }
        for (id, expression) in typed.body().expressions().iter() {
            if let ExpressionKind::Place(place) = expression.kind {
                if let Some(local) = cells.local(typed, Referent::Place(place)) {
                    cells.values.insert(id, local);
                }
            } else if expression.ty.normal().is_some_and(|access| {
                access.mode() == ValueAccessMode::Own
                    && typed.program().data_kind(access.data()) == Some(BuiltinType::Usize)
            }) {
                let cell = cells.fresh()?;
                cells.values.insert(id, cell);
            }
        }
        for (_, point) in graph.points().iter() {
            for action in &point.actions {
                if let super::Operation::PrepareOperand {
                    expression,
                    operand,
                    value,
                } = action.operation
                    && cells.values.contains_key(&value)
                    && !cells.operands.contains_key(&(expression, operand))
                {
                    let cell = cells.fresh()?;
                    cells.operands.insert((expression, operand), cell);
                }
            }
        }
        cells.histories = history::Histories::prepare(typed, graph, &mut cells.count)?;
        Ok(cells)
    }
    fn local(&self, typed: &TypedQuery, referent: Referent) -> Option<usize> {
        let Referent::Place(id) = referent else {
            return None;
        };
        let place = typed.body().places().get(id)?;
        place
            .fields
            .is_root()
            .then(|| self.locals.get(&place.local).copied())
            .flatten()
    }
    fn value(&self, expression: AccessExpressionId) -> Option<usize> {
        self.values.get(&expression).copied()
    }
    fn operand(&self, expression: AccessExpressionId, index: usize) -> Option<usize> {
        self.operands.get(&(expression, index)).copied()
    }
}

/// Facts at one reached graph location, before its operation. No fact licenses a read from an
/// uninitialized/moved slot. Impossible normal continuations furnish no numeric proof.
#[derive(Clone, Debug)]
pub struct NumericFacts {
    cells: Arc<Cells>,
    equalities: Equalities,
    prefix_extensions: BTreeSet<AccessExpressionId>,
    prefix_frontiers: BTreeMap<AccessExpressionId, ArrayFrontier>,
}
impl NumericFacts {
    #[must_use]
    pub fn constant(&self, expression: AccessExpressionId) -> Option<u64> {
        self.equalities.constant(self.cells.value(expression)?)
    }
    /// Exact mathematical left - right, conditional on normal checked execution.
    #[must_use]
    pub fn difference(&self, left: AccessExpressionId, right: AccessExpressionId) -> Option<i128> {
        self.equalities
            .difference(self.cells.value(left)?, self.cells.value(right)?)
    }
    /// The current operation's transfer starts at zero or follows the last committed prefix
    /// endpoint by one, in every reached numerical history. Availability/ownership still need
    /// independent validation; this creates no saved endpoint, hole or execution operation.
    #[must_use]
    pub fn extends_prefix(&self, selection: AccessExpressionId) -> bool {
        self.prefix_extensions.contains(&selection)
    }
    /// A finite cyclic progression needs this array's prefix lifecycle. Straight-line moves and
    /// complete replacement loops retain their selected-point realization. No backing is created.
    #[must_use]
    pub fn prefix_frontier(&self, selection: AccessExpressionId) -> Option<ArrayFrontier> {
        self.prefix_frontiers.get(&selection).copied()
    }
}

#[derive(Clone, Debug)]
pub(super) struct Analysis {
    cells: Arc<Cells>,
    entries: BTreeMap<AccessFlowPointId, State>,
    progressions: progressions::Plan,
}
impl Analysis {
    pub(in crate::value_access::flow) fn prefix(
        &self,
        at: Location,
        selection: AccessExpressionId,
    ) -> Option<ArrayFrontier> {
        self.progressions
            .prefixes
            .get(&at)?
            .get(&selection)
            .copied()
    }
    pub(in crate::value_access::flow) fn available(
        &self,
        at: Location,
    ) -> Option<&super::resources::Place> {
        self.progressions.available.get(&at)
    }
    pub(super) fn at(
        &self,
        typed: &TypedQuery,
        graph: &Graph,
        at: Location,
    ) -> Option<NumericFacts> {
        let point = graph.points().get(at.point)?;
        if at.action >= point.actions.len() {
            return None;
        }
        let mut state = self.entries.get(&at.point)?.clone();
        for action in point.actions.iter().take(at.action) {
            state.action(typed, &self.cells, &action.operation).ok()?;
        }
        let equalities = state.common()?;
        let transfers = state
            .action(typed, &self.cells, &point.actions.get(at.action)?.operation)
            .ok()?;
        Some(NumericFacts {
            cells: Arc::clone(&self.cells),
            equalities,
            prefix_extensions: transfers.proven,
            prefix_frontiers: self
                .progressions
                .prefixes
                .get(&at)
                .cloned()
                .unwrap_or_default(),
        })
    }
}

pub(super) fn analyze(
    typed: &TypedQuery,
    graph: &Graph,
    no_caller_writes: BTreeSet<AccessExpressionId>,
) -> Result<Analysis, IntegrityError> {
    let mut cells = Cells::prepare(typed, graph)?;
    cells.no_caller_writes = no_caller_writes;
    let cells = Arc::new(cells);
    let entry = State::entry(&cells).ok_or(INVALID)?;
    let mut entries = BTreeMap::from([(graph.entry(), entry)]);
    let mut pending = BTreeSet::from([graph.entry()]);
    while let Some(id) = pending.pop_first() {
        let point = graph.points().get(id).ok_or(INVALID)?;
        let mut state = entries.get(&id).ok_or(INVALID)?.clone();
        for action in &point.actions {
            state.action(typed, &cells, &action.operation)?;
        }
        for successor in point.terminator.successors() {
            let mut incoming = state.clone();
            incoming.edge(typed, &cells, point.terminator, successor)?;
            let changed = if let Some(current) = entries.get_mut(&successor) {
                current.merge(&incoming).ok_or(INVALID)?
            } else {
                entries.insert(successor, incoming);
                true
            };
            if changed {
                pending.insert(successor);
            }
        }
    }
    let progressions = progressions::prepare(typed, graph, &cells, &entries)?;
    Ok(Analysis {
        cells,
        entries,
        progressions,
    })
}
