//! One phase order for plan validation and execution; source completion is not a resource phase.
use super::{Alternative, Effect, Operation, Place};

pub(super) enum Step<'a> {
    Before(&'a Effect),
    Operation(&'a Operation),
    After(&'a Effect),
    Context(&'a Place, usize),
}
impl Alternative {
    pub(super) fn sequence<'a>(
        &'a self,
        operation: &'a Operation,
    ) -> impl Iterator<Item = Step<'a>> {
        self.before
            .iter()
            .map(|effect| match effect {
                Effect::Context { place, data } => Step::Context(place, *data),
                _ => Step::Before(effect),
            })
            .chain(std::iter::once(Step::Operation(operation)))
            .chain(self.after.iter().map(|effect| match effect {
                Effect::Context { place, data } => Step::Context(place, *data),
                _ => Step::After(effect),
            }))
    }
}
