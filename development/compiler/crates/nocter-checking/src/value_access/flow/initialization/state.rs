//! Finite abstract availability. Containers and complete values are distinct queries over one map.
use super::{InitializationState as I, Location, Slot};
use nocter_model::FieldPath;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Storage {
    root: Slot,
    selections: BTreeMap<FieldPath, Slot>,
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
        let mut state = self.root.clone();
        for (prefix, slot) in &self.selections {
            if prefix.is_prefix_of(path) {
                state.and(slot);
            }
        }
        state
    }
    pub(super) fn complete(&self, path: &FieldPath) -> Slot {
        let mut state = self.present(path);
        for (selected, slot) in &self.selections {
            if path.is_prefix_of(selected) {
                state.and(slot);
            }
        }
        state
    }
    pub(super) fn set(&mut self, path: &FieldPath, state: I, at: Option<Location>) {
        let slot = Slot {
            state,
            causes: at.into_iter().collect(),
        };
        if path.is_root() {
            self.root = slot;
            self.selections.clear();
        } else {
            self.selections
                .retain(|selected, _| !path.is_prefix_of(selected));
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
                let joined = self.present(&path).join(&incoming.present(&path));
                (joined != root).then_some((path, joined))
            })
            .collect();
        let next = Self { root, selections };
        let changed = *self != next;
        *self = next;
        changed
    }
}
