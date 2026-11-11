//! Finite numerical-history definedness partitions, not resource ownership alternatives.
use super::{
    Cells, Equalities, INVALID, Transfers,
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
struct Facts {
    equalities: Equalities,
    // May-executed transfer sites in the current array epoch. This finite source provenance
    // never equates two numbers or owns an element/backing. Join uses union, reset forgets sites.
    seen: BTreeSet<AccessExpressionId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct State {
    partitions: BTreeMap<Box<[Phase]>, Facts>,
}
impl State {
    pub(super) fn entry(cells: &Cells) -> Option<Self> {
        Some(Self {
            partitions: BTreeMap::from([(
                vec![Phase::Empty; cells.histories.len()].into(),
                Facts {
                    equalities: Equalities::top(cells.count)?,
                    seen: BTreeSet::new(),
                },
            )]),
        })
    }
    fn insert(&mut self, phases: Box<[Phase]>, incoming: Facts) -> Option<bool> {
        if !incoming.equalities.possible() {
            return Some(false);
        }
        if let Some(current) = self.partitions.get_mut(&phases) {
            let next = Facts {
                equalities: current.equalities.join(&incoming.equalities)?,
                seen: current.seen.union(&incoming.seen).copied().collect(),
            };
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
        let mut result = entries.next()?.equalities.clone();
        for state in entries {
            result = result.join(&state.equalities)?;
        }
        Some(result)
    }
    pub(super) fn carried_prefixes(&self, cells: &Cells) -> BTreeSet<AccessExpressionId> {
        self.partitions
            .iter()
            .flat_map(|(phases, state)| cells.histories.carried_prefixes(phases, &state.seen))
            .collect()
    }
    pub(super) fn points(&self, cells: &Cells) -> BTreeSet<AccessExpressionId> {
        let mut entries = self.partitions.iter();
        let Some((phases, first)) = entries.next() else {
            return BTreeSet::new();
        };
        let mut result = cells.histories.point_sources(phases, &first.seen);
        for (phases, state) in entries {
            let incoming = cells.histories.point_sources(phases, &state.seen);
            result.retain(|source| incoming.contains(source));
        }
        result
    }
    pub(super) fn conditional_restoration(
        &self,
        cells: &Cells,
        operation: &Operation,
    ) -> Option<(AccessExpressionId, BTreeSet<AccessExpressionId>)> {
        let Operation::Assign {
            target: crate::value_access::Referent::Evaluated(selection),
            ..
        } = *operation
        else {
            return None;
        };
        let mut sources = BTreeSet::new();
        for (phases, state) in &self.partitions {
            sources.extend(cells.histories.conditional_point(
                cells,
                phases,
                &state.equalities,
                &state.seen,
                selection,
            )?);
        }
        (!sources.is_empty()).then_some((selection, sources))
    }
    pub(super) fn available(
        &self,
        cells: &Cells,
        selection: AccessExpressionId,
    ) -> Option<super::super::resources::Place> {
        let mut array = None;
        for (phases, state) in &self.partitions {
            array = Some(
                cells
                    .histories
                    .available(cells, phases, &state.equalities, selection)?,
            );
        }
        array
    }
    pub(super) fn action(
        &mut self,
        typed: &TypedQuery,
        cells: &Cells,
        operation: &Operation,
    ) -> Result<Transfers, IntegrityError> {
        let mut next = Self {
            partitions: BTreeMap::new(),
        };
        let mut common: Option<Transfers> = None;
        for (phases, state) in &self.partitions {
            let mut phases = phases.clone();
            let mut state = state.clone();
            let proven = history::action(
                typed,
                cells,
                &mut phases,
                &mut state.equalities,
                &mut state.seen,
                operation,
            )?;
            transfer::action(typed, cells, &mut state.equalities, operation)?;
            // Only reached normal completions contribute obligations. Bottom cannot establish
            // a vacuously true prefix proof or poison a reachable history at a join.
            if !state.equalities.possible() {
                continue;
            }
            if let Some(common) = &mut common {
                common.proven.retain(|id| proven.proven.contains(id));
                common.repeated.extend(proven.repeated);
                for (site, peers) in proven.peers {
                    common.peers.entry(site).or_default().extend(peers);
                }
            } else {
                common = Some(proven);
            }
            next.insert(phases, state).ok_or(INVALID)?;
        }
        *self = next;
        let mut common = common.unwrap_or_default();
        common.repeated.retain(|id| common.proven.contains(id));
        common.peers.retain(|id, _| common.proven.contains(id));
        Ok(common)
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
            transfer::edge(typed, cells, &mut state.equalities, terminator, successor)?;
            next.insert(phases.clone(), state).ok_or(INVALID)?;
        }
        *self = next;
        Ok(())
    }
}
