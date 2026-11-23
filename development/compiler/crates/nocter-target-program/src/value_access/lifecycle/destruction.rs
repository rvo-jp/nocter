//! Close destructor bodies selected by the frozen retirement schedules.
use super::{Body, Failure, INVALID, InstanceProgram, Location, Step, lower_body};
use crate::value_access::{InstanceKey, build::Builder};
use nocter_checking::value_access::{
    InstantiationFailure, accepted::Unit, flow::resources::Disposal,
};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::DataInstanceKey;
use nocter_model::AccessInstanceId;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[cfg(test)]
mod tests;

pub(super) fn extend(
    instances: Arc<InstanceProgram>,
    bodies: &mut BTreeMap<AccessInstanceId, Body>,
    work: &mut WorkBudget,
) -> Result<Arc<InstanceProgram>, Failure> {
    let unit = &instances.unit;
    if !unit.program().has_destructors() {
        return Ok(instances);
    }
    let mut retired = Retirements {
        seen: BTreeSet::new(),
        qualified: unit.destructor_completions().clone(),
        new: BTreeSet::new(),
    };
    for (&id, body) in bodies.iter() {
        retired.body(unit, id, body, work)?;
    }
    if retired.new.is_empty() {
        return Ok(instances);
    }
    let mut builder = Builder::retain(&instances);
    while !retired.new.is_empty() {
        for frame in std::mem::take(&mut retired.new) {
            let completed = retired.qualified.get(&frame).ok_or(INVALID)?.clone();
            builder
                .add_destructor(frame, completed)
                .map_err(Failure::Body)?;
        }
        while let Some((id, instance)) = builder.next(unit, work).map_err(Failure::Body)? {
            let body = lower_body(unit, id, instance, work)?;
            retired.body(unit, id, &body, work)?;
            bodies.insert(id, body);
        }
    }
    builder
        .finish(Arc::clone(unit), instances.root())
        .map(Arc::new)
        .map_err(Failure::Body)
}

struct Retirements {
    seen: BTreeSet<DataInstanceKey>,
    qualified: BTreeMap<InstanceKey, DataInstanceKey>,
    new: BTreeSet<InstanceKey>,
}

impl Retirements {
    fn body(
        &mut self,
        unit: &Unit,
        instance: AccessInstanceId,
        body: &Body,
        work: &mut WorkBudget,
    ) -> Result<(), Failure> {
        for (&at, action) in body.actions() {
            let mut require = |data: &DataInstanceKey| self.require(unit, instance, at, data, work);
            for alternative in action.alternatives() {
                for step in alternative.before().iter().chain(alternative.after()) {
                    let payload = match step {
                        Step::DisposeEnvironment { payload, .. } => Some(payload),
                        Step::CancelLocalFuture { captures, .. } => {
                            for input in captures.fields() {
                                let data = unit
                                    .program()
                                    .instantiate_closed_data(input.payload.data())
                                    .map_err(|_| INVALID)?;
                                require(&data)?;
                            }
                            None
                        }
                        _ => step.resource().and_then(|step| step.retired_payload()),
                    };
                    if let Some(payload) =
                        payload.filter(|payload| payload.disposal() != Disposal::Trivial)
                    {
                        require(payload.instance())?;
                    }
                }
            }
        }
        Ok(())
    }

    fn require(
        &mut self,
        unit: &Unit,
        instance: AccessInstanceId,
        at: Location,
        data: &DataInstanceKey,
        work: &mut WorkBudget,
    ) -> Result<(), Failure> {
        let program = unit.program();
        let mut pending = vec![data.clone()];
        while let Some(data) = pending.pop() {
            if data.is_uninhabited() || !self.seen.insert(data.clone()) {
                continue;
            }
            work.step().map_err(|_| Failure::Payload {
                instance,
                at,
                failure: InstantiationFailure::WorkLimit,
            })?;
            let cleanup = if program.data_destructor(data.data()).is_some() {
                let key = program.bind_destructor(&data).map_err(|_| INVALID)?;
                if !self.qualified.contains_key(&key) {
                    self.qualified.extend(
                        unit.qualify_destructor(data, work)
                            .map_err(Failure::Destructor)?,
                    );
                }
                self.new.insert(key.clone());
                self.qualified.get(&key).ok_or(INVALID)?
            } else {
                &data
            };
            pending.extend(cleanup.cleanup_components(program).map_err(|_| INVALID)?);
        }
        Ok(())
    }
}
