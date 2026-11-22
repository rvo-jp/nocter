//! Elision retains logical ownership and requires every actual future use to stay unstarted.
use super::{Failure, INVALID, reject};
use crate::value_access::{
    flow::{
        self, Location, Operation,
        resources::{Disposal, Holder, Payload, Step},
    },
    results,
};
use nocter_model::{AccessCodeId, TypeId, ValueCode};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default)]
pub(in crate::value_access::accepted) struct Elisions {
    bodies: BTreeMap<ValueCode, BTreeSet<TypeId>>,
}

impl Elisions {
    pub(in crate::value_access::accepted) fn contains(
        &self,
        code: ValueCode,
        data: TypeId,
    ) -> bool {
        self.bodies
            .get(&code)
            .is_some_and(|set| set.contains(&data))
    }

    pub(in crate::value_access::accepted) fn body(
        &self,
        code: ValueCode,
    ) -> Option<&BTreeSet<TypeId>> {
        self.bodies.get(&code)
    }

    pub(in crate::value_access::accepted) fn step(&self, code: ValueCode, step: &Step) -> bool {
        local_payload(step).is_some_and(|payload| self.contains(code, payload.data()))
    }

    pub(in crate::value_access::accepted) fn prepare<'a>(
        bodies: impl IntoIterator<Item = &'a flow::Query>,
        verified: &results::Verified<'_, '_>,
    ) -> Result<Self, Failure> {
        let bodies = bodies
            .into_iter()
            .try_fold(BTreeMap::new(), |mut all, body| {
                if let Some(elided) = body_data(body, verified)? {
                    all.insert(body.typed().code(), elided);
                }
                Ok::<_, Failure>(all)
            })?;
        Ok(Self { bodies })
    }
}

fn body_data(
    body: &flow::Query,
    verified: &results::Verified<'_, '_>,
) -> Result<Option<BTreeSet<TypeId>>, Failure> {
    let mut data = BTreeSet::new();
    if !body.graph().points().iter().any(|(point, block)| {
        body.initialization().reachable(point)
            && block
                .actions
                .iter()
                .any(|action| matches!(action.operation, Operation::ConstructFuture(_)))
    }) {
        return Ok(None);
    }
    let program = body.typed().program();
    for (point, block) in body.graph().points().iter() {
        if !body.initialization().reachable(point) {
            continue;
        }
        for (action, event) in block.actions.iter().enumerate() {
            let location = Location { point, action };
            for transition in body.access().resources().at(location) {
                for step in transition.steps() {
                    if let Some(payload) = local_payload(step) {
                        if let Step::Construct { destination, .. } = step
                            && !matches!(event.operation, Operation::ConstructFuture(call) if *destination == Holder::Expression(call))
                        {
                            return Err(INVALID);
                        }
                        let Disposal::UnstartedFuture(producer) = payload.disposal() else {
                            return Err(INVALID);
                        };
                        if !matches!(
                            program.code_execution(producer),
                            Some(nocter_declarations::CallableExecution::Deferred { .. })
                        ) {
                            return Err(INVALID);
                        }
                        data.insert(payload.data());
                    } else if mentions_future(step) {
                        return Err(reject(
                            verified,
                            body,
                            location,
                            "this future use requires a runtime representation",
                        ));
                    }
                }
            }
        }
    }
    if data.is_empty() {
        return Err(INVALID);
    }
    Ok(Some(data))
}

fn local_payload(step: &Step) -> Option<Payload> {
    let local = |holder| matches!(holder, Holder::Local(_) | Holder::Expression(_));
    let payload = match step {
        Step::Construct {
            destination: Holder::Expression(_),
            payload,
        } => *payload,
        Step::Move {
            source,
            destination,
            payload,
        } if source.fields().is_root()
            && destination.fields().is_root()
            && local(source.holder())
            && local(destination.holder()) =>
        {
            *payload
        }
        Step::Dispose { source, payload }
            if source.fields().is_root() && local(source.holder()) =>
        {
            *payload
        }
        _ => return None,
    };
    matches!(payload.disposal(), Disposal::UnstartedFuture(_)).then_some(payload)
}

fn mentions_future(step: &Step) -> bool {
    step.try_map_payloads(|payload| match payload.disposal() {
        Disposal::UnstartedFuture(producer) => Err::<(), AccessCodeId>(producer),
        _ => Ok(()),
    })
    .is_err()
}
