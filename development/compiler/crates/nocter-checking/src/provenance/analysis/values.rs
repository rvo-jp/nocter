use nocter_declarations::{ProvenanceOrigin, RequirementKind, StructuralCapability};
use nocter_model::{BodyNodeId, CallableId, TypeId};

use super::Analyzer;
use crate::provenance::invocation_place_can_reach_result;
use crate::provenance::state::ProvenanceState;
use crate::{
    AggregateConstruction, AllocationSelection, AmbientStorageDependence, BodyCheckError,
    BodyCheckInternalError, CallTarget, CheckedCall, CheckedIteratorAcquisition, CheckedOperation,
    CheckedOutcome, CheckedReceiver, CheckedSequence, IterationAcquisition, PlaceRoot,
    ProvenanceProjection, ProvenanceSource, ReceiverPreparation, SequenceElement, StaticDispatch,
    ValueProvenance,
};

struct CallableValueProvenance {
    value: ValueProvenance,
    storage: Option<ValueProvenance>,
}

/// The two distinct storage channels available through a method receiver.
///
/// `carried` belongs to the receiver value itself (for example a view iterator's source). `place`
/// belongs to the temporary borrow used to invoke a method on an owned place. A declared direct
/// borrow result may retain the latter; an associated or generic result fixed independently of the
/// invocation may retain only the former.
struct ReceiverProvenance {
    carried: ValueProvenance,
    place: Option<ValueProvenance>,
}

impl ReceiverProvenance {
    fn carried(value: ValueProvenance) -> Self {
        Self {
            carried: value,
            place: None,
        }
    }
}

struct ArgumentProvenance {
    carried: ValueProvenance,
    place: Option<ValueProvenance>,
}

impl ArgumentProvenance {
    fn carried(value: ValueProvenance) -> Self {
        Self {
            carried: value,
            place: None,
        }
    }

    fn retained(&self, retain_invocation_place: bool) -> &ValueProvenance {
        if retain_invocation_place {
            self.place.as_ref().unwrap_or(&self.carried)
        } else {
            &self.carried
        }
    }
}

struct EvaluatedCall {
    callable: Option<CallableValueProvenance>,
    receiver: Option<ReceiverProvenance>,
    arguments: Vec<ArgumentProvenance>,
}

impl Analyzer<'_, '_> {
    pub(super) fn evaluate_iterator_acquisition(
        &mut self,
        acquisition: &CheckedIteratorAcquisition,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        let Some(source) = self.evaluate_receiver(acquisition.source(), state)? else {
            return Ok((ValueProvenance::independent(), false));
        };
        let result = match acquisition.acquisition() {
            IterationAcquisition::Direct => source.carried,
            IterationAcquisition::Expansion(selection) => match selection.dispatch() {
                StaticDispatch::Direct(callable) => self.map_callable_summary(
                    callable,
                    Some(&source),
                    &[],
                    state.current_allocation(),
                )?,
                StaticDispatch::StructuralRequirement(requirement) => {
                    if !matches!(
                        self.graph
                            .declarations()
                            .requirements()
                            .get(requirement)
                            .map(nocter_declarations::Requirement::kind),
                        Some(RequirementKind::Expansion { .. })
                    ) {
                        return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
                    }
                    source.carried
                }
                StaticDispatch::InterfaceMethod { .. }
                | StaticDispatch::InterfaceSelfMethod { .. }
                | StaticDispatch::InterfaceDefault { .. }
                | StaticDispatch::OpaqueMethod { .. } => {
                    return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
                }
            },
        };
        Ok((result, true))
    }

    pub(super) fn iteration_item_provenance(
        &self,
        iteration: &crate::TypedIteration,
        iterator: &ValueProvenance,
        current_allocation: &ValueProvenance,
    ) -> Result<ValueProvenance, BodyCheckInternalError> {
        let acquisition = self
            .body
            .nodes()
            .get(iteration.iterator())
            .and_then(|node| match node.operation() {
                CheckedOperation::IteratorAcquisition(acquisition) => Some(acquisition),
                _ => None,
            })
            .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
        if acquisition.source().preparation() == ReceiverPreparation::Owned
            && matches!(
                self.types.get(iteration.item()),
                Some(nocter_model::TypeKind::Borrow { .. })
            )
        {
            return Ok(ValueProvenance::from_source(ProvenanceSource::Temporary(
                iteration.iterator(),
            )));
        }
        let callable = match iteration.next().dispatch() {
            StaticDispatch::Direct(callable)
            | StaticDispatch::InterfaceMethod {
                method: callable, ..
            }
            | StaticDispatch::InterfaceSelfMethod {
                method: callable, ..
            }
            | StaticDispatch::InterfaceDefault {
                method: callable, ..
            }
            | StaticDispatch::OpaqueMethod {
                method: callable, ..
            } => callable,
            StaticDispatch::StructuralRequirement(_) => {
                return Err(BodyCheckInternalError::ProvenanceAnalysis);
            }
        };
        Ok(self
            .map_callable_summary(
                callable,
                Some(&ReceiverProvenance::carried(iterator.clone())),
                &[],
                current_allocation,
            )?
            .projected(ProvenanceProjection::OutcomeValue))
    }

    pub(super) fn evaluate_aggregate(
        &mut self,
        aggregate: &AggregateConstruction,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        let mut result = ValueProvenance::independent();
        match aggregate {
            AggregateConstruction::Struct { fields, .. } => {
                for (field, node) in fields {
                    let (value, reaches) = self.evaluate(*node, state)?;
                    if !reaches {
                        return Ok((ValueProvenance::independent(), false));
                    }
                    result.insert_projection(ProvenanceProjection::Field((*field).into()), value);
                }
            }
            AggregateConstruction::Enum { variant, payload } => {
                let declaration = self
                    .graph
                    .declarations()
                    .variants()
                    .get(*variant)
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                for (parameter, node) in declaration.payload().iter().zip(payload) {
                    let (value, reaches) = self.evaluate(*node, state)?;
                    if !reaches {
                        return Ok((ValueProvenance::independent(), false));
                    }
                    result.insert_projection(
                        ProvenanceProjection::VariantPayload {
                            variant: *variant,
                            parameter: *parameter,
                        },
                        value,
                    );
                }
            }
            AggregateConstruction::FixedArray(elements) => {
                let mut values = ValueProvenance::independent();
                for element in elements {
                    let (value, reaches) = self.evaluate(*element, state)?;
                    if !reaches {
                        return Ok((ValueProvenance::independent(), false));
                    }
                    values.union_with(&value);
                }
                result.insert_projection(ProvenanceProjection::Element, values);
            }
        }
        Ok((result, true))
    }

    pub(super) fn evaluate_outcome(
        &mut self,
        node: BodyNodeId,
        outcome: &CheckedOutcome,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        match outcome {
            CheckedOutcome::Absent => Ok((ValueProvenance::independent(), true)),
            CheckedOutcome::Inject { payload, .. } => {
                let (value, reaches) = self.evaluate(*payload, state)?;
                Ok((
                    ValueProvenance::from_projection(ProvenanceProjection::OutcomeValue, value),
                    reaches,
                ))
            }
            CheckedOutcome::Failure(payload) => {
                let (value, reaches) = self.evaluate(*payload, state)?;
                Ok((
                    ValueProvenance::from_projection(ProvenanceProjection::OutcomeFailure, value),
                    reaches,
                ))
            }
            CheckedOutcome::Force { operand, .. } => {
                let (value, reaches) = self.evaluate(*operand, state)?;
                Ok((value.projected(ProvenanceProjection::OutcomeValue), reaches))
            }
            CheckedOutcome::Propagate { operand, .. } => {
                let (value, reaches) = self.evaluate(*operand, state)?;
                if reaches {
                    let failure = value.projected(ProvenanceProjection::OutcomeFailure);
                    if !failure.all_sources().is_empty() {
                        self.record_return(node, failure);
                    }
                }
                Ok((value.projected(ProvenanceProjection::OutcomeValue), reaches))
            }
            CheckedOutcome::Recover {
                operand,
                binding,
                fallback,
                ..
            } => {
                let (operand, reaches) = self.evaluate(*operand, state)?;
                if !reaches {
                    return Ok((ValueProvenance::independent(), false));
                }
                let entry = state.clone();
                let mut fallback_state = entry.clone();
                if let Some(binding) = binding {
                    fallback_state.set_value(
                        PlaceRoot::Local(*binding),
                        operand.projected(ProvenanceProjection::OutcomeFailure),
                    );
                }
                let (fallback_value, fallback_reaches) =
                    self.evaluate(*fallback, &mut fallback_state)?;
                let mut incoming = vec![entry];
                if fallback_reaches {
                    incoming.push(fallback_state);
                }
                state.join(&incoming);
                let mut result = operand.projected(ProvenanceProjection::OutcomeValue);
                if fallback_reaches {
                    result.union_with(&fallback_value);
                }
                Ok((result, true))
            }
        }
    }

    pub(super) fn evaluate_call(
        &mut self,
        call: &CheckedCall,
        state: &mut ProvenanceState,
        result_type: TypeId,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        let Some(evaluated) = self.evaluate_call_inputs(call, state)? else {
            return Ok((ValueProvenance::independent(), false));
        };
        let mut result = self.map_call_result(call, &evaluated, state, result_type)?;
        if !self.types.may_carry_storage(result_type) {
            result = ValueProvenance::independent();
        }
        Ok((result, true))
    }

    fn evaluate_call_inputs(
        &mut self,
        call: &CheckedCall,
        state: &mut ProvenanceState,
    ) -> Result<Option<EvaluatedCall>, BodyCheckError> {
        let callable = match call.target() {
            CallTarget::CallableValue {
                value, capability, ..
            }
            | CallTarget::ClosureValue {
                value, capability, ..
            } => {
                let (provenance, reaches) = self.evaluate(*value, state)?;
                if !reaches {
                    return Ok(None);
                }
                let storage = if *capability == nocter_model::CallableCapability::Owned {
                    None
                } else {
                    let checked = self
                        .body
                        .nodes()
                        .get(*value)
                        .ok_or(BodyCheckInternalError::MissingNode(*value))?;
                    let CheckedOperation::Place(place) = checked.operation() else {
                        return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
                    };
                    Some(self.place_storage(*place, state)?)
                };
                Some(CallableValueProvenance {
                    value: provenance,
                    storage,
                })
            }
            CallTarget::Static(_) => None,
        };
        let receiver = if let Some(receiver) = call.receiver() {
            let provenance = self.evaluate_receiver(receiver, state)?;
            let Some(provenance) = provenance else {
                return Ok(None);
            };
            Some(provenance)
        } else {
            None
        };
        let mut arguments = Vec::with_capacity(call.arguments().len());
        for argument in call.arguments() {
            let (value, reaches) = self.evaluate(*argument, state)?;
            if !reaches {
                return Ok(None);
            }
            let checked = self
                .body
                .nodes()
                .get(*argument)
                .ok_or(BodyCheckInternalError::MissingNode(*argument))?;
            let argument = match checked.operation() {
                CheckedOperation::Borrow { place, .. } => ArgumentProvenance {
                    carried: self.read_place(*place, state)?,
                    place: Some(value),
                },
                _ => ArgumentProvenance::carried(value),
            };
            arguments.push(argument);
        }
        Ok(Some(EvaluatedCall {
            callable,
            receiver,
            arguments,
        }))
    }

    fn map_call_result(
        &self,
        call: &CheckedCall,
        evaluated: &EvaluatedCall,
        state: &ProvenanceState,
        result_type: TypeId,
    ) -> Result<ValueProvenance, BodyCheckError> {
        Ok(match call.target() {
            CallTarget::Static(selection) => {
                let callable = static_callable(selection.dispatch())
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                self.map_callable_summary(
                    callable,
                    evaluated.receiver.as_ref(),
                    &evaluated.arguments,
                    state.current_allocation(),
                )?
            }
            CallTarget::CallableValue { dispatch, .. } => {
                let contract = match dispatch.dispatch() {
                    StaticDispatch::StructuralRequirement(requirement) => self
                        .graph
                        .declarations()
                        .requirements()
                        .get(requirement)
                        .and_then(|requirement| match requirement.kind() {
                            RequirementKind::Capability {
                                capability: StructuralCapability::Callable(contract),
                                ..
                            } => Some(contract),
                            _ => None,
                        })
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?,
                    _ => return Err(BodyCheckInternalError::ProvenanceAnalysis.into()),
                };
                let mut mapped = ValueProvenance::independent();
                let retain_place =
                    invocation_place_can_reach_result(self.graph, self.types, result_type);
                for origin in contract.provenance().origins() {
                    let argument = evaluated
                        .arguments
                        .get(origin.position())
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    mapped.union_with(&argument.retained(retain_place).flattened());
                }
                let callable = evaluated
                    .callable
                    .as_ref()
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                mapped.union_with(&callable.value.flattened());
                if invocation_place_can_reach_result(self.graph, self.types, result_type)
                    && let Some(environment) = &callable.storage
                {
                    mapped.union_with(&environment.flattened());
                }
                mapped
            }
            CallTarget::ClosureValue { closure, .. } => {
                let summary = self
                    .closure_summaries
                    .get(closure)
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                let callable = evaluated
                    .callable
                    .as_ref()
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                let mut mapped = ValueProvenance::independent();
                let retain_place =
                    invocation_place_can_reach_result(self.graph, self.types, result_type);
                for origin in &summary.parameters {
                    let argument = evaluated
                        .arguments
                        .get(origin.position())
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    mapped.union_with(&argument.retained(retain_place).flattened());
                }
                for capture in &summary.captures {
                    let value = callable
                        .value
                        .projected(ProvenanceProjection::ClosureCaptureValue(*capture));
                    mapped.union_with(&value.flattened());
                }
                if summary.environment {
                    let environment = callable
                        .storage
                        .as_ref()
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    mapped.union_with(&environment.flattened());
                }
                match summary.ambient {
                    AmbientStorageDependence::Independent => {}
                    AmbientStorageDependence::Current => {
                        mapped.union_with(state.current_allocation());
                    }
                    AmbientStorageDependence::Unknown => {
                        mapped.union_with(&ValueProvenance::from_source(ProvenanceSource::Unknown));
                    }
                }
                mapped
            }
        })
    }

    fn evaluate_receiver(
        &mut self,
        receiver: &CheckedReceiver,
        state: &mut ProvenanceState,
    ) -> Result<Option<ReceiverProvenance>, BodyCheckError> {
        let (value, reaches) = self.evaluate(receiver.value(), state)?;
        if !reaches {
            return Ok(None);
        }
        let provenance = match receiver.preparation() {
            ReceiverPreparation::Owned
            | ReceiverPreparation::PreserveBorrow(_)
            | ReceiverPreparation::WeakenReadwriteBorrow => ReceiverProvenance::carried(value),
            ReceiverPreparation::BorrowTemporary(_) => ReceiverProvenance {
                carried: value,
                place: Some(ValueProvenance::from_source(ProvenanceSource::Temporary(
                    receiver.value(),
                ))),
            },
            ReceiverPreparation::BorrowPlace(_) => {
                let checked = self
                    .body
                    .nodes()
                    .get(receiver.value())
                    .ok_or(BodyCheckInternalError::MissingNode(receiver.value()))?;
                let CheckedOperation::Place(place) = checked.operation() else {
                    return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
                };
                ReceiverProvenance {
                    carried: value,
                    place: Some(self.place_storage(*place, state)?),
                }
            }
        };
        Ok(Some(provenance))
    }

    fn map_callable_summary(
        &self,
        callable: CallableId,
        receiver: Option<&ReceiverProvenance>,
        arguments: &[ArgumentProvenance],
        current_allocation: &ValueProvenance,
    ) -> Result<ValueProvenance, BodyCheckInternalError> {
        let declaration = self
            .graph
            .declarations()
            .callables()
            .get(callable)
            .ok_or(BodyCheckInternalError::MissingCallable(callable))?;
        let summary = self
            .summaries
            .get(&callable)
            .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
        let mut result = ValueProvenance::independent();
        for origin in &summary.origins {
            match origin {
                ProvenanceOrigin::Receiver => {
                    let receiver = receiver.ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    let receiver = if invocation_place_can_reach_result(
                        self.graph,
                        self.types,
                        declaration.result(),
                    ) {
                        receiver.place.as_ref().unwrap_or(&receiver.carried)
                    } else {
                        &receiver.carried
                    };
                    result.union_with(&receiver.flattened());
                }
                ProvenanceOrigin::Parameter(parameter) => {
                    let position = declaration
                        .parameters()
                        .iter()
                        .position(|candidate| candidate == parameter)
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    let argument = arguments
                        .get(position)
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    let retain_place = invocation_place_can_reach_result(
                        self.graph,
                        self.types,
                        declaration.result(),
                    );
                    result.union_with(&argument.retained(retain_place).flattened());
                }
            }
        }
        match summary.ambient {
            AmbientStorageDependence::Independent => {}
            AmbientStorageDependence::Current => {
                result.union_with(&current_allocation.flattened());
            }
            AmbientStorageDependence::Unknown => {
                result.union_with(&ValueProvenance::from_source(ProvenanceSource::Unknown));
            }
        }
        Ok(result)
    }

    pub(super) fn evaluate_sequence(
        &mut self,
        sequence: &CheckedSequence,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        let mut result = self.allocation_provenance(sequence.allocation(), state)?;
        let mut elements = ValueProvenance::independent();
        for element in sequence.elements() {
            match element {
                SequenceElement::Value(value) => {
                    let (value, reaches) = self.evaluate(*value, state)?;
                    if !reaches {
                        return Ok((ValueProvenance::independent(), false));
                    }
                    elements.union_with(&value);
                }
                SequenceElement::Spread {
                    mode, iteration, ..
                } => {
                    let (iterator, reaches) = self.evaluate(iteration.iterator(), state)?;
                    if !reaches {
                        return Ok((ValueProvenance::independent(), false));
                    }
                    let contribution = mode
                        .contribution_type(self.types, iteration.item())
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    if !self.types.may_carry_storage(contribution) {
                        continue;
                    }
                    let value = self.iteration_item_provenance(
                        iteration,
                        &iterator,
                        state.current_allocation(),
                    )?;
                    elements.union_with(&value);
                }
            }
        }
        result.insert_projection(ProvenanceProjection::Element, elements);
        Ok((result, true))
    }

    pub(super) fn allocation_provenance(
        &mut self,
        allocation: AllocationSelection,
        state: &mut ProvenanceState,
    ) -> Result<ValueProvenance, BodyCheckError> {
        match allocation {
            AllocationSelection::CurrentRegion => Ok(state.current_allocation().clone()),
            AllocationSelection::Explicit(allocator) => {
                let (value, reaches) = self.evaluate(allocator, state)?;
                if !reaches {
                    return Ok(ValueProvenance::independent());
                }
                Ok(value)
            }
        }
    }
}

fn static_callable(dispatch: StaticDispatch) -> Option<nocter_model::CallableId> {
    match dispatch {
        StaticDispatch::Direct(callable)
        | StaticDispatch::InterfaceMethod {
            method: callable, ..
        }
        | StaticDispatch::InterfaceSelfMethod {
            method: callable, ..
        }
        | StaticDispatch::InterfaceDefault {
            method: callable, ..
        }
        | StaticDispatch::OpaqueMethod {
            method: callable, ..
        } => Some(callable),
        StaticDispatch::StructuralRequirement(_) => None,
    }
}
