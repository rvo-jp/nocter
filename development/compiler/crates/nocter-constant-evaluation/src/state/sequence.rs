//! One phase order for plan validation and execution; source completion is not a resource phase.
use super::{Alternative, Effect, Operation};

pub(super) enum Step<'a> {
    Before(&'a Effect),
    Operation(&'a Operation),
    After(&'a Effect),
}
impl Alternative {
    pub(super) fn sequence<'a>(
        &'a self,
        operation: &'a Operation,
    ) -> impl Iterator<Item = Step<'a>> {
        self.before
            .iter()
            .map(Step::Before)
            .chain(std::iter::once(Step::Operation(operation)))
            .chain(self.after.iter().map(Step::After))
    }
}
