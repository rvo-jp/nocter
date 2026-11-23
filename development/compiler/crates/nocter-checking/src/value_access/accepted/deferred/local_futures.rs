//! Close captured input layouts and optional producer selectors for actual local activations.
use super::super::{FutureCaptureCases, FutureInputCleanup, FutureSelectorMove, LocalFutureMove};
use super::{Failure, INVALID, reject};
use crate::value_access::{
    flow::{
        self, Location, Operation,
        resources::{ActivationInput, Disposal, Holder, Payload, Step},
    },
    results,
};
use nocter_declarations::value_access::FunctionInput;
use nocter_model::{AccessCodeId, AccessExpressionId, TypeId, ValueCode, ValueInputOperation};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub(in crate::value_access::accepted) struct LocalFutures {
    bodies: BTreeMap<ValueCode, Body>,
}

#[derive(Clone, Debug, Default)]
struct Body {
    data: BTreeSet<TypeId>,
    tagged: BTreeSet<Holder>,
    drives: BTreeMap<AccessExpressionId, Drive>,
    constructions: BTreeMap<AccessExpressionId, (TypeId, AccessCodeId)>,
    tags: BTreeMap<AccessCodeId, u64>,
    captures: BTreeMap<(Holder, AccessCodeId), Arc<[FunctionInput]>>,
    moves: BTreeMap<Movement, LocalFutureMove>,
    cancellations: BTreeMap<(Location, Holder), FutureCaptureCases<FutureInputCleanup>>,
}

#[derive(Clone, Debug)]
struct Drive {
    data: TypeId,
    operand: Holder,
    producers: BTreeSet<AccessCodeId>,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Movement {
    at: Location,
    source: Holder,
    destination: Holder,
}

struct CapturedInputs {
    fields: Arc<[FunctionInput]>,
    cleanup: Arc<[FutureInputCleanup]>,
}

struct ObservedCaptures<T> {
    data: TypeId,
    producers: BTreeMap<AccessCodeId, Arc<[T]>>,
}
impl<T: PartialEq> ObservedCaptures<T> {
    fn new(data: TypeId) -> Self {
        Self {
            data,
            producers: BTreeMap::new(),
        }
    }

    fn record(
        &mut self,
        data: TypeId,
        producer: AccessCodeId,
        fields: &Arc<[T]>,
    ) -> Result<(), Failure> {
        if self.data != data {
            return Err(INVALID);
        }
        self.producers
            .entry(producer)
            .or_insert_with(|| Arc::clone(fields));
        Ok(())
    }

    fn needs_selector(&self) -> bool {
        let mut fields = self.producers.values();
        fields
            .next()
            .is_some_and(|first| fields.any(|candidate| candidate != first))
    }

    fn close(self, tags: &BTreeMap<AccessCodeId, u64>) -> Result<FutureCaptureCases<T>, Failure> {
        if !self.needs_selector() {
            let fields = self.producers.into_values().next().ok_or(INVALID)?;
            return Ok(FutureCaptureCases::Direct(fields));
        }
        let cases = self
            .producers
            .into_iter()
            .map(|(producer, fields)| Ok((tags.get(&producer).copied().ok_or(INVALID)?, fields)))
            .collect::<Result<_, Failure>>()?;
        Ok(FutureCaptureCases::Selected(cases))
    }
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
        let &(_, producer) = body.constructions.get(&expression)?;
        if !body.tagged.contains(&Holder::Expression(expression)) {
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
        producer: AccessCodeId,
    ) -> Option<&Arc<[FunctionInput]>> {
        self.bodies.get(&code)?.captures.get(&(holder, producer))
    }
    pub(in crate::value_access::accepted) fn movement(
        &self,
        code: ValueCode,
        at: Location,
        source: Holder,
        destination: Holder,
    ) -> Option<&LocalFutureMove> {
        self.bodies.get(&code)?.moves.get(&Movement {
            at,
            source,
            destination,
        })
    }

    pub(in crate::value_access::accepted) fn cancellation(
        &self,
        code: ValueCode,
        at: Location,
        source: Holder,
    ) -> Option<&FutureCaptureCases<FutureInputCleanup>> {
        self.bodies.get(&code)?.cancellations.get(&(at, source))
    }

    pub(in crate::value_access::accepted) fn contains_step(
        &self,
        code: ValueCode,
        step: &Step,
    ) -> bool {
        local_payload(step).is_some_and(|payload| self.contains(code, payload.data()))
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
    let mut moves = BTreeMap::new();
    let mut cancellations = BTreeMap::new();
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
                        let data = payload.data();
                        match step {
                            Step::Move {
                                source,
                                destination,
                                ..
                            } => {
                                let key = Movement {
                                    at: location,
                                    source: source.holder(),
                                    destination: destination.holder(),
                                };
                                moves
                                    .entry(key)
                                    .or_insert_with(|| ObservedCaptures::new(data))
                                    .record(data, producer, &signature.fields)?;
                            }
                            Step::Dispose { source, .. } => {
                                cancellations
                                    .entry((location, source.holder()))
                                    .or_insert_with(|| ObservedCaptures::new(data))
                                    .record(data, producer, &signature.cleanup)?;
                            }
                            _ => {}
                        }
                        found.record_captures(step, producer, signature)?;
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
    found.close_selections(moves, cancellations)?;
    Ok(Some(found))
}

impl Body {
    fn close_selections(
        &mut self,
        moves: BTreeMap<Movement, ObservedCaptures<FunctionInput>>,
        cancellations: BTreeMap<(Location, Holder), ObservedCaptures<FutureInputCleanup>>,
    ) -> Result<(), Failure> {
        let mut needed = self
            .drives
            .values()
            .filter(|drive| drive.producers.len() > 1)
            .map(|drive| drive.operand)
            .collect::<BTreeSet<_>>();
        let mut uncertain_inputs = BTreeMap::<_, BTreeSet<_>>::new();
        for (site, movement) in &moves {
            if movement.needs_selector() {
                needed.insert(site.source);
            }
            if movement.producers.len() > 1 {
                uncertain_inputs
                    .entry(site.destination)
                    .or_default()
                    .insert(site.source);
            }
        }
        needed.extend(
            cancellations
                .iter()
                .filter(|(_, cancellation)| cancellation.needs_selector())
                .map(|(&(_, holder), _)| holder),
        );
        // A known incoming producer supplies its constant at the move. Only an
        // ambiguous incoming edge needs the selector in its original source slot.
        while let Some(holder) = needed.pop_first() {
            if self.tagged.insert(holder)
                && let Some(inputs) = uncertain_inputs.remove(&holder)
            {
                needed.extend(inputs);
            }
        }
        let producers = self
            .captures
            .keys()
            .filter_map(|&(holder, producer)| self.tagged.contains(&holder).then_some(producer))
            .collect::<BTreeSet<_>>();
        self.tags = producers.into_iter().zip(0_u64..).collect();
        for (site, movement) in moves {
            let selector = if !self.tagged.contains(&site.destination) {
                None
            } else if movement.producers.len() == 1 {
                let producer = movement.producers.first_key_value().ok_or(INVALID)?.0;
                Some(FutureSelectorMove::Known(
                    *self.tags.get(producer).ok_or(INVALID)?,
                ))
            } else {
                Some(FutureSelectorMove::FromSource)
            };
            self.moves.insert(
                site,
                LocalFutureMove {
                    inputs: movement.close(&self.tags)?,
                    selector,
                },
            );
        }
        for (site, cancellation) in cancellations {
            self.cancellations
                .insert(site, cancellation.close(&self.tags)?);
        }
        Ok(())
    }

    fn record_captures(
        &mut self,
        step: &Step,
        producer: AccessCodeId,
        signature: &CapturedInputs,
    ) -> Result<(), Failure> {
        let (source, destination) = match step {
            Step::ConstructActivation {
                destination,
                inputs,
                ..
            } => {
                if inputs
                    .iter()
                    .map(ActivationInput::contract)
                    .ne(signature.fields.iter().copied())
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
            self.captures
                .entry((holder, producer))
                .or_insert_with(|| Arc::clone(&signature.fields));
        }
        Ok(())
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
                    operand: source.holder(),
                    producers: BTreeSet::new(),
                });
                if drive.data != payload.data() || drive.operand != source.holder() {
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
) -> Result<CapturedInputs, Failure> {
    let fields = program
        .codes()
        .get(producer)
        .ok_or(INVALID)?
        .parameters()
        .iter()
        .map(|&id| {
            program
                .parameters()
                .get(id)
                .map(|parameter| FunctionInput {
                    data: parameter.data(),
                    operation: parameter.operation(),
                })
                .ok_or(INVALID)
        })
        .collect::<Result<Arc<[_]>, Failure>>()?;
    let mut cleanup = Vec::new();
    for (position, input) in fields.iter().enumerate().rev() {
        if input.operation == ValueInputOperation::Take {
            let payload = flow::resources::local_future_input(program, input.data, input.operation)
                .ok_or(INVALID)?;
            if payload.disposal() != Disposal::Trivial {
                cleanup.push(FutureInputCleanup { position, payload });
            }
        }
    }
    Ok(CapturedInputs {
        fields,
        cleanup: cleanup.into(),
    })
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
