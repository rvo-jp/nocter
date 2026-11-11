//! Finite numerical-history definedness partitions, not resource ownership alternatives.
use super::{
    Cells, Equalities, INVALID,
    history::{self, Phase},
    transfer,
};
use crate::value_access::{
    Query as TypedQuery,
    flow::{IntegrityError, Operation, Terminator},
};
use nocter_model::{AccessExpressionId, AccessFlowPointId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct State {
    partitions: BTreeMap<Box<[Phase]>, Equalities>,
}
impl State {
    pub(super) fn entry(cells: &Cells) -> Option<Self> {
        Some(Self {
            partitions: BTreeMap::from([(
                vec![Phase::Empty; cells.histories.len()].into(),
                Equalities::top(cells.count)?,
            )]),
        })
    }
    fn insert(&mut self, phases: Box<[Phase]>, incoming: Equalities) -> Option<bool> {
        if !incoming.possible() {
            return Some(false);
        }
        if let Some(current) = self.partitions.get_mut(&phases) {
            let next = current.join(&incoming)?;
            let changed = *current != next;
            *current = next;
            Some(changed)
        } else {
            self.partitions.insert(phases, incoming);
            Some(true)
        }
    }
    pub(super) fn merge(&mut self, incoming: &Self) -> Option<bool> {
        let mut changed = false;
        for (phases, state) in &incoming.partitions {
            changed |= self.insert(phases.clone(), state.clone())?;
        }
        Some(changed)
    }
    pub(super) fn common(&self) -> Option<Equalities> {
        let mut entries = self.partitions.values();
        let mut result = entries.next()?.clone();
        for state in entries {
            result = result.join(state)?;
        }
        Some(result)
    }
    pub(super) fn action(
        &mut self,
        typed: &TypedQuery,
        cells: &Cells,
        operation: &Operation,
    ) -> Result<BTreeSet<AccessExpressionId>, IntegrityError> {
        let mut next = Self {
            partitions: BTreeMap::new(),
        };
        let mut common: Option<BTreeSet<AccessExpressionId>> = None;
        for (phases, state) in &self.partitions {
            let mut phases = phases.clone();
            let mut state = state.clone();
            let proven = history::action(typed, cells, &mut phases, &mut state, operation)?;
            transfer::action(typed, cells, &mut state, operation)?;
            // Only reached normal completions contribute obligations. Bottom cannot establish
            // a vacuously true prefix proof or poison a reachable history at a join.
            if !state.possible() {
                continue;
            }
            if let Some(common) = &mut common {
                common.retain(|id| proven.contains(id));
            } else {
                common = Some(proven);
            }
            next.insert(phases, state).ok_or(INVALID)?;
        }
        *self = next;
        Ok(common.unwrap_or_default())
    }
    pub(super) fn edge(
        &mut self,
        typed: &TypedQuery,
        cells: &Cells,
        terminator: Terminator,
        successor: AccessFlowPointId,
    ) -> Result<(), IntegrityError> {
        let mut next = Self {
            partitions: BTreeMap::new(),
        };
        for (phases, state) in &self.partitions {
            let mut state = state.clone();
            transfer::edge(typed, cells, &mut state, terminator, successor)?;
            next.insert(phases.clone(), state).ok_or(INVALID)?;
        }
        *self = next;
        Ok(())
    }
}
