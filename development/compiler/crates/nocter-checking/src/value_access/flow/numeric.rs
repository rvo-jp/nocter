//! Normal checked usize relationships over the existing operation graph. These are numeric facts,
//! never permission to access a value, skip evaluation or remove its original bounds check.
mod equalities;
#[cfg(test)]
mod tests;
mod transfer;

use super::{Graph, IntegrityError, Location};
use crate::value_access::{ExpressionKind, Query as TypedQuery, Referent};
use equalities::Equalities;
use nocter_model::{
    AccessExpressionId, AccessFlowPointId, AccessLocalId, BuiltinType, ValueAccessMode,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;

/// Fixed body-local cells. Reassigning a cell changes its current value; a syntax identity does
/// not denote the same number on every execution. Prepared operands have their own frozen cells.
#[derive(Clone, Debug, Default)]
struct Cells {
    count: usize,
    locals: BTreeMap<AccessLocalId, usize>,
    values: BTreeMap<AccessExpressionId, usize>,
    operands: BTreeMap<(AccessExpressionId, usize), usize>,
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
}

#[derive(Clone, Debug)]
pub(super) struct Analysis {
    cells: Arc<Cells>,
    entries: BTreeMap<AccessFlowPointId, Equalities>,
}
impl Analysis {
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
        let mut equalities = self.entries.get(&at.point)?.clone();
        for action in point.actions.iter().take(at.action) {
            transfer::action(typed, &self.cells, &mut equalities, &action.operation).ok()?;
        }
        Some(NumericFacts {
            cells: Arc::clone(&self.cells),
            equalities,
        })
    }
}

pub(super) fn analyze(typed: &TypedQuery, graph: &Graph) -> Result<Analysis, IntegrityError> {
    let cells = Arc::new(Cells::prepare(typed, graph)?);
    let entry = Equalities::top(cells.count).ok_or(INVALID)?;
    let mut entries = BTreeMap::from([(graph.entry(), entry)]);
    let mut pending = BTreeSet::from([graph.entry()]);
    while let Some(id) = pending.pop_first() {
        let point = graph.points().get(id).ok_or(INVALID)?;
        let mut state = entries.get(&id).ok_or(INVALID)?.clone();
        for action in &point.actions {
            transfer::action(typed, &cells, &mut state, &action.operation)?;
        }
        for successor in point.terminator.successors() {
            let mut incoming = state.clone();
            transfer::edge(typed, &cells, &mut incoming, point.terminator, successor)?;
            let changed = if let Some(current) = entries.get_mut(&successor) {
                let joined = current.join(&incoming).ok_or(INVALID)?;
                let changed = *current != joined;
                *current = joined;
                changed
            } else {
                entries.insert(successor, incoming);
                true
            };
            if changed {
                pending.insert(successor);
            }
        }
    }
    Ok(Analysis { cells, entries })
}
