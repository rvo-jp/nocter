//! Consume checking's ordered resource data projection; no second source substitution model.
use super::{Condition, Failure, Instance, Location, Resource, ResourceStep, Step};
use nocter_checking::value_access::accepted::{LocalFutureMove, LocalFutureStorage, Unit};
use nocter_checking::value_access::flow::resources::{Holder, Missing, Place, Transition};
use nocter_checking::value_access::{ResourceDataFailure, ResourceDataProjection};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_model::{AccessInstanceId, ValueCode};
use std::{borrow::Cow, collections::BTreeMap, sync::Arc};
#[cfg(test)]
mod tests;

pub(super) struct Projection<'a, 'work> {
    inner: ResourceDataProjection<'a, 'work>,
    id: AccessInstanceId,
    at: Location,
    instance: &'a Instance,
    destinations: BTreeMap<Place, Arc<[Missing]>>,
    stable: BTreeMap<Holder, Arc<[Missing]>>,
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
        transition: &Transition,
        work: &'work mut WorkBudget,
    ) -> Result<Self, Failure> {
        let condition = transition.condition();
        let steps = transition.steps();
        let stable = stable_carriers(transition, work).map_err(|()| Failure::Payload {
            instance: id,
            at,
            failure: nocter_checking::value_access::InstantiationFailure::WorkLimit,
        })?;
        let destinations =
            destinations(steps, &condition, work).map_err(|()| Failure::Payload {
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
            stable,
        })
    }
    pub(super) fn steps(&mut self, steps: &[ResourceStep]) -> Result<Vec<Step>, Failure> {
        self.project(steps, false)
    }
    pub(super) fn after_steps(&mut self, steps: &[ResourceStep]) -> Result<Vec<Step>, Failure> {
        self.project(steps, true)
    }
    fn project(&mut self, steps: &[ResourceStep], completed: bool) -> Result<Vec<Step>, Failure> {
        let unit = self.instance.queries.unit();
        let code = self.instance.key().code();
        let (steps, activations) = partition_local_future_steps(unit, code, self.at, steps)?;
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
            self.inner.after_steps_with_roots(&steps)
        } else {
            self.inner.steps_with_roots(&steps)
        }
        .map_err(|error| failure(self.id, self.at, error))?;
        let ordinary = projected
            .into_iter()
            .zip(roots)
            .map(|(step, roots)| {
                if let Some((before, after, carriers)) = roots {
                    let carrier_missing = carriers
                        .before()
                        .iter()
                        .filter_map(|carrier| {
                            let nocter_checking::value_access::ValueSourceSlot::Local(local) =
                                carrier.selection().slot
                            else {
                                return None;
                            };
                            let holder = Holder::Local(local);
                            self.stable
                                .get(&holder)
                                .map(|missing| (holder, Arc::clone(missing)))
                        })
                        .collect();
                    Ok(Step::ScopedReplacement(super::ScopedReplacement {
                        operation: Box::new(step),
                        carrier_missing,
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
            .collect::<Result<Vec<_>, Failure>>()?;
        if activations.is_empty() {
            return Ok(ordinary);
        }
        let mut output = Vec::with_capacity(ordinary.len() + activations.len());
        let mut activations = activations.into_iter().peekable();
        for (index, step) in ordinary.into_iter().enumerate() {
            while let Some((_, selected)) = activations.next_if(|(before, _)| *before == index) {
                output.push(selected);
            }
            output.push(step);
        }
        output.extend(activations.map(|(_, step)| step));
        Ok(output)
    }
}

type Partition<'a> = (Cow<'a, [ResourceStep]>, Vec<(usize, Step)>);
fn partition_local_future_steps<'a>(
    unit: &Unit,
    code: ValueCode,
    at: Location,
    steps: &'a [ResourceStep],
) -> Result<Partition<'a>, Failure> {
    let mut activations = Vec::new();
    if !steps
        .iter()
        .any(|step| unit.future_step_storage(code, step).is_some())
    {
        return Ok((Cow::Borrowed(steps), activations));
    }
    let mut ordinary = Vec::new();
    for step in steps {
        match (unit.future_step_storage(code, step), step) {
            (None, _) => ordinary.push(step.clone()),
            (
                Some(backing),
                ResourceStep::Move {
                    source,
                    destination,
                    ..
                },
            ) => {
                let selector = backing == LocalFutureStorage::Selector;
                let source = source.holder();
                let destination = destination.holder();
                let captures = unit
                    .future_move(code, at, source, destination)
                    .ok_or(Failure::InvalidInstanceProgram)?;
                if selector
                    || !matches!(captures, LocalFutureMove::Direct(fields) if fields.is_empty())
                {
                    activations.push((
                        ordinary.len(),
                        Step::MoveLocalFuture {
                            source,
                            destination,
                            selector,
                            captures: captures.clone(),
                        },
                    ));
                }
            }
            (
                Some(_),
                ResourceStep::ConstructActivation {
                    destination,
                    inputs,
                    ..
                },
            ) if !inputs.is_empty() => {
                let inputs = inputs
                    .iter()
                    .map(|(source, payload)| {
                        if !source.fields().is_root()
                            || !matches!(source.holder(), Holder::Prepared { .. })
                        {
                            return Err(Failure::InvalidInstanceProgram);
                        }
                        Ok((source.holder(), payload.data()))
                    })
                    .collect::<Result<Box<[_]>, Failure>>()?;
                activations.push((
                    ordinary.len(),
                    Step::CaptureFutureInputs {
                        destination: *destination,
                        inputs,
                    },
                ));
            }
            (Some(_), _) => {}
        }
    }
    Ok((Cow::Owned(ordinary), activations))
}

/// Stable sparse exclusions come from one matched original transition. This freezes metadata,
/// without replaying ownership, inventing source initialization or selecting a physical layout.
fn stable_carriers(
    transition: &Transition,
    work: &mut WorkBudget,
) -> Result<BTreeMap<Holder, Arc<[Missing]>>, ()> {
    if !transition
        .steps()
        .iter()
        .any(|step| matches!(step, ResourceStep::ReplaceReferent { .. }))
    {
        return Ok(BTreeMap::new());
    }
    let mut stable = BTreeMap::new();
    for (holder, missing) in transition.operation_condition().owned() {
        work.step().map_err(|_| ())?;
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
        if transition
            .missing_before(holder)
            .is_some_and(|paths| paths.eq(missing.iter()))
            && transition
                .missing_after(holder)
                .is_some_and(|paths| paths.eq(missing.iter()))
        {
            stable.insert(holder, Arc::from(missing));
        }
    }
    Ok(stable)
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
