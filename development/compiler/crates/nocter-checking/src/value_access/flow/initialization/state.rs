//! Finite abstract availability. Containers and complete values are distinct queries over one map.
use super::{InitializationState as I, Location, Slot};
use nocter_model::FieldPath;
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Storage {
    root: Slot,
    selections: BTreeMap<crate::value_access::AccessPath, Slot>,
}
impl Slot {
    pub(super) fn new(state: I) -> Self {
        Self {
            state,
            causes: BTreeSet::new(),
        }
    }
    fn join(&self, other: &Self) -> Self {
        Self {
            state: self.state.join(other.state),
            causes: self.causes.union(&other.causes).copied().collect(),
        }
    }
    fn and(&mut self, other: &Self) {
        self.state = match (self.state, other.state) {
            (I::Uninitialized, _) | (_, I::Uninitialized) => I::Uninitialized,
            (I::Initialized, I::Initialized) => I::Initialized,
            _ => I::Maybe,
        };
        self.causes.extend(&other.causes);
    }
}
impl Storage {
    pub(super) fn new(state: I) -> Self {
        Self {
            root: Slot::new(state),
            selections: BTreeMap::new(),
        }
    }
    pub(super) fn present(&self, path: &FieldPath) -> Slot {
        self.selection(
            &crate::value_access::paths::known(path),
            super::InitializationRequirement::Container,
        )
    }
    pub(super) fn complete(&self, path: &FieldPath) -> Slot {
        self.selection(
            &crate::value_access::paths::known(path),
            super::InitializationRequirement::Value,
        )
    }
    pub(super) fn selection(
        &self,
        path: &crate::value_access::AccessPath,
        requirement: super::InitializationRequirement,
    ) -> Slot {
        use crate::value_access::paths;
        let mut state = self.root.clone();
        for (selected, slot) in &self.selections {
            // Deeper missing children do not remove a containing place. A complete value must
            // instead exclude every possibly overlapping hole, including unknown indices.
            if (requirement == super::InitializationRequirement::Value
                || selected.fields().len() <= path.fields().len())
                && paths::relation(selected, path) != nocter_model::PathRelation::Disjoint
            {
                state.and(slot);
            }
        }
        state
    }
    pub(super) fn set(&mut self, path: &FieldPath, state: I, at: Option<Location>) {
        self.set_path(&crate::value_access::paths::known(path), state, at);
    }
    pub(super) fn remove_path(&mut self, path: &crate::value_access::AccessPath, at: Location) {
        self.set_path(path, I::Uninitialized, Some(at));
    }
    /// The caller proved one matching committed point and a present parent. Unknown key
    /// equality alone cannot close a hole, and conditional completeness is not repaired here.
    pub(super) fn restore_point(
        &mut self,
        path: &crate::value_access::AccessPath,
        retirements: &BTreeSet<Location>,
    ) -> bool {
        let Some(slot) = self.point_from_sources(path, retirements) else {
            return false;
        };
        if slot.state != I::Uninitialized {
            return false;
        }
        self.selections.remove(path);
        true
    }
    fn point_from_sources(
        &self,
        path: &crate::value_access::AccessPath,
        retirements: &BTreeSet<Location>,
    ) -> Option<&Slot> {
        self.selections.get(path).filter(|slot| {
            slot.state != I::Initialized
                && !slot.causes.is_empty()
                && slot.causes.is_subset(retirements)
        })
    }
    pub(super) fn restore_conditional_point(
        &mut self,
        parent: &FieldPath,
        suffix: &FieldPath,
        retirements: &BTreeSet<Location>,
    ) -> bool {
        let array = crate::value_access::paths::known(parent);
        let suffix = crate::value_access::paths::known(suffix);
        let mut closed = Vec::new();
        for (path, slot) in &self.selections {
            let relation = crate::value_access::paths::relation(&array, path);
            if slot.state == I::Initialized || relation == nocter_model::PathRelation::Disjoint {
                continue;
            }
            if relation != nocter_model::PathRelation::Ancestor
                || !path.matches_array_element_shape(&array, &suffix)
                || self.point_from_sources(path, retirements).is_none()
            {
                return false;
            }
            closed.push(path.clone());
        }
        for path in &closed {
            self.selections.remove(path);
        }
        !closed.is_empty()
    }
    pub(super) fn uncertain_replacement(&self, path: &FieldPath) -> bool {
        let path = crate::value_access::paths::known(path);
        self.selections.iter().any(|(hole, slot)| {
            slot.state != I::Initialized
                && crate::value_access::paths::relation(hole, &path)
                    == nocter_model::PathRelation::MayOverlap
        })
    }
    fn set_path(&mut self, path: &crate::value_access::AccessPath, state: I, at: Option<Location>) {
        let slot = Slot {
            state,
            causes: at.into_iter().collect(),
        };
        if path.is_root() {
            self.root = slot;
            self.selections.clear();
        } else {
            self.selections.retain(|selected, _| {
                !matches!(
                    crate::value_access::paths::relation(path, selected),
                    nocter_model::PathRelation::Equal | nocter_model::PathRelation::Ancestor
                )
            });
            self.selections.insert(path.clone(), slot);
        }
    }
    pub(super) fn merge(&mut self, incoming: &Self) -> bool {
        let root = self.root.join(&incoming.root);
        let keys: BTreeSet<_> = self
            .selections
            .keys()
            .chain(incoming.selections.keys())
            .cloned()
            .collect();
        let selections = keys
            .into_iter()
            .filter_map(|path| {
                let joined = self
                    .selection(&path, super::InitializationRequirement::Container)
                    .join(&incoming.selection(&path, super::InitializationRequirement::Container));
                (joined != root).then_some((path, joined))
            })
            .collect();
        let next = Self { root, selections };
        let changed = *self != next;
        *self = next;
        changed
    }
}
