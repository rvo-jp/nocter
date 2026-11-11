//! Replay authoritative numerical facts once through a block for downstream checking passes.
use super::{Analysis, IntegrityError, NumericFacts, State, TypedQuery};
use crate::value_access::flow::{Location, Operation};
use nocter_model::AccessFlowPointId;
use std::sync::Arc;

pub(in crate::value_access::flow) struct Cursor<'a> {
    analysis: &'a Analysis,
    typed: &'a TypedQuery,
    state: Option<State>,
    point: AccessFlowPointId,
    action: usize,
}
impl Analysis {
    pub(in crate::value_access::flow) fn cursor<'a>(
        &'a self,
        typed: &'a TypedQuery,
        point: AccessFlowPointId,
    ) -> Cursor<'a> {
        Cursor {
            analysis: self,
            typed,
            state: self.entries.get(&point).cloned(),
            point,
            action: 0,
        }
    }
}
impl Cursor<'_> {
    pub(in crate::value_access::flow) fn next(
        &mut self,
        operation: &Operation,
    ) -> Result<Option<NumericFacts>, IntegrityError> {
        let at = Location {
            point: self.point,
            action: self.action,
        };
        self.action += 1;
        let Some(state) = &mut self.state else {
            return Ok(None);
        };
        let equalities = state.common();
        let point_sources = state.points(&self.analysis.cells);
        let conditional_restoration =
            state.conditional_restoration(&self.analysis.cells, operation);
        let transfers = state.action(self.typed, &self.analysis.cells, operation)?;
        Ok(equalities.map(|equalities| NumericFacts {
            cells: Arc::clone(&self.analysis.cells),
            equalities,
            point_sources,
            conditional_restoration,
            prefix_extensions: transfers.proven,
            prefix_frontiers: self
                .analysis
                .progressions
                .prefixes
                .get(&at)
                .cloned()
                .unwrap_or_default(),
        }))
    }
}
