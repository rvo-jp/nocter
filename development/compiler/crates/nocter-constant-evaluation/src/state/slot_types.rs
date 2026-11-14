//! Current representation views retain immutable storage roles and one logical slot identity.
use super::{Place, Program, Rule, Slot};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
// Candidates name existing data types; their union is never a fabricated evaluator type or value.
pub(super) type Contexts = Arc<BTreeMap<usize, Arc<BTreeSet<usize>>>>;

pub(super) trait SlotTypes {
    fn get(&self, slot: usize) -> Option<Slot>;
    fn select(&self, program: &Program, place: &Place) -> Result<usize, Rule> {
        let data = self.get(place.slot).ok_or(Rule::InvalidPlan)?.data;
        super::validation::project_data(program, self, place, data)
    }
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
        if let Some(candidates) = self.contexts.get(&slot) {
            if candidates.len() != 1 {
                return None;
            }
            definition.data = *candidates.first()?;
        }
        Some(definition)
    }
    fn select(&self, program: &Program, place: &Place) -> Result<usize, Rule> {
        let initial = self.initial.get(place.slot).ok_or(Rule::InvalidPlan)?;
        let Some(candidates) = self.contexts.get(&place.slot) else {
            return super::validation::project_data(program, self, place, initial.data);
        };
        let mut selected = None;
        for &data in candidates.iter() {
            let data = super::validation::project_data(program, self, place, data)?;
            if selected.is_some_and(|previous| previous != data) {
                return Err(Rule::InvalidPlan);
            }
            selected = Some(data);
        }
        selected.ok_or(Rule::InvalidPlan)
    }
}
