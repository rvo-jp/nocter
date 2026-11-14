//! Current representation views retain immutable storage roles and one logical slot identity.
use super::Slot;
use std::{collections::BTreeMap, sync::Arc};
// None is unresolved planning information, never a fabricated evaluator type or value.
pub(super) type Contexts = Arc<BTreeMap<usize, Option<usize>>>;

pub(super) trait SlotTypes {
    fn get(&self, slot: usize) -> Option<Slot>;
}
impl SlotTypes for [Slot] {
    fn get(&self, slot: usize) -> Option<Slot> {
        self.get(slot).copied()
    }
}
pub(super) struct View<'a> {
    pub(super) initial: &'a [Slot],
    pub(super) contexts: &'a Contexts,
}
impl SlotTypes for View<'_> {
    fn get(&self, slot: usize) -> Option<Slot> {
        let mut definition = *self.initial.get(slot)?;
        if let Some(&data) = self.contexts.get(&slot) {
            definition.data = data?;
        }
        Some(definition)
    }
}
