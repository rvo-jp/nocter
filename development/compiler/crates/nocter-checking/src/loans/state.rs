use std::collections::BTreeMap;

use super::liveness::{LivePlace, LiveSlot};
use super::value::LoanValue;
use crate::{PlaceRoot, ProvenanceProjection};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct LoanState {
    values: BTreeMap<LiveSlot, LoanValue>,
}

impl LoanState {
    pub(super) fn value(&self, slot: &LiveSlot) -> LoanValue {
        match slot {
            LiveSlot::Node(_) => self.values.get(slot).cloned().unwrap_or_default(),
            LiveSlot::Place(place) => self.place_value(place),
        }
    }

    pub(super) fn set_node(&mut self, node: nocter_model::BodyNodeId, value: LoanValue) {
        self.values.insert(LiveSlot::Node(node), value);
    }

    pub(super) fn set_root(&mut self, root: PlaceRoot, value: LoanValue) {
        self.values.insert(
            LiveSlot::Place(LivePlace::from_parts(root, Box::new([]))),
            value,
        );
    }

    pub(super) fn set_place(&mut self, place: &LivePlace, value: LoanValue) {
        let root = LiveSlot::Place(LivePlace::from_parts(place.root(), Box::new([])));
        let mut current = self.values.get(&root).cloned().unwrap_or_default();
        let path = place
            .fields()
            .iter()
            .copied()
            .map(ProvenanceProjection::Field)
            .collect::<Vec<_>>();
        current.replace_projection(&path, value);
        self.values.insert(root, current);
    }

    pub(super) fn remove_place(&mut self, place: &LivePlace) {
        let root = LiveSlot::Place(LivePlace::from_parts(place.root(), Box::new([])));
        let Some(mut current) = self.values.get(&root).cloned() else {
            return;
        };
        let path = place
            .fields()
            .iter()
            .copied()
            .map(ProvenanceProjection::Field)
            .collect::<Vec<_>>();
        current.remove_projection(&path);
        self.values.insert(root, current);
    }

    pub(super) fn remove_root(&mut self, root: PlaceRoot) {
        self.values
            .remove(&LiveSlot::Place(LivePlace::from_parts(root, Box::new([]))));
    }

    pub(super) fn join(&mut self, incoming: &[Self]) -> bool {
        let mut changed = false;
        for state in incoming {
            for (slot, value) in &state.values {
                if let Some(current) = self.values.get_mut(slot) {
                    changed |= current.union_with(value);
                } else {
                    self.values.insert(slot.clone(), value.clone());
                    changed = true;
                }
            }
        }
        changed
    }

    fn place_value(&self, place: &LivePlace) -> LoanValue {
        let root = LiveSlot::Place(LivePlace::from_parts(place.root(), Box::new([])));
        let mut value = self.values.get(&root).cloned().unwrap_or_default();
        for field in place.fields() {
            value = value.projected(ProvenanceProjection::Field(*field));
        }
        value
    }
}
