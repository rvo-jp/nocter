//! Consume checking's ordered resource data projection; no second source substitution model.
use super::{Condition, Failure, Instance, Location, Resource, ResourceStep, Step};
use nocter_checking::value_access::flow::resources::{Missing, Place};
use nocter_checking::value_access::{ResourceDataFailure, ResourceDataProjection};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_model::AccessInstanceId;
use std::{collections::BTreeMap, sync::Arc};
#[cfg(test)]
mod tests;

pub(super) struct Projection<'a, 'work> {
    inner: ResourceDataProjection<'a, 'work>,
    id: AccessInstanceId,
    at: Location,
    instance: &'a Instance,
    destinations: BTreeMap<Place, Arc<[Missing]>>,
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
        instance: &'a Instance,
        id: AccessInstanceId,
        at: Location,
        condition: &Condition,
        steps: &[ResourceStep],
        work: &'work mut WorkBudget,
    ) -> Result<Self, Failure> {
        let destinations = destinations(steps, condition, work).map_err(|()| Failure::Payload {
            instance: id,
            at,
            failure: nocter_checking::value_access::InstantiationFailure::WorkLimit,
        })?;
        let inner = instance
            .queries
            .resource_data(at, work)
            .map_err(|error| failure(id, at, error))?;
        Ok(Self {
            inner,
            id,
            at,
            instance,
            destinations,
        })
    }
    pub(super) fn steps(&mut self, steps: &[ResourceStep]) -> Result<Vec<Step>, Failure> {
        self.project(steps, false)
    }
    pub(super) fn after_steps(&mut self, steps: &[ResourceStep]) -> Result<Vec<Step>, Failure> {
        self.project(steps, true)
    }
    fn project(&mut self, steps: &[ResourceStep], completed: bool) -> Result<Vec<Step>, Failure> {
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
        let projected = if completed {
            self.inner.after_steps_with_roots(steps)
        } else {
            self.inner.steps_with_roots(steps)
        }
        .map_err(|error| failure(self.id, self.at, error))?;
        projected
            .into_iter()
            .zip(roots)
            .map(|(step, roots)| {
                if let Some((before, after, carriers)) = roots {
                    Ok(Step::ScopedReplacement(super::ScopedReplacement {
                        operation: Box::new(step),
                        before,
                        after,
                        carriers,
                    }))
                } else {
                    let destination_missing = step
                        .destination_roots()
                        .map(|roots| {
                            self.destinations
                                .get(roots.destination())
                                .cloned()
                                .ok_or(Failure::InvalidInstanceProgram)
                        })
                        .transpose()?;
                    Ok(Step::Resource(Resource {
                        data: step,
                        destination_missing,
                    }))
                }
            })
            .collect()
    }
}

/// Freeze only partial receipt metadata before borrowing the shared data-projection budget.
/// The map is discarded with this one action; emitted steps share each immutable annotation.
fn destinations(
    steps: &[ResourceStep],
    condition: &Condition,
    work: &mut WorkBudget,
) -> Result<BTreeMap<Place, Arc<[Missing]>>, ()> {
    let mut output = BTreeMap::new();
    for step in steps {
        let ResourceStep::Move { destination, .. } = step else {
            continue;
        };
        if destination.fields().is_root() || output.contains_key(destination) {
            continue;
        }
        work.step().map_err(|_| ())?;
        for _ in destination.fields().fields() {
            work.step().map_err(|_| ())?;
        }
        for (holder, missing) in condition.owned() {
            work.step().map_err(|_| ())?;
            if holder != destination.holder() {
                continue;
            }
            for hole in missing {
                work.step().map_err(|_| ())?;
                let path = match hole {
                    Missing::Value(path) => path,
                    Missing::ArrayPrefix { array, .. } => array,
                };
                for _ in path.fields() {
                    work.step().map_err(|_| ())?;
                }
            }
            output.insert(destination.clone(), Arc::from(missing));
            break;
        }
    }
    Ok(output)
}
