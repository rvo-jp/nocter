use std::collections::BTreeMap;

use nocter_model::LocalBindingId;

use crate::{PlaceRoot, ProvenanceSource, ValueProvenance};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProvenanceState {
    values: BTreeMap<PlaceRoot, ValueProvenance>,
    current_allocation: ValueProvenance,
}

impl Default for ProvenanceState {
    fn default() -> Self {
        Self {
            values: BTreeMap::new(),
            current_allocation: ValueProvenance::from_source(ProvenanceSource::CurrentAllocation),
        }
    }
}

impl ProvenanceState {
    pub(super) fn value(&self, root: PlaceRoot) -> ValueProvenance {
        self.values
            .get(&root)
            .cloned()
            .unwrap_or_else(|| ValueProvenance::from_source(ProvenanceSource::Unknown))
    }

    pub(super) fn set_value(&mut self, root: PlaceRoot, value: ValueProvenance) {
        self.values.insert(root, value);
    }

    pub(super) fn remove(&mut self, root: PlaceRoot) {
        self.values.remove(&root);
    }

    pub(super) fn values(&self) -> impl Iterator<Item = (PlaceRoot, &ValueProvenance)> {
        self.values.iter().map(|(root, value)| (*root, value))
    }

    pub(super) const fn current_allocation(&self) -> &ValueProvenance {
        &self.current_allocation
    }

    pub(super) fn enter_region(&mut self, region: LocalBindingId) -> ValueProvenance {
        std::mem::replace(
            &mut self.current_allocation,
            ValueProvenance::from_source(ProvenanceSource::Region(region)),
        )
    }

    pub(super) fn leave_region(&mut self, previous: ValueProvenance) {
        self.current_allocation = previous;
    }

    pub(super) fn join(&mut self, incoming: &[Self]) -> bool {
        let mut changed = false;
        for state in incoming {
            for (root, value) in &state.values {
                changed |= self.values.entry(*root).or_default().union_with(value);
            }
            changed |= self
                .current_allocation
                .union_with(&state.current_allocation);
        }
        changed
    }
}
