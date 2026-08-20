use std::collections::{BTreeMap, BTreeSet};

use crate::{LoanId, ProvenanceProjection};

/// Field-sensitive set of source loans carried by one checked value.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct LoanValue {
    loans: BTreeSet<LoanId>,
    projections: BTreeMap<ProvenanceProjection, LoanValue>,
}

impl LoanValue {
    pub(super) fn independent() -> Self {
        Self::default()
    }

    pub(super) fn from_loan(loan: LoanId) -> Self {
        Self {
            loans: BTreeSet::from([loan]),
            projections: BTreeMap::new(),
        }
    }

    pub(super) fn from_projection(projection: ProvenanceProjection, value: LoanValue) -> Self {
        Self {
            loans: BTreeSet::new(),
            projections: BTreeMap::from([(projection, value)]),
        }
    }

    pub(super) fn all_loans(&self) -> BTreeSet<LoanId> {
        let mut loans = BTreeSet::new();
        let mut pending = vec![self];
        while let Some(value) = pending.pop() {
            loans.extend(value.loans.iter().copied());
            pending.extend(value.projections.values());
        }
        loans
    }

    pub(super) fn flattened(&self) -> Self {
        Self {
            loans: self.all_loans(),
            projections: BTreeMap::new(),
        }
    }

    pub(super) fn projected(&self, projection: ProvenanceProjection) -> Self {
        let mut result = Self {
            loans: self.loans.clone(),
            projections: BTreeMap::new(),
        };
        if let Some(value) = self.projections.get(&projection) {
            result.union_with(value);
        }
        result
    }

    pub(super) fn union_with(&mut self, another: &Self) -> bool {
        let previous = self.loans.len();
        self.loans.extend(another.loans.iter().copied());
        let mut changed = self.loans.len() != previous;
        for (projection, value) in &another.projections {
            if let Some(current) = self.projections.get_mut(projection) {
                changed |= current.union_with(value);
            } else {
                self.projections.insert(*projection, value.clone());
                changed = true;
            }
        }
        changed
    }

    pub(super) fn insert_projection(&mut self, projection: ProvenanceProjection, value: LoanValue) {
        self.projections
            .entry(projection)
            .and_modify(|current| {
                current.union_with(&value);
            })
            .or_insert(value);
    }

    pub(super) fn replace_projection(&mut self, path: &[ProvenanceProjection], value: LoanValue) {
        let Some((first, remaining)) = path.split_first() else {
            *self = value;
            return;
        };
        self.projections
            .entry(*first)
            .or_default()
            .replace_projection(remaining, value);
    }

    pub(super) fn remove_projection(&mut self, path: &[ProvenanceProjection]) {
        let Some((first, remaining)) = path.split_first() else {
            *self = Self::independent();
            return;
        };
        if remaining.is_empty() {
            self.projections.remove(first);
        } else if let Some(value) = self.projections.get_mut(first) {
            value.remove_projection(remaining);
        }
    }
}
