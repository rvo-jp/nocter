//! Consume checking's ordered resource data projection; no second source substitution model.
use super::{Failure, Instance, InstanceProgram, Location, ResourceStep, Step};
use nocter_checking::value_access::{ResourceDataFailure, ResourceDataProjection};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_model::AccessInstanceId;
#[cfg(test)]
mod tests;

pub(super) struct Projection<'a, 'work> {
    inner: ResourceDataProjection<'a, 'work>,
    id: AccessInstanceId,
    at: Location,
}
fn failure(id: AccessInstanceId, at: Location, error: ResourceDataFailure) -> Failure {
    match error {
        ResourceDataFailure::Integrity => Failure::InvalidInstanceProgram,
        ResourceDataFailure::Instantiation(failure) => Failure::Payload {
            instance: id,
            at,
            failure,
        },
    }
}
impl<'a, 'work> Projection<'a, 'work> {
    pub(super) fn new(
        instances: &'a InstanceProgram,
        instance: &'a Instance,
        id: AccessInstanceId,
        at: Location,
        work: &'work mut WorkBudget,
    ) -> Result<Self, Failure> {
        let inner = instances
            .unit()
            .project_resource_data(instance.key(), at, work)
            .map_err(|error| failure(id, at, error))?;
        Ok(Self { inner, id, at })
    }
    pub(super) fn steps(&mut self, steps: &[ResourceStep]) -> Result<Vec<Step>, Failure> {
        self.inner
            .steps(steps)
            .map(|steps| steps.into_iter().map(Step::Resource).collect())
            .map_err(|error| failure(self.id, self.at, error))
    }
}
