use nocter_declarations::{ProvenanceOrigin, RequirementKind, StructuralCapability};
use nocter_model::{BodyNodeId, CallableId, TypeId};

use super::Analyzer;
use crate::provenance::state::ProvenanceState;
use crate::{
    AggregateConstruction, AllocationSelection, AmbientStorageDependence, BodyCheckError,
    BodyCheckInternalError, CallTarget, CheckedCall, CheckedCallReceiver, CheckedOperation,
    CheckedOutcome, CheckedSequence, PlaceRoot, ProvenanceProjection, ProvenanceSource,
    ReceiverPreparation, SequenceElement, StaticDispatch, ValueProvenance,
};

struct CallableValueProvenance {
    value: ValueProvenance,
    storage: Option<ValueProvenance>,
}

struct EvaluatedCall {
    callable: Option<CallableValueProvenance>,
    receiver: Option<ValueProvenance>,
    arguments: Vec<ValueProvenance>,
}

impl Analyzer<'_, '_> {
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
                    result.insert_projection(ProvenanceProjection::Field(*field), value);
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
        let mut result = self.map_call_result(call, &evaluated, state)?;
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
            arguments.push(value);
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
    ) -> Result<ValueProvenance, BodyCheckError> {
        Ok(match call.target() {
            CallTarget::Static(selection) => {
                let callable = match selection.dispatch() {
                    StaticDispatch::Direct(callable)
                    | StaticDispatch::InterfaceMethod {
                        method: callable, ..
                    } => callable,
                    StaticDispatch::StructuralRequirement(_) => {
                        return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
                    }
                };
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
                for origin in contract.provenance().origins() {
                    let argument = evaluated
                        .arguments
                        .get(origin.position())
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    mapped.union_with(argument);
                }
                let callable = evaluated
                    .callable
                    .as_ref()
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                mapped.union_with(&callable.value);
                if let Some(environment) = &callable.storage {
                    mapped.union_with(environment);
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
                for origin in &summary.parameters {
                    mapped.union_with(
                        evaluated
                            .arguments
                            .get(origin.position())
                            .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?,
                    );
                }
                for capture in &summary.captures {
                    mapped.union_with(
                        &callable
                            .value
                            .projected(ProvenanceProjection::ClosureCaptureValue(*capture)),
                    );
                }
                if summary.environment {
                    mapped.union_with(
                        callable
                            .storage
                            .as_ref()
                            .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?,
                    );
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
        receiver: &CheckedCallReceiver,
        state: &mut ProvenanceState,
    ) -> Result<Option<ValueProvenance>, BodyCheckError> {
        let (value, reaches) = self.evaluate(receiver.value(), state)?;
        if !reaches {
            return Ok(None);
        }
        let provenance = match receiver.preparation() {
            ReceiverPreparation::Owned
            | ReceiverPreparation::PreserveBorrow(_)
            | ReceiverPreparation::WeakenReadwriteBorrow => value,
            ReceiverPreparation::BorrowTemporary(_) => {
                ValueProvenance::from_source(ProvenanceSource::Temporary(receiver.value()))
            }
            ReceiverPreparation::BorrowPlace(_) => {
                let checked = self
                    .body
                    .nodes()
                    .get(receiver.value())
                    .ok_or(BodyCheckInternalError::MissingNode(receiver.value()))?;
                let CheckedOperation::Place(place) = checked.operation() else {
                    return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
                };
                self.place_storage(*place, state)?
            }
        };
        Ok(Some(provenance))
    }

    fn map_callable_summary(
        &self,
        callable: CallableId,
        receiver: Option<&ValueProvenance>,
        arguments: &[ValueProvenance],
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
                    result.union_with(receiver.ok_or(BodyCheckInternalError::ProvenanceAnalysis)?);
                }
                ProvenanceOrigin::Parameter(parameter) => {
                    let position = declaration
                        .parameters()
                        .iter()
                        .position(|candidate| candidate == parameter)
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    result.union_with(
                        arguments
                            .get(position)
                            .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?,
                    );
                }
            }
        }
        match summary.ambient {
            AmbientStorageDependence::Independent => {}
            AmbientStorageDependence::Current => {
                result.union_with(current_allocation);
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
                SequenceElement::Spread { iteration, .. } => {
                    let (value, reaches) = self.evaluate(iteration.source(), state)?;
                    if !reaches {
                        return Ok((ValueProvenance::independent(), false));
                    }
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
