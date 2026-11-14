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
    instance: &'a Instance,
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
        Ok(Self {
            inner,
            id,
            at,
            instance,
        })
    }
    pub(super) fn steps(&mut self, steps: &[ResourceStep]) -> Result<Vec<Step>, Failure> {
        // Preflight metadata matching before the atomic data projection changes its bindings.
        let roots = steps
            .iter()
            .map(|step| {
                let ResourceStep::ReplaceReferent { destination, .. } = step else {
                    return Ok(None);
                };
                let (target, carriers) = self
                    .inner
                    .replacement_carriers(destination)
                    .map_err(|error| failure(self.id, self.at, error))?;
                let before = self
                    .instance
                    .referent_root(self.at, target)
                    .ok_or(Failure::InvalidInstanceProgram)?
                    .clone();
                let after = self
                    .instance
                    .referent_root_after(self.at, target)
                    .ok_or(Failure::InvalidInstanceProgram)?
                    .clone();
                Ok(Some((before, after, carriers)))
            })
            .collect::<Result<Vec<_>, Failure>>()?;
        let projected = self
            .inner
            .steps(steps)
            .map_err(|error| failure(self.id, self.at, error))?;
        Ok(projected
            .into_iter()
            .zip(roots)
            .map(|(step, roots)| match roots {
                Some((before, after, carriers)) => {
                    Step::ScopedReplacement(super::ScopedReplacement {
                        operation: Box::new(step),
                        before,
                        after,
                        carriers,
                    })
                }
                None => Step::Resource(step),
            })
            .collect())
    }
}
