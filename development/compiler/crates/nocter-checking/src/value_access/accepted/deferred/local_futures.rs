//! Close captured input layouts and optional producer selectors for actual local activations.
use super::super::LocalFutureStorage;
use super::{Failure, INVALID, reject};
use crate::value_access::{
    flow::{
        self, Location, Operation,
        resources::{Disposal, Holder, Payload, Step},
    },
    results,
};
use nocter_model::{AccessCodeId, AccessExpressionId, TypeId, ValueCode};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub(in crate::value_access::accepted) struct LocalFutures {
    bodies: BTreeMap<ValueCode, Body>,
}

#[derive(Clone, Debug, Default)]
struct Body {
    data: BTreeSet<TypeId>,
    tagged: BTreeSet<TypeId>,
    drives: BTreeMap<AccessExpressionId, Drive>,
    constructions: BTreeMap<AccessExpressionId, (TypeId, AccessCodeId)>,
    tags: BTreeMap<AccessCodeId, u64>,
    captures: BTreeMap<Holder, Arc<[TypeId]>>,
}

#[derive(Clone, Debug)]
struct Drive {
    data: TypeId,
    producers: BTreeSet<AccessCodeId>,
}

impl LocalFutures {
    pub(in crate::value_access::accepted) fn contains(
        &self,
        code: ValueCode,
        data: TypeId,
    ) -> bool {
        self.body(code).is_some_and(|set| set.contains(&data))
    }

    pub(in crate::value_access::accepted) fn body(
        &self,
        code: ValueCode,
    ) -> Option<&BTreeSet<TypeId>> {
        self.bodies.get(&code).map(|body| &body.data)
    }

    pub(in crate::value_access::accepted) fn producers(
        &self,
        code: ValueCode,
        result: AccessExpressionId,
    ) -> Option<&BTreeSet<AccessCodeId>> {
        Some(&self.bodies.get(&code)?.drives.get(&result)?.producers)
    }

    pub(in crate::value_access::accepted) fn tag(
        &self,
        code: ValueCode,
        producer: AccessCodeId,
    ) -> Option<u64> {
        self.bodies.get(&code)?.tags.get(&producer).copied()
    }

    pub(in crate::value_access::accepted) fn construction_tag(
        &self,
        code: ValueCode,
        expression: AccessExpressionId,
    ) -> Option<u64> {
        let body = self.bodies.get(&code)?;
        let &(data, producer) = body.constructions.get(&expression)?;
        if !body.tagged.contains(&data) {
            return None;
        }
        body.tags.get(&producer).copied()
    }

    pub(in crate::value_access::accepted) fn has_tags(&self, code: ValueCode) -> bool {
        self.bodies
            .get(&code)
            .is_some_and(|body| !body.tagged.is_empty())
    }
    pub(in crate::value_access::accepted) fn captures(
        &self,
        code: ValueCode,
        holder: Holder,
    ) -> Option<&[TypeId]> {
        self.bodies
            .get(&code)?
            .captures
            .get(&holder)
            .map(AsRef::as_ref)
    }

    pub(in crate::value_access::accepted) fn step(
        &self,
        code: ValueCode,
        step: &Step,
    ) -> Option<LocalFutureStorage> {
        let data = local_payload(step)?.data();
        let body = self.bodies.get(&code)?;
        if !body.data.contains(&data) {
            return None;
        }
        Some(if body.tagged.contains(&data) {
            LocalFutureStorage::Selector
        } else {
            LocalFutureStorage::Direct
        })
    }

    pub(in crate::value_access::accepted) fn prepare<'a>(
        bodies: impl IntoIterator<Item = &'a flow::Query>,
        verified: &results::Verified<'_, '_>,
    ) -> Result<Self, Failure> {
        let bodies = bodies
            .into_iter()
            .try_fold(BTreeMap::new(), |mut all, body| {
                if let Some(futures) = inspect(body, verified)? {
                    all.insert(body.typed().code(), futures);
                }
                Ok::<_, Failure>(all)
            })?;
        Ok(Self { bodies })
    }
}

fn inspect(
    body: &flow::Query,
    verified: &results::Verified<'_, '_>,
) -> Result<Option<Body>, Failure> {
    let mut found = Body::default();
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
    let mut signatures = BTreeMap::new();
    for (point, block) in body.graph().points().iter() {
        if !body.initialization().reachable(point) {
            continue;
        }
        for (action, event) in block.actions.iter().enumerate() {
            let location = Location { point, action };
            for transition in body.access().resources().at(location) {
                for step in transition.steps() {
                    if let Some(payload) = local_payload(step) {
                        let Disposal::UnstartedFuture(producer) = payload.disposal() else {
                            return Err(INVALID);
                        };
                        if !matches!(
                            program.code_execution(producer),
                            Some(nocter_declarations::CallableExecution::Deferred { .. })
                        ) {
                            return Err(INVALID);
                        }
                        let signature = match signatures.entry(producer) {
                            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                            std::collections::btree_map::Entry::Vacant(entry) => {
                                entry.insert(inputs(program, producer)?)
                            }
                        };
                        if !found.match_captures(step, signature)? {
                            return Err(reject(
                                verified,
                                body,
                                location,
                                "this future holder needs a layout for different captured inputs",
                            ));
                        }
                        found.observe(step, &event.operation, payload, producer)?;
                        found.data.insert(payload.data());
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
    if found.data.is_empty() {
        return Err(INVALID);
    }
    found.tagged.extend(
        found
            .drives
            .values()
            .filter(|drive| drive.producers.len() > 1)
            .map(|drive| drive.data),
    );
    let producers = found
        .constructions
        .values()
        .filter_map(|&(data, producer)| found.tagged.contains(&data).then_some(producer))
        .collect::<BTreeSet<_>>();
    found.tags = producers.into_iter().zip(0_u64..).collect();
    Ok(Some(found))
}

impl Body {
    fn match_captures(&mut self, step: &Step, signature: &Arc<[TypeId]>) -> Result<bool, Failure> {
        let (source, destination) = match step {
            Step::ConstructActivation {
                destination,
                inputs,
                ..
            } => {
                if inputs
                    .iter()
                    .map(|(_, payload)| payload.data())
                    .ne(signature.iter().copied())
                {
                    return Err(INVALID);
                }
                (None, Some(*destination))
            }
            Step::Move {
                source,
                destination,
                ..
            } => (Some(source.holder()), Some(destination.holder())),
            Step::Dispose { source, .. } | Step::DriveFuture { source, .. } => {
                (Some(source.holder()), None)
            }
            _ => return Err(INVALID),
        };
        for holder in [source, destination].into_iter().flatten() {
            let known = self
                .captures
                .entry(holder)
                .or_insert_with(|| Arc::clone(signature));
            if known.as_ref() != signature.as_ref() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn observe(
        &mut self,
        step: &Step,
        operation: &Operation,
        payload: Payload,
        producer: AccessCodeId,
    ) -> Result<(), Failure> {
        match (step, operation) {
            (Step::ConstructActivation { destination, .. }, Operation::ConstructFuture(call))
                if *destination == Holder::Expression(*call) =>
            {
                if self
                    .constructions
                    .insert(*call, (payload.data(), producer))
                    .is_some_and(|previous| previous != (payload.data(), producer))
                {
                    return Err(INVALID);
                }
            }
            (Step::DriveFuture { source, .. }, Operation::DriveFuture { future, result })
                if *source == Holder::Expression(*future).place() =>
            {
                let drive = self.drives.entry(*result).or_insert_with(|| Drive {
                    data: payload.data(),
                    producers: BTreeSet::new(),
                });
                if drive.data != payload.data() {
                    return Err(INVALID);
                }
                drive.producers.insert(producer);
            }
            (Step::ConstructActivation { .. } | Step::DriveFuture { .. }, _) => {
                return Err(INVALID);
            }
            _ => {}
        }
        Ok(())
    }
}

fn inputs(
    program: &nocter_declarations::value_access::Program,
    producer: AccessCodeId,
) -> Result<Arc<[TypeId]>, Failure> {
    program
        .codes()
        .get(producer)
        .ok_or(INVALID)?
        .parameters()
        .iter()
        .map(|&id| {
            program
                .parameters()
                .get(id)
                .map(nocter_declarations::value_access::Parameter::data)
                .ok_or(INVALID)
        })
        .collect()
}

fn local_payload(step: &Step) -> Option<Payload> {
    let local = |holder| matches!(holder, Holder::Local(_) | Holder::Expression(_));
    let payload = match step {
        Step::ConstructActivation {
            destination: Holder::Expression(_),
            payload,
            ..
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
        Step::DriveFuture { source, payload }
            if source.fields().is_root() && matches!(source.holder(), Holder::Expression(_)) =>
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
